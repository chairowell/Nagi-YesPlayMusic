use std::{
    collections::HashMap,
    env, fs,
    io::{Cursor, ErrorKind, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::{
    image::Image as TauriImage,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::PageLoadEvent,
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, RunEvent, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSWindow, NSWindowButton};

const API_PORT: u16 = 12_754;
const DEV_WEB_PORT: u16 = 1_420;
const RELEASE_WEB_PORT: u16 = 28_232;
const SIDECAR_HEALTH_PATH: &str = "/__yesplaymusic/health";
const SIDECAR_HEALTH_BODY: &str = r#"{"service":"yesplaymusic-sidecar","protocol":1}"#;
const SIDECAR_HEALTH_TOKEN_HEADER: &str = "X-YesPlayMusic-Health-Token";

struct SidecarState(Mutex<Option<CommandChild>>);

#[derive(Default)]
struct TrayCoverState(Mutex<Option<String>>);

#[derive(Default)]
struct GlobalShortcutRegistration {
    actions: HashMap<u32, String>,
    settings: Option<serde_json::Value>,
    temporarily_disabled: bool,
}

struct GlobalShortcutRegistrationState(Mutex<GlobalShortcutRegistration>);

const MAX_TRAY_COVER_BYTES: u64 = 2 * 1024 * 1024;

fn tray_cover_url(payload: &serde_json::Value) -> Option<&str> {
    payload
        .get("coverUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
}

fn decode_tray_cover(bytes: &[u8]) -> Result<TauriImage<'static>, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let cover = reader
        .decode()
        .map_err(|error| error.to_string())?
        .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    Ok(TauriImage::new_owned(cover.into_raw(), 64, 64))
}

async fn download_tray_cover(url: &str) -> Result<TauriImage<'static>, String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("封面地址只允许 HTTP(S)".to_string());
    }

    let mut response = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?
        .get(parsed)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    if response.content_length().unwrap_or(0) > MAX_TRAY_COVER_BYTES {
        return Err("菜单栏封面响应过大".to_string());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(64 * 1024)
            .min(MAX_TRAY_COVER_BYTES) as usize,
    );
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_TRAY_COVER_BYTES {
            return Err("菜单栏封面响应过大".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    decode_tray_cover(&bytes)
}

fn update_tray_cover(app: &AppHandle, payload: &serde_json::Value) {
    let Some(cover_url) = tray_cover_url(payload) else {
        return;
    };
    let state = app.state::<TrayCoverState>();
    let mut current = match state.0.lock() {
        Ok(current) => current,
        Err(error) => {
            eprintln!("[tauri] 无法锁定菜单栏封面状态：{error}");
            return;
        }
    };
    if current.as_deref() == Some(cover_url) {
        return;
    }
    let cover_url = cover_url.to_string();
    *current = Some(cover_url.clone());
    drop(current);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match download_tray_cover(&cover_url).await {
            Ok(icon) => {
                let is_current = app
                    .state::<TrayCoverState>()
                    .0
                    .lock()
                    .map(|current| current.as_deref() == Some(cover_url.as_str()))
                    .unwrap_or(false);
                if is_current {
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        if let Err(error) = tray.set_icon(Some(icon)) {
                            eprintln!("[tauri] 无法更新菜单栏封面：{error}");
                        }
                    }
                }
            }
            Err(error) => {
                // 下载失败后清掉去重状态；下一次歌词更新会自然重试。
                if let Ok(mut current) = app.state::<TrayCoverState>().0.lock() {
                    if current.as_deref() == Some(cover_url.as_str()) {
                        *current = None;
                    }
                }
                eprintln!("[tauri] 无法下载菜单栏封面：{error}");
            }
        }
    });
}

fn emit_desktop_event(app: &AppHandle, event: &str) {
    let _ = app.emit(&format!("desktop://{event}"), ());
}

#[cfg(target_os = "macos")]
fn set_window_button_visibility(window: WebviewWindow, visible: bool) -> Result<(), String> {
    let window_on_main = window.clone();
    window
        .run_on_main_thread(move || match window_on_main.ns_window() {
            Ok(pointer) => {
                let ns_window = unsafe { &*(pointer.cast::<NSWindow>()) };
                for kind in [
                    NSWindowButton::CloseButton,
                    NSWindowButton::MiniaturizeButton,
                    NSWindowButton::ZoomButton,
                ] {
                    if let Some(button) = ns_window.standardWindowButton(kind) {
                        button.setHidden(!visible);
                    }
                }
            }
            Err(error) => eprintln!("[tauri] 无法取得 macOS 窗口按钮：{error}"),
        })
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn set_window_button_visibility(_window: WebviewWindow, _visible: bool) -> Result<(), String> {
    Ok(())
}

fn normalize_electron_shortcut(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part {
            "CommandOrControl" => "CmdOrCtrl",
            "Right" => "ArrowRight",
            "Left" => "ArrowLeft",
            "Up" => "ArrowUp",
            "Down" => "ArrowDown",
            _ => part,
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn register_global_shortcuts(app: &AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| error.to_string())?;

    let state = app.state::<GlobalShortcutRegistrationState>();
    let (settings, temporarily_disabled) = {
        let mut registration = state.0.lock().map_err(|error| error.to_string())?;
        registration.actions.clear();
        (
            registration.settings.clone(),
            registration.temporarily_disabled,
        )
    };
    let Some(settings) = settings else {
        return Ok(());
    };
    if temporarily_disabled
        || !settings
            .get("enableGlobalShortcut")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
    {
        return Ok(());
    }

    let shortcuts = settings
        .get("shortcuts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "快捷键设置缺少 shortcuts 数组".to_string())?;
    let mut actions = HashMap::new();
    for item in shortcuts {
        let Some(action) = item.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(accelerator) = item
            .get("globalShortcut")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Ok(shortcut) = normalize_electron_shortcut(accelerator).parse::<Shortcut>() else {
            eprintln!("[tauri] 忽略无法解析的快捷键：{accelerator}");
            continue;
        };
        let shortcut_id = shortcut.id();
        if let Err(error) = app.global_shortcut().register(shortcut) {
            // 单个组合被系统或其他应用占用时，其余快捷键仍应可用。
            eprintln!("[tauri] 无法注册快捷键 {accelerator}: {error}");
            continue;
        }
        actions.insert(shortcut_id, action.to_string());
    }

    state.0.lock().map_err(|error| error.to_string())?.actions = actions;
    Ok(())
}

fn update_shortcut_settings(
    app: &AppHandle,
    channel: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    let state = app.state::<GlobalShortcutRegistrationState>();
    {
        let mut registration = state.0.lock().map_err(|error| error.to_string())?;
        match channel {
            "settings" => registration.settings = Some(payload),
            "switchGlobalShortcutStatusTemporary" => {
                registration.temporarily_disabled = payload.as_str() == Some("disable");
            }
            "updateShortcut" => {
                if let (Some(settings), Some(id), Some(shortcut)) = (
                    registration.settings.as_mut(),
                    payload.get("id").and_then(serde_json::Value::as_str),
                    payload.get("shortcut").and_then(serde_json::Value::as_str),
                ) {
                    if payload.get("type").and_then(serde_json::Value::as_str)
                        == Some("globalShortcut")
                    {
                        if let Some(item) = settings
                            .get_mut("shortcuts")
                            .and_then(serde_json::Value::as_array_mut)
                            .and_then(|items| {
                                items.iter_mut().find(|item| {
                                    item.get("id").and_then(serde_json::Value::as_str) == Some(id)
                                })
                            })
                        {
                            item["globalShortcut"] =
                                serde_json::Value::String(shortcut.to_string());
                        }
                    }
                }
            }
            "restoreDefaultShortcuts" => {
                registration.settings = Some(payload);
            }
            _ => {}
        }
    }
    register_global_shortcuts(app)
}

fn parse_legacy_settings(config: &str) -> Result<Option<serde_json::Value>, String> {
    let value: serde_json::Value =
        serde_json::from_str(config).map_err(|error| error.to_string())?;
    Ok(value.get("settings").cloned())
}

#[tauri::command]
fn read_legacy_settings(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    let config_path = app
        .path()
        .home_dir()
        .map_err(|error| error.to_string())?
        .join("Library/Application Support/yesplaymusic/config.json");
    let metadata = match fs::metadata(&config_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if metadata.len() > 1_048_576 {
        return Err("旧版设置文件异常大，已拒绝读取".to_string());
    }
    let config = fs::read_to_string(config_path).map_err(|error| error.to_string())?;
    parse_legacy_settings(&config)
}

#[tauri::command]
fn desktop_event(
    app: AppHandle,
    channel: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    match channel.as_str() {
        "settings"
        | "switchGlobalShortcutStatusTemporary"
        | "updateShortcut"
        | "restoreDefaultShortcuts" => {
            update_shortcut_settings(&app, &channel, payload)?;
        }
        "updateTrayTooltip" => {
            if let (Some(tray), Some(title)) = (app.tray_by_id("main-tray"), payload.as_str()) {
                tray.set_tooltip(Some(title))
                    .map_err(|error| error.to_string())?;
            }
        }
        "updateTrayNowPlaying" => {
            if let (Some(tray), Some(title)) = (
                app.tray_by_id("main-tray"),
                payload.get("title").and_then(serde_json::Value::as_str),
            ) {
                tray.set_title(Some(title.trim()))
                    .map_err(|error| error.to_string())?;
            }
            update_tray_cover(&app, &payload);
        }
        "setWindowButtonVisibility" => {
            let visible = payload
                .as_bool()
                .ok_or_else(|| "窗口按钮显隐参数必须是布尔值".to_string())?;
            if let Some(window) = app.get_webview_window("main") {
                set_window_button_visibility(window, visible)?;
            }
        }
        // 这些事件在 Electron 里由 MPRIS、Discord、代理或动态图标消费；
        // Tauri macOS 端明确接收迁移期的 no-op，避免 renderer 出现未处理拒绝。
        "updateTrayPlayState"
        | "updateTrayLikeState"
        | "updateTrayIcon"
        | "player"
        | "setProxy"
        | "removeProxy"
        | "seeked"
        | "playerCurrentTrackTime"
        | "switchRepeatMode"
        | "switchShuffle" => {}
        _ => return Err(format!("不允许的桌面事件：{channel}")),
    }
    Ok(())
}

#[tauri::command]
fn is_always_on_top(window: WebviewWindow) -> Result<bool, String> {
    window.is_always_on_top().map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_always_on_top(window: WebviewWindow) -> Result<bool, String> {
    let next = !window
        .is_always_on_top()
        .map_err(|error| error.to_string())?;
    window
        .set_always_on_top(next)
        .map_err(|error| error.to_string())?;
    Ok(next)
}

fn window_frame_has_reachable_area(
    frame: (i32, i32, u32, u32),
    monitors: &[(i32, i32, u32, u32)],
) -> bool {
    let (window_x, window_y, window_width, window_height) = frame;
    if window_width == 0 || window_height == 0 {
        return false;
    }
    // 48px 的边角虽然数学上仍在屏幕内，实际很难发现和拖回；播放条至少保留一段可识别区域。
    let minimum_width = i64::from(window_width.min(160));
    let minimum_height = i64::from(window_height.min(80));
    monitors.iter().any(|&(x, y, width, height)| {
        let overlap_width = (i64::from(window_x) + i64::from(window_width))
            .min(i64::from(x) + i64::from(width))
            - i64::from(window_x).max(i64::from(x));
        let overlap_height = (i64::from(window_y) + i64::from(window_height))
            .min(i64::from(y) + i64::from(height))
            - i64::from(window_y).max(i64::from(y));
        overlap_width >= minimum_width && overlap_height >= minimum_height
    })
}

fn monitor_work_areas(window: &WebviewWindow) -> Result<Vec<(i32, i32, u32, u32)>, String> {
    window
        .available_monitors()
        .map_err(|error| error.to_string())
        .map(|monitors| {
            monitors
                .iter()
                .map(|monitor| {
                    let area = monitor.work_area();
                    (
                        area.position.x,
                        area.position.y,
                        area.size.width,
                        area.size.height,
                    )
                })
                .collect()
        })
}

fn ensure_main_window_reachable(window: &WebviewWindow) -> Result<(), String> {
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let frame = (position.x, position.y, size.width, size.height);
    if !window_frame_has_reachable_area(frame, &monitor_work_areas(window)?) {
        window.center().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn restore_compact_window(
    window: WebviewWindow,
    x: Option<i32>,
    y: Option<i32>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if !width.is_finite()
        || !height.is_finite()
        || !(300.0..=8192.0).contains(&width)
        || !(48.0..=8192.0).contains(&height)
    {
        return Err("窗口尺寸超出安全范围".to_string());
    }
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    let restored_size = window.outer_size().map_err(|error| error.to_string())?;
    let monitors = monitor_work_areas(&window)?;
    let reachable = x.zip(y).map(|(x, y)| {
        window_frame_has_reachable_area(
            (x, y, restored_size.width, restored_size.height),
            &monitors,
        )
    });
    if let Some((x, y)) = x.zip(y).filter(|_| reachable == Some(true)) {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    } else {
        window.center().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let play = MenuItem::with_id(app, "play", "播放/暂停", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "上一首", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let like = MenuItem::with_id(app, "like", "喜欢/取消喜欢", true, None::<&str>)?;
    let repeat = MenuItem::with_id(app, "repeat", "切换循环", true, None::<&str>)?;
    let shuffle = MenuItem::with_id(app, "shuffle", "切换随机播放", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&play, &previous, &next, &like, &repeat, &shuffle, &quit],
    )?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("YesPlayMusic")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "play" | "previous" | "next" | "like" | "repeat" | "shuffle" => {
                emit_desktop_event(app, event.id.as_ref());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false)
                    {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn is_smoke_test<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == "--smoke-test")
}

fn is_webview_smoke_test<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .any(|arg| arg.as_ref() == "--webview-smoke-test")
}

fn response_has_sidecar_identity(response: &str, expected_token: &str) -> bool {
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    let status_ok = headers
        .lines()
        .next()
        .map(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "))
        .unwrap_or(false);
    let token_matches = headers.lines().skip(1).any(|line| {
        line.split_once(':')
            .map(|(name, value)| {
                name.eq_ignore_ascii_case(SIDECAR_HEALTH_TOKEN_HEADER)
                    && value.trim() == expected_token
            })
            .unwrap_or(false)
    });
    status_ok && token_matches && body.trim() == SIDECAR_HEALTH_BODY
}

fn sidecar_identity_matches(address: SocketAddr, expected_token: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let request = format!(
        "GET {SIDECAR_HEALTH_PATH} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = String::new();
    if stream.take(8 * 1024).read_to_string(&mut response).is_err() {
        return false;
    }
    response_has_sidecar_identity(&response, expected_token)
}

fn wait_for_sidecar(port: u16, expected_token: &str, timeout: Duration) -> Result<(), String> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if sidecar_identity_matches(address, expected_token) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(format!(
        "等待 YesPlayMusic sidecar 身份握手超时（端口 {port}）"
    ))
}

fn generate_sidecar_health_token() -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut token = String::with_capacity(64);
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(token)
}

fn start_sidecar(
    app: &tauri::App,
    health_token: &str,
) -> Result<CommandChild, Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    let command = app
        .shell()
        .command("bun")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "../src/sidecar.js",
            "--api-only",
            "--api-port",
            &API_PORT.to_string(),
        ]);

    #[cfg(not(debug_assertions))]
    let command = {
        let renderer_dir = app.path().resource_dir()?.join("renderer");
        app.shell().sidecar("yesplaymusic-sidecar")?.args([
            "--api-port".to_string(),
            API_PORT.to_string(),
            "--web-port".to_string(),
            RELEASE_WEB_PORT.to_string(),
            "--renderer-dir".to_string(),
            renderer_dir.to_string_lossy().into_owned(),
        ])
    };

    let (mut events, mut child) = command.spawn()?;
    // 匿名 stdin 管道不会像参数或环境变量那样出现在进程列表里。
    if let Err(error) = child.write(format!("{health_token}\n").as_bytes()) {
        let _ = child.kill();
        return Err(error.into());
    }
    tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    println!("[sidecar] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    eprintln!("[sidecar] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Terminated(status) => {
                    println!("[sidecar] exited: {:?}", status.code);
                }
                _ => {}
            }
        }
    });

    Ok(child)
}

fn create_main_window(
    app: &tauri::App,
    show_after_creation: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = if cfg!(debug_assertions) {
        DEV_WEB_PORT
    } else {
        RELEASE_WEB_PORT
    };
    let url = format!("http://127.0.0.1:{port}").parse()?;
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("YesPlayMusic")
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .inner_size(1_440.0, 840.0)
        .min_inner_size(300.0, 48.0)
        .visible(false)
        .on_page_load(|_, payload| {
            if payload.event() == PageLoadEvent::Finished {
                println!("[tauri] webview-ready: {}", payload.url());
            }
        })
        .build()?;
    ensure_main_window_reachable(&window)?;

    let window_for_close = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_for_close.hide();
        }
    });

    if show_after_creation {
        window.show()?;
    }
    Ok(())
}

fn main() {
    let app = tauri::Builder::default()
        // 单实例必须最先注册，避免第二个实例先启动 sidecar 抢占端口。
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let action = app
                        .state::<GlobalShortcutRegistrationState>()
                        .0
                        .lock()
                        .ok()
                        .and_then(|registration| registration.actions.get(&shortcut.id()).cloned());
                    match action.as_deref() {
                        Some("minimize") => {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        Some(action) => emit_desktop_event(app, action),
                        None => {}
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        // 插件按物理像素保存尺寸，混合 Retina/普通屏时会把 1060×720 当成 2120×1440 恢复。
        // 双档逻辑尺寸由渲染进程接管；这里只保留插件的退出写盘，跳过有歧义的启动恢复。
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .skip_initial_state("main")
                .build(),
        )
        .setup(|app| {
            app.manage(GlobalShortcutRegistrationState(Mutex::new(
                GlobalShortcutRegistration::default(),
            )));
            app.manage(TrayCoverState::default());
            let health_token = generate_sidecar_health_token()?;
            let child = start_sidecar(app, &health_token)?;
            app.manage(SidecarState(Mutex::new(Some(child))));

            let ready_port = if cfg!(debug_assertions) {
                API_PORT
            } else {
                RELEASE_WEB_PORT
            };
            wait_for_sidecar(ready_port, &health_token, Duration::from_secs(15))?;
            println!(
                "[tauri] ready: pid={}, port={ready_port}",
                std::process::id()
            );

            let core_smoke_test = is_smoke_test(env::args());
            let webview_smoke_test = is_webview_smoke_test(env::args());
            if core_smoke_test || webview_smoke_test {
                // 隐藏验收不进入 Dock、不抢焦点；正式启动仍保持普通音乐应用行为。
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            if core_smoke_test {
                // CI/性能验收只验证后台核心，不创建窗口，也不会抢用户焦点。
                let handle = app.handle().clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(12));
                    handle.exit(0);
                });
            } else if webview_smoke_test {
                create_main_window(app, false)?;
                let handle = app.handle().clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(25));
                    handle.exit(0);
                });
            } else {
                create_main_window(app, true)?;
                create_tray(app)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_event,
            is_always_on_top,
            toggle_always_on_top,
            restore_compact_window,
            read_legacy_settings
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Tauri application");

    app.run(|app, event| match event {
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<SidecarState>() {
                if let Some(child) = state.0.lock().ok().and_then(|mut guard| guard.take()) {
                    let _ = child.kill();
                }
            }
        }
        RunEvent::Reopen { .. } => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::{
        decode_tray_cover, is_smoke_test, is_webview_smoke_test, normalize_electron_shortcut,
        parse_legacy_settings, response_has_sidecar_identity, tray_cover_url,
        window_frame_has_reachable_area,
    };
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn smoke_test_must_be_explicit() {
        assert!(is_smoke_test(["yesplaymusic-tauri", "--smoke-test"]));
        assert!(!is_smoke_test(["yesplaymusic-tauri"]));
    }

    #[test]
    fn webview_smoke_test_must_be_explicit() {
        assert!(is_webview_smoke_test([
            "yesplaymusic-tauri",
            "--webview-smoke-test"
        ]));
        assert!(!is_webview_smoke_test([
            "yesplaymusic-tauri",
            "--smoke-test"
        ]));
    }

    #[test]
    fn occupied_port_must_answer_with_the_sidecar_identity() {
        let expected_token = "a".repeat(64);
        let valid = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "X-YesPlayMusic-Health-Token: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n\r\n",
            "{\"service\":\"yesplaymusic-sidecar\",\"protocol\":1}"
        );
        let replayed = concat!(
            "HTTP/1.1 200 OK\r\n",
            "X-YesPlayMusic-Health-Token: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\r\n\r\n",
            "{\"service\":\"yesplaymusic-sidecar\",\"protocol\":1}"
        );
        let unrelated = "HTTP/1.1 200 OK\r\n\r\n{\"service\":\"other-app\"}";

        assert!(response_has_sidecar_identity(valid, &expected_token));
        assert!(!response_has_sidecar_identity(replayed, &expected_token));
        assert!(!response_has_sidecar_identity(unrelated, &expected_token));
        assert!(!response_has_sidecar_identity(
            "HTTP/1.1 404 Not Found\r\n\r\n",
            &expected_token
        ));
    }

    #[test]
    fn restored_window_must_have_a_reachable_area() {
        let monitors = [(0, 0, 2560, 1410), (5120, 0, 3024, 1900)];
        assert!(window_frame_has_reachable_area(
            (837, 30, 920, 620),
            &monitors
        ));
        assert!(!window_frame_has_reachable_area(
            (8064, 100, 3812, 268),
            &monitors
        ));
        assert!(!window_frame_has_reachable_area(
            (8080, 100, 500, 200),
            &monitors
        ));
    }

    #[test]
    fn electron_default_shortcuts_can_be_parsed_by_tauri() {
        for accelerator in [
            "Alt+CommandOrControl+P",
            "Alt+CommandOrControl+Right",
            "Alt+CommandOrControl+Left",
            "Alt+CommandOrControl+Up",
            "Alt+CommandOrControl+Down",
            "Alt+CommandOrControl+L",
            "Alt+CommandOrControl+M",
        ] {
            let normalized = normalize_electron_shortcut(accelerator);
            assert!(
                normalized.parse::<Shortcut>().is_ok(),
                "Tauri 无法解析 {accelerator}（转换后为 {normalized}）"
            );
        }
    }

    #[test]
    fn legacy_config_only_exposes_settings() {
        let settings =
            parse_legacy_settings(r#"{"settings":{"lang":"zh-CN"},"window":{"width":1440}}"#)
                .unwrap()
                .unwrap();
        assert_eq!(settings["lang"], "zh-CN");
        assert!(settings.get("window").is_none());
    }

    #[test]
    fn now_playing_payload_exposes_cover_url() {
        let payload = serde_json::json!({
            "title": "雨爱",
            "coverUrl": "https://example.com/cover.jpg?param=64y64"
        });

        assert_eq!(
            tray_cover_url(&payload),
            Some("https://example.com/cover.jpg?param=64y64")
        );
        assert_eq!(tray_cover_url(&serde_json::json!({ "coverUrl": "" })), None);
    }

    #[test]
    fn jpeg_cover_is_decoded_to_a_small_square_tray_icon() {
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut encoded)
            .encode(&[33, 66, 99], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();

        let icon = decode_tray_cover(&encoded).unwrap();
        assert_eq!((icon.width(), icon.height()), (64, 64));
        assert_eq!(icon.rgba().len(), 64 * 64 * 4);
    }
}

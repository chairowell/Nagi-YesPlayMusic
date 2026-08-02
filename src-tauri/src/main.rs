use std::{
    collections::HashMap,
    env, fs,
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::PageLoadEvent,
    AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

const API_PORT: u16 = 12_754;
const DEV_WEB_PORT: u16 = 1_420;
const RELEASE_WEB_PORT: u16 = 28_232;

struct SidecarState(Mutex<Option<CommandChild>>);

#[derive(Default)]
struct GlobalShortcutRegistration {
    actions: HashMap<u32, String>,
    settings: Option<serde_json::Value>,
    temporarily_disabled: bool,
}

struct GlobalShortcutRegistrationState(Mutex<GlobalShortcutRegistration>);

fn emit_desktop_event(app: &AppHandle, event: &str) {
    let _ = app.emit(&format!("desktop://{event}"), ());
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
        }
        // 这些事件在 Electron 里由 MPRIS、Discord、代理或动态图标消费；
        // Tauri macOS 端明确接收迁移期的 no-op，避免 renderer 出现未处理拒绝。
        "updateTrayPlayState"
        | "updateTrayLikeState"
        | "updateTrayIcon"
        | "player"
        | "setProxy"
        | "removeProxy"
        | "setWindowButtonVisibility"
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

fn wait_for_port(port: u16, timeout: Duration) -> Result<(), String> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(format!("等待本机端口 {port} 超时"))
}

fn start_sidecar(app: &tauri::App) -> Result<CommandChild, Box<dyn std::error::Error>> {
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

    let (mut events, child) = command.spawn()?;
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
        .inner_size(1_440.0, 840.0)
        .min_inner_size(300.0, 48.0)
        .visible(false)
        .on_page_load(|_, payload| {
            if payload.event() == PageLoadEvent::Finished {
                println!("[tauri] webview-ready: {}", payload.url());
            }
        })
        .build()?;

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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            app.manage(GlobalShortcutRegistrationState(Mutex::new(
                GlobalShortcutRegistration::default(),
            )));
            let child = start_sidecar(app)?;
            app.manage(SidecarState(Mutex::new(Some(child))));

            let ready_port = if cfg!(debug_assertions) {
                API_PORT
            } else {
                RELEASE_WEB_PORT
            };
            wait_for_port(ready_port, Duration::from_secs(15))?;
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
        is_smoke_test, is_webview_smoke_test, normalize_electron_shortcut, parse_legacy_settings,
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
}

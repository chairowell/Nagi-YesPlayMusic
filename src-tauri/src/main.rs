use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

const API_PORT: u16 = 12_754;
const DEV_WEB_PORT: u16 = 1_420;
const RELEASE_WEB_PORT: u16 = 28_232;

struct SidecarState(Mutex<Option<CommandChild>>);

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

fn create_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
        .build()?;

    let window_for_close = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_for_close.hide();
        }
    });

    window.show()?;
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
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let child = start_sidecar(app)?;
            app.manage(SidecarState(Mutex::new(Some(child))));

            let ready_port = if cfg!(debug_assertions) {
                API_PORT
            } else {
                RELEASE_WEB_PORT
            };
            wait_for_port(ready_port, Duration::from_secs(15))?;
            create_main_window(app)?;
            Ok(())
        })
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

//! Single shared system-tray icon coordinated across the Synapt apps.
//!
//! On startup each app tries to claim the tray host (see [`synapt_core::tray`]).
//! The host owns the one tray icon and a unified menu; menu actions for this app
//! are handled locally, and actions for the sibling app are forwarded over the
//! control channel (launching it if it is not running). If the host process
//! exits, a connected client claims the host role and rebuilds the tray.

use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use synapt_core::tray::{self, AppKind, TrayCommand};
use tauri::{AppHandle, Manager};

const SELF_KIND: AppKind = AppKind::Synapt;
const WINDOW_LABEL: &str = "main";

/// Connected clients (by app) and apps with a pending show after launch.
#[derive(Clone, Default)]
struct Host {
    clients: Arc<Mutex<HashMap<AppKind, TcpStream>>>,
    pending: Arc<Mutex<HashSet<AppKind>>>,
}

/// Begin tray coordination: become the host, or attach as a client.
pub fn start(app: &tauri::App) {
    let handle = app.handle().clone();
    match tray::claim_host() {
        Some(listener) => {
            let host = Host::default();
            if let Err(e) = build_tray(&handle, host.clone()) {
                tracing::error!("failed to build shared tray: {}", e);
            }
            spawn_accept(listener, host);
        }
        None => spawn_client(handle),
    }
}

fn build_tray(handle: &AppHandle, host: Host) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show_synapt = MenuItem::with_id(handle, "show_synapt", "Show Synapt", true, None::<&str>)?;
    let show_clip = MenuItem::with_id(handle, "show_clip", "Show SynaptClip", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&show_synapt, &show_clip, &quit])?;

    let menu_host = host.clone();
    let builder = TrayIconBuilder::with_id("main")
        .tooltip("Synapt")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show_synapt" => route(app, &menu_host, TrayCommand::Show { app: AppKind::Synapt }),
            "show_clip" => route(app, &menu_host, TrayCommand::Show { app: AppKind::SynaptClip }),
            "quit" => route(app, &menu_host, TrayCommand::Quit),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_self(tray.app_handle());
            }
        });

    let builder = match handle.default_window_icon().cloned() {
        Some(icon) => builder.icon(icon),
        None => builder,
    };
    builder.build(handle)?;
    Ok(())
}

fn spawn_accept(listener: TcpListener, host: Host) {
    thread::spawn(move || {
        for conn in listener.incoming() {
            let stream = match conn {
                Ok(s) => s,
                Err(_) => continue,
            };
            let host = host.clone();
            thread::spawn(move || handle_client(stream, host));
        }
    });
}

fn handle_client(stream: TcpStream, host: Host) {
    let mut registered: Option<AppKind> = None;
    tray::read_commands(&stream, |cmd| {
        if let TrayCommand::Register { app } = cmd {
            if let Ok(write_clone) = stream.try_clone() {
                lock(&host.clients).insert(app, write_clone);
                registered = Some(app);
                if lock(&host.pending).remove(&app) {
                    if let Some(s) = lock(&host.clients).get_mut(&app) {
                        let _ = tray::write_command(s, &TrayCommand::Show { app });
                    }
                }
            }
        }
    });
    if let Some(app) = registered {
        lock(&host.clients).remove(&app);
    }
}

fn spawn_client(handle: AppHandle) {
    thread::spawn(move || loop {
        if let Some(mut stream) = tray::connect_host() {
            let _ = tray::write_command(&mut stream, &TrayCommand::Register { app: SELF_KIND });
            tray::read_commands(&stream, |cmd| handle_self_command(&handle, cmd));
        }
        // Host gone (or never reachable): try to take over the host role.
        if let Some(listener) = tray::claim_host() {
            let host = Host::default();
            let h = handle.clone();
            let host_for_tray = host.clone();
            let _ = handle.run_on_main_thread(move || {
                if let Err(e) = build_tray(&h, host_for_tray) {
                    tracing::error!("failed to rebuild shared tray after handoff: {}", e);
                }
            });
            spawn_accept(listener, host);
            return;
        }
        thread::sleep(Duration::from_millis(500));
    });
}

fn handle_self_command(handle: &AppHandle, cmd: TrayCommand) {
    match cmd {
        TrayCommand::Show { app } if app == SELF_KIND => show_self(handle),
        TrayCommand::Toggle { app } if app == SELF_KIND => toggle_self(handle),
        TrayCommand::Quit => handle.exit(0),
        _ => {}
    }
}

fn route(handle: &AppHandle, host: &Host, cmd: TrayCommand) {
    match cmd {
        TrayCommand::Show { app } | TrayCommand::Toggle { app } if app == SELF_KIND => {
            if matches!(cmd, TrayCommand::Toggle { .. }) {
                toggle_self(handle);
            } else {
                show_self(handle);
            }
        }
        TrayCommand::Show { app } | TrayCommand::Toggle { app } => {
            let sent = {
                let mut clients = lock(&host.clients);
                match clients.get_mut(&app) {
                    Some(stream) => tray::write_command(stream, &cmd).is_ok(),
                    None => false,
                }
            };
            if !sent {
                lock(&host.pending).insert(app);
                launch_sibling(app);
            }
        }
        TrayCommand::Quit => {
            let mut clients = lock(&host.clients);
            for stream in clients.values_mut() {
                let _ = tray::write_command(stream, &TrayCommand::Quit);
            }
            drop(clients);
            handle.exit(0);
        }
        TrayCommand::Register { .. } => {}
    }
}

fn show_self(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle_self(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn launch_sibling(target: AppKind) {
    let candidates: &[&str] = match target {
        AppKind::SynaptClip => &["synaptclip", "synapt-clip", "SynaptClip"],
        AppKind::Synapt => &["synapt", "Synapt"],
    };
    for name in candidates {
        if std::process::Command::new(name).spawn().is_ok() {
            return;
        }
    }
    tracing::warn!("could not launch sibling app {:?}", target);
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

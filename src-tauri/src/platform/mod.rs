#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod autostart;

/// The active display server or OS platform.
// Variants are platform-specific; not all are constructed on every target.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayServer {
    WaylandGnome,
    WaylandWlroots,
    X11,
    Windows,
    MacOS,
}

/// Detect the platform and run the current OS's setup hook.
pub fn setup_current() {
    tracing::info!("display server: {:?}", detect());
    #[cfg(target_os = "linux")]
    linux::setup();
    #[cfg(target_os = "windows")]
    windows::setup();
    // macOS setup requires the Tauri App handle and is invoked from main().
}

/// Ask the user to confirm quitting, running `on_confirm` only if they agree.
///
/// macOS shows a native NSAlert, dispatched to the main thread because
/// `runModal` drives a nested AppKit run loop. Other platforms quit straight
/// away, matching the behaviour they had before the dialog existed.
pub fn confirm_quit<F>(handle: &tauri::AppHandle, on_confirm: F)
where
    F: FnOnce() + Send + 'static,
{
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = handle.run_on_main_thread(move || {
            if macos::confirm_quit_dialog() {
                on_confirm();
            } else {
                tracing::info!("quit cancelled from the confirmation dialog");
            }
        }) {
            tracing::warn!("could not show quit confirmation on the main thread: {e}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = handle;
        on_confirm();
    }
}

/// Detect the current platform at runtime.
pub fn detect() -> DisplayServer {
    #[cfg(target_os = "windows")]
    return DisplayServer::Windows;

    #[cfg(target_os = "macos")]
    return DisplayServer::MacOS;

    #[cfg(target_os = "linux")]
    {
        match std::env::var("XDG_SESSION_TYPE").as_deref() {
            Ok("wayland") => {
                if is_wlroots_available() {
                    DisplayServer::WaylandWlroots
                } else {
                    DisplayServer::WaylandGnome
                }
            }
            _ => DisplayServer::X11,
        }
    }
}

#[cfg(target_os = "linux")]
fn is_wlroots_available() -> bool {
    std::process::Command::new("wl-paste")
        .arg("--list-types")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
fn is_wlroots_available() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_variant() {
        let ds = detect();
        // Just ensure it does not panic and returns a valid variant
        let _ = format!("{:?}", ds);
    }

    #[test]
    fn wlroots_check_does_not_panic() {
        let _ = is_wlroots_available();
    }
}

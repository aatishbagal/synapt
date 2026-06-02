pub mod linux;
pub mod windows;
pub mod macos;

/// The active display server or OS platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayServer {
    WaylandGnome,
    WaylandWlroots,
    X11,
    Windows,
    MacOS,
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

//! Cross-platform auto-start on login, without requiring elevated permissions.
//!
//! Linux uses a systemd user service, Windows a per-user `Run` registry value,
//! and macOS a LaunchAgent plist — all scoped to the current user.

use thiserror::Error;

/// Errors raised while configuring auto-start.
#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path error: {0}")]
    Path(String),
    /// Only constructed on targets other than Linux, Windows, and macOS.
    #[allow(dead_code)]
    #[error("not supported on this platform")]
    NotSupported,
}

/// Enable auto-start on login for the current platform.
pub fn enable(exe_path: &str) -> Result<(), AutostartError> {
    #[cfg(target_os = "linux")]
    {
        linux::enable(exe_path)
    }
    #[cfg(target_os = "windows")]
    {
        windows::enable(exe_path)
    }
    #[cfg(target_os = "macos")]
    {
        macos::enable(exe_path)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = exe_path;
        Err(AutostartError::NotSupported)
    }
}

/// Disable auto-start on login.
pub fn disable() -> Result<(), AutostartError> {
    #[cfg(target_os = "linux")]
    {
        linux::disable()
    }
    #[cfg(target_os = "windows")]
    {
        windows::disable()
    }
    #[cfg(target_os = "macos")]
    {
        macos::disable()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(AutostartError::NotSupported)
    }
}

/// Check whether auto-start is currently enabled.
pub fn is_enabled() -> Result<bool, AutostartError> {
    #[cfg(target_os = "linux")]
    {
        linux::is_enabled()
    }
    #[cfg(target_os = "windows")]
    {
        windows::is_enabled()
    }
    #[cfg(target_os = "macos")]
    {
        macos::is_enabled()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(AutostartError::NotSupported)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::AutostartError;
    use std::path::PathBuf;
    use std::process::Command;

    const SERVICE_NAME: &str = "synapt.service";

    fn service_path() -> Result<PathBuf, AutostartError> {
        let mut p = dirs::config_dir().ok_or_else(|| AutostartError::Path("no config dir".into()))?;
        p.push("systemd");
        p.push("user");
        Ok(p.join(SERVICE_NAME))
    }

    /// The systemd user unit that launches Synapt on graphical login.
    pub(super) fn service_contents(exe_path: &str) -> String {
        format!(
            "[Unit]\n\
             Description=Synapt launcher\n\
             After=graphical-session.target\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exe_path}\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
        )
    }

    pub fn enable(exe_path: &str) -> Result<(), AutostartError> {
        let path = service_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, service_contents(exe_path))?;
        Command::new("systemctl").args(["--user", "daemon-reload"]).status()?;
        Command::new("systemctl").args(["--user", "enable", SERVICE_NAME]).status()?;
        Ok(())
    }

    pub fn disable() -> Result<(), AutostartError> {
        let _ = Command::new("systemctl").args(["--user", "disable", SERVICE_NAME]).status();
        if let Ok(path) = service_path() {
            let _ = std::fs::remove_file(path);
        }
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, AutostartError> {
        let output = Command::new("systemctl")
            .args(["--user", "is-enabled", SERVICE_NAME])
            .output()?;
        Ok(output.status.success())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::AutostartError;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Synapt";

    pub fn enable(exe_path: &str) -> Result<(), AutostartError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (run, _) = hkcu.create_subkey(RUN_KEY)?;
        run.set_value(VALUE_NAME, &exe_path.to_string())?;
        Ok(())
    }

    pub fn disable() -> Result<(), AutostartError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE)?;
        let _ = run.delete_value(VALUE_NAME);
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, AutostartError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
            Ok(run) => Ok(run.get_value::<String, _>(VALUE_NAME).is_ok()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::AutostartError;
    use std::path::PathBuf;
    use std::process::Command;

    const LABEL: &str = "dev.synapt.app";

    fn plist_path() -> Result<PathBuf, AutostartError> {
        let mut p = dirs::home_dir().ok_or_else(|| AutostartError::Path("no home dir".into()))?;
        p.push("Library");
        p.push("LaunchAgents");
        Ok(p.join(format!("{LABEL}.plist")))
    }

    /// The LaunchAgent plist that launches Synapt at login.
    pub(super) fn plist_contents(exe_path: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict>\n\
             \x20\x20<key>Label</key><string>{LABEL}</string>\n\
             \x20\x20<key>ProgramArguments</key><array><string>{exe_path}</string></array>\n\
             \x20\x20<key>RunAtLoad</key><true/>\n\
             \x20\x20<key>KeepAlive</key><false/>\n\
             </dict></plist>\n"
        )
    }

    pub fn enable(exe_path: &str) -> Result<(), AutostartError> {
        let path = plist_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, plist_contents(exe_path))?;
        Command::new("launchctl").arg("load").arg(&path).status()?;
        Ok(())
    }

    pub fn disable() -> Result<(), AutostartError> {
        if let Ok(path) = plist_path() {
            let _ = Command::new("launchctl").arg("unload").arg(&path).status();
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, AutostartError> {
        let path = plist_path()?;
        if !path.exists() {
            return Ok(false);
        }
        let output = Command::new("launchctl").arg("list").output()?;
        Ok(String::from_utf8_lossy(&output.stdout).contains(LABEL))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: enable()/disable() are intentionally not exercised here. On Linux and
    // macOS they create and register a real per-user login service as a side
    // effect, which a test run must not do to the developer's machine. is_enabled()
    // is read-only and safe, and the unit-content helpers are pure.

    #[test]
    fn is_enabled_returns_bool_without_panicking() {
        let _ = is_enabled();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_service_contents_includes_exec_path() {
        let c = linux::service_contents("/usr/bin/synapt");
        assert!(c.contains("ExecStart=/usr/bin/synapt"));
        assert!(c.contains("WantedBy=default.target"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_plist_contents_includes_exec_path() {
        let c = macos::plist_contents("/Applications/Synapt.app/Contents/MacOS/synapt");
        assert!(c.contains("/Applications/Synapt.app/Contents/MacOS/synapt"));
        assert!(c.contains("dev.synapt.app"));
    }
}

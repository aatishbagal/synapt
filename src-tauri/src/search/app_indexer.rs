use crate::storage::{AppRow, Db, DbError};
use thiserror::Error;

/// Errors raised while discovering or indexing installed applications.
#[derive(Debug, Error)]
pub enum AppIndexError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Scan all installed applications on the current platform and populate the
/// applications table, replacing any previously indexed apps. Returns the count.
pub async fn run_app_scan(db: &Db) -> Result<usize, AppIndexError> {
    db.clear_apps().await?;
    let apps = discover_apps()?;
    let count = apps.len();
    for app in apps {
        if let Err(e) = db.upsert_app(&app).await {
            tracing::warn!("app_indexer: failed to store app {}: {}", app.name, e);
        }
    }
    tracing::info!("app_indexer: indexed {} applications", count);
    Ok(count)
}

/// Discover installed applications for the current platform.
fn discover_apps() -> Result<Vec<AppRow>, AppIndexError> {
    #[cfg(target_os = "linux")]
    return linux::discover();
    #[cfg(target_os = "windows")]
    return windows::discover();
    #[cfg(target_os = "macos")]
    return macos::discover();
    #[allow(unreachable_code)]
    Ok(vec![])
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs;

    /// Scan /usr/share/applications and ~/.local/share/applications for .desktop files.
    pub fn discover() -> Result<Vec<AppRow>, AppIndexError> {
        let mut apps = Vec::new();
        let dirs = vec![
            std::path::PathBuf::from("/usr/share/applications"),
            dirs::data_local_dir().unwrap_or_default().join("applications"),
        ];
        for dir in dirs {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Ok(app) = parse_desktop_file(&path) {
                    apps.push(app);
                }
            }
        }
        Ok(apps)
    }

    fn parse_desktop_file(path: &std::path::Path) -> Result<AppRow, AppIndexError> {
        let content = fs::read_to_string(path)?;
        parse_desktop_content(&content, &path.to_string_lossy())
    }

    /// Parse the `[Desktop Entry]` section of a .desktop file body into an [`AppRow`].
    /// Returns an error for hidden entries, non-application types, or missing fields.
    fn parse_desktop_content(content: &str, source_path: &str) -> Result<AppRow, AppIndexError> {
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut in_desktop_entry = false;
        for line in content.lines() {
            let line = line.trim();
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            }
            if line.starts_with('[') {
                in_desktop_entry = false;
            }
            if !in_desktop_entry {
                continue;
            }
            // `strip_prefix("Name=")` only matches the canonical key, so localised
            // variants such as `Name[fr]=` are skipped automatically.
            if let Some(value) = line.strip_prefix("Name=") {
                name = Some(value.to_string());
            } else if let Some(raw) = line.strip_prefix("Exec=") {
                // Strip field codes like %f %u %F %U from the exec string.
                exec = Some(
                    raw.split_whitespace()
                        .filter(|s| !s.starts_with('%'))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            } else if let Some(value) = line.strip_prefix("Icon=") {
                icon = Some(value.to_string());
            } else if line.starts_with("NoDisplay=true") || line.starts_with("Hidden=true") {
                return Err(AppIndexError::Parse("hidden app".into()));
            } else if line.starts_with("Type=") && !line.contains("Application") {
                return Err(AppIndexError::Parse("not an application".into()));
            }
        }
        match (name, exec) {
            (Some(n), Some(e)) => Ok(AppRow {
                id: 0,
                name: n,
                exec: e,
                icon_path: icon,
                platform: "linux".into(),
                source_path: source_path.to_string(),
            }),
            _ => Err(AppIndexError::Parse("missing Name or Exec".into())),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const FIREFOX: &str = "\
[Desktop Entry]
Name=Firefox
Name[fr]=Renard de feu
Exec=firefox %u
Icon=firefox
Type=Application";

        #[test]
        fn parse_desktop_returns_name_and_exec() {
            let app = parse_desktop_content(FIREFOX, "/usr/share/applications/firefox.desktop")
                .expect("should parse");
            assert_eq!(app.name, "Firefox");
            assert_eq!(app.exec, "firefox");
            assert_eq!(app.icon_path.as_deref(), Some("firefox"));
            assert_eq!(app.platform, "linux");
        }

        #[test]
        fn parse_desktop_nodisplay_returns_err() {
            let hidden = "[Desktop Entry]\nName=Secret\nExec=secret\nNoDisplay=true";
            assert!(parse_desktop_content(hidden, "/x.desktop").is_err());
        }

        #[test]
        fn parse_desktop_strips_field_codes() {
            let entry = "[Desktop Entry]\nName=Editor\nExec=editor --flag %F %u %i\nType=Application";
            let app = parse_desktop_content(entry, "/x.desktop").unwrap();
            assert_eq!(app.exec, "editor --flag");
        }

        #[test]
        fn parse_desktop_missing_exec_returns_err() {
            let entry = "[Desktop Entry]\nName=NoExec\nType=Application";
            assert!(parse_desktop_content(entry, "/x.desktop").is_err());
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    /// Scan the per-user and system Start Menu Programs directories for .lnk shortcuts.
    pub fn discover() -> Result<Vec<AppRow>, AppIndexError> {
        let mut apps = Vec::new();
        let dirs = vec![
            dirs::data_dir()
                .unwrap_or_default()
                .join("Microsoft\\Windows\\Start Menu\\Programs"),
            std::path::PathBuf::from(
                "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs",
            ),
        ];
        for dir in dirs {
            if !dir.exists() {
                continue;
            }
            scan_lnk_dir(&dir, &mut apps)?;
        }
        Ok(apps)
    }

    fn scan_lnk_dir(dir: &std::path::Path, apps: &mut Vec<AppRow>) -> Result<(), AppIndexError> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                scan_lnk_dir(&path, apps)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                apps.push(AppRow {
                    id: 0,
                    name,
                    // The .lnk path is launched via the shell, which resolves the target.
                    exec: path.to_string_lossy().to_string(),
                    icon_path: None,
                    platform: "windows".into(),
                    source_path: path.to_string_lossy().to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    /// Scan /Applications and ~/Applications for .app bundles.
    pub fn discover() -> Result<Vec<AppRow>, AppIndexError> {
        let mut apps = Vec::new();
        let dirs = vec![
            std::path::PathBuf::from("/Applications"),
            dirs::home_dir().unwrap_or_default().join("Applications"),
        ];
        for dir in dirs {
            if !dir.exists() {
                continue;
            }
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    if let Ok(app) = parse_app_bundle(&path) {
                        apps.push(app);
                    }
                }
            }
        }
        Ok(apps)
    }

    fn parse_app_bundle(path: &std::path::Path) -> Result<AppRow, AppIndexError> {
        // Prefer the display name from Contents/Info.plist, falling back to the
        // bundle file stem when the plist is absent or unreadable.
        let plist_path = path.join("Contents").join("Info.plist");
        let name = if plist_path.exists() {
            let val: plist::Value =
                plist::from_file(&plist_path).map_err(|e| AppIndexError::Parse(e.to_string()))?;
            let dict = val
                .as_dictionary()
                .ok_or_else(|| AppIndexError::Parse("plist not a dict".into()))?;
            dict.get("CFBundleDisplayName")
                .or_else(|| dict.get("CFBundleName"))
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                })
        } else {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        };
        if name.is_empty() {
            return Err(AppIndexError::Parse("empty name".into()));
        }
        Ok(AppRow {
            id: 0,
            name,
            // Launched via `open <path>` at launch time.
            exec: path.to_string_lossy().to_string(),
            icon_path: None,
            platform: "macos".into(),
            source_path: path.to_string_lossy().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_app_scan_does_not_panic_without_app_dirs() {
        // run_app_scan must complete cleanly regardless of whether any platform
        // application directories exist on the test machine.
        let db = Db::open_in_memory().await.unwrap();
        let result = run_app_scan(&db).await;
        assert!(result.is_ok(), "app scan should not error: {result:?}");
    }
}

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
        let mut app = parse_desktop_content(&content, &path.to_string_lossy())?;
        // Resolve the raw `Icon=` value (a theme name or path) to a renderable
        // file path, or None when no icon file can be found.
        app.icon_path = app.icon_path.as_deref().and_then(resolve_icon);
        Ok(app)
    }

    /// Resolve a .desktop `Icon=` value to an absolute PNG/SVG path, or None.
    /// Absolute paths are returned as-is when they exist; theme names are looked
    /// up in the standard XDG hicolor and pixmaps directories.
    fn resolve_icon(name_or_path: &str) -> Option<String> {
        if name_or_path.starts_with('/') {
            return std::path::Path::new(name_or_path)
                .exists()
                .then(|| name_or_path.to_string());
        }
        let name = name_or_path;
        let local = dirs::data_local_dir().unwrap_or_default();
        let search_dirs = [
            local.join(format!("icons/hicolor/48x48/apps/{}.png", name)),
            local.join(format!("icons/hicolor/scalable/apps/{}.svg", name)),
            std::path::PathBuf::from(format!("/usr/share/icons/hicolor/48x48/apps/{}.png", name)),
            std::path::PathBuf::from(format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", name)),
            std::path::PathBuf::from(format!("/usr/share/icons/hicolor/128x128/apps/{}.png", name)),
            std::path::PathBuf::from(format!("/usr/share/pixmaps/{}.png", name)),
            std::path::PathBuf::from(format!("/usr/share/pixmaps/{}.svg", name)),
        ];
        search_dirs
            .into_iter()
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
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

        #[test]
        fn resolve_icon_returns_existing_absolute_path() {
            let file = std::env::temp_dir().join(format!("synapt_icon_{}.png", uuid::Uuid::new_v4()));
            std::fs::write(&file, b"x").unwrap();
            let path = file.to_string_lossy().to_string();
            assert_eq!(resolve_icon(&path), Some(path.clone()));
            std::fs::remove_file(&file).ok();
        }

        #[test]
        fn resolve_icon_missing_absolute_path_returns_none() {
            let path = format!("/nonexistent/{}/icon.png", uuid::Uuid::new_v4());
            assert_eq!(resolve_icon(&path), None);
        }

        #[test]
        fn resolve_icon_unknown_theme_name_returns_none() {
            // A name that cannot exist in any icon directory resolves to None.
            let name = format!("synapt-no-such-icon-{}", uuid::Uuid::new_v4());
            assert_eq!(resolve_icon(&name), None);
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
                    // Extract the shortcut's icon to a PNG cache file; None if it fails.
                    icon_path: extract_icon(&path),
                    platform: "windows".into(),
                    source_path: path.to_string_lossy().to_string(),
                });
            }
        }
        Ok(())
    }

    /// Extract a shortcut's icon, encode it as a PNG, cache it under the user's
    /// cache directory, and return that path. None if extraction fails.
    fn extract_icon(path: &std::path::Path) -> Option<String> {
        use sha2::{Digest, Sha256};
        let png = icon_png_bytes(path)?;
        let cache_dir = dirs::cache_dir()?.join("synapt").join("app-icons");
        std::fs::create_dir_all(&cache_dir).ok()?;
        let digest = Sha256::digest(path.to_string_lossy().as_bytes());
        let out = cache_dir.join(format!("{:x}.png", digest));
        std::fs::write(&out, &png).ok()?;
        Some(out.to_string_lossy().to_string())
    }

    /// Load the file's associated large icon via the shell and encode it as PNG
    /// RGBA bytes. The shell resolves a `.lnk` to its target's icon.
    fn icon_png_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
        use std::os::windows::ffi::OsStrExt;
        use ::windows::core::PCWSTR;
        use ::windows::Win32::UI::Shell::{
            SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
        };
        use ::windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            let mut shfi = SHFILEINFOW::default();
            let ok = SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                Default::default(),
                Some(&mut shfi as *mut _),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if ok == 0 || shfi.hIcon.0.is_null() {
                return None;
            }
            let result = hicon_to_png(shfi.hIcon);
            let _ = DestroyIcon(shfi.hIcon);
            result
        }
    }

    /// Convert an HICON to PNG (RGBA) bytes via its color bitmap.
    unsafe fn hicon_to_png(
        hicon: ::windows::Win32::UI::WindowsAndMessaging::HICON,
    ) -> Option<Vec<u8>> {
        use ::windows::Win32::Graphics::Gdi::{
            DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
        };
        use ::windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

        let mut info = ICONINFO::default();
        GetIconInfo(hicon, &mut info).ok()?;
        let color = info.hbmColor;
        let mask = info.hbmMask;

        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            HGDIOBJ(color.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as *mut core::ffi::c_void),
        );
        let width = bmp.bmWidth;
        let height = bmp.bmHeight;
        if got == 0 || width <= 0 || height <= 0 {
            let _ = DeleteObject(HGDIOBJ(color.0));
            let _ = DeleteObject(HGDIOBJ(mask.0));
            return None;
        }

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // top-down rows
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0 as u32;

        let mut buf = vec![0u8; (width * height * 4) as usize];
        let hdc = GetDC(None);
        let scan = GetDIBits(
            hdc,
            color,
            0,
            height as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        if scan == 0 {
            return None;
        }

        // GetDIBits yields BGRA; PNG wants RGBA.
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }

        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&buf).ok()?;
        }
        Some(out)
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

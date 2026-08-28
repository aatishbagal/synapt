//! Crash reporting: a panic hook that leaves a readable report on disk.

use std::io::Write;
use std::panic;
use std::path::PathBuf;

/// Name shown at the top of the report.
const APP_NAME: &str = "Synapt";
/// Directory under the platform data dir that holds the log.
const DATA_DIR: &str = "synapt";
/// Where the user is asked to file the report.
const ISSUES_URL: &str = "https://github.com/aatishbagal/synapt/issues";

/// Install the global panic hook. Call once, first thing in `main`.
///
/// Reports are appended rather than overwritten, so a crash loop leaves the
/// whole history instead of only its last iteration.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Run the default hook first so behaviour under a terminal, and the
        // backtrace RUST_BACKTRACE asks for, are unchanged.
        default_hook(info);

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let report = format!(
            "{APP_NAME} crash report\n\
             ===================\n\
             Version:   {}\n\
             Timestamp: {}\n\
             OS:        {} {}\n\
             \n\
             Panic location: {location}\n\
             Panic message:  {}\n\
             \n\
             Please report this at {ISSUES_URL}\n\
             Attach this file when filing the report.\n\n",
            env!("CARGO_PKG_VERSION"),
            chrono::Utc::now().to_rfc3339(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            payload_message(info),
        );

        // Deliberately not `tracing` here: the panic may have come from inside
        // the logging stack, and stderr is the one sink still safe to use while
        // the process is unwinding.
        match write_report(&report) {
            Ok(path) => eprintln!("Crash log written to: {}", path.display()),
            Err(e) => eprintln!("Could not write crash log: {e}"),
        }
    }));
}

/// Absolute path of the crash log, whether or not it exists yet.
pub fn log_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(DATA_DIR).join("crash.log"))
}

/// Pull a readable message out of a panic payload.
fn payload_message(info: &panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Append `report` to the crash log, returning where it landed.
fn write_report(report: &str) -> std::io::Result<PathBuf> {
    let path = log_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no platform data directory")
    })?;
    append_report(&path, report)?;
    Ok(path)
}

/// Append `report` to `path`, creating any missing parent directories.
///
/// Split out from [`write_report`] so the file handling can be exercised
/// against a temporary path instead of the real crash log.
fn append_report(path: &std::path::Path, report: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    write!(file, "{report}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log() -> PathBuf {
        std::env::temp_dir()
            .join(format!("synapt_crash_{}", uuid::Uuid::new_v4()))
            .join("crash.log")
    }

    #[test]
    fn log_path_sits_under_the_app_data_dir() {
        let path = log_path().expect("a data dir exists on every supported platform");
        assert!(path.ends_with("crash.log"));
        assert!(path.to_string_lossy().contains(DATA_DIR));
    }

    #[test]
    fn append_report_creates_missing_parent_directories() {
        let path = temp_log();
        assert!(!path.exists());
        append_report(&path, "report\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "report\n");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn append_report_keeps_earlier_crashes() {
        let path = temp_log();
        append_report(&path, "first\n").unwrap();
        append_report(&path, "second\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}

/// Platform-specific setup hook for macOS.
///
/// Hides the app from the Dock (tray-only app) by setting the activation policy
/// to accessory. The system tray uses Tauri's built-in tray (NSStatusBar) and
/// does not depend on AppIndicator/libayatana.
///
/// Also disables the main window's drop shadow, since a transparent undecorated
/// window otherwise gets a default AppKit shadow that shows up as a faint halo
/// around the CSS-rounded overlay content.
// The `objc` crate's `msg_send!`/`class!` macros expand a `cfg(feature = "cargo-clippy")`
// check that clippy's stable check-cfg lint doesn't recognize; this is upstream, not ours.
#[allow(unexpected_cfgs)]
pub fn setup(app: &tauri::App) {
    use tauri::Manager;

    // NSApplicationActivationPolicyAccessory = 1 (no Dock icon).
    unsafe {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: 1_isize];
    }
    tracing::info!("macOS: set activation policy to accessory (no dock icon)");

    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.set_shadow(false) {
            tracing::warn!("macOS: failed to disable window shadow: {e}");
        }
    }
}

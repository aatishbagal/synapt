/// Platform-specific setup hook for macOS.
///
/// Hides the app from the Dock (tray-only app) by setting the activation policy
/// to accessory. The system tray uses Tauri's built-in tray (NSStatusBar) and
/// does not depend on AppIndicator/libayatana.
pub fn setup(app: &tauri::App) {
    let _ = app;
    // NSApplicationActivationPolicyAccessory = 1 (no Dock icon).
    unsafe {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: 1_isize];
    }
    tracing::info!("macOS: set activation policy to accessory (no dock icon)");
}

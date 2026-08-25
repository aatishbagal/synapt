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
/// Show a native NSAlert asking the user to confirm quitting.
///
/// Returns true when the user chooses Quit. Must be called on the main thread,
/// since `runModal` drives a nested AppKit run loop.
// The `objc` crate's `msg_send!`/`class!` macros expand a `cfg(feature = "cargo-clippy")`
// check that clippy's stable check-cfg lint doesn't recognize; this is upstream, not ours.
#[allow(unexpected_cfgs)]
pub fn confirm_quit_dialog() -> bool {
    use objc::runtime::{Object, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CString;

    // Returned by runModal for the first button added to the alert.
    const NS_ALERT_FIRST_BUTTON_RETURN: i64 = 1000;
    // NSAlertStyleInformational.
    const NS_ALERT_STYLE_INFORMATIONAL: usize = 1;

    let strings = (
        CString::new("Quit Synapt?"),
        CString::new(
            "Your paired devices and file index are kept. Synapt will stop responding to its global shortcut until you launch it again.",
        ),
        CString::new("Quit"),
        CString::new("Cancel"),
    );
    let (message, informative, quit, cancel) = match strings {
        (Ok(m), Ok(i), Ok(q), Ok(c)) => (m, i, q, c),
        _ => {
            tracing::warn!("macOS: could not build quit dialog text, quitting without confirmation");
            return true;
        }
    };

    unsafe {
        // Synapt runs as an accessory app with no Dock icon, so it is not the
        // active application. Without activating first the alert opens behind
        // whatever the user is currently looking at.
        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: YES];

        let alert: *mut Object = msg_send![class!(NSAlert), alloc];
        let alert: *mut Object = msg_send![alert, init];
        let _: () = msg_send![alert, setAlertStyle: NS_ALERT_STYLE_INFORMATIONAL];
        let _: () = msg_send![alert, setMessageText: ns_string(&message)];
        let _: () = msg_send![alert, setInformativeText: ns_string(&informative)];
        // The first button added is the default, triggered by Return.
        let _: () = msg_send![alert, addButtonWithTitle: ns_string(&quit)];
        let _: () = msg_send![alert, addButtonWithTitle: ns_string(&cancel)];

        let response: i64 = msg_send![alert, runModal];
        let _: () = msg_send![alert, release];

        response == NS_ALERT_FIRST_BUTTON_RETURN
    }
}

/// Wrap a NUL-terminated C string in an autoreleased NSString.
///
/// # Safety
/// Must be called on the main thread, where AppKit keeps an autorelease pool
/// around for the duration of the current event loop iteration.
#[allow(unexpected_cfgs)]
unsafe fn ns_string(text: &std::ffi::CStr) -> *mut objc::runtime::Object {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    let s: *mut Object = msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()];
    s
}

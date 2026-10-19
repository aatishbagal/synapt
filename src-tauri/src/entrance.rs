//! Hiding the main window so its next show starts from the entrance animation.
//!
//! macOS puts a re-shown window on screen with the last frame it painted before
//! it was hidden, and the webview only replays its entrance once it hears about
//! the focus that follows. A window hidden while fully drawn therefore flashed
//! fully drawn on the next show and then snapped back to replay the entrance.
//! Every hide of the main window goes through [`hide`], which first has the
//! webview drop to the entrance's start state, so the frame left behind is the
//! one the entrance begins from.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{Emitter, WebviewWindow};

/// Event the webview answers by resetting to its entrance start state.
const WILL_HIDE_EVENT: &str = "window:will-hide";

/// Time for the webview to handle the event and paint the reset frame.
const PAINT_DELAY: Duration = Duration::from_millis(60);

/// Bumped on every show, so a hide still waiting out its delay can tell the
/// window was shown again meanwhile and leave it on screen.
static SHOW_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Record that the main window is being shown. Call before every show.
pub fn note_show() {
    SHOW_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Reset the webview to its entrance start state, then hide the window.
pub fn hide(window: &WebviewWindow) {
    let generation = SHOW_GENERATION.load(Ordering::SeqCst);
    if let Err(e) = window.emit(WILL_HIDE_EVENT, ()) {
        tracing::warn!("could not emit {WILL_HIDE_EVENT}: {e}");
    }
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PAINT_DELAY).await;
        if SHOW_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        if let Err(e) = window.hide() {
            tracing::warn!("could not hide the main window: {e}");
        }
    });
}

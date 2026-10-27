//! Continuity Camera: Import from iPhone or iPad (Take Photo, Scan
//! Documents, Add Sketch) in F2's right-click menus, as Finder has it, for
//! a folder, or the one a window shows. What the iPhone sends is saved
//! into it (see `macos/menu.m`: a scan as a PDF, a photo as it came), and
//! a note says so; the window shows it as it would any new file.

use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::sync::OnceLock;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::App;

static SAVED: OnceLock<UnboundedSender<PathBuf>> = OnceLock::new();

extern "C" fn on_saved(path: *const c_char) {
    if path.is_null() {
        return;
    }
    let path = PathBuf::from(unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned());
    if let Some(tx) = SAVED.get() {
        let _ = tx.unbounded_send(path);
    }
}

pub fn init(cx: &mut App) {
    let (tx, mut rx) = unbounded();
    let _ = SAVED.set(tx);
    crate::macos::import_listen(on_saved);
    crate::macos::continuity_setup();
    cx.spawn(async move |cx| {
        while let Some(path) = rx.next().await {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let folder = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let what = if path.extension().is_some_and(|e| e == "pdf") { "Scan Saved" } else { "Photo Saved" };
            cx.update(|cx| crate::shots::note(format!("{what} from iPhone"), format!("{name}, in {folder}"), None, cx));
        }
    })
    .detach();
}

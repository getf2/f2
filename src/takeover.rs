//! Finder2 instead of Finder, as far as macOS lets it. Finder can't be
//! removed (nor its Dock icon), and macOS 26 keeps opening folders for
//! itself (Launch Services refuses another app for public.folder). What
//! can come here: other apps' "Show in Finder" (NSFileViewer); and, as an
//! experiment, the windows Finder opens (from its Dock icon, or a folder
//! another app opens), closed there and shown here instead.

use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedSender;
use futures::future::Either;
use gpui::*;

use crate::macos;
use crate::settings::Settings;

pub const BUNDLE_ID: &str = "dev.newfinder.NewFinder";

/// Other apps' "Show in Finder" comes here.
pub fn is_default() -> bool {
    Command::new("/usr/bin/defaults")
        .args(["read", "-g", "NSFileViewer"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == BUNDLE_ID)
}

/// Send it here (or back to Finder).
pub fn set_default(ours: bool) -> Result<(), String> {
    let args: Vec<&str> = if ours { vec!["write", "-g", "NSFileViewer", "-string", BUNDLE_ID] } else { vec!["delete", "-g", "NSFileViewer"] };
    let out = Command::new("/usr/bin/defaults").args(args).output().map_err(|e| e.to_string())?;
    // (Deleting what isn't there fails, and is fine.)
    if out.status.success() || !ours { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Finder came to the front (see `watch_finder`).
static FORWARD: OnceLock<UnboundedSender<()>> = OnceLock::new();

extern "C" fn finder_came_forward() {
    if let Some(tx) = FORWARD.get() {
        let _ = tx.unbounded_send(());
    }
}

/// How long after Finder comes forward its new window is looked for
/// closely (it shows a moment after: its Dock icon opens one).
const CLOSELY: Duration = Duration::from_millis(1500);

/// With Settings' take_finder_windows on: while Finder is in front, show
/// each window it opens here instead (one that isn't of a folder, as its
/// Dock icon's Recents, as a new window). F2's window comes up over it
/// first, then it's closed behind, so nothing's gone missing in between.
/// Each window is looked at once, so AirDrop's, which stays, or one F2 may
/// not close (no permission), is left be.
pub fn watch_finder(cx: &mut App) {
    let (tx, mut forward) = futures::channel::mpsc::unbounded();
    if FORWARD.set(tx).is_ok() {
        macos::on_finder_forward(finder_came_forward);
    }
    cx.spawn(async move |cx| {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut tried: HashSet<i64> = HashSet::new();
        let mut closely_until = Instant::now();
        loop {
            let on = cx.update(|cx| Settings::get(cx).take_finder_windows);
            if !on {
                seen.clear();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                continue;
            }
            // Now and then; every frame for a moment after Finder comes
            // forward, when its window's about to show.
            let wait = if Instant::now() < closely_until { Duration::from_millis(16) } else { Duration::from_millis(150) };
            let timer = cx.background_executor().timer(wait);
            if let Either::Right(_) = futures::future::select(timer, forward.next()).await {
                closely_until = Instant::now() + CLOSELY;
            }
            let Some(windows) = cx.background_executor().spawn(async { macos::front_finder_windows() }).await else { continue };
            // (Forget the closed, so window numbers used again count anew.)
            seen.retain(|id| windows.contains(id));
            if windows.iter().all(|id| seen.contains(id)) {
                continue;
            }
            seen.extend(&windows);
            let noticed = Instant::now();
            let asked = cx.background_executor().spawn(async { macos::finder_windows().map(|w| (w, macos::read_only_image_volumes())) }).await;
            let Some((found, images)) = asked else { continue };
            tried.retain(|id| found.iter().any(|w| w.id == *id));
            // Not AirDrop's (`airdrop` uses it), nor a read-only disk
            // image's: an app's DMG, laid out for dragging it to
            // Applications.
            let take: Vec<macos::FinderWindow> = found
                .into_iter()
                .filter(|w| !tried.contains(&w.id))
                .filter(|w| match &w.folder {
                    Some(folder) => !images.contains(folder),
                    None => w.name != "AirDrop",
                })
                .collect();
            if take.is_empty() {
                continue;
            }
            tried.extend(take.iter().map(|w| w.id));
            cx.update(|cx| {
                for w in &take {
                    // (Not a folder: what a new window shows here.)
                    let folder = w.folder.clone().unwrap_or_else(|| crate::finder::new_window_folder(cx));
                    crate::browser::open(folder, None, cx);
                }
                cx.activate(true);
            });
            let shown = noticed.elapsed();
            // Closed once F2's is up in front of it.
            cx.background_executor().timer(Duration::from_millis(150)).await;
            let ids: Vec<i64> = take.iter().map(|w| w.id).collect();
            cx.background_executor().spawn(async move { macos::close_finder_windows(&ids) }).await;
            eprintln!("Finder's window shown here {} ms after it was noticed", shown.as_millis());
        }
    })
    .detach();
}

//! About F2, the open-source licenses it's built with, updates (Sparkle,
//! in a release: see scripts/release-macos.sh), and its log.

use gpui::*;

use crate::{APP_NAME, macos};

actions!(about, [About, CheckForUpdates, Acknowledgements, ShowLog]);

pub fn init(cx: &mut App) {
    let updates = macos::updates_start();
    eprintln!("updates {}", if updates { "on" } else { "off (not a release build)" });
    cx.on_action(|_: &About, _| show());
    cx.on_action(|_: &CheckForUpdates, cx| check(cx));
    cx.on_action(|_: &Acknowledgements, cx| acknowledgements(cx));
    cx.on_action(|_: &ShowLog, cx| show_log(cx));
}

/// The version as the About panel shows it: "0.1.0 (412)".
pub fn version() -> String {
    let short = macos::bundle_value("CFBundleShortVersionString").unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
    match macos::bundle_value("CFBundleVersion") {
        Some(build) => format!("{short} ({build})"),
        None => short,
    }
}

/// macOS's About panel (the version and copyright from Info.plist).
pub fn show() {
    macos::about(&format!(
        "A file browser, launcher and toolbox for your Mac.\nBuilt with GPUI and fsearch.\n\nHelp > Acknowledgements lists the open-source software in {APP_NAME}."
    ));
}

/// Look for an update now (Sparkle's window says what it finds).
pub fn check(cx: &mut App) {
    if macos::updates_check() {
        cx.activate(true);
    } else {
        crate::shots::note("Updates are off".into(), format!("This {APP_NAME} wasn't built as a release, so it doesn't update itself."), None, cx);
    }
}

/// The licenses of everything F2 is built with (written into the app
/// when it's bundled).
pub fn acknowledgements(cx: &mut App) {
    match macos::resource("Acknowledgements.txt") {
        Some(path) => cx.open_with_system(&path),
        None => crate::shots::note("No acknowledgements here".into(), format!("They're in {APP_NAME}.app (see scripts/bundle-macos.sh)."), None, cx),
    }
}

/// The log, in Console.
pub fn show_log(cx: &mut App) {
    let path = crate::log::file();
    if path.exists() {
        cx.open_with_system(&path);
    } else {
        crate::shots::note("No log yet".into(), format!("{APP_NAME} prints to the terminal it's run from."), None, cx);
    }
}

//! Finder2: a Finder alternative on GPUI, searching with fsearch.

// GPUI's derive macros expand to `gpui::` paths; gpui-kit stands in for it.
extern crate gpui_kit as gpui;

mod about;
mod air;
mod apps;
mod archives;
mod awake;
mod bill;
mod busy;
mod calc;
mod calendar;
mod chance;
mod cleanup;
mod clips;
mod colors;
mod commands;
mod continuity;
mod currency;
mod dates;
mod dev;
mod devices;
mod dictation;
mod dictionary;
mod disk_images;
mod ds_store;
mod browser;
mod disk_usage;
mod downloads;
mod emoji;
mod extensions;
mod fileops;
mod finder;
mod flights;
mod holidays;
mod hotkeys;
mod icons;
mod info;
mod keyboard;
mod keymap;
mod kitchen;
mod listing;
mod media;
mod log;
mod macos;
mod maps;
mod menus;
mod music;
mod natural;
mod net;
mod notes;
mod passwords;
mod people;
mod prices;
mod onboarding;
mod online;
mod permissions;
mod quick_actions;
mod qr;
mod quick_panel;
mod recent;
mod reminders;
mod remote;
mod reports;
mod resize;
mod rules;
mod search;
mod screens;
mod settings;
mod settings_window;
mod sftp;
mod shots;
mod sizes;
mod snippets;
mod store;
mod status_item;
mod sun;
mod style;
mod takeover;
mod tidy;
mod this_mac;
mod timers;
mod tracking;
mod unfinished;
mod toggles;
mod tools;
mod translate;
mod uninstall;
mod usage;
mod weather;
mod windows;
mod words;
mod world_time;

use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;

use gpui::*;
use gpui_kit::component::Theme;

actions!(new_finder, [Quit, CloseWindow, CloseAllWindows, OpenDiskUsage, ShowWelcome]);

/// The search engine's allocator: blocks of a megabyte and up straight from
/// and back to the system (see `fsearch::alloc`).
#[global_allocator]
static GLOBAL: fsearch::alloc::Alloc = fsearch::alloc::Alloc;

/// What the app calls itself (menus, messages).
pub const APP_NAME: &str = "F2";

/// Bring a window of ours forward, if it's still open: whether it is. (By
/// the list of windows, and after this: asked from inside that very window,
/// ⌘, in Settings, updating it now fails, and another was opened.)
pub fn raise(handle: AnyWindowHandle, cx: &mut App) -> bool {
    if !cx.windows().iter().any(|w| w.window_id() == handle.window_id()) {
        return false;
    }
    cx.defer(move |cx| {
        handle.update(cx, |_, window, _| window.activate_window()).ok();
    });
    true
}

fn main() {
    // An extension's JavaScript host (see `extensions`): only that, then out.
    if std::env::var_os("F2_EXTENSION_HOST").is_some() {
        std::process::exit(macos::js_host());
    }
    // Run by ssh to ask something (see `sftp`): only that, then out.
    if std::env::var_os("F2_ASKPASS").is_some() {
        let prompt = std::env::args().nth(1).unwrap_or_default();
        let kind = std::env::var("SSH_ASKPASS_PROMPT").ok();
        match macos::askpass(&prompt, kind.as_deref()) {
            Some(answer) => {
                println!("{answer}");
                std::process::exit(0)
            }
            None => std::process::exit(1),
        }
    }
    space_efficient();
    log::init();
    let search = match search::Search::start() {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            eprintln!("fsearch failed to start, search is off: {e}");
            None
        }
    };
    // The folder to open, if one's given. (Not "-psn_..." from older macOS,
    // nor a setting like "-AppleLanguages (en-GB)" and its value.)
    let mut args = std::env::args_os().skip(1);
    let mut start = None;
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        if text.starts_with("-psn") {
            continue;
        }
        if text.starts_with('-') {
            args.next();
            continue;
        }
        start = Some(PathBuf::from(&arg));
        break;
    }
    let start = start.map(|p| std::fs::canonicalize(&p).unwrap_or(p));

    // Folders and files sent to the app (`open -a`, dropped on its Dock
    // icon, "Show in Finder" once it's the file viewer) open in a window.
    let (sent_tx, mut sent) = futures::channel::mpsc::unbounded::<Vec<String>>();
    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    app.on_open_urls(move |urls| {
        let _ = sent_tx.unbounded_send(urls);
    });
    app.on_reopen(|cx| {
        // Clicking the Dock icon with no window open: open one.
        if cx.windows().is_empty() {
            browser::open(finder::new_window_folder(cx), None, cx);
        }
    });
    app.run(move |cx: &mut App| {
        gpui_kit::init(cx);
        style::load_fonts(cx);
        Theme::sync_system_appearance(None, cx);
        cx.set_global(settings::Settings::load());
        cx.set_global(browser::Shared { search });
        finder::bind_keys(cx);
        browser::init(cx);
        quick_actions::warm();
        // (⌘W closes a tab in a browser window: this is for Get Info and
        // Settings. A binding without a context would outrank the tab's.)
        // ⌘Q closes the windows but keeps Finder2 running, as Finder is:
        // it's quit from its menu bar item (or the Dock, or logging out).
        cx.bind_keys([KeyBinding::new("cmd-q", CloseAllWindows, None), KeyBinding::new("cmd-w", CloseWindow, Some("Utility"))]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CloseAllWindows, cx| {
            let windows = cx.windows();
            cx.defer(move |cx| {
                for window in windows {
                    window.update(cx, |_, window, _| window.remove_window()).ok();
                }
                // Out of the way, as if quit: the app before it comes forward.
                cx.hide();
            });
        });
        // ⌘W in Get Info and Settings (a browser window closes a tab).
        cx.on_action(|_: &CloseWindow, cx| {
            // (After this key's handling: the window is busy with it now.)
            if let Some(window) = cx.active_window() {
                cx.defer(move |cx| {
                    window.update(cx, |_, window, _| window.remove_window()).ok();
                });
            }
        });
        cx.on_action(|_: &finder::OpenSettings, cx| settings_window::open(cx));
        cx.on_action(|_: &OpenDiskUsage, cx| disk_usage::open(cx));
        // (With no window to take it, as when F2 starts in the menu bar: Quick
        // Open from the File menu all the same.)
        cx.on_action(|_: &finder::QuickOpen, cx| quick_panel::show(None, cx));
        // (Quick Look's panel made once F2's up: the first look of the
        // session isn't the slow one.)
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(3)).await;
            cx.update(|_| macos::quicklook_warm());
        })
        .detach();
        cx.on_action(|_: &ShowWelcome, cx| onboarding::open(cx));
        about::init(cx);
        menus::set(cx);
        // (Once every key's bound: Finder's kept, Settings' changes over them.)
        keymap::init(cx);
        status_item::install(cx);
        takeover::watch_finder(cx);
        reports::init(cx);
        hotkeys::init(cx);
        keyboard::init(cx);
        quick_panel::init(cx);
        shots::init(cx);
        continuity::init(cx);
        clips::init(cx);
        windows::init(cx);
        snippets::init(cx);
        dictation::init(cx);
        // Servers over SSH: ssh asks for passwords in F2's dialog (F2 run
        // again, see the top of `main`), and files opened from a server go
        // back when they're saved.
        sftp::set_askpass(std::env::current_exe().ok());
        let (saved_tx, mut saved) = futures::channel::mpsc::unbounded::<remote::Saved>();
        remote::watch(move |s| {
            let _ = saved_tx.unbounded_send(s);
        });
        cx.spawn(async move |cx| {
            while let Some((path, result)) = saved.next().await {
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let server = remote::here(&path).map(|h| h.server).unwrap_or_default();
                let message = match result {
                    Ok(()) => format!("Saved “{name}” to {server}."),
                    Err(e) => format!("Couldn't save “{name}” to {server}: {e}"),
                };
                cx.update(|cx| browser::notice(message, cx));
            }
        })
        .detach();
        std::thread::spawn(archives::tidy);
        tidy::watch_desktop(cx);
        rules::watch(cx);
        // Frames kept for drawing, as Settings say (and when they change).
        let mut lean = None;
        let mut drawing = move |cx: &mut App| {
            let now = settings::Settings::get(cx).lean_drawing;
            if lean != Some(now) {
                lean = Some(now);
                macos::set_drawables(if now { 2 } else { 3 });
            }
        };
        drawing(cx);
        cx.observe_global::<settings::Settings>(drawing).detach();

        // What was sent before launch finished opens instead of the usual
        // first window.
        let mut pending = Vec::new();
        while let Ok(urls) = sent.try_recv() {
            pending.extend(urls);
        }
        // The first time: the welcome, and a window once it's done (unless
        // a folder was asked for: then that, the welcome over it). Else a
        // window, unless Settings say not to (and nothing was asked for):
        // then F2 starts in the menu bar, not in front.
        let welcome = !settings::Settings::get(cx).onboarded;
        let window = settings::Settings::get(cx).open_window_at_launch;
        let mut shown = true;
        if pending.is_empty() && start.is_none() && welcome {
            onboarding::open(cx);
        } else {
            if !pending.is_empty() {
                open_urls(pending, cx);
            } else if start.is_some() || window {
                browser::open(start.unwrap_or_else(|| finder::new_window_folder(cx)), None, cx);
            } else {
                shown = false;
            }
            if welcome {
                onboarding::open_soon(cx);
            }
        }
        cx.spawn(async move |cx| {
            while let Some(urls) = sent.next().await {
                cx.update(|cx| open_urls(urls, cx));
            }
        })
        .detach();
        if shown {
            cx.activate(true);
        }
    });
}

/// macOS's malloc keeps the small blocks it frees dirty: what the search
/// engine frees after reading its index in, and after each search, stayed
/// F2's (30 MB and more). In its space-efficient mode, read only as a
/// process starts, it hands them back, for searches a little slower.
/// LaunchServices starts F2 in it (Info.plist's LSEnvironment); here it's
/// only kept from what F2 runs. (Not by F2 starting itself again in it, as
/// fsearch's daemon does: macOS then hid its menu bar item.)
fn space_efficient() {
    // (Safety: nothing else runs yet to read the environment.)
    unsafe { std::env::remove_var("MallocSpaceEfficient") };
}

/// A window for each folder sent; a file's folder, with it selected.
fn open_urls(urls: Vec<String>, cx: &mut App) {
    for url in urls {
        let Some(path) = file_url_path(&url) else { continue };
        // (A disk image, F2 being what opens them: see `disk_images`.)
        if disk_images::is_disk_image(&path) {
            disk_images::open(path, None, cx);
            continue;
        }
        if path.is_dir() && !listing::is_package(&path) {
            browser::open(path, None, cx);
        } else if let Some(parent) = path.parent() {
            browser::open(parent.to_path_buf(), Some(path.clone()), cx);
        }
    }
    cx.activate(true);
}

/// The path of a `file://` URL ("%20" and the like decoded).
fn file_url_path(url: &str) -> Option<PathBuf> {
    let encoded = url.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = encoded.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(out));
    Some(std::fs::canonicalize(&path).unwrap_or(path))
}

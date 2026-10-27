//! Screenshots, as BetterShot and CleanShot take them: an area (Space
//! there picks a window instead), a window, the screen, the text in an
//! area, or a color, each from a shortcut in any app (Settings choose
//! them), the menu bar item or Quick Open. macOS's own `screencapture`
//! takes them, with its crosshair. Then the screenshot is copied, saved if
//! Settings say so, and shown in the corner: to drag into an app, copy,
//! save, pin over everything, or edit (arrows, shapes, text, numbered
//! steps, blur, crop, a background).
//!
//! Each is kept in the data folder (the newest 50), for the corner, the
//! editor, and Quick Open's Screenshots (see `history`).

mod editor;
mod marks;
mod pin;
mod preview;
pub mod record;
mod scrolling;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;

use crate::macos;
use crate::settings::{Settings, Shots};

pub use editor::open as edit;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Area,
    Window,
    Screen,
    Text,
    Color,
    /// A QR code (or barcode) in an area, read.
    Qr,
    /// A screen recording: the screen the pointer's on, or an area (see
    /// `record`). Again: stopped.
    RecordScreen,
    RecordArea,
    /// An area, scrolled down and taken all the way (see `scrolling`).
    Scrolling,
}

impl Action {
    pub const ALL: [Action; 9] = [Action::Area, Action::Window, Action::Scrolling, Action::Screen, Action::Text, Action::Qr, Action::Color, Action::RecordScreen, Action::RecordArea];

    /// Its shortcut's id (see `hotkeys`).
    fn id(self) -> u32 {
        10 + self as u32
    }

    pub fn title(self) -> &'static str {
        match self {
            Action::Area => "Capture Area",
            Action::Window => "Capture Window",
            Action::Screen => "Capture Screen",
            Action::Text => "Capture Text",
            Action::Color => "Pick Color",
            Action::Qr => "Scan QR Code",
            Action::RecordScreen => "Record Screen",
            Action::RecordArea => "Record Area",
            Action::Scrolling => "Scrolling Capture",
        }
    }

    /// What it does, for Settings and Quick Open.
    pub fn about(self) -> &'static str {
        match self {
            Action::Area => "Drag out an area (Space: a window instead)",
            Action::Window => "Click a window",
            Action::Screen => "The screen the pointer is on",
            Action::Text => "Copy the text in an area",
            Action::Color => "Copy a color on the screen, kept in Colors",
            Action::Qr => "Read a QR code on the screen",
            Action::RecordScreen => "A video of the screen the pointer is on (again: stop)",
            Action::RecordArea => "A video of an area you drag out (Esc: never mind; again: stop)",
            Action::Scrolling => "Drag out an area: it scrolls and takes all of it (Esc: stop)",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Action::Area => "rectangle.dashed",
            Action::Window => "macwindow",
            Action::Screen => "display",
            Action::Text => "text.viewfinder",
            Action::Color => "eyedropper",
            Action::Qr => "qrcode.viewfinder",
            Action::RecordScreen => "record.circle",
            Action::RecordArea => "rectangle.dashed.badge.record",
            Action::Scrolling => "scroll",
        }
    }

    pub fn shortcut(self, shots: &Shots) -> Option<&str> {
        match self {
            Action::Area => shots.area.as_deref(),
            Action::Window => shots.window.as_deref(),
            Action::Screen => shots.screen.as_deref(),
            Action::Text => shots.text.as_deref(),
            Action::Color => shots.color.as_deref(),
            Action::Qr => shots.qr.as_deref(),
            Action::RecordScreen => shots.record_screen.as_deref(),
            Action::RecordArea => shots.record_area.as_deref(),
            Action::Scrolling => shots.scrolling.as_deref(),
        }
    }

    pub fn set_shortcut(self, shots: &mut Shots, shortcut: Option<String>) {
        *match self {
            Action::Area => &mut shots.area,
            Action::Window => &mut shots.window,
            Action::Screen => &mut shots.screen,
            Action::Text => &mut shots.text,
            Action::Color => &mut shots.color,
            Action::Qr => &mut shots.qr,
            Action::RecordScreen => &mut shots.record_screen,
            Action::RecordArea => &mut shots.record_area,
            Action::Scrolling => &mut shots.scrolling,
        } = shortcut;
    }
}

/// The shortcuts that couldn't be had (another app holds them).
#[derive(Default)]
pub struct Taken(pub HashSet<Action>);

impl Global for Taken {}

/// Take the shortcuts (again whenever Settings change them).
pub fn init(cx: &mut App) {
    let mut registered: Vec<Option<String>> = Vec::new();
    register(&mut registered, cx);
    cx.observe_global::<Settings>(move |cx| register(&mut registered, cx)).detach();
    std::thread::spawn(prune);
}

fn register(registered: &mut Vec<Option<String>>, cx: &mut App) {
    let shots = Settings::get(cx).shots.clone();
    let wanted: Vec<Option<String>> = Action::ALL.iter().map(|a| a.shortcut(&shots).map(str::to_string)).collect();
    if *registered == wanted {
        return;
    }
    *registered = wanted;
    let mut taken = HashSet::new();
    for action in Action::ALL {
        if !crate::hotkeys::set(action.id(), action.shortcut(&shots)) {
            eprintln!("{}'s shortcut is taken by another app", action.title());
            taken.insert(action);
        }
    }
    cx.set_global(Taken(taken));
}

/// A shortcut was pressed (see `hotkeys`).
pub fn pressed(id: u32, cx: &mut App) {
    if scrolling::pressed(id) {
        return;
    }
    if let Some(action) = Action::ALL.into_iter().find(|a| a.id() == id) {
        capture(action, cx);
    }
}

/// A note in the corner (as a screenshot's shows): a title, what it's
/// about, a link it opens if clicked.
pub fn note(title: String, detail: String, link: Option<&'static str>, cx: &mut App) {
    preview::note(title, detail, None, link, cx);
}

/// Where they're kept.
fn kept() -> PathBuf {
    crate::settings::data_dir().join("Screenshots")
}

/// The newest one taken (an edited one aside), if any.
fn last() -> Option<PathBuf> {
    std::fs::read_dir(kept())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png") && !e.file_name().to_string_lossy().contains("(edited"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// The screenshots kept, newest first, as Quick Open lists them (in its
/// clipboard's rows: an image each, its file the kept one).
pub fn history() -> Vec<crate::clips::Clip> {
    let mut shots: Vec<(u64, PathBuf)> = std::fs::read_dir(kept())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| {
            let at = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            Some((at, e.path()))
        })
        .collect();
    shots.sort_by(|a, b| b.0.cmp(&a.0));
    shots
        .into_iter()
        .map(|(at, path)| {
            let (w, h) = png_size(&path).unwrap_or((0, 0));
            let edited = path.file_name().is_some_and(|n| n.to_string_lossy().contains("(edited"));
            let hash = path.to_string_lossy().bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
            crate::clips::Clip {
                id: hash,
                // (A path from the root: the clipboard's folder isn't joined on.)
                what: crate::clips::What::Image { file: path.to_string_lossy().into_owned(), w, h, hash },
                app: Some(if edited { "Edited Screenshot" } else { "Screenshot" }.into()),
                app_path: None,
                at,
                pinned: false,
                text: None,
            }
        })
        .collect()
}

/// A PNG's size in pixels, from its header.
fn png_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    (&head[12..16] == b"IHDR").then(|| (u32::from_be_bytes(head[16..20].try_into().unwrap()), u32::from_be_bytes(head[20..24].try_into().unwrap())))
}

/// Whether `path` is one of the screenshots kept.
pub fn is_kept(path: &Path) -> bool {
    path.starts_with(kept())
}

/// Pin one over everything.
pub fn pin(path: PathBuf, cx: &mut App) {
    pin::open(path, cx);
}

/// Save a copy where Settings save them; where it went.
pub fn save_copy(file: &Path, cx: &App) -> Result<PathBuf, String> {
    save(file, cx)
}

/// Edit the newest one (or say there's none).
pub fn edit_last(cx: &mut App) {
    match last() {
        Some(path) => edit(path, cx),
        None => preview::note("No Screenshots Yet".into(), "Take one first: an area, a window, or the screen.".into(), None, None, cx),
    }
}

/// Only the newest 50 are kept.
fn prune() {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(kept())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in files.into_iter().skip(50) {
        let _ = std::fs::remove_file(path);
    }
}

/// A new file's path in `dir`, named as macOS names screenshots.
fn new_file(dir: &Path, suffix: &str) -> PathBuf {
    let name = chrono::Local::now().format("Screenshot %Y-%m-%d at %H.%M.%S").to_string();
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{name}{suffix}.png") } else { format!("{name}{suffix} ({n}).png") }))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{name}{suffix}.png")))
}

/// Taking one now (a second press waits for it).
static BUSY: AtomicBool = AtomicBool::new(false);

/// Take one (or pick a color).
pub fn capture(action: Action, cx: &mut App) {
    if action == Action::Color {
        return pick_color(cx);
    }
    if matches!(action, Action::RecordScreen | Action::RecordArea) {
        return record::toggle(action == Action::RecordArea, cx);
    }
    if !macos::capture_allowed() {
        macos::capture_request();
        preview::note(
            "Screen Recording is off".into(),
            format!("Turn {} on in System Settings > Privacy & Security > Screen & System Audio Recording.", crate::APP_NAME),
            None,
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"),
            cx,
        );
        return;
    }
    if action == Action::Scrolling {
        return scrolling::start(cx);
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let shots = Settings::get(cx).shots.clone();
    let dir = kept();
    let _ = std::fs::create_dir_all(&dir);
    let file = new_file(&dir, "");
    let mut args: Vec<String> = Vec::new();
    if !shots.sound {
        args.push("-x".into());
    }
    if !shots.shadow {
        args.push("-o".into());
    }
    match action {
        Action::Area | Action::Text | Action::Qr => args.push("-i".into()),
        Action::Window => args.extend(["-i".into(), "-W".into()]),
        Action::Screen => {
            // (Where it is among the screens: GPUI's bounds don't say.)
            if let Some(display) = crate::screens::with_pointer(cx) {
                let (x, y, w, h) = macos::display_frame(u64::from(display.id()) as u32);
                args.push(format!("-R{x},{y},{w},{h}"));
            }
        }
        Action::Color | Action::RecordScreen | Action::RecordArea | Action::Scrolling => {}
    }
    args.push(file.to_string_lossy().into_owned());
    cx.spawn(async move |cx| {
        let taken = cx
            .background_executor()
            .spawn({
                let file = file.clone();
                async move {
                    let status = std::process::Command::new("/usr/sbin/screencapture").args(&args).status();
                    // (Put away with Escape: no file.)
                    status.is_ok_and(|s| s.success()) && file.exists()
                }
            })
            .await;
        BUSY.store(false, Ordering::SeqCst);
        if taken {
            cx.update(|cx| captured(action, file, cx));
        }
    })
    .detach();
}

/// After it's taken: the text read and copied; or the screenshot copied,
/// saved, shown.
fn captured(action: Action, file: PathBuf, cx: &mut App) {
    if action == Action::Qr {
        cx.spawn(async move |cx| {
            let found = cx.background_executor().spawn({
                let file = file.clone();
                async move { macos::read_qr(&file) }
            });
            let found = found.await;
            let _ = std::fs::remove_file(&file);
            cx.update(|cx| match found {
                Some(text) => {
                    macos::copy_text(&text);
                    // (A link: a click on the note opens it. Not at once: a
                    // code can point anywhere.)
                    let link = (text.starts_with("https://") || text.starts_with("http://")).then(|| &*Box::leak(text.clone().into_boxed_str()));
                    let title = if link.is_some() { "Copied the Link" } else { "Copied What It Says" };
                    preview::note(title.into(), text, None, link, cx);
                }
                None => preview::note("No QR Code Found".into(), "There's no QR code (or barcode) in that area.".into(), None, None, cx),
            });
        })
        .detach();
        return;
    }
    if action == Action::Text {
        cx.spawn(async move |cx| {
            let text = cx.background_executor().spawn({
                let file = file.clone();
                async move { macos::recognize_text(&file) }
            });
            let text = text.await;
            let _ = std::fs::remove_file(&file);
            cx.update(|cx| match text {
                Some(text) => {
                    macos::copy_text(&text);
                    let lines = text.lines().count();
                    let title = if lines == 1 { "Copied the Text".to_string() } else { format!("Copied {lines} Lines") };
                    preview::note(title, text, None, None, cx);
                }
                None => preview::note("No Text Found".into(), "There's no text in that area to copy.".into(), None, None, cx),
            });
        })
        .detach();
        return;
    }
    let shots = Settings::get(cx).shots.clone();
    if shots.copy {
        macos::copy_image(&file);
    }
    if shots.save
        && let Err(e) = save(&file, cx)
    {
        eprintln!("couldn't save the screenshot: {e}");
    }
    if shots.preview {
        preview::show(file, cx);
    }
}

/// Where Settings save them.
pub fn folder(cx: &App) -> PathBuf {
    Settings::get(cx).shots.folder.clone().unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Desktop"))
}

/// A copy of `file` in the folder Settings save them to; where.
fn save(file: &Path, cx: &App) -> Result<PathBuf, String> {
    let dir = folder(cx);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = file.file_name().ok_or("no name")?;
    let mut to = dir.join(name);
    if to.exists() {
        let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        to = (2..).map(|n| dir.join(format!("{stem} ({n}).png"))).find(|p| !p.exists()).unwrap_or(to);
    }
    std::fs::copy(file, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

fn pick_color(cx: &mut App) {
    let (tx, rx) = futures::channel::oneshot::channel();
    macos::pick_color(tx);
    cx.spawn(async move |cx| {
        if let Ok(Some(hex)) = rx.await {
            cx.update(|cx| {
                // (Kept in Colors, every way it's written.)
                let (copied, color) = crate::colors::picked(&hex, &Settings::get(cx).shots.color_format);
                macos::copy_text(&copied);
                preview::note(format!("Copied {copied}"), "Colors, in Quick Open, keeps it.".into(), color, None, cx);
            });
        }
    })
    .detach();
}

/// Which of the system's own screenshot shortcuts are on (and so keep the
/// key, whatever's set here): those of `shots`' shortcuts.
pub fn held_by_macos(shots: &Shots) -> Vec<String> {
    let on = macos::system_screenshot_keys();
    // (macOS's ids: 28 ⇧⌘3, 29 ⌃⇧⌘3, 30 ⇧⌘4, 31 ⌃⇧⌘4, 184 ⇧⌘5.)
    let system = [(28, "shift-cmd-3"), (29, "ctrl-shift-cmd-3"), (30, "shift-cmd-4"), (31, "ctrl-shift-cmd-4"), (184, "shift-cmd-5")];
    Action::ALL
        .iter()
        .filter_map(|a| a.shortcut(shots))
        .filter(|s| system.iter().any(|(id, key)| key == s && on.contains(id)))
        .map(crate::hotkeys::label)
        .collect()
}

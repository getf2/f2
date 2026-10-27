//! Quick Open, floating as Raycast and Spotlight do, and a launcher as
//! Raycast is: from ⌘P in a window, the menu bar item, or a shortcut in any
//! app (Settings choose it, ⌘Space at first: Spotlight's, so for when
//! Spotlight's is turned off in System Settings, which else keeps it).
//!
//! What's typed is looked for as a sum (the calculator), a web address,
//! an app's name, a command's (the system's, System Settings' panes,
//! Finder2's own; "quit safari"), and a file or folder's, what you've worked
//! on lately first; failing all that, a web search. A path ("~/Do…") goes
//! to a folder. With nothing typed it lists, to scroll through as Raycast
//! does: Recent (what was chosen here last), the sidebar's Favorites, the
//! commands, every app, and System Settings' panes. Return does it (an app
//! opens, a folder in Finder2, a file in its app), ⌘Return shows it in its
//! folder, ⌘K lists what else can be done with it. It goes once you're
//! done, or click elsewhere, and the app you were in stays in front: the
//! panel doesn't make Finder2 the active app.
//!
//! It has two more views, as Raycast's: the clipboard's history (`clips`),
//! with a preview of the one picked, and emoji and symbols (`emoji`), in a
//! grid. Commands open them, and shortcuts Settings set; ":" before a
//! word looks for emoji too. Return pastes the one picked into the app
//! you were in (with Accessibility: else it's copied), ⌘Return copies it;
//! escape goes back to the search.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures::StreamExt;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::input::{Escape, IndentInline, Input, InputEvent, InputState, MoveDown, MoveUp};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::commands::{self, CALCULATOR, Command, Icon};
use crate::finder::Finder;
use crate::finder::items::{file_image, location};
use crate::icons::Icons;
use crate::listing::Entry;
use crate::macos;
use crate::recent::{self, Chosen};
use crate::settings::Settings;
use crate::tools::{self, Kind};
use crate::style::{MEDIUM, Palette, SEMIBOLD, symbol};

mod pages;
pub use pages::{show_extension, show_native};

const WIDTH: f32 = 680.;
/// Results found at most, and of them apps and commands, and Recent's.
const SHOWN: usize = 9;
const APPS: usize = 4;
const COMMANDS: usize = 4;
const RECENT: usize = 8;
const ICON: f32 = 20.;
/// The panel's parts' heights, to fit it to what it shows (the search
/// field's: see `field_height`).
const ROW: f32 = 40.;
const FOOTER: f32 = 40.;
const NOTE: f32 = 44.;
const HEADING: f32 = 30.;
/// The list's height at most: more scrolls (a row cut off says so).
const LIST: f32 = 440.;
/// An action's line, in ⌘K's list.
const ACTION: f32 = 36.;
const RADIUS: f32 = 16.;

/// The shortcuts Settings offer (see `hotkeys`): name, label.
pub const SHORTCUTS: [(&str, &str); 5] = [
    ("cmd-space", "⌘Space"),
    ("alt-space", "⌥Space"),
    ("ctrl-alt-space", "⌃⌥Space"),
    ("shift-cmd-space", "⇧⌘Space"),
    ("off", "Off"),
];

/// The fonts Settings offer: name, label, family. The system's own designs
/// (by their hidden names), then others if they're installed.
pub const FONTS: [(&str, &str, &str); 8] = [
    ("system", "SF Pro", ".SystemUIFont"),
    ("rounded", "SF Pro Rounded", ".AppleSystemUIFontRounded"),
    ("serif", "New York", ".AppleSystemUIFontSerif"),
    ("mono", "SF Mono", ".AppleSystemUIFontMonospaced"),
    ("geist", "Geist", "Geist"),
    ("inter", "Inter", "Inter"),
    ("avenir", "Avenir Next", "Avenir Next"),
    ("helvetica", "Helvetica Neue", "Helvetica Neue"),
];

/// The search field's text sizes Settings offer.
pub const TEXT_SIZES: [f32; 5] = [18., 20., 22., 24., 26.];

/// "grinning face": "Grinning face".
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// What can follow a word, shown after it in gray as it's typed (Tab takes
/// it): the whole example, and how much of it must be typed first.
const HINTS: &[(&str, &str)] = &[
    ("time in tokyo", "time"),
    ("what time is it in london", "what time"),
    ("define serendipity", "define"),
    ("meaning of ephemeral", "meaning"),
    ("translate good morning to japanese", "translate"),
    ("how do you say thank you in french", "how do you"),
    ("remind me to call mom tomorrow at 5pm", "remind"),
    ("reminder pay rent on friday", "reminder"),
    ("kill port 3000", "kill"),
    ("timer 10 min", "timer"),
    ("keep awake for 1 hour", "keep awake"),
    ("volume 50", "volume"),
    ("brightness 70", "brightness"),
    ("weather in tokyo", "weather"),
    ("days until christmas", "days until"),
    ("event lunch with sara friday at 1pm", "event"),
    ("call mom", "call"),
    ("facetime mom", "facetime"),
    ("message mom", "message"),
    ("note buy milk", "note"),
    ("connect airpods", "connect"),
    ("latest download", "latest"),
    ("generate password", "generate"),
    ("photos from last week", "photos from"),
    ("videos from this month", "videos from"),
    ("pdfs from this year", "pdfs from"),
    ("screenshots from today", "screenshots from"),
];

/// The rest of `whole` after `typed` (any case), if it goes on.
fn rest_of(whole: &str, typed: &str) -> Option<String> {
    let rest: String = whole.chars().skip(typed.chars().count()).collect();
    (whole.to_lowercase().starts_with(&typed.to_lowercase()) && !rest.is_empty()).then_some(rest)
}

/// What to show after what's typed (see also `QuickPanel::completion`):
/// the rest of the place, language or unit it's the start of ("time in
/// reyk": "javík"; "translate hi to jap": "anese"); what a conversion or a
/// time is turned into ("5 km": " in mi"; "100 usd": " in eur"; "3pm": " in
/// london"); what a reminder or a translation still needs, once a word's
/// done ("tomorrow at 9am"); or an example of what can follow ("time": " in
/// tokyo").
fn hint(text: &str) -> Option<String> {
    if text.trim().is_empty() || text.starts_with(' ') || text.chars().count() > 60 {
        return None;
    }
    let lower = text.to_lowercase();
    if let Some(language) = crate::translate::complete(text) {
        let at = [" to ", " into ", " in "].iter().filter_map(|w| lower.rfind(w).map(|at| at + w.len())).max()?;
        return rest_of(language, &lower[at..]);
    }
    crate::world_time::hint(text)
        .or_else(|| crate::currency::hint(text))
        .or_else(|| crate::calc::hint(text))
        .or_else(|| crate::reminders::hint(text))
        .or_else(|| crate::translate::hint(text))
        .or_else(|| HINTS.iter().find(|(whole, first)| lower.len() >= first.len() && whole.starts_with(lower.as_str())).and_then(|(whole, _)| rest_of(whole, &lower)))
}

/// The rest of a result, as it'd be typed, if what's typed is its start:
/// an app's or a command's name ("saf": "ari"; "quit sa": "fari"), what a
/// command has typed ("trans": "late "), a folder's path ("~/Doc":
/// "uments/").
fn completion(item: &Item, typed: &str) -> Option<String> {
    if typed.chars().count() < 2 {
        return None;
    }
    let whole = match item {
        Item::App(_) => QuickPanel::title(item).to_string(),
        Item::Command(Command { run: commands::Run::Fill(fill), .. }) => fill.clone(),
        Item::Command(c) if c.kind != "Web Search" => c.title.clone(),
        Item::File(e) if typed.starts_with(['/', '~']) => {
            let path = e.path.to_string_lossy().into_owned();
            let home = std::env::var("HOME").unwrap_or_default();
            let shown = match path.strip_prefix(&home) {
                Some(rest) if typed.starts_with('~') && !home.is_empty() => format!("~{rest}"),
                _ => path,
            };
            if e.is_dir && !e.is_bundle() { format!("{shown}/") } else { shown }
        }
        _ => return None,
    };
    rest_of(&whole, typed)
}

/// An app's entry, called what Finder calls it ("Safari", not "Safari.app";
/// "メモ" on a Mac in Japanese).
fn named(mut entry: Entry, name: Option<String>) -> Entry {
    if let Some(name) = name.or_else(|| crate::apps::name_of(&entry.path)) {
        entry.name = name.into();
    }
    entry
}

/// The fonts that are here: the system's, and those of the rest installed.
pub fn fonts(cx: &App) -> &'static [(&'static str, &'static str, &'static str)] {
    static HERE: OnceLock<Vec<(&str, &str, &str)>> = OnceLock::new();
    HERE.get_or_init(|| {
        let installed = cx.text_system().all_font_names();
        FONTS.into_iter().filter(|(_, _, family)| family.starts_with('.') || installed.iter().any(|n| n == family)).collect()
    })
}

/// The font Settings chose (its family), and the search field's size.
pub fn font(settings: &Settings) -> (&'static str, f32) {
    let family = FONTS.iter().find(|(name, ..)| *name == settings.quick_open_font).map_or(FONTS[0].2, |f| f.2);
    (family, settings.quick_open_text_size.clamp(14., 32.))
}

/// The search field's height, for its text's size.
fn field_height(cx: &App) -> f32 {
    font(Settings::get(cx)).1 + 34.
}

/// What the empty search field says, and in what gray: lighter than the
/// theme's (gpui-kit's input paints its own placeholder in that, dark),
/// as macOS's and Raycast's are.
pub const PLACEHOLDER: &str = "Search for apps, commands and files…";

pub fn placeholder_color(palette: &Palette) -> Hsla {
    palette.label.opacity(0.36)
}

/// Which list it shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Search,
    Clipboard,
    Emoji,
    /// The screenshots kept (in the clipboard's rows: see `shots::history`).
    Screenshots,
    /// The windows open, front to back, to switch to (see `window_results`).
    Windows,
    /// An extension's page (the last of `QuickPanel::pages`: see `pages`).
    Extension,
}

/// Emoji per row, and a row's height.
const COLUMNS: usize = 9;
const CELL: f32 = 58.;
/// The clipboard's list's width (its preview's beside it).
const CLIPS_WIDTH: f32 = 300.;
/// The back button before the field (in a list of its own: the clipboard's,
/// the emoji, the screenshots), and the room after it.
const BACK: f32 = 28.;
const BACK_GAP: f32 = 12.;

/// The panel, while it's up.
#[derive(Default)]
struct Up(Option<(AnyWindowHandle, WeakEntity<QuickPanel>)>);

impl Global for Up {}

/// The list with nothing typed, as last found (and the apps running then),
/// so the panel comes up with it, as Raycast's does, and finds it again
/// after; and how tall it was, so it's made that tall (drawn as it's made,
/// that frame is the one shown, not drawn again).
#[derive(Default)]
struct Seed(Option<(Vec<Item>, Vec<(usize, SharedString)>, HashSet<PathBuf>)>, f32);

impl Global for Seed {}

/// The panel's icons, kept from one time it's up to the next: else each
/// time they're asked for again and come in after the list.
struct PanelIcons(Icons);

impl Global for PanelIcons {}

/// What the panel's icons may hold in memory (apps' icons, a few previews).
const ICONS_KEPT: usize = 32 << 20;

/// Shown once the list is in, or after this, whichever is first.
const REVEAL_BY: std::time::Duration = std::time::Duration::from_millis(150);

/// Whether the shortcut Settings chose could be had (another app may hold
/// it).
pub struct ShortcutTaken(pub bool);

impl Global for ShortcutTaken {}

/// Take the shortcut (and again when Settings change it; `hotkeys` hears
/// it).
pub fn init(cx: &mut App) {
    // (Programs' PATH, from your shell: asked ahead, with Developer Mode
    // on. Extensions' settings kept beside them, for what runs off the main
    // thread.)
    if Settings::get(cx).extensions_on() && Settings::get(cx).extension_developer_mode {
        crate::extensions::warm();
    }
    crate::extensions::sync(Settings::get(cx));
    cx.observe_global::<Settings>(|cx| crate::extensions::sync(Settings::get(cx))).detach();
    let mut registered = String::new();
    register(&mut registered, cx);
    cx.observe_global::<Settings>(move |cx| register(&mut registered, cx)).detach();
    // The icons, and each one as it's ready (the panel shown, drawn again).
    let (icons, mut finished) = Icons::with_limit(ICONS_KEPT);
    cx.set_global(PanelIcons(icons));
    cx.spawn(async move |cx| {
        while let Some(done) = finished.next().await {
            cx.update(|cx| {
                for evicted in cx.global_mut::<PanelIcons>().0.finish(done) {
                    cx.drop_image(evicted, None);
                }
                if let Some(panel) = cx.try_global::<Up>().and_then(|up| up.0.clone()).and_then(|(_, panel)| panel.upgrade()) {
                    panel.update(cx, |_, cx| cx.notify());
                }
            });
        }
    })
    .detach();
    // The list found once ahead, so the first time it's up is as quick as
    // the rest (the apps' names are read then, a quarter of a second).
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
        let Some((search, extra)) = cx.update(|cx| Some((cx.global::<crate::browser::Shared>().search.clone()?, extra(cx, None, None)))) else { return };
        let found = cx.background_executor().spawn(async move { (results("", Mode::Search, &search, extra), running()) }).await;
        if let (Ok((items, sections)), running) = found {
            cx.update(|cx| {
                if cx.try_global::<Seed>().is_none_or(|seed| seed.0.is_none()) {
                    cx.set_global(Seed(Some((items, sections, running)), 0.));
                }
            });
        }
    })
    .detach();
}

/// What a search goes by besides what's typed.
fn extra(cx: &App, repo: Option<crate::dev::Repo>, selection: Option<String>) -> Extra {
    let settings = Settings::get(cx);
    Extra {
        favorites: crate::finder::favorite_list(cx).into_iter().map(|f| f.path).filter(|p| p.extension().is_none_or(|e| e != "savedSearch")).collect(),
        quicklinks: settings.quicklinks.clone(),
        snippets: settings.snippets.clone(),
        repo,
        selection,
        extensions: Vec::new(),
        broken: Vec::new(),
        color_format: settings.shots.color_format.clone(),
    }
    .with_extensions(settings.extensions_on())
}

impl Extra {
    fn with_extensions(mut self, on: bool) -> Extra {
        if on {
            (self.extensions, self.broken) = crate::extensions::all();
        }
        self
    }
}

/// The apps running, by where they are.
fn running() -> HashSet<PathBuf> {
    macos::running_apps().into_iter().map(|(_, _, path)| path).collect()
}

/// Quick Open's shortcut, as Settings have it: a preset (`SHORTCUTS`) or
/// one recorded ("ctrl-alt-k"); None: off.
pub fn shortcut(settings: &Settings) -> Option<&str> {
    Some(settings.quick_open_shortcut.as_str()).filter(|s| !s.is_empty() && *s != "off")
}

fn register(registered: &mut String, cx: &mut App) {
    let settings = Settings::get(cx);
    let (clipboard, emoji, selection) = (settings.clipboard_shortcut.clone(), settings.emoji_shortcut.clone(), settings.selection_shortcut.clone());
    let wanted = settings.quick_open_shortcut.clone();
    let all = format!("{wanted} {clipboard:?} {emoji:?} {selection:?}");
    if *registered == all {
        return;
    }
    *registered = all;
    crate::hotkeys::set(crate::hotkeys::CLIPBOARD, clipboard.as_deref());
    crate::hotkeys::set(crate::hotkeys::EMOJI, emoji.as_deref());
    crate::hotkeys::set(crate::hotkeys::SELECTION, selection.as_deref());
    let had = crate::hotkeys::set(crate::hotkeys::QUICK_OPEN, shortcut(settings));
    if !had {
        eprintln!("Quick Open's shortcut {wanted} is taken by another app");
    }
    cx.set_global(ShortcutTaken(!had));
}

/// Bring it up, over whatever app is in front, on the screen the pointer
/// is on (or take it down, if it's up). `origin`: the Finder2 tab it's
/// opened from, where folders then open.
pub fn show(origin: Option<(AnyWindowHandle, WeakEntity<Finder>)>, cx: &mut App) {
    open(origin, Mode::Search, cx);
}

/// Bring it up showing `mode`'s list (or switch to it, if it's up; or take
/// it down, if it's showing it already).
pub fn show_mode(mode: Mode, cx: &mut App) {
    if let Some((handle, panel)) = cx.try_global::<Up>().and_then(|up| up.0.clone())
        && let Some(panel) = panel.upgrade()
    {
        let switched = handle.update(cx, |_, window, cx| {
            panel.update(cx, |this, cx| {
                if this.mode == mode && mode == Mode::Windows {
                    // (Switch Window's shortcut pressed again: the next one.)
                    this.next_window(cx);
                } else if this.mode == mode {
                    this.close(window, cx);
                } else {
                    this.set_mode(mode, window, cx);
                }
            })
        });
        if switched.is_ok() {
            return;
        }
    }
    open(None, mode, cx);
    // Switch Window's shortcut, its keys still held: let go, and it
    // switches, as ⌘Tab does.
    if mode == Mode::Windows
        && macos::modifiers_held()
        && let Some(panel) = cx.try_global::<Up>().and_then(|up| up.0.clone()).and_then(|(_, panel)| panel.upgrade())
    {
        panel.update(cx, |this, _| this.switching = Switching::Held);
    }
}

/// The windows listed once, off the main thread, so the first time Switch
/// Window's shortcut is pressed isn't slow (each app answers slowly only
/// the first time it's asked).
pub fn warm_windows() {
    std::thread::spawn(|| {
        macos::app_windows();
    });
}

/// Quick Open, with the text selected in the app in front (as it says,
/// through Accessibility; else copied with ⌘C, the clipboard put back
/// after): what to do with it listed.
pub fn show_selected(cx: &mut App) {
    let pid = macos::front_pid();
    if pid == std::process::id() as i32 || !macos::accessibility(false) {
        // (F2's own, or not allowed to look: Quick Open as it is; and, not
        // allowed, the way to allow it.)
        if !macos::accessibility(false) {
            macos::accessibility(true);
        }
        return show(None, cx);
    }
    cx.spawn(async move |cx| {
        let taken = cx.background_executor().spawn(async move { take_selection(pid) }).await;
        cx.update(|cx| match taken {
            Some(text) => show_with_selection(text, cx),
            None => {
                show(None, cx);
                crate::shots::note("Nothing Selected".into(), "Select some text in an app first.".into(), None, cx);
            }
        });
    })
    .detach();
}

/// The text selected in the app `pid` (in front): as it says, through
/// Accessibility; else (Chrome's, an Electron app's, which don't) copied
/// with ⌘C, the clipboard put back after, kept out of its history. Waits
/// for it: not on the main thread.
fn take_selection(pid: i32) -> Option<String> {
    if let Some(text) = macos::selected_text(pid) {
        return Some(text);
    }
    crate::clips::pause(true);
    macos::clipboard_save();
    let before = macos::pasteboard_changes();
    macos::post_copy();
    let mut copied = None;
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if macos::pasteboard_changes() != before {
            copied = macos::clipboard_text();
            break;
        }
    }
    macos::clipboard_restore();
    crate::clips::ignore_this();
    crate::clips::pause(false);
    copied.filter(|t| !t.trim().is_empty())
}

/// Quick Open with `text` as what's selected.
fn show_with_selection(text: String, cx: &mut App) {
    if cx.try_global::<Up>().and_then(|up| up.0.clone()).is_none() {
        open(None, Mode::Search, cx);
    }
    if let Some((handle, panel)) = cx.try_global::<Up>().and_then(|up| up.0.clone())
        && let Some(panel) = panel.upgrade()
    {
        handle
            .update(cx, |_, window, cx| {
                panel.update(cx, |this, cx| {
                    this.selection = Some(text);
                    this.input.update(cx, |i, cx| i.set_value("", window, cx));
                    this.find(cx);
                })
            })
            .ok();
    }
}

/// The text selected in the app in front, its spelling fixed in place
/// (see `macos::fix_spelling`), and a note of what changed: a shortcut's,
/// or Quick Open's once it's out of the way.
pub fn fix_spelling(cx: &mut App) {
    if !macos::accessibility(false) {
        macos::accessibility(true);
        return crate::shots::note("Allow Accessibility".into(), "F2 needs it to fix what's selected in another app.".into(), None, cx);
    }
    cx.spawn(async move |cx| {
        macos::given_back(cx.background_executor()).await;
        // (A shortcut's keys let go first: held, they'd go with the ⌘C and
        // the ⌘V.)
        for _ in 0..50 {
            if !macos::modifiers_held() {
                break;
            }
            cx.background_executor().timer(std::time::Duration::from_millis(20)).await;
        }
        cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
        let pid = macos::front_pid();
        let found = if pid == std::process::id() as i32 {
            None
        } else {
            cx.background_executor().spawn(async move { take_selection(pid).map(|text| macos::fix_spelling(&text, true)) }).await
        };
        let (title, detail) = match found {
            None => ("Nothing Selected".into(), "Select some text in an app, then try again.".into()),
            Some(None) => ("No Spelling Mistakes".into(), "None macOS's spell checker is sure how to fix.".into()),
            Some(Some((text, fixes))) => {
                cx.background_executor().spawn(async move { crate::snippets::paste_in(text, 0) }).await;
                ("Spelling Fixed".into(), cut(&spelling_fixes(&fixes), 2, 50))
            }
        };
        cx.update(|cx| crate::shots::note(title, detail, None, cx));
    })
    .detach();
}

/// Spelling fixes as said: "teh → the, recieve → receive".
fn spelling_fixes(fixes: &[(String, String)]) -> String {
    fixes.iter().map(|(from, to)| format!("{from} → {to}")).collect::<Vec<_>>().join(", ")
}

/// Quick Open with only `command` in it, asking first (a shortcut's, for
/// what can't be undone): Return again does it.
pub fn show_asking(command: Command, cx: &mut App) {
    if cx.try_global::<Up>().and_then(|up| up.0.clone()).is_none() {
        open(None, Mode::Search, cx);
    }
    if let Some((handle, panel)) = cx.try_global::<Up>().and_then(|up| up.0.clone())
        && let Some(panel) = panel.upgrade()
    {
        handle
            .update(cx, |_, _, cx| {
                panel.update(cx, |this, cx| {
                    // (What it was finding, dropped: this instead.)
                    this.task = Task::ready(());
                    (this.items, this.sections) = (vec![Item::Command(command)], Vec::new());
                    this.lay_out();
                    (this.selected, this.acting, this.confirming) = (0, None, Some(0));
                    cx.notify();
                })
            })
            .ok();
    }
}

/// What's copied, as text only, pasted into the app in front as a
/// snippet is: the clipboard put back after, formatting and all.
pub fn paste_plain(cx: &mut App) {
    let text = macos::clipboard_text().unwrap_or_default();
    if text.is_empty() {
        return crate::shots::note("Nothing to Paste".into(), "Copy some text first.".into(), None, cx);
    }
    paste_in_front(text, 0, cx);
}

/// A snippet's text pasted into the app in front.
pub fn paste_snippet(text: &str, cx: &mut App) {
    let (text, back) = crate::snippets::resolve(text);
    paste_in_front(text, back, cx);
}

/// `text` pasted into the app in front, the text cursor `back` from its
/// end, once that app has the keyboard back (and a shortcut's keys are let
/// go: held, they'd go with the ⌘V). Without Accessibility, only copied
/// (and macOS asked).
fn paste_in_front(text: String, back: usize, cx: &mut App) {
    if !macos::accessibility(false) {
        macos::copy_text(&text);
        macos::accessibility(true);
        return;
    }
    cx.spawn(async move |cx| {
        macos::given_back(cx.background_executor()).await;
        for _ in 0..50 {
            if !macos::modifiers_held() {
                break;
            }
            cx.background_executor().timer(std::time::Duration::from_millis(20)).await;
        }
        cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
        cx.background_executor().spawn(async move { crate::snippets::paste_in(text, back) }).await;
    })
    .detach();
}

/// Bring Quick Open up with `text` typed in it (a Wi-Fi password's, once
/// it's been looked up).
pub fn show_query(text: String, cx: &mut App) {
    if cx.try_global::<Up>().and_then(|up| up.0.clone()).is_none() {
        open(None, Mode::Search, cx);
    }
    if let Some((handle, panel)) = cx.try_global::<Up>().and_then(|up| up.0.clone())
        && let Some(panel) = panel.upgrade()
    {
        handle
            .update(cx, |_, window, cx| {
                panel.update(cx, |this, cx| {
                    this.input.update(cx, |i, cx| i.set_value(text, window, cx));
                    this.find(cx);
                })
            })
            .ok();
    }
}

fn open(origin: Option<(AnyWindowHandle, WeakEntity<Finder>)>, mode: Mode, cx: &mut App) {
    // (Your Shortcuts again: one may have been made since.)
    crate::quick_actions::warm();
    if let Some((handle, _)) = cx.try_global::<Up>().and_then(|up| up.0.clone()) {
        cx.set_global(Up(None));
        if handle.update(cx, |_, window, _| window.remove_window()).is_ok() {
            macos::panel_give_back();
            return;
        }
    }
    let Some(display) = crate::screens::for_quick_open(Settings::get(cx).quick_open_screen, cx) else { return };
    let size = display.bounds().size;
    // (As tall as it was last time, with the same list: see `Seed`.)
    let seeded = cx.try_global::<Seed>().filter(|seed| mode == Mode::Search && seed.0.is_some() && seed.1 > 0.).map(|seed| seed.1);
    let height = seeded.unwrap_or(field_height(cx) + FOOTER);
    let bounds = Bounds { origin: point((size.width - px(WIDTH)) / 2., size.height * 0.22), size: gpui::size(px(WIDTH), px(height)) };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        // (Clear: the system's material goes behind, see `panel_setup`.)
        window_background: WindowBackgroundAppearance::Transparent,
        // (Made hidden: shown once it's all there, see `appear`.)
        show: false,
        focus: false,
        ..Default::default()
    };
    // (As gpui-kit's `open_window`, but its root clear from the start: it
    // paints the theme's background behind every window, and here, round the
    // card, it mustn't. Set after, it was drawn again before it was shown.)
    let mut built = None;
    let opened = cx.open_window(options, |window, cx| {
        let panel = cx.new(|cx| QuickPanel::new(origin, mode, window, cx));
        built = Some(panel.clone());
        cx.new(|cx| {
            let mut root = gpui_kit::base::Root::new(panel, window, cx);
            root.style().background = Some(transparent_black().into());
            root
        })
    });
    match opened.map(|handle| (AnyWindowHandle::from(handle), built.expect("the panel was made"))) {
        Ok((handle, panel)) => {
            cx.set_global(Up(Some((handle, panel.downgrade()))));
            // Its list kept from last time: shown with it at once. Unless
            // what brought it up gives it something else, right after (a
            // query, the text selected, a command to ask about): then once
            // that's in.
            cx.defer(move |cx| {
                panel.update(cx, |panel, cx| {
                    if !panel.items.is_empty() && panel.searched.is_empty() && panel.selection.is_none() {
                        panel.appear(cx);
                    }
                })
            });
        }
        Err(e) => eprintln!("couldn't open Quick Open: {e}"),
    }
}

/// A light running along the search field's lower edge, again and
/// again: something's on its way from the internet.
fn loading_light(color: Hsla) -> impl IntoElement {
    let half = |from: f32, to: f32| div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(color.opacity(from), 0.), linear_color_stop(color.opacity(to), 1.)));
    div().absolute().left_0().right_0().bottom(px(-1.)).h(px(2.)).overflow_hidden().child(
        div()
            .absolute()
            .top_0()
            .h_full()
            .w(relative(0.4))
            .flex()
            .child(half(0., 1.))
            .child(half(1., 0.))
            .with_animation("loading", Animation::new(std::time::Duration::from_millis(1200)).repeat().with_easing(ease_in_out), |el, t| el.left(relative(-0.4 + 1.4 * t))),
    )
}

/// A line of results.
#[derive(Clone)]
enum Item {
    /// The calculator's (or a time's): as shown, as copied, what was worked
    /// out, what it is and its icon.
    Answer { shown: String, copied: String, sum: String, kind: &'static str, icon: (&'static str, u32) },
    /// A longer answer, in a card: a translation, a word's meaning.
    Card(Card),
    Command(Command),
    App(Entry),
    File(Entry),
    /// One from the clipboard's history.
    Clip(crate::clips::Clip),
    Emoji(&'static crate::emoji::Emoji),
    /// A git repo of the user's.
    Repo(crate::dev::Repo),
    /// Half a question ("18% of"): what it's the start of, and what's still
    /// to be typed. Not picked (Return's the next row's): it says the
    /// answer's coming.
    Unfinished { typed: String, wanted: String, kind: String, icon: (&'static str, u32) },
    /// An answer still on its way from the internet (a price, the air):
    /// what's asked, shown waiting; `QuickPanel::find` puts its rows in
    /// its place.
    Waiting { asked: String, kind: &'static str, icon: (&'static str, u32), fetch: Fetch },
    /// An extension's row (see `pages`).
    Ext(std::sync::Arc<pages::ExtRow>),
}

/// What a waiting row's waiting for.
#[derive(Clone)]
enum Fetch {
    /// A price: what's asked, in what, and what was typed.
    Price(crate::prices::Asked, Option<&'static str>, String),
    Air(crate::air::Asked),
    /// A country's holidays, by its code.
    Holidays(String),
    PublicIp,
    /// What's slowing the Mac (measured over half a second).
    Busy,
    /// Whether the internet's down, a step at a time.
    Online,
    /// What's playing, in Music or Spotify.
    NowPlaying,
}

impl Fetch {
    /// Its rows. (Waits on the network: not on the main thread.)
    fn rows(&self) -> Vec<Item> {
        match self {
            Fetch::Price(ask, wanted, q) => price_rows(ask, *wanted, q),
            Fetch::Air(asked) => air_rows(asked),
            Fetch::Holidays(code) => holiday_rows(code),
            Fetch::PublicIp => vec![match crate::this_mac::public_ip() {
                Some(ip) => Item::Answer { shown: ip.clone(), copied: ip, sum: "To the internet".into(), kind: "Public IP", icon: ("globe", commands::NETWORK) },
                None => row("Public IP Unknown".into(), "No answer from the internet".into(), "Public IP", ("globe", commands::NETWORK), commands::Run::Fill("my ip".into())),
            }],
            Fetch::Busy => busy_rows(),
            Fetch::Online => online_rows(),
            Fetch::NowPlaying => now_playing_rows(),
        }
    }
}

/// The rows fetched (by their waiting rows' places) in those places; the
/// sections after moved down to fit.
fn fill(items: &mut Vec<Item>, sections: &mut [(usize, SharedString)], fetched: Vec<(usize, Vec<Item>)>) {
    // (Last first: the places before stay put.)
    for (at, rows) in fetched.into_iter().rev() {
        if at >= items.len() || rows.is_empty() {
            continue;
        }
        let more = rows.len() - 1;
        items.splice(at..at + 1, rows);
        for (start, _) in sections.iter_mut() {
            if *start > at {
                *start += more;
            }
        }
    }
}

/// The waiting rows' places, and what they wait for.
fn waiting(items: &[Item]) -> Vec<(usize, Fetch)> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            Item::Waiting { fetch, .. } => Some((i, fetch.clone())),
            _ => None,
        })
        .collect()
}

impl Item {
    fn entry(&self) -> Option<&Entry> {
        match self {
            Item::App(e) | Item::File(e) => Some(e),
            _ => None,
        }
    }

    /// Where it is on disk: a file's, an app's, a repo's folder.
    fn path(&self) -> Option<PathBuf> {
        match self {
            Item::Repo(repo) => Some(repo.root.clone()),
            item => item.entry().map(|e| e.path.clone()),
        }
    }
}

/// A longer answer's card: a translation ("Spanish → English" over it), a
/// word (how it's said beside it, its senses under it).
#[derive(Clone)]
struct Card {
    over: String,
    text: String,
    aside: String,
    /// A line each.
    under: String,
    kind: &'static str,
    icon: (&'static str, u32),
    /// What's copied, and the action's name ("Copy Translation").
    copy: Option<(String, &'static str)>,
    /// What's opened, and the action's name ("Open in Dictionary").
    link: Option<(String, &'static str)>,
    /// Return opens the link (else it copies, if there's something to).
    opens: bool,
    /// Still to be worked out (a translation, the weather): `QuickPanel::find`
    /// does it after.
    pending: Option<Pending>,
    /// An image, before its text (a QR code): what's copied, then.
    image: Option<PathBuf>,
    /// What it copies is a password (a Wi-Fi network's): copied as one, out
    /// of clipboard histories' way, not its image.
    secret: bool,
}

/// What a card's still to be: translated (by Apple), or the weather (from
/// Open-Meteo).
#[derive(Clone)]
enum Pending {
    Translate(String, &'static crate::translate::Language, &'static crate::translate::Language),
    Weather(String, (f32, f32)),
}

/// A card's image's side.
const CARD_IMAGE: f32 = 132.;

/// Copy what a card has to copy: its image, or its text.
fn copy_card(card: &Card) {
    if card.secret
        && let Some((secret, _)) = &card.copy
    {
        return macos::copy_secret(secret, 90.);
    }
    match (&card.image, &card.copy) {
        (Some(image), _) => {
            macos::copy_image(image);
        }
        (None, Some((copied, _))) => macos::copy_text(copied),
        _ => {}
    }
}

/// Its text's lines at most, and its senses'.
const CARD_TEXT: usize = 3;
const CARD_UNDER: usize = 6;

/// Its text's characters a line, and its senses' (a guess, on the low side:
/// what's shown is cut to it, so it fits the lines the card has room for).
const CARD_TEXT_PER: usize = 58;
const CARD_UNDER_PER: usize = 84;

/// `text`, cut to `lines` lines of `per` characters (with "…").
fn cut(text: &str, lines: usize, per: usize) -> String {
    let max = lines * per;
    if columns(text) <= max {
        return text.to_string();
    }
    let mut used = 0;
    let kept: String = text
        .chars()
        .take_while(|c| {
            used += column(*c);
            used < max
        })
        .collect();
    // (At a word.)
    let kept = kept.rsplit_once(' ').map_or(kept.as_str(), |(a, _)| a);
    format!("{}…", kept.trim_end_matches([',', ';', ':', ' ', '•', '|']))
}

/// How wide a character is, in a Latin letter's: Korean, Chinese and
/// Japanese (and emoji) are two.
fn column(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe4f | 0xff00..=0xff60 | 0xffe0..=0xffe6 | 0x1f300..=0x1faff | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// How wide `text` is, in a Latin letter's.
fn columns(text: &str) -> usize {
    text.chars().map(column).sum()
}

impl Card {
    /// How many lines `text` takes at `per` characters a line (a guess:
    /// the card's as tall as its lines).
    fn lines(text: &str, per: usize) -> usize {
        text.lines().map(|l| columns(l).div_ceil(per).max(1)).sum()
    }

    fn text_lines(&self) -> usize {
        Self::lines(&self.text, CARD_TEXT_PER).clamp(1, CARD_TEXT)
    }

    /// The senses shown, each with its lines.
    fn under_lines(&self) -> Vec<(&str, usize)> {
        let mut left = CARD_UNDER;
        let mut out = Vec::new();
        for line in self.under.lines().filter(|l| !l.trim().is_empty()) {
            if left == 0 {
                break;
            }
            let n = Self::lines(line, CARD_UNDER_PER).min(2).min(left);
            out.push((line, n));
            left -= n;
        }
        out
    }

    fn height(&self) -> f32 {
        let over = if self.over.is_empty() { 0. } else { 18. };
        let under: usize = self.under_lines().iter().map(|(_, n)| n).sum();
        let under = if under == 0 { 0. } else { 6. + under as f32 * 18. };
        let text = 24. + over + self.text_lines() as f32 * 22. + under;
        if self.image.is_some() { text.max(CARD_IMAGE + 24.) } else { text }
    }
}

/// A line of the list: a section's heading, a result (by its index), a
/// card (by its index, and its height), or a row of emoji (from, to).
#[derive(Clone)]
enum Line {
    Heading(SharedString),
    Item(usize),
    Card(usize, f32),
    Cells(usize, usize),
}

impl Line {
    fn height(&self) -> f32 {
        match self {
            Line::Heading(_) => HEADING,
            Line::Item(_) => ROW,
            Line::Card(_, height) => *height,
            Line::Cells(..) => CELL,
        }
    }

    fn holds(&self, ix: usize) -> bool {
        match *self {
            Line::Item(i) | Line::Card(i, _) => i == ix,
            Line::Cells(from, to) => (from..to).contains(&ix),
            Line::Heading(_) => false,
        }
    }
}

/// What ⌘K offers for a result.
#[derive(Clone, Copy, PartialEq)]
enum Act {
    Open,
    Reveal,
    Terminal,
    Editor,
    QuickLook,
    CopyPath,
    Trash,
    Quit,
    ForceQuit,
    Copy,
    Run,
    /// A clip or an emoji: into the app in front, or onto the clipboard.
    Paste,
    CopyIt,
    OpenLink,
    Forget,
    CopyBranch,
    Uninstall,
    /// A file: made smaller, converted (a Quick Action), next to it.
    Convert(crate::quick_actions::QuickAction),
    /// A screenshot: into the editor, pinned over everything, saved.
    EditShot,
    PinShot,
    SaveShot,
    /// An image as the desktop picture.
    Wallpaper,
    /// A file sent by AirDrop, or by email.
    AirDrop,
    Email,
    /// A clip kept for good (or let go of again).
    Pin,
    Unpin,
    /// An extension's page, run again.
    Again,
    /// A command, an app or a folder given a shortcut, in Settings.
    SetShortcut,
    /// An extension's row's (or page's) action, by its place; its output
    /// shown; its passwords forgotten; its settings.
    Ext(usize),
    ExtOutput,
    ExtForget,
    ExtSettings,
}

/// The results for `text`, with their sections' headings (where each
/// starts). With nothing typed, everything (`favorites`: the sidebar's
/// folders). Off the main thread: it reads the disk.
/// What the results come from that's Settings', or the app in front's:
/// read on the main thread, then looked through off it.
struct Extra {
    /// The sidebar's folders.
    favorites: Vec<PathBuf>,
    quicklinks: Vec<crate::settings::Quicklink>,
    snippets: Vec<crate::settings::Snippet>,
    /// The repo you're in (the window's folder, or the editor's file).
    repo: Option<crate::dev::Repo>,
    /// The text selected in the app you were in, if taken.
    selection: Option<String>,
    /// The extensions (Settings' `extensions` on), and those that don't
    /// work.
    extensions: Vec<std::sync::Arc<crate::extensions::Extension>>,
    broken: Vec<crate::extensions::Broken>,
    /// How a color's copied first (Settings' `shots.color_format`).
    color_format: String,
}

/// A quicklink's link with `query` in it (encoded, for a web address).
fn filled(link: &str, query: &str) -> String {
    let query = if link.contains("://") { crate::dev::url_encode(query) } else { query.to_string() };
    link.replace("{query}", &query).replace("{argument}", &query)
}

/// Quicklinks for `q`: one's keyword first ("gh rust": GitHub's search for
/// "rust"; "gh": ready to type what for), or one by its name.
fn quicklink_results(q: &str, links: &[crate::settings::Quicklink]) -> Vec<Item> {
    let lower = q.to_lowercase();
    let (first, rest) = lower.split_once(' ').map_or((lower.as_str(), ""), |(a, b)| (a, b.trim()));
    let rest_typed = q.split_once(' ').map_or("", |(_, b)| b.trim());
    links
        .iter()
        .filter_map(|l| {
            let takes = l.link.contains("{query}") || l.link.contains("{argument}");
            let keyword = !l.keyword.trim().is_empty() && l.keyword.trim().eq_ignore_ascii_case(first);
            let icon = if l.link.contains("://") { Icon::Tile("link", commands::QUICKLINKS) } else { Icon::Of(crate::finder::expand(&l.link)) };
            let command = |title: String, subtitle: String, run| Command { title, subtitle, kind: "Quicklink", icon, run, confirm: None, key: None };
            if keyword && takes && !rest.is_empty() {
                Some(command(format!("{}: {rest_typed}", l.name), l.keyword.clone(), commands::Run::Quicklink(filled(&l.link, rest_typed))))
            } else if (keyword && rest.is_empty()) || crate::apps::name_score(&l.name, &lower).is_some_and(|s| s >= 60.) {
                Some(if takes {
                    command(l.name.clone(), format!("Type “{} ” and what to look for", l.keyword), commands::Run::Fill(format!("{} ", l.keyword)))
                } else {
                    command(l.name.clone(), l.link.clone(), commands::Run::Quicklink(l.link.clone()))
                })
            } else {
                None
            }
        })
        .map(Item::Command)
        .collect()
}

const CALCULATOR_ICON: (&str, u32) = ("equal", CALCULATOR);

/// A word's card (or, not in the Dictionary, a way to look it up there).
fn definition(word: &str) -> Item {
    let link = format!("dict://{}", crate::dev::url_encode(word));
    match crate::dictionary::define(word) {
        Some(d) => {
            let copied = format!("{}{}\n{}", d.word, if d.said.is_empty() { String::new() } else { format!(" | {} |", d.said) }, d.senses);
            Item::Card(Card {
                over: String::new(),
                text: if d.word.is_empty() { word.to_string() } else { d.word },
                aside: d.said,
                under: d.senses,
                kind: "Dictionary",
                icon: ("character.book.closed.fill", commands::DICTIONARY),
                copy: Some((copied, "Copy Definition")),
                link: Some((link, "Open in Dictionary")),
                opens: true,
                pending: None,
                image: None,
                secret: false,
            })
        }
        None => Item::Command(Command {
            title: format!("Look Up “{word}” in Dictionary"),
            subtitle: "Not found here: Dictionary may have it".into(),
            kind: "Dictionary",
            icon: Icon::Tile("character.book.closed.fill", commands::DICTIONARY),
            run: commands::Run::Url(link),
            confirm: None,
            key: None,
        }),
    }
}

/// A translation's card, to be worked out (Apple's), or Google Translate.
fn translation(request: &crate::translate::Request) -> Item {
    use crate::translate::{Plan, direction, google, plan};
    match plan(request) {
        Plan::Apple { text, from, to } => Item::Card(Card {
            over: direction(Some(from), Some(to)),
            text: "Translating…".into(),
            aside: String::new(),
            under: String::new(),
            kind: "Translation",
            icon: ("translate", commands::TRANSLATE),
            copy: None,
            link: Some((google(&text, Some(from), Some(to)), "Open in Google Translate")),
            opens: false,
            pending: Some(Pending::Translate(text, from, to)),
            image: None,
            secret: false,
        }),
        Plan::Google { text, from, to } => Item::Command(Command {
            title: format!("Translate “{text}” in Google Translate"),
            subtitle: direction(from, to),
            kind: "Translation",
            icon: Icon::Tile("translate", commands::TRANSLATE),
            run: commands::Run::Url(google(&text, from, to)),
            confirm: None,
            key: None,
        }),
    }
}

const WEATHER_APP: &str = "file:///System/Applications/Weather.app";
const CALENDAR_APP: &str = "file:///System/Applications/Calendar.app";

/// A command's row, of a kind, with its tile.
fn row(title: String, subtitle: String, kind: &'static str, icon: (&'static str, u32), run: commands::Run) -> Item {
    Item::Command(Command { title, subtitle, kind, icon: Icon::Tile(icon.0, icon.1), run, confirm: None, key: None })
}

/// The weather's card.
fn weather_card(f: &crate::weather::Forecast) -> Card {
    let (now, aside, days) = crate::weather::said(f);
    let (_, symbol, color) = crate::weather::sky(f.code, f.day);
    Card {
        over: format!("{} · now", f.place),
        copy: Some((format!("{now} in {}", f.place), "Copy")),
        text: now,
        aside,
        under: days.join("\n"),
        kind: "Weather",
        icon: (symbol, color),
        link: Some((WEATHER_APP.into(), "Open Weather")),
        opens: true,
        pending: None,
        image: None,
        secret: false,
    }
}

/// The weather there (None: here), as kept or to be fetched.
fn weather(name: Option<&str>) -> Option<Item> {
    let (place, at) = crate::weather::place(name)?;
    Some(Item::Card(match crate::weather::kept(at) {
        Some(f) => weather_card(&f),
        None => Card {
            over: place.clone(),
            text: "Looking at the sky…".into(),
            aside: String::new(),
            under: String::new(),
            kind: "Weather",
            icon: ("cloud.sun.fill", commands::WEATHER),
            copy: None,
            link: Some((WEATHER_APP.into(), "Open Weather")),
            opens: true,
            pending: Some(Pending::Weather(place, at)),
            image: None,
            secret: false,
        },
    }))
}

/// What's due in Reminders, to check off (or the way to let F2 see them).
fn todo_rows() -> Vec<Item> {
    let open = row("Open Reminders".into(), "All your lists".into(), "Application", ("checklist", commands::REMINDERS), commands::Run::Url("file:///System/Applications/Reminders.app".into()));
    let Some(todos) = macos::reminders_due(8) else {
        return vec![row("Show What's Due".into(), format!("{} asks to see Reminders (macOS asks once)", crate::APP_NAME), "Reminders", ("checklist", commands::REMINDERS), commands::Run::RemindersAccess)];
    };
    if todos.is_empty() {
        return vec![row("Nothing Due Today".into(), "No reminders left to do".into(), "Reminders", ("checkmark.circle.fill", 0x30d158), commands::Run::Url("file:///System/Applications/Reminders.app".into()))];
    }
    let now = crate::timers::now();
    let today = chrono::Local::now().date_naive();
    let mut rows: Vec<Item> = todos
        .into_iter()
        .map(|todo| {
            let day = todo.due.and_then(|d| chrono::DateTime::from_timestamp(d, 0)).map(|d| d.with_timezone(&chrono::Local).date_naive());
            let late = match (todo.due, day) {
                (Some(due), _) if todo.timed => due < now,
                (_, Some(day)) => day < today,
                _ => false,
            };
            let when = match todo.due {
                Some(due) => {
                    let said = crate::reminders::when(&crate::reminders::Due { at: due, timed: todo.timed });
                    if late { format!("Overdue · {said}") } else { said }
                }
                None => "No date".into(),
            };
            let about = if todo.list.is_empty() { when } else { format!("{when} · {}", todo.list) };
            let icon = if late { ("exclamationmark.circle.fill", commands::SYSTEM) } else { ("circle", commands::REMINDERS) };
            row(todo.title.clone(), about, "Check Off", icon, commands::Run::ReminderDone { id: todo.id, title: todo.title })
        })
        .collect();
    rows.push(open);
    rows
}

/// The air there (or here): fine dust and what to do, and the UV.
fn air_rows(asked: &crate::air::Asked) -> Vec<Item> {
    let Some((place, at)) = crate::weather::place(asked.place.as_deref()) else {
        return vec![row(format!("No Place Called “{}”", asked.place.clone().unwrap_or_default()), "Try a city: “air quality in tokyo”".into(), "Air Quality", ("aqi.medium", commands::WEATHER), commands::Run::Fill("air quality in ".into()))];
    };
    let air = match crate::air::fetch(&place, at) {
        Ok(air) => air,
        Err(why) => return vec![row("Couldn't Look Up the Air".into(), why, "Air Quality", ("aqi.medium", commands::WEATHER), commands::Run::Fill("air quality".into()))],
    };
    let korea = macos::region_code().as_deref() == Some("KR");
    let (grade, advice, level) = crate::air::grade(&air, korea);
    let (symbol, color) = [("aqi.low", 0x30d158), ("aqi.medium", 0xffd60a), ("aqi.high", 0xff9f0a), ("aqi.high", 0xff453a)][level as usize];
    let dust = Item::Answer {
        shown: grade.clone(),
        copied: format!("{grade} in {} · PM2.5 {:.0} · PM10 {:.0} µg/m³", air.place, air.pm25, air.pm10),
        sum: format!("{} · PM2.5 {:.0} · PM10 {:.0} µg/m³ · {advice}", air.place, air.pm25, air.pm10),
        kind: "Air Quality",
        icon: (symbol, color),
    };
    let (uv_now, _) = crate::air::uv_grade(air.uv);
    let (uv_high, uv_advice) = crate::air::uv_grade(air.uv_high);
    let uv = Item::Answer {
        shown: format!("UV {:.0} · {uv_now}", air.uv),
        copied: format!("UV {:.0} in {}", air.uv, air.place),
        sum: format!("{} · today's high {:.0} ({}) · {uv_advice}", air.place, air.uv_high, uv_high.to_lowercase()),
        kind: "UV Index",
        icon: ("sun.max.fill", 0xff9f0a),
    };
    if asked.uv { vec![uv, dust] } else { vec![dust, uv] }
}

/// A price's rows: what it is (and how it's gone today), and where to see
/// more; or why not.
fn price_rows(ask: &crate::prices::Asked, wanted: Option<&'static str>, q: &str) -> Vec<Item> {
    use crate::prices::Asked;
    let (price, coin) = match ask {
        Asked::Coin { id, name, amount } => (crate::prices::coin(id, &if *amount == 1. { name.clone() } else { format!("{} {name}", crate::calc::grouped(&amount.to_string())) }, *amount, wanted), true),
        Asked::Stock(stock) => (crate::prices::stock(stock, wanted), false),
        Asked::Named(named) => match crate::prices::named(named, wanted) {
            Ok((price, coin)) => (Ok(price), coin),
            Err(why) => (Err(why), false),
        },
    };
    let kind = "Price";
    match price {
        Ok(p) => {
            let up = p.change.is_none_or(|c| c >= 0.);
            let shown = format!("{} {}", crate::prices::amount(p.value, &p.currency), p.currency);
            let change = crate::prices::change(&p, coin);
            let sum = [p.name.as_str(), change.as_str(), p.about.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
            let trend = if up { ("chart.line.uptrend.xyaxis", 0x30d158) } else { ("chart.line.downtrend.xyaxis", 0xff453a) };
            let site = if coin { "CoinGecko" } else { "Yahoo Finance" };
            vec![
                Item::Answer { shown: shown.clone(), copied: crate::prices::amount(p.value, &p.currency).replace(',', ""), sum, kind, icon: trend },
                row(format!("Open on {site}"), "Its chart and news".into(), "Website", ("safari.fill", commands::WEB), commands::Run::Url(p.link)),
            ]
        }
        Err(why) => vec![row("No Price".into(), why, kind, ("chart.line.flattrend.xyaxis", commands::SYSTEM), commands::Run::Fill(q.to_string()))],
    }
}

/// The next holidays (a country's, by its code); or why not.
fn holiday_rows(code: &str) -> Vec<Item> {
    let today = chrono::Local::now().date_naive();
    match crate::holidays::upcoming(code, today, 6) {
        Ok(days) => days
            .into_iter()
            .map(|h| {
                let days = (h.date - today).num_days();
                let when = match days {
                    0 => "Today".to_string(),
                    1 => "Tomorrow".to_string(),
                    n => format!("in {n} days"),
                };
                let local = if h.local.is_empty() || h.local == h.name { String::new() } else { format!(" · {}", h.local) };
                Item::Answer {
                    shown: format!("{} · {}", h.name, h.date.format("%a %-d %b")),
                    copied: format!("{}, {}", h.name, h.date.format("%-d %B %Y")),
                    sum: format!("{when}{local}"),
                    kind: "Holiday",
                    icon: ("gift.fill", commands::CALENDAR),
                }
            })
            .collect(),
        Err(why) => vec![row("No Holidays Found".into(), why, "Holidays", ("gift.fill", commands::CALENDAR), commands::Run::Fill("holidays".into()))],
    }
}

/// What's slowing the Mac: how busy it is, then the apps using the most,
/// to quit, and Activity Monitor.
fn busy_rows() -> Vec<Item> {
    let busy = crate::busy::measure();
    let (word, advice, level) = crate::busy::verdict(&busy);
    let size = |bytes: u64| {
        let gb = bytes as f64 / 1_073_741_824.;
        if gb >= 1. { format!("{gb:.1} GB") } else { format!("{:.0} MB", gb * 1024.) }
    };
    let summary = format!("Processor {:.0}% · Memory {} of {} used", busy.cpu, size(busy.used), size(busy.total).replace(".0 GB", " GB"));
    let icon = [("gauge.with.dots.needle.33percent", 0x30d158), ("gauge.with.dots.needle.67percent", 0xff9f0a), ("gauge.with.dots.needle.100percent", 0xff453a)][level as usize];
    let mut rows = vec![Item::Answer { shown: word.into(), copied: summary.clone(), sum: format!("{summary} · {advice}"), kind: "This Mac", icon }];
    rows.extend(crate::busy::heaviest(&busy).into_iter().map(|app| {
        let cpu = if app.cpu >= 1. { format!("{:.0}% CPU · ", app.cpu) } else { String::new() };
        Item::Command(Command {
            title: app.name,
            subtitle: format!("{cpu}{} memory", size(app.memory)),
            kind: "Quit App",
            icon: Icon::Of(app.path),
            run: commands::Run::Quit { pid: app.pid, force: false },
            confirm: None,
            key: None,
        })
    }));
    rows.push(row("Open Activity Monitor".into(), "Everything running, in detail".into(), "Application", ("waveform.path.ecg", commands::SYSTEM), commands::Run::Url("file:///System/Applications/Utilities/Activity%20Monitor.app".into())));
    rows
}

/// Whether the internet's down: what's wrong (or isn't), then each step
/// checked, and what to do.
fn online_rows() -> Vec<Item> {
    use crate::online::millis;
    let checked = crate::online::check();
    let (said, what, level) = crate::online::verdict(&checked);
    let (good, half, bad) = (("checkmark.circle.fill", 0x30d158), ("exclamationmark.circle.fill", 0xff9f0a), ("xmark.circle.fill", 0xff453a));
    let mut rows = vec![Item::Answer { shown: said.clone(), copied: said, sum: what, kind: "Internet", icon: [good, half, bad][level as usize] }];
    let step = |shown: String, sum: String, icon| Item::Answer { copied: shown.clone(), shown, sum, kind: "Checked", icon };
    if let Some((kind, ip)) = &checked.network {
        rows.push(step(format!("{kind}: connected"), format!("This Mac is {ip} on it"), good));
        match &checked.router {
            Some((ip, Some(ms))) => rows.push(step("Router: answering".into(), format!("{ip} · in {}", millis(*ms)), good)),
            Some((ip, None)) => rows.push(step("Router: not answering".into(), format!("{ip} · some routers never do"), half)),
            None => rows.push(step("Router: none found".into(), "This network has no way out".into(), bad)),
        }
        if !checked.names {
            rows.push(step("Names (DNS): not found".into(), "Websites' addresses aren't being looked up".into(), bad));
        }
        match (checked.internet, checked.sign_in) {
            (_, true) => rows.push(step("Internet: wants you to sign in".into(), "This Wi-Fi network's sign-in page came instead".into(), half)),
            (Some(ms), _) => rows.push(step("Internet: answering".into(), format!("Apple's servers, in {}", millis(ms)), good)),
            (None, _) => rows.push(step("Internet: not answering".into(), "Apple's servers didn't answer".into(), bad)),
        }
    }
    if checked.sign_in {
        rows.push(row("Open the Sign-In Page".into(), "To get on this network".into(), "Website", ("safari.fill", commands::WEB), commands::Run::Url("http://captive.apple.com".into())));
    }
    if checked.internet.is_some() {
        rows.push(row("Speed Test".into(), "Your internet's download and upload".into(), "Command", ("speedometer", commands::NETWORK), commands::Run::SpeedTest));
    }
    rows.push(row("Open Wi-Fi Settings".into(), "Networks, and joining one".into(), "System Settings", ("wifi", commands::NETWORK), commands::Run::Settings("com.apple.wifi-settings-extension")));
    rows
}

/// What's playing: its card (the artwork, the song, how far in), then
/// pausing, skipping, loving it and its lyrics; or why not.
fn now_playing_rows() -> Vec<Item> {
    use crate::music::{Do, Now, Player, clock};
    let music = ("music.note", commands::MUSIC);
    match crate::music::now() {
        Now::Playing(p) => {
            let at = if p.length > 0 { format!("{} of {}", clock(p.at), clock(p.length)) } else { String::new() };
            let under = [p.artist.as_str(), p.album.as_str(), at.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
            let open = p.player.app().map(|app| (format!("file://{}", app.to_string_lossy().replace(' ', "%20")), if p.player == Player::Music { "Open Music" } else { "Open Spotify" }));
            let card = Card {
                over: format!("{} · {}", if p.playing { "Playing" } else { "Paused" }, p.player.name()),
                text: p.title.clone(),
                aside: String::new(),
                under,
                kind: p.player.name(),
                icon: music,
                copy: Some((format!("{} — {}", p.title, p.artist), "Copy Song")),
                link: open,
                opens: true,
                pending: None,
                image: p.artwork.clone(),
                secret: false,
            };
            let (toggle, symbol) = if p.playing { ("Pause", "pause.fill") } else { ("Play", "play.fill") };
            let mut rows = vec![
                Item::Card(card),
                row(toggle.into(), String::new(), "Music", (symbol, commands::MUSIC), commands::Run::Music(p.player, Do::PlayPause)),
                row("Next Track".into(), String::new(), "Music", ("forward.fill", commands::MUSIC), commands::Run::Music(p.player, Do::Next)),
                row("Previous Track".into(), String::new(), "Music", ("backward.fill", commands::MUSIC), commands::Run::Music(p.player, Do::Previous)),
            ];
            if p.player == Player::Music && p.loved != Some(true) {
                rows.push(row("Love".into(), "Mark it a favorite, in Music".into(), "Music", ("heart.fill", commands::MUSIC), commands::Run::Music(p.player, Do::Love)));
            }
            let lyrics = crate::dev::url_encode(&format!("{} {} lyrics", p.title, p.artist));
            rows.push(row("Lyrics".into(), "On the web".into(), "Website", ("quote.bubble.fill", commands::MUSIC), commands::Run::Url(format!("https://www.google.com/search?q={lyrics}"))));
            rows
        }
        Now::Stopped(player) => vec![row("Nothing's Playing".into(), format!("{} is open: Return plays", player.name()), "Music", music, commands::Run::Music(player, Do::Resume))],
        Now::NotOpen => vec![row("Nothing's Playing".into(), "Music and Spotify aren't open: Return opens Music".into(), "Music", music, commands::Run::Url("file:///System/Applications/Music.app".into()))],
        Now::Ask(player) => vec![row(format!("Show What's Playing in {}", player.name()), format!("{} asks {} (macOS asks you once)", crate::APP_NAME, player.name()), "Music", music, commands::Run::MusicAccess(player))],
        Now::Denied(player) => vec![row(
            format!("{} Can't Ask {}", crate::APP_NAME, player.name()),
            "Allow it in System Settings > Privacy & Security > Automation".into(),
            "System Settings",
            ("lock.fill", commands::SYSTEM),
            commands::Run::Settings("com.apple.preference.security?Privacy_Automation"),
        )],
    }
}

/// Something to play: the best match in the Music library, or a search in
/// Apple Music or Spotify (first if it's said, or it's what's open).
fn play_rows(what: &str, said: Option<crate::music::Player>) -> Vec<Item> {
    use crate::music::{Do, Player};
    let (apple, spotify) = crate::music::search_links(what);
    let music = PathBuf::from("/System/Applications/Music.app");
    let command = |title: String, subtitle: &str, kind: &'static str, app: PathBuf, run| Item::Command(Command { title, subtitle: subtitle.into(), kind, icon: Icon::Of(app), run, confirm: None, key: None });
    let mut rows = vec![
        command(format!("Play “{what}”"), "The best match in your Music library", "Music", music.clone(), commands::Run::Music(Player::Music, Do::Play(what.to_string()))),
        command(format!("Search Apple Music for “{what}”"), "In Music", "Search", music, commands::Run::Url(apple)),
    ];
    if let Some(app) = Player::Spotify.app() {
        let spotify = command(format!("Search Spotify for “{what}”"), "In Spotify", "Search", app, commands::Run::Url(spotify));
        let first = said == Some(Player::Spotify) || (said.is_none() && crate::music::open_players() == [Player::Spotify]);
        if first { rows.insert(0, spotify) } else { rows.push(spotify) }
    }
    rows
}

/// Half a question's row, and its section's heading: the calculator's,
/// the bill's, or the command's it's the start of.
fn unfinished_row(u: crate::unfinished::Unfinished) -> Option<(String, Item)> {
    use crate::unfinished::Kind;
    let (title, icon): (String, (&'static str, u32)) = match u.kind {
        Kind::Sum | Kind::Conversion => ("Calculator".into(), CALCULATOR_ICON),
        Kind::Currency => ("Currency".into(), CALCULATOR_ICON),
        Kind::Bill => ("Bill".into(), ("percent", commands::CALCULATOR)),
        Kind::Kitchen => ("Kitchen".into(), ("fork.knife", commands::CALCULATOR)),
        Kind::Pick => ("Pick".into(), ("hand.point.right.fill", commands::CALCULATOR)),
        Kind::Date => ("Date".into(), ("calendar", commands::CALENDAR)),
        Kind::Ask(key) => {
            let command = commands::by_key(key)?;
            let Icon::Tile(symbol, color) = command.icon else { return None };
            (command.title, (symbol, color))
        }
    };
    let kind = match u.kind {
        Kind::Sum | Kind::Conversion | Kind::Currency => "Calculator".to_string(),
        _ => title.clone(),
    };
    Some((title, Item::Unfinished { typed: u.typed, wanted: u.wanted, kind, icon }))
}

/// A QR code's card (of what was last copied, for none), or the way to
/// say what it's of.
fn qr_card(text: Option<String>) -> Item {
    let text = text.or_else(crate::qr::last_copied);
    match text.as_ref().and_then(|t| crate::qr::image(t).map(|image| (t, image))) {
        Some((text, image)) => Item::Card(Card {
            over: "QR Code".into(),
            text: text.clone(),
            aside: String::new(),
            under: "Scan it with a phone's camera.".into(),
            kind: "QR Code",
            icon: ("qrcode", commands::QR),
            copy: Some((text.clone(), "Copy QR Code")),
            link: Some((format!("file://{}", image.to_string_lossy().replace(' ', "%20")), "Open in Preview")),
            opens: false,
            pending: None,
            image: Some(image),
            secret: false,
        }),
        None => row("QR Code".into(), "Type what it's of: a link, some text".into(), "QR Code", ("qrcode", commands::QR), commands::Run::Fill("qr ".into())),
    }
}

/// A Wi-Fi network's password, with the code a phone's camera joins by.
/// (The code's image in the temporary folder, not kept with F2's data: it
/// holds the password.)
fn wifi_card(network: &str, password: &str) -> Item {
    let code = crate::net::join_code(network, password);
    let image = std::env::temp_dir().join(format!("F2 Wi-Fi {:x}.png", {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        code.hash(&mut hasher);
        hasher.finish()
    }));
    let image = (image.exists() || macos::qr_png(&code, &image, 512)).then_some(image);
    Item::Card(Card {
        over: format!("Wi-Fi · {network}"),
        text: password.to_string(),
        aside: String::new(),
        under: "Scan it with a phone's camera to join. Return copies the password.".into(),
        kind: "Wi-Fi",
        icon: ("wifi", commands::NETWORK),
        copy: Some((password.to_string(), "Copy Password")),
        link: None,
        opens: false,
        pending: None,
        image,
        secret: true,
    })
}

/// A word's synonyms, from the thesaurus: its first sense's, then the
/// others', and its opposites.
fn synonyms(word: &str) -> Item {
    let link = format!("dict://{}", crate::dev::url_encode(word));
    let Some(entry) = macos::synonyms(word).and_then(|text| crate::words::read(&text)) else {
        return row(format!("No Synonyms for “{word}”"), "Not in the thesaurus: look it up in Dictionary".into(), "Thesaurus", ("character.book.closed.fill", commands::WORDS), commands::Run::Url(link));
    };
    let joined = |words: &[String], n: usize| words.iter().take(n).cloned().collect::<Vec<_>>().join(", ");
    let text = joined(&entry.senses[0].0, 12);
    let mut under: Vec<String> = entry.senses.iter().enumerate().skip(1).take(3).map(|(i, (words, _))| format!("{}. {}", i + 1, joined(words, 8))).collect();
    let mut opposites: Vec<String> = Vec::new();
    for word in entry.senses.iter().flat_map(|(_, o)| o) {
        if !opposites.contains(word) {
            opposites.push(word.clone());
        }
    }
    if !opposites.is_empty() {
        under.push(format!("Opposites: {}", joined(&opposites, 6)));
    }
    Item::Card(Card {
        over: format!("Synonyms of “{word}” · {}", entry.part),
        copy: Some((text.clone(), "Copy Synonyms")),
        text,
        aside: String::new(),
        under: under.join("\n"),
        kind: "Thesaurus",
        icon: ("character.book.closed.fill", commands::WORDS),
        link: Some((link, "Open in Dictionary")),
        opens: false,
        pending: None,
        image: None,
        secret: false,
    })
}

/// The next sunrise and sunset (as asked) there, in its time (and here's,
/// if it's elsewhere), and how long the day is.
fn sun_rows(what: crate::sun::Asked, label: &str, at: (f32, f32), zone: &str, elsewhere: bool) -> Vec<Item> {
    use crate::sun::Asked;
    let here = macos::local_zone();
    let said = |t: i64| {
        let there = macos::zone_format(zone, t, "jmm");
        if elsewhere && zone != here { format!("{there} ({} here)", macos::zone_format(&here, t, "jmm")) } else { there }
    };
    let mut rows = Vec::new();
    for (kind, name, symbol, never) in [(Asked::Sunrise, "Sunrise", "sunrise.fill", "The sun doesn't rise there today"), (Asked::Sunset, "Sunset", "sunset.fill", "The sun doesn't set there today")] {
        if what != Asked::Both && what != kind {
            continue;
        }
        match crate::sun::next(kind, at, zone) {
            Some((t, tomorrow)) => {
                let secs = (t - crate::timers::now()).max(0) as u64;
                rows.push(Item::Answer {
                    shown: format!("{name} {}", said(t)),
                    copied: macos::zone_format(zone, t, "jmm"),
                    sum: format!("{label} · {} · in {}", if tomorrow { "Tomorrow" } else { "Today" }, crate::timers::till(secs)),
                    kind: name,
                    icon: (symbol, 0xff9f0a),
                });
            }
            None => rows.push(row(name.into(), never.into(), name, (symbol, 0xff9f0a), commands::Run::Url(WEATHER_APP.into()))),
        }
    }
    if what == Asked::Both
        && let Some(minutes) = crate::sun::day_length(at, zone)
    {
        let length = format!("{} h {} min", minutes / 60, minutes % 60);
        rows.push(Item::Answer { shown: format!("{length} of daylight"), copied: length, sum: format!("{label} · today"), kind: "Daylight", icon: ("sun.max.fill", 0xff9f0a) });
    }
    rows
}

/// What can be done with text selected in an app: translated (if it's in
/// another language than the Mac's), its spelling fixed, looked up (a
/// word), opened (a link), tracked, worked out, searched, counted, made a
/// QR code of, copied plain.
fn selection_rows(text: &str) -> Vec<Item> {
    let t = text.trim();
    let one_line = !t.contains('\n');
    let words = t.split_whitespace().count();
    let mut rows = Vec::new();
    let language = |code: &str| code.split(['-', '_']).next().unwrap_or("").to_string();
    let mine = macos::preferred_languages().first().map(|c| language(c));
    let its = macos::language_of(t).map(|c| language(&c));
    if its.is_some() && its != mine
        && let Some(request) = crate::translate::request(&format!("translate {t}"))
    {
        rows.push(translation(&request));
    }
    // (Pasted over it.)
    if let Some((fixed, fixes)) = macos::fix_spelling(text, false) {
        rows.push(row("Fix Spelling".into(), cut(&spelling_fixes(&fixes), 1, 60), "Replace", ("textformat.abc.dottedunderline", commands::WORDS), commands::Run::PasteText(fixed)));
    }
    if one_line && (1..=3).contains(&words) && t.chars().count() <= 40 {
        rows.push(definition(t));
        if macos::synonyms(t).is_some() {
            rows.push(synonyms(t));
        }
    }
    if one_line && (t.starts_with("http://") || t.starts_with("https://")) {
        rows.push(row("Open Link".into(), t.into(), "Website", ("safari.fill", commands::WEB), commands::Run::Url(t.into())));
    }
    for carrier in crate::tracking::carriers(t, &macos::local_currency().unwrap_or_default()) {
        rows.push(row(format!("Track with {}", carrier.name), t.into(), "Package", ("shippingbox.fill", commands::PACKAGE), commands::Run::Url(carrier.link)));
    }
    if let Some((shown, copied)) = crate::calc::answer(t) {
        rows.push(Item::Answer { shown, copied, sum: t.to_string(), kind: "Calculator", icon: CALCULATOR_ICON });
    }
    rows.push(Item::Command(commands::web(t)));
    let (w, chars, _, lines) = crate::words::count(t);
    let n = |n: usize| crate::calc::grouped(&n.to_string());
    rows.push(Item::Answer {
        shown: format!("{} word{}", n(w), if w == 1 { "" } else { "s" }),
        copied: w.to_string(),
        sum: format!("{} characters · {} line{}", n(chars), n(lines), if lines == 1 { "" } else { "s" }),
        kind: "Word Count",
        icon: ("number", commands::WORDS),
    });
    if t.chars().count() <= 1000 {
        rows.push(row("Make a QR Code".into(), "To scan with a phone".into(), "QR Code", ("qrcode", commands::QR), commands::Run::Fill(format!("qr {t}"))));
    }
    rows.push(Item::Answer { shown: "Copy as Plain Text".into(), copied: t.to_string(), sum: cut(t, 1, 60), kind: "Copy", icon: ("doc.plaintext", commands::CLIPBOARD) });
    // (Changed where it's selected: pasted over it.)
    for tool in crate::words::Tool::ALL {
        let changed = tool.apply(t);
        if changed != t {
            rows.push(row(format!("Make It {}", tool.title()), cut(&changed.replace('\n', " "), 1, 60), "Replace", (tool.symbol(), commands::WORDS), commands::Run::PasteText(changed)));
        }
    }
    rows
}

/// An event's row, to add it.
fn event_row(event: crate::calendar::NewEvent) -> Item {
    let when = crate::calendar::when(event.start, event.end, event.all_day);
    row(format!("Add “{}” to Calendar", event.title), when, "New Event", ("calendar.badge.plus", commands::CALENDAR), commands::Run::AddEvent(event))
}

/// What's on: today's and tomorrow's events, each to join or open (or the
/// way to let F2 see the calendar).
fn agenda() -> Vec<Item> {
    let calendar = ("calendar", commands::CALENDAR);
    match crate::macos::calendar_access() {
        None => vec![row("Show Your Calendar Here".into(), "macOS asks, the first time".into(), "Calendar", calendar, commands::Run::CalendarAccess)],
        Some(false) => vec![row(
            "Calendar Is Off Limits".into(),
            "System Settings > Privacy & Security > Calendars can allow it".into(),
            "Calendar",
            calendar,
            commands::Run::Url("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars".into()),
        )],
        Some(true) => {
            let events = crate::calendar::upcoming(6).unwrap_or_default();
            if events.is_empty() {
                return vec![row("Nothing Coming Up".into(), "Today and tomorrow".into(), "Event", calendar, commands::Run::Url(CALENDAR_APP.into()))];
            }
            events
                .into_iter()
                .map(|e| {
                    let soon = crate::calendar::soon(e.start, e.end);
                    let mut about = crate::calendar::when(e.start, e.end, e.all_day);
                    if !soon.is_empty() && !e.all_day {
                        about = format!("{about} · {soon}");
                    }
                    if !e.calendar.is_empty() {
                        about = format!("{about} · {}", e.calendar);
                    }
                    match e.link {
                        Some(link) => row(e.title, about, "Join", ("video.fill", 0x34c759), commands::Run::Url(link)),
                        None => row(e.title, about, "Event", calendar, commands::Run::Url(CALENDAR_APP.into())),
                    }
                })
                .collect()
        }
    }
}

/// The sound's and the screen's rows, for what's asked.
fn sound_rows(ask: crate::media::Ask) -> Vec<Item> {
    use crate::media::{Ask, Level, percent, target};
    use commands::Run;
    let speaker = ("speaker.wave.2.fill", commands::MUSIC);
    let sun = ("sun.max.fill", commands::WEATHER);
    let volume = macos::volume(None);
    let muted = macos::muted(None) == Some(true);
    let now = |v: Option<f32>| match v {
        Some(v) if muted => format!("Now {} (muted)", percent(v)),
        Some(v) => format!("Now {}", percent(v)),
        None => String::new(),
    };
    let step = |level: Level, v: Option<f32>| v.map_or(String::new(), |v| format!("Now {}, then {}", percent(v), percent(target(level, v))));
    match ask {
        Ask::Volume(Level::To(v)) => vec![row(format!("Set the Volume to {}", percent(v)), now(volume), "Sound", speaker, Run::Sound(ask))],
        Ask::Volume(level @ (Level::Up | Level::Down)) => {
            let title = if level == Level::Up { "Turn the Volume Up" } else { "Turn the Volume Down" };
            vec![row(title.into(), step(level, volume), "Sound", speaker, Run::Sound(ask))]
        }
        Ask::Volume(Level::Show) => vec![
            row("Turn the Volume Up".into(), step(Level::Up, volume), "Sound", speaker, Run::Sound(Ask::Volume(Level::Up))),
            row("Turn the Volume Down".into(), step(Level::Down, volume), "Sound", speaker, Run::Sound(Ask::Volume(Level::Down))),
            if muted {
                row("Unmute".into(), now(volume), "Sound", speaker, Run::Sound(Ask::Mute(false)))
            } else {
                row("Mute".into(), now(volume), "Sound", ("speaker.slash.fill", commands::MUSIC), Run::Sound(Ask::Mute(true)))
            },
        ],
        Ask::Mute(on) => vec![row(if on { "Mute" } else { "Unmute" }.into(), now(volume), "Sound", speaker, Run::Sound(ask))],
        Ask::Brightness(level) => {
            let bright = macos::brightness(None);
            if bright.is_none() {
                return vec![row("Brightness".into(), "A Mac's own screen's: there's none here".into(), "Screen", sun, Run::Settings("com.apple.Displays-Settings.extension"))];
            }
            let title = match level {
                Level::To(v) => format!("Set the Brightness to {}", percent(v)),
                Level::Up => "Turn the Brightness Up".into(),
                Level::Down => "Turn the Brightness Down".into(),
                Level::Show => "Turn the Brightness Up".into(),
            };
            let level = if level == Level::Show { Level::Up } else { level };
            let mut rows = vec![row(title, step(level, bright), "Screen", sun, Run::Sound(Ask::Brightness(level)))];
            if ask == Ask::Brightness(Level::Show) {
                rows.push(row("Turn the Brightness Down".into(), step(Level::Down, bright), "Screen", ("sun.min.fill", commands::WEATHER), Run::Sound(Ask::Brightness(Level::Down))));
            }
            rows
        }
    }
}

fn results(text: &str, mode: Mode, search: &crate::search::Search, extra: Extra) -> Result<(Vec<Item>, Vec<(usize, SharedString)>), String> {
    let favorites = extra.favorites.clone();
    let q = text.trim();
    let mut items: Vec<Item> = Vec::new();
    let mut sections = Vec::new();
    match mode {
        Mode::Clipboard => {
            let q = q.to_lowercase();
            let (pinned, rest): (Vec<_>, Vec<_>) = crate::clips::list().into_iter().filter(|c| c.matches(&q)).partition(|c| c.pinned);
            if !pinned.is_empty() {
                sections.push((0, "Pinned".into()));
                items.extend(pinned.into_iter().map(Item::Clip));
            }
            if !rest.is_empty() {
                sections.push((items.len(), if q.is_empty() { "Copied Lately" } else { "Found" }.into()));
                items.extend(rest.into_iter().map(Item::Clip));
            }
            return Ok((items, sections));
        }
        Mode::Emoji => return Ok(emoji_results(q)),
        Mode::Windows => return Ok(window_results(q)),
        // (Its page's rows: see `pages`.)
        Mode::Extension => return Ok((items, sections)),
        Mode::Screenshots => {
            // (Found by their names' dates: "2026-10-20", "17.28".)
            let q = q.to_lowercase();
            items.extend(
                crate::shots::history()
                    .into_iter()
                    .filter(|c| q.is_empty() || c.image().is_some_and(|p| p.to_string_lossy().to_lowercase().contains(&q)))
                    .map(Item::Clip),
            );
            if !items.is_empty() {
                sections.push((0, if q.is_empty() { "Taken Lately" } else { "Found" }.into()));
            }
            return Ok((items, sections));
        }
        Mode::Search => {}
    }
    // ":smile": emoji, from the search.
    if let Some(rest) = q.strip_prefix(':')
        && !rest.trim().is_empty()
        && !rest.trim().starts_with(|c: char| c.is_ascii_digit())
    {
        return Ok(emoji_results(rest));
    }
    // "kill port 3000", "ports": what's listening.
    if let Some(port) = crate::dev::port_query(q) {
        // Servers; an app (or macOS's own) only when its port's asked for,
        // and then asked first.
        let listening: Vec<Item> = crate::dev::listeners()
            .into_iter()
            .filter(|l| port.is_none_or(|p| l.port == p) && (port.is_some() || !l.app))
            .map(|l| {
                Item::Command(Command {
                    title: format!("Stop {} on Port {}", l.command, l.port),
                    subtitle: format!("pid {}", l.pid),
                    kind: if l.app { "App" } else { "Process" },
                    icon: Icon::Tile("network", commands::DEV),
                    run: commands::Run::Stop { pid: l.pid, name: l.command.clone(), port: l.port },
                    confirm: l.app.then(|| format!("{} is an app, not a server of yours: Return again to stop it.", l.command)),
                    key: None,
                })
            })
            .collect();
        if !listening.is_empty() {
            sections.push((0, if port.is_some() { "Listening on That Port" } else { "Listening Ports" }.into()));
        }
        items.extend(listening);
        return Ok((items, sections));
    }
    let entry = |path: PathBuf| {
        let entry = Entry::from_path(path)?;
        let app = entry.is_bundle() && entry.path.extension().is_some_and(|e| e == "app");
        Some(if app { Item::App(named(entry, None)) } else { Item::File(entry) })
    };
    // A path: that place, and the folders it might go on to.
    if q.starts_with('/') || q.starts_with('~') {
        let path = crate::finder::expand(q);
        let mut places: Vec<PathBuf> = path.exists().then(|| path.clone()).into_iter().collect();
        places.extend(crate::finder::completions(q).into_iter().filter(|p| *p != path));
        sections.push((0, "Go to Folder".into()));
        items.extend(places.into_iter().filter_map(entry).take(SHOWN));
        return Ok((items, sections));
    }
    let mut section = |items: &mut Vec<Item>, title: &str, new: Vec<Item>| {
        if !new.is_empty() {
            sections.push((items.len(), SharedString::from(title.to_string())));
            items.extend(new);
        }
    };
    if q.is_empty()
        && let Some(text) = &extra.selection
    {
        section(&mut items, "Selected Text", selection_rows(text));
        return Ok((items, sections));
    }
    if q.is_empty() {
        let recent = recent::list().into_iter().filter_map(|chosen| match chosen {
            Chosen::Path(path) => entry(path),
            Chosen::Command(key) => commands::by_key(&key).map(Item::Command),
        });
        section(&mut items, "Recent", recent.take(RECENT).collect());
        section(&mut items, "Favorites", favorites.into_iter().filter_map(entry).collect());
        let (commands, panes) = commands::every();
        section(&mut items, "Commands", commands.into_iter().map(Item::Command).collect());
        let apps = crate::apps::installed().iter().filter_map(|app| Some(Item::App(named(Entry::from_path(app.path.clone())?, Some(app.name.clone()))))).collect();
        section(&mut items, "Applications", apps);
        section(&mut items, "System Settings", panes.into_iter().map(Item::Command).collect());
        return Ok((items, sections));
    }
    // (Quitting or uninstalling: the apps are the commands' own.)
    let quitting = ["quit", "force quit", "uninstall ", "restart ", "relaunch ", "reopen "].iter().any(|p| q.to_lowercase().starts_with(p));
    // Extensions' commands, and their rows: a keyword's, typed after it,
    // first of all ("gh f2": your repo, before the quicklink that's gh too).
    let (ext_commands, ext_sections) = pages::results(q, &extra.extensions, &extra.broken);
    let (ext_first, ext_after): (Vec<_>, Vec<_>) = ext_sections.into_iter().partition(|(_, _, by_keyword)| *by_keyword);
    for (heading, rows, _) in ext_first {
        section(&mut items, &heading, rows);
    }
    section(&mut items, "Quicklinks", quicklink_results(q, &extra.quicklinks));
    let sum = (CALCULATOR_ICON, "Calculator");
    if let Some((shown, copied)) = crate::dev::timestamp(q) {
        section(&mut items, "Unix Time", vec![Item::Answer { shown, copied, sum: format!("Unix time {q}"), kind: sum.1, icon: sum.0 }]);
    } else if let Some(times) = crate::world_time::answer(q) {
        let times = times.into_iter().map(|t| Item::Answer { shown: t.shown, copied: t.copied, sum: t.about, kind: "Time", icon: ("clock.fill", commands::TIME) });
        section(&mut items, "Time", times.collect());
    } else if let Some(answer) = crate::dates::answer(q) {
        section(&mut items, "Date", vec![Item::Answer { shown: answer.shown, copied: answer.copied, sum: answer.about, kind: "Date", icon: ("calendar", commands::CALENDAR) }]);
    } else if let Some((shown, copied)) = crate::calc::answer(q) {
        section(&mut items, "Calculator", vec![Item::Answer { shown, copied, sum: q.to_string(), kind: sum.1, icon: sum.0 }]);
    } else if let Some(answer) = crate::currency::answer(q) {
        section(&mut items, "Currency", vec![Item::Answer { shown: answer.shown, copied: answer.copied, sum: answer.about, kind: sum.1, icon: sum.0 }]);
    }
    // A color, written as one ("#ff8800", "rgb(…)", "color tomato"): to
    // copy each way it's written (Colors' own row).
    if let Some(row) = crate::colors::answer(q, &extra.color_format) {
        let from = std::sync::Arc::new(pages::Origin { ext: crate::colors::extension(), command: "colors".into(), args: serde_json::Value::Null });
        section(&mut items, "Color", vec![Item::Ext(std::sync::Arc::new(pages::ExtRow { row: std::sync::Arc::new(row), from }))]);
    }
    // The bill: a tip, split.
    if let Some(lines) = crate::bill::answer(q) {
        let rows = lines.into_iter().map(|l| Item::Answer { shown: l.shown, copied: l.copied, sum: l.about, kind: "Bill", icon: ("percent", commands::CALCULATOR) }).collect();
        section(&mut items, "Bill", rows);
    }
    // Cups and grams, of what's measured.
    if let Some((shown, copied, about)) = crate::kitchen::answer(q) {
        section(&mut items, "Kitchen", vec![Item::Answer { shown, copied, sum: about, kind: "Kitchen", icon: ("fork.knife", commands::CALCULATOR) }]);
    }
    // Shoe and clothing sizes.
    if let Some(lines) = crate::sizes::answer(q) {
        let rows = lines.into_iter().map(|(shown, copied, rest)| {
            let symbol = if rest.contains("shoes") { "shoe.fill" } else { "tshirt.fill" };
            Item::Answer { shown, copied, sum: rest, kind: "Size", icon: (symbol, commands::CONTACTS) }
        });
        section(&mut items, "Sizes", rows.collect());
    }
    // Left to chance: a coin, a die, a number, one of some. ("Again": the
    // same asked again, so another.)
    if let Some(chance) = crate::chance::asked(q) {
        let (shown, about) = crate::chance::outcome(&chance);
        let (kind, symbol, again) = match chance {
            crate::chance::Chance::Coin => ("Coin Flip", "circle.lefthalf.filled", "Flip Again"),
            crate::chance::Chance::Dice { .. } => ("Dice", "die.face.5.fill", "Roll Again"),
            crate::chance::Chance::Number { .. } => ("Random Number", "number", "Another"),
            crate::chance::Chance::Pick(_) => ("Pick", "hand.point.right.fill", "Pick Again"),
        };
        let rows = vec![
            Item::Answer { shown: shown.clone(), copied: shown, sum: about, kind, icon: (symbol, commands::CALCULATOR) },
            row(again.into(), String::new(), "Command", ("arrow.clockwise", commands::CALCULATOR), commands::Run::Fill(q.to_string())),
        ];
        section(&mut items, kind, rows);
    }
    // A break, again and again ("eye break every 20 min"): not a reminder.
    let breaks = crate::timers::breaks_asked(q);
    if let Some((every, label)) = breaks.clone() {
        let title = format!("Start “{label}”: Every {}", crate::timers::till(every));
        section(&mut items, "Breaks", vec![row(title, "A note each time; the menu bar item stops them".into(), "Breaks", ("figure.walk", commands::TIMER), commands::Run::Breaks { every, label })]);
    }
    // What's due, to check off.
    if crate::reminders::todo_asked(q) {
        section(&mut items, "To-Do", todo_rows());
    }
    if let Some(reminder) = crate::reminders::parse(q).filter(|_| breaks.is_none()) {
        let when = reminder.due.as_ref().map_or_else(|| "No date".to_string(), crate::reminders::when);
        let command = Command {
            title: format!("Add Reminder “{}”", reminder.title),
            subtitle: when,
            kind: "Reminders",
            icon: Icon::Tile("checklist", commands::REMINDERS),
            run: commands::Run::Remind(reminder),
            confirm: None,
            key: None,
        };
        section(&mut items, "Reminders", vec![Item::Command(command)]);
    }
    if let Some(word) = crate::dictionary::word(q).or_else(|| crate::dictionary::word_after(q)) {
        section(&mut items, "Dictionary", vec![definition(&word)]);
    }
    if let Some(request) = crate::translate::request(q) {
        section(&mut items, "Translation", vec![translation(&request)]);
    }
    let asked = q.to_lowercase();
    // A timer, and those going.
    if let Some((secs, label)) = crate::timers::parse(q) {
        let done = macos::zone_format(&macos::local_zone(), crate::timers::now() + secs as i64, "jmm");
        let title = if label.is_empty() { format!("Start Timer: {}", crate::timers::spoken(secs)) } else { format!("Start “{label}” Timer: {}", crate::timers::spoken(secs)) };
        section(&mut items, "Timer", vec![row(title, format!("Done at {done}"), "Timer", ("timer", commands::TIMER), commands::Run::Timer { secs, label })]);
    }
    // The stopwatch: started, or paused, resumed, stopped.
    if crate::timers::stopwatch_asked(q) {
        use crate::timers::Watch;
        let watch = ("stopwatch", commands::TIMER);
        let rows = match crate::timers::stopwatch() {
            None => vec![row("Start Stopwatch".into(), "It counts up in the menu bar".into(), "Stopwatch", watch, commands::Run::Stopwatch(Watch::Start))],
            Some((ran, running)) => {
                let so_far = format!("{} so far", crate::timers::stopwatch_said(ran));
                let mut rows = vec![row("Stop Stopwatch".into(), so_far.clone(), "Stopwatch", watch, commands::Run::Stopwatch(Watch::Stop))];
                rows.push(if running {
                    row("Pause Stopwatch".into(), so_far, "Stopwatch", ("pause.fill", commands::TIMER), commands::Run::Stopwatch(Watch::Pause))
                } else {
                    row("Resume Stopwatch".into(), so_far, "Stopwatch", ("play.fill", commands::TIMER), commands::Run::Stopwatch(Watch::Resume))
                });
                rows.push(row("Start Over".into(), "From 0:00".into(), "Stopwatch", ("arrow.counterclockwise", commands::TIMER), commands::Run::Stopwatch(Watch::Start)));
                rows
            }
        };
        section(&mut items, "Stopwatch", rows);
    }
    if matches!(
        asked.as_str(),
        "timer" | "timers" | "my timers" | "cancel timer" | "cancel timers" | "stop timer" | "stop timers" | "alarms" | "my alarms" | "cancel alarm" | "cancel alarms" | "breaks" | "my breaks" | "stop breaks" | "cancel breaks" | "stop eye breaks" | "break reminders"
    ) {
        let now = crate::timers::now();
        let going = crate::timers::list()
            .into_iter()
            .map(|t| {
                let done = macos::zone_format(&macos::local_zone(), t.ends, "jmm");
                if t.every > 0 {
                    return row(format!("Stop “{}”", t.label), format!("Every {} · next at {done}", crate::timers::till(t.every)), "Breaks", ("figure.walk", commands::TIMER), commands::Run::CancelTimer(t.id));
                }
                let kind = if t.alarm { "Alarm" } else { "Timer" };
                let what = if t.label.is_empty() { format!("Cancel {kind}") } else { format!("Cancel “{}” {kind}", t.label) };
                let about = if t.alarm { format!("At {done} · in {}", crate::timers::till((t.ends - now).max(0) as u64)) } else { format!("{} left · done at {done}", crate::timers::countdown(t.ends - now)) };
                row(what, about, kind, (if t.alarm { "alarm.fill" } else { "timer" }, commands::TIMER), commands::Run::CancelTimer(t.id))
            })
            .collect();
        section(&mut items, if asked.contains("alarm") { "Alarms" } else if asked.contains("break") { "Breaks" } else { "Timers" }, going);
    }
    // An alarm, at a time.
    if let Some((at, label)) = crate::timers::alarm_asked(q) {
        let zone = macos::local_zone();
        let when = macos::zone_format(&zone, at, "jmm");
        let day = match (chrono::DateTime::from_timestamp(at, 0).map(|t| t.with_timezone(&chrono::Local).date_naive()), chrono::Local::now().date_naive()) {
            (Some(d), today) if d == today => "Today".to_string(),
            (Some(d), today) if Some(d) == today.succ_opt() => "Tomorrow".to_string(),
            _ => macos::zone_format(&zone, at, "EEEE"),
        };
        let title = if label.is_empty() { format!("Set Alarm for {when}") } else { format!("Set “{label}” Alarm for {when}") };
        let secs = (at - crate::timers::now()).max(0) as u64;
        section(&mut items, "Alarm", vec![row(title, format!("{day} · in {}", crate::timers::till(secs)), "Alarm", ("alarm.fill", commands::TIMER), commands::Run::Alarm { at, label })]);
    }
    // Keeping awake (and the way to stop it).
    if let Some(until) = crate::awake::parse(q) {
        let cup = ("cup.and.saucer.fill", commands::AWAKE);
        let mut rows = Vec::new();
        let state = crate::awake::state();
        if let Some(now) = state {
            rows.push(row("Stop Keeping Awake".into(), format!("Kept awake {}", crate::awake::till(now)), "Command", cup, commands::Run::StopAwake));
        }
        if !(until == crate::awake::Until::Stopped && state.is_some()) {
            let title = match until {
                crate::awake::Until::Stopped => "Keep Awake".to_string(),
                crate::awake::Until::Downloads => "Keep Awake Until Downloads Finish".to_string(),
                crate::awake::Until::At(_) => format!("Keep Awake {}", crate::awake::till(until)),
            };
            rows.push(row(title, "The Mac and its screen".into(), "Command", cup, commands::Run::KeepAwake(until)));
        }
        section(&mut items, "Keep Awake", rows);
    }
    // The sound, the screen.
    if let Some(ask) = crate::media::parse(q) {
        section(&mut items, "Sound & Screen", sound_rows(ask));
    }
    // A number to call (typed, or copied: "call" alone), with no one to
    // find in Contacts.
    let number = match crate::people::asked(q) {
        Some((way, who)) => crate::people::phone_number(&who).map(|n| (way, n)),
        None if asked.trim() == "call" => macos::clipboard_text().and_then(|t| crate::people::phone_number(&t)).map(|n| (crate::people::Way::Call, n)),
        // (Typed alone: not a date, nor an address, that looks like one.)
        None => crate::people::phone_number(q)
            .filter(|n| n.starts_with(['+', '0', '(']) || n.contains([' ', '-']))
            .filter(|n| n.parse::<std::net::Ipv4Addr>().is_err() && ["%Y-%m-%d", "%d-%m-%Y", "%Y.%m.%d", "%d.%m.%Y"].iter().all(|f| chrono::NaiveDate::parse_from_str(n, f).is_err()))
            .map(|n| (crate::people::Way::Call, n)),
    };
    if let Some((way, number)) = &number {
        let ways = if *way == crate::people::Way::Call && crate::people::asked(q).is_none() { vec![crate::people::Way::Call, crate::people::Way::Message] } else { vec![*way] };
        let rows = ways.into_iter().map(|way| row(format!("{} {number}", way.verb()), "Through your iPhone, or FaceTime".into(), way.verb(), (way.symbol(), commands::PHONE), commands::Run::Url(way.link(number, false)))).collect();
        section(&mut items, "Phone", rows);
    }
    // Someone to call, message, FaceTime or email.
    if let Some((way, who)) = crate::people::asked(q).filter(|_| number.is_none()) {
        let person = ("person.crop.circle.fill", commands::CONTACTS);
        let rows = match macos::contacts_access() {
            Some(true) => crate::people::find(way, &who, 4)
                .unwrap_or_default()
                .into_iter()
                .map(|reach| row(reach.title, reach.about, way.verb(), (way.symbol(), commands::PHONE), commands::Run::Url(reach.link)))
                .collect(),
            None => vec![row(format!("Find “{who}” in Contacts"), "To call, message or email: macOS asks, the first time".into(), "Contacts", person, commands::Run::ContactsAccess)],
            Some(false) => vec![row(
                "Contacts Is Off Limits".into(),
                "System Settings > Privacy & Security > Contacts can allow it".into(),
                "Contacts",
                person,
                commands::Run::Url("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts".into()),
            )],
        };
        section(&mut items, "Contacts", rows);
    }
    // A password, made new each time.
    if let Some(kinds) = crate::passwords::asked(q) {
        let rows = kinds
            .into_iter()
            .map(|kind| {
                let made = crate::passwords::make(kind);
                row(made.clone(), crate::passwords::about(kind), "Password", ("key.fill", commands::PASSWORDS), commands::Run::CopySecret(made))
            })
            .collect();
        section(&mut items, "Password", rows);
    }
    // The battery: the Mac's, and its Bluetooth devices' ("airpods battery":
    // theirs only).
    if crate::devices::battery_asked(q) {
        let of = asked.trim().trim_end_matches('?').strip_suffix(" battery").map(str::trim).filter(|w| !matches!(*w, "my" | "the" | "how much"));
        let mac = of.is_none_or(|w| matches!(w, "mac" | "macbook" | "laptop" | "this mac" | "computer"));
        let mut rows = Vec::new();
        if mac && let Some((percent, about, symbol)) = crate::devices::mac_battery() {
            rows.push(row(format!("This Mac: {percent}"), about, "Battery", (symbol, commands::BATTERY), commands::Run::Settings("com.apple.Battery-Settings.extension")));
        }
        for device in crate::devices::paired() {
            if device.connected && !device.batteries.is_empty() && of.is_none_or(|w| commands::matches(&device.name, &device.words(), w).is_some_and(|s| s >= 60.)) {
                rows.push(row(device.name.clone(), device.charge(), "Battery", (device.symbol(), commands::BLUETOOTH), commands::Run::Settings("com.apple.BluetoothSettings")));
            }
        }
        if rows.is_empty() {
            rows.push(row("No Batteries to Show".into(), "Bluetooth devices show theirs once they're connected".into(), "Battery", ("battery.0percent", commands::BATTERY), commands::Run::Settings("com.apple.BluetoothSettings")));
        }
        section(&mut items, "Battery", rows);
    }
    // Where the sound goes.
    let outputs: Vec<Item> = crate::devices::outputs_for(q)
        .into_iter()
        .map(|(output, symbol, what)| {
            let about = if output.used { format!("{what} · playing now") } else { what.to_string() };
            row(format!("Play Sound on {}", output.name), about, "Sound Output", (symbol, commands::MUSIC), commands::Run::Output { id: output.id, name: output.name })
        })
        .collect();
    section(&mut items, "Sound Output", outputs);
    // Bluetooth devices to connect, or disconnect.
    if let Some((connect, devices)) = crate::devices::to_connect(q) {
        let rows: Vec<Item> = devices
            .into_iter()
            .map(|d| {
                let about = if d.batteries.is_empty() { d.kind.clone() } else { d.charge() };
                let title = format!("{} {}", if connect { "Connect" } else { "Disconnect" }, d.name);
                row(title, about, "Bluetooth", (d.symbol(), commands::BLUETOOTH), commands::Run::Connect { address: d.address, name: d.name, connect })
            })
            .collect();
        section(&mut items, "Bluetooth", rows);
    }
    // A note, into Notes.
    if let Some(text) = crate::notes::asked(q) {
        let note = ("note.text", commands::NOTES);
        let rows = if text.is_empty() {
            vec![row("New Note".into(), "Type what it says: “note buy milk”".into(), "Notes", note, commands::Run::Fill("note ".into()))]
        } else {
            vec![row(format!("Add to Notes: “{text}”"), "Into Notes".into(), "Notes", note, commands::Run::AddNote(text))]
        };
        section(&mut items, "Notes", rows);
    }
    // What came into Downloads last.
    if crate::downloads::asked(q) {
        section(&mut items, "Latest Downloads", crate::downloads::latest(6).into_iter().filter_map(entry).collect());
    }
    // The internet's speed: the last test's (if lately), and a new one.
    if crate::net::speed_asked(q) {
        let speed = ("speedometer", commands::NETWORK);
        let test = row(
            if crate::net::testing() { "Testing Your Internet…" } else { "Test Internet Speed" }.into(),
            "About 20 seconds, with macOS's own test".into(),
            "Speed Test",
            speed,
            commands::Run::SpeedTest,
        );
        let rows = match crate::net::last() {
            Some((s, ago)) => {
                let said = crate::net::said(&s);
                let ago = if ago.as_secs() < 90 { "just now".to_string() } else { format!("{} min ago", ago.as_secs() / 60) };
                vec![Item::Answer { shown: said.clone(), copied: said, sum: format!("{} · tested {ago}", crate::net::about(&s)), kind: "Speed Test", icon: speed }, test]
            }
            None => vec![test],
        };
        section(&mut items, "Speed Test", rows);
    }
    // A Wi-Fi network's password, and the code to join by.
    if crate::net::wifi_asked(q) {
        let wifi = ("wifi", commands::NETWORK);
        let (current, before) = crate::net::networks(5);
        let show = |network: &str, about: &str| match crate::net::known_password(network) {
            Some(password) => wifi_card(network, &password),
            None => row(format!("Show Password for “{network}”"), about.into(), "Wi-Fi", wifi, commands::Run::WifiPassword(network.into())),
        };
        let mut rows = Vec::new();
        match &current {
            Some(network) => rows.push(show(network, "Connected now · macOS asks for your Mac's password")),
            None if macos::location_access().is_none() => rows.push(row(
                "Find the Network You're On".into(),
                "macOS asks to allow Location: it's how apps are told the Wi-Fi's name".into(),
                "Wi-Fi",
                ("location.fill", commands::NETWORK),
                commands::Run::LocationAccess,
            )),
            None => {}
        }
        rows.extend(before.iter().map(|network| show(network, "Joined before · macOS asks for your Mac's password")));
        section(&mut items, "Wi-Fi", rows);
    }
    // Spelling, and synonyms.
    if let Some(word) = crate::words::spelling_asked(q) {
        let abc = ("a.magnify", commands::WORDS);
        let rows = match macos::spell(&word) {
            Ok(()) => vec![Item::Answer { shown: word.clone(), copied: word.clone(), sum: "Spelled right".into(), kind: "Spelling", icon: ("checkmark.circle.fill", 0x34c759) }],
            Err(guesses) if guesses.is_empty() => vec![row(format!("No Suggestions for “{word}”"), "Not a word the Mac knows".into(), "Spelling", abc, commands::Run::Url(format!("dict://{}", crate::dev::url_encode(&word))))],
            Err(guesses) => guesses.into_iter().take(5).map(|g| Item::Answer { shown: g.clone(), copied: g, sum: format!("Not “{word}”"), kind: "Spelling", icon: abc }).collect(),
        };
        section(&mut items, "Spelling", rows);
    }
    if let Some(word) = crate::words::synonyms_asked(q) {
        section(&mut items, "Thesaurus", vec![synonyms(&word)]);
    }
    // The words in what's copied.
    if crate::words::count_asked(q) {
        let item = match macos::clipboard_text().filter(|t| !t.trim().is_empty()) {
            Some(text) => {
                let (words, chars, letters, lines) = crate::words::count(&text);
                let n = |n: usize| crate::calc::grouped(&n.to_string());
                Item::Answer {
                    shown: format!("{} word{}", n(words), if words == 1 { "" } else { "s" }),
                    copied: words.to_string(),
                    sum: format!("{} characters ({} without spaces) · {} line{} · in what you copied", n(chars), n(letters), n(lines), if lines == 1 { "" } else { "s" }),
                    kind: "Word Count",
                    icon: ("number", commands::WORDS),
                }
            }
            None => row("Nothing Copied to Count".into(), "Copy some text, then ask again".into(), "Word Count", ("number", commands::WORDS), commands::Run::Fill("word count".into())),
        };
        section(&mut items, "Word Count", vec![item]);
    }
    // Text tools, on what's copied: Return copies it changed.
    let tools = crate::words::tools_asked(q);
    if !tools.is_empty()
        && let Some(text) = macos::clipboard_text().filter(|t| !t.trim().is_empty())
    {
        let rows = tools
            .into_iter()
            .map(|tool| {
                let changed = tool.apply(&text);
                Item::Answer { shown: cut(&changed.replace('\n', " "), 1, 60), copied: changed, sum: format!("{} · of what you copied", tool.title()), kind: "Text", icon: (tool.symbol(), commands::WORDS) }
            })
            .collect();
        section(&mut items, "Text Tools", rows);
    }
    // A sleep timer: the Mac asleep, or the music stopped, later.
    if let Some((then, secs)) = crate::timers::sleep_asked(q) {
        let done = macos::zone_format(&macos::local_zone(), crate::timers::now() + secs as i64, "jmm");
        let (title, about, symbol) = match then {
            crate::timers::Then::Sleep => (format!("Sleep in {}", crate::timers::till(secs)), format!("At {done}: the Mac sleeps"), "moon.zzz.fill"),
            _ => (format!("Stop the Music in {}", crate::timers::till(secs)), format!("At {done}: Music and Spotify pause"), "music.note"),
        };
        section(&mut items, "Sleep Timer", vec![row(title, about, "Timer", (symbol, commands::TIMER), commands::Run::SleepTimer { then, secs })]);
    }
    // A tracking number: its carrier's page.
    let carriers = crate::tracking::carriers(q, &macos::local_currency().unwrap_or_default());
    let number = q.trim().to_string();
    let carriers: Vec<Item> = carriers.into_iter().map(|c| row(format!("Track with {}", c.name), number.clone(), "Package", ("shippingbox.fill", commands::PACKAGE), commands::Run::Url(c.link))).collect();
    section(&mut items, "Track Package", carriers);
    // A flight: its page, today's.
    if let Some((code, number, airline)) = crate::flights::asked(q) {
        let about = format!("{}{code} {number} · today's times, gate and where it is", airline.map_or(String::new(), |a| format!("{a} ")));
        let rows = crate::flights::pages(&code, &number).into_iter().map(|(site, link)| row(format!("{code} {number} on {site}"), about.clone(), "Flight", ("airplane", commands::WEATHER), commands::Run::Url(link))).collect();
        section(&mut items, "Flight", rows);
    }
    // Directions, and places near here.
    if let Some(map) = crate::maps::asked(q) {
        let korea = macos::region_code().as_deref() == Some("KR");
        let (title, about) = match &map {
            crate::maps::Map::Directions { to, from, going } => {
                let how = match going {
                    Some(crate::maps::Going::Walking) => " · walking",
                    Some(crate::maps::Going::Transit) => " · by transit",
                    Some(crate::maps::Going::Driving) => " · driving",
                    None => "",
                };
                (format!("Directions to {to}"), format!("{}{how}", from.as_ref().map_or("From where you are".to_string(), |f| format!("From {f}"))))
            }
            crate::maps::Map::Find { what, near: true } => (format!("Find “{what}” Nearby"), "Near where you are".to_string()),
            crate::maps::Map::Find { what, .. } => (format!("Show “{what}”"), "On the map".to_string()),
        };
        let rows = crate::maps::links(&map, korea)
            .into_iter()
            .map(|(name, link)| {
                let color = match name {
                    "Apple Maps" => 0x34c759,
                    "Google Maps" => 0x4285f4,
                    "Naver Map" => 0x03c75a,
                    _ => 0xffcd00,
                };
                row(format!("{title} in {name}"), about.clone(), "Maps", ("map.fill", color), commands::Run::Url(link))
            })
            .collect();
        section(&mut items, "Maps", rows);
    }
    // The calendar: what's on, and an event said for sure ("event …").
    if crate::calendar::agenda_asked(q) {
        section(&mut items, "Calendar", agenda());
    }
    let event = crate::calendar::parse(q);
    let mut maybe_event = None;
    match event {
        Some((event, true)) => section(&mut items, "Calendar", vec![event_row(event)]),
        Some((event, false)) => maybe_event = Some(event),
        None => {}
    }
    // This Mac: what it is, its space, its addresses.
    if crate::this_mac::about_asked(q) {
        let a = crate::this_mac::about();
        let version = macos::os_version().replace("Version ", "macOS ");
        let summary = [a.chip.as_str(), a.memory.as_str(), version.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let mut rows = vec![Item::Answer { shown: a.model.clone(), copied: format!("{} ({}) · {summary} · Serial {}", a.model, a.identifier, a.serial), sum: summary, kind: "This Mac", icon: ("laptopcomputer", commands::SYSTEM) }];
        if !a.serial.is_empty() {
            rows.push(Item::Answer { shown: a.serial.clone(), copied: a.serial.clone(), sum: format!("Serial number · {}", a.identifier), kind: "Serial", icon: ("number", commands::SYSTEM) });
        }
        rows.push(row("About This Mac".into(), "In System Settings".into(), "System Settings", ("info.circle.fill", commands::SYSTEM), commands::Run::Settings("com.apple.SystemProfiler.AboutExtension")));
        section(&mut items, "This Mac", rows);
    }
    if crate::this_mac::storage_asked(q)
        && let Some((free, size)) = crate::this_mac::storage()
    {
        let (free_s, size_s) = (crate::listing::human_size(free), crate::listing::human_size(size));
        let used = 100 - (free * 100 / size.max(1));
        let rows = vec![
            Item::Answer { shown: format!("{free_s} free of {size_s}"), copied: format!("{free_s} free of {size_s}"), sum: format!("Macintosh HD · {used}% used"), kind: "Storage", icon: ("internaldrive.fill", commands::FINDER2) },
            row("Free Up Space".into(), "What's safe to let go of".into(), "Command", ("sparkles", commands::FINDER2), commands::Run::FreeUpSpace),
            row("Disk Usage".into(), "What's taking up the space".into(), "Command", ("chart.pie.fill", commands::FINDER2), commands::Run::DiskUsage),
        ];
        section(&mut items, "Storage", rows);
    }
    if crate::this_mac::ip_asked(q) {
        let mut rows: Vec<Item> = crate::this_mac::local_ips()
            .into_iter()
            .take(2)
            .map(|(interface, ip)| Item::Answer { shown: ip.clone(), copied: ip, sum: format!("On this network ({interface})"), kind: "Local IP", icon: ("wifi", commands::NETWORK) })
            .collect();
        rows.push(Item::Waiting { asked: "To the internet".into(), kind: "Public IP", icon: ("globe", commands::NETWORK), fetch: Fetch::PublicIp });
        section(&mut items, "IP Address", rows);
    }
    // A price: a coin's, or a stock's (fetched once the rest is shown).
    if let Some((ask, wanted)) = crate::prices::asked(q) {
        use crate::prices::Asked;
        let asked = match &ask {
            Asked::Coin { name, .. } => name.clone(),
            // (A ticker as one: "AAPL".)
            Asked::Stock(stock) if stock.len() <= 5 && !stock.contains(' ') => stock.to_uppercase(),
            Asked::Stock(stock) => stock.clone(),
            Asked::Named(named) => named.clone(),
        };
        section(&mut items, "Price", vec![Item::Waiting { asked, kind: "Price", icon: ("chart.line.uptrend.xyaxis", 0x8e8e93), fetch: Fetch::Price(ask, wanted, q.to_string()) }]);
    }
    // What's slowing the Mac; whether the internet's down; what's playing
    // (each worked out once the rest is shown).
    if crate::busy::asked(q) {
        let waiting = Item::Waiting { asked: "Measuring what each app uses".into(), kind: "This Mac", icon: ("gauge.with.dots.needle.33percent", 0x8e8e93), fetch: Fetch::Busy };
        section(&mut items, "What's Slowing Your Mac", vec![waiting]);
    }
    if crate::online::asked(q) {
        let waiting = Item::Waiting { asked: "Checking Wi-Fi, the router and the internet".into(), kind: "Internet", icon: ("wifi", commands::NETWORK), fetch: Fetch::Online };
        section(&mut items, "Internet", vec![waiting]);
    }
    if crate::music::now_asked(q) {
        let waiting = Item::Waiting { asked: "Asking what's playing".into(), kind: "Music", icon: ("music.note", commands::MUSIC), fetch: Fetch::NowPlaying };
        section(&mut items, "Now Playing", vec![waiting]);
    }
    // Something to play: from the Music library, or a search.
    if let Some((what, said)) = crate::music::play_asked(q) {
        section(&mut items, "Music", play_rows(&what, said));
    } else if crate::music::resume_asked(q) {
        let player = crate::music::open_players().first().copied().unwrap_or(crate::music::Player::Music);
        let title = format!("Play in {}", player.name());
        section(&mut items, "Music", vec![row(title, "What was playing, or what's next".into(), "Music", ("play.fill", commands::MUSIC), commands::Run::Music(player, crate::music::Do::Resume))]);
    }
    // Cleaning mode: the keyboard and clicks off a while.
    if let Some(secs) = crate::timers::clean_asked(q) {
        let subtitle = format!("Keys and clicks off for {}, to wipe it", crate::timers::spoken(u64::from(secs)));
        section(&mut items, "Cleaning Mode", vec![row("Clean Keyboard".into(), subtitle, "Command", ("keyboard", commands::SYSTEM), commands::Run::CleanKeyboard(secs))]);
    }
    // Public holidays: here, or a country's.
    if let Some(place) = crate::holidays::asked(q)
        && let Some((code, country)) = crate::holidays::country(place.as_deref())
    {
        let waiting = Item::Waiting { asked: "The next public holidays".into(), kind: "Holiday", icon: ("gift.fill", commands::CALENDAR), fetch: Fetch::Holidays(code) };
        section(&mut items, &format!("Holidays in {country}"), vec![waiting]);
    }
    // Sunrise and sunset: here (the city of the Mac's time zone), or there.
    if let Some((what, place)) = crate::sun::asked(q) {
        let found = match place.as_deref() {
            Some(name) => crate::world_time::locate_zoned(name),
            None => crate::world_time::here().map(|(label, at)| (label, at, macos::local_zone())),
        };
        if let Some((label, at, zone)) = found {
            section(&mut items, "Sun", sun_rows(what, &label, at, &zone, place.is_some()));
        }
    }
    // The air: fine dust, and the UV.
    if let Some(air) = crate::air::asked(q) {
        let asked = air.place.clone().unwrap_or_else(|| crate::weather::place(None).map_or_else(String::new, |(place, _)| place));
        let (kind, icon) = if air.uv { ("UV Index", ("sun.max.fill", 0xff9f0a)) } else { ("Air Quality", ("aqi.medium", commands::WEATHER)) };
        section(&mut items, "Air Quality", vec![Item::Waiting { asked, kind, icon, fetch: Fetch::Air(air) }]);
    }
    // The weather; a QR code.
    if let Some(place) = crate::weather::asked(q)
        && let Some(card) = weather(place.as_deref())
    {
        section(&mut items, "Weather", vec![card]);
    }
    if let Some(text) = crate::qr::asked(q) {
        section(&mut items, "QR Code", vec![qr_card(text)]);
    }
    // Half a question ("18% of", "tip", "price of"), when nothing else is
    // answered: what it's going to be, so it's clear the answer's coming.
    if items.is_empty()
        && let Some((title, row)) = crate::unfinished::asked(text).and_then(unfinished_row)
    {
        section(&mut items, &title, vec![row]);
    }
    section(&mut items, "Website", commands::url(q).map(Item::Command).into_iter().collect());
    let lower = q.to_lowercase();
    let apps = if quitting { Vec::new() } else { crate::apps::find(q, APPS, |p| search.lately(p)) };
    let app_best = apps.first().map_or(0., |(score, ..)| *score);
    let apps: Vec<Item> = apps.into_iter().filter_map(|(_, name, p)| Some(Item::App(named(Entry::from_path(p)?, Some(name))))).collect();
    let mut found = commands::find_scored(q, if quitting { SHOWN } else { COMMANDS });
    for (_, command) in &mut found {
        commands::live(command);
    }
    // (Not what's shown above already: the volume's rows, keeping awake's, a
    // command that types what's typed, "weather".)
    let shown_above = |run: &commands::Run| match run {
        commands::Run::Sound(_) => crate::media::parse(q).is_some(),
        commands::Run::KeepAwake(_) | commands::Run::StopAwake => crate::awake::parse(q).is_some(),
        commands::Run::Fill(fill) => fill.trim().eq_ignore_ascii_case(q.trim()),
        commands::Run::SpeedTest => crate::net::speed_asked(q),
        commands::Run::CleanKeyboard(_) => crate::timers::clean_asked(q).is_some(),
        _ => false,
    };
    found.retain(|(_, c)| !shown_above(&c.run));
    if !ext_commands.is_empty() && !quitting {
        found.extend(ext_commands);
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        found.truncate(COMMANDS + 1);
    }
    // The repo you're in: on GitHub, its branch.
    if let Some(repo) = &extra.repo
        && !quitting
    {
        let about = format!("{}{}", repo.name, repo.branch.as_ref().map_or(String::new(), |b| format!(" · {b}")));
        let mut add = |title: &str, words: &str, symbol: &'static str, run: commands::Run| {
            if let Some(score) = commands::matches(title, words, &lower) {
                let command = Command { title: title.into(), subtitle: about.clone(), kind: "Command", icon: Icon::Tile(symbol, commands::DEV), run, confirm: None, key: None };
                found.push((score, command));
            }
        };
        if let Some(web) = &repo.web {
            add("Open Repo on GitHub", "github repo open remote web origin", "arrow.up.forward.square", commands::Run::Url(web.clone()));
            add("Open Pull Requests", "pull requests pr prs github", "arrow.triangle.pull", commands::Run::Url(format!("{web}/pulls")));
        }
        if let Some(branch) = &repo.branch {
            add("Copy Branch Name", "branch copy git name", "arrow.triangle.branch", commands::Run::CopyText(branch.clone()));
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        found.truncate(if quitting { SHOWN } else { COMMANDS + 1 });
    }
    let command_best = found.first().map_or(0., |f| f.0);
    let found: Vec<Item> = found.into_iter().map(|(_, c)| Item::Command(c)).collect();
    // Apps first, unless a command's the better match ("center": the
    // window command, not Print Center).
    if command_best > app_best {
        section(&mut items, "Commands", found);
        section(&mut items, "Applications", apps);
    } else {
        section(&mut items, "Applications", apps);
        section(&mut items, "Commands", found);
    }
    for (heading, rows, _) in ext_after {
        section(&mut items, &heading, rows);
    }
    // Your Shortcuts, by name ("shortcut …" or "run …" before it, or not).
    if !quitting && lower.len() >= 2 {
        let wanted = ["run shortcut ", "shortcut ", "run "].iter().find_map(|p| lower.strip_prefix(p)).unwrap_or(&lower).trim().to_string();
        let mut shortcuts: Vec<(f32, String)> = crate::quick_actions::shortcuts()
            .into_iter()
            .filter(|name| name != crate::toggles::SHORTCUT)
            .filter_map(|name| crate::apps::name_score(&name, &wanted).filter(|s| *s >= 60.).map(|s| (s, name)))
            .collect();
        shortcuts.sort_by(|a, b| b.0.total_cmp(&a.0));
        let app = PathBuf::from("/System/Applications/Shortcuts.app");
        let rows = shortcuts
            .into_iter()
            .take(4)
            .map(|(_, name)| Item::Command(Command { title: name.clone(), subtitle: "Shortcut".into(), kind: "Run Shortcut", icon: Icon::Of(app.clone()), run: commands::Run::Shortcut(name), confirm: None, key: None }))
            .collect();
        section(&mut items, "Shortcuts", rows);
    }
    // (An event, by the look of it: "lunch with Sara friday at 1". After the
    // apps and commands, so it doesn't take Return from them.)
    if let Some(event) = maybe_event {
        section(&mut items, "Calendar", vec![event_row(event)]);
    }
    if !quitting {
        let snippets: Vec<Item> = crate::snippets::find(&extra.snippets, &lower)
            .into_iter()
            .take(4)
            .map(|(_, sn)| {
                Item::Command(Command {
                    title: sn.name.clone(),
                    subtitle: sn.text.lines().next().unwrap_or("").chars().take(80).collect(),
                    kind: "Snippet",
                    icon: Icon::Tile("text.quote", commands::SNIPPETS),
                    run: commands::Run::Snippet(sn.text.clone()),
                    confirm: None,
                    key: None,
                })
            })
            .collect();
        section(&mut items, "Snippets", snippets);
        // Repos by name.
        if lower.len() >= 2 {
            let mut repos: Vec<(f32, PathBuf)> = search
                .repos()
                .into_iter()
                .filter_map(|root| {
                    let name = root.file_name()?.to_string_lossy().into_owned();
                    let score = crate::apps::name_score(&name, &lower)?;
                    (score >= 60.).then_some((score, root))
                })
                .collect();
            repos.sort_by(|a, b| b.0.total_cmp(&a.0));
            let repos: Vec<Item> = repos.into_iter().take(3).filter_map(|(_, root)| crate::dev::repo_at(&root)).map(Item::Repo).collect();
            section(&mut items, "Repositories", repos);
        }
        // (Fewer under apps and commands: the panel stays a launcher. In
        // plain words, "photos from last week": files are what's asked for,
        // and the heading says what was understood.)
        let plain = crate::natural::translate(q);
        let room = if plain.is_some() { SHOWN } else { SHOWN.saturating_sub(items.len()).min(if items.is_empty() { SHOWN } else { 5 }).max(3) };
        let mut files = search.quick_open(q, room + APPS)?;
        files.retain(|f| !items.iter().any(|i| i.entry().is_some_and(|e| e.path == f.path)));
        files.truncate(room);
        let heading = plain.map_or_else(|| "Files and Folders".to_string(), |p| p.about);
        section(&mut items, &heading, files.into_iter().map(Item::File).collect());
        section(&mut items, "Web", vec![Item::Command(commands::web(q))]);
    }
    Ok((items, sections))
}

/// Emoji matching `q` (or all, the ones used lately first), by section.
/// The windows open, front to back (see `macos::app_windows`), or those
/// whose title or app matches what's typed, best first. Each with its app's
/// icon, and whether it's minimized or hidden.
fn window_results(q: &str) -> (Vec<Item>, Vec<(usize, SharedString)>) {
    let Some(windows) = macos::app_windows() else {
        let allow = row(
            "Allow Accessibility".into(),
            "Quick Open needs it to see other apps' windows".into(),
            "Privacy",
            ("hand.raised.fill", commands::WINDOWS),
            commands::Run::Url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility".into()),
        );
        return (vec![allow], Vec::new());
    };
    let q = q.to_lowercase();
    let mut found: Vec<(f32, macos::AppWindow)> = windows
        .into_iter()
        .filter_map(|w| {
            let score = if q.is_empty() { Some(0.) } else { commands::matches(&w.title, &w.app.to_lowercase(), &q) };
            score.map(|s| (s, w))
        })
        .collect();
    // (Typed: the best first; else as they're stacked.)
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    let items: Vec<Item> = found
        .into_iter()
        .map(|(_, w)| {
            let state = if w.minimized {
                " · Minimized"
            } else if w.hidden {
                " · Hidden"
            } else if w.elsewhere {
                " · On another desktop"
            } else {
                ""
            };
            Item::Command(Command {
                title: if w.title.trim().is_empty() { w.app.clone() } else { w.title.clone() },
                subtitle: format!("{}{state}", w.app),
                kind: "Window",
                icon: Icon::Of(w.path),
                run: commands::Run::FocusWindow { pid: w.pid, id: w.id },
                confirm: None,
                key: None,
            })
        })
        .collect();
    let sections = if items.is_empty() { Vec::new() } else { vec![(0, "Windows".into())] };
    (items, sections)
}

fn emoji_results(q: &str) -> (Vec<Item>, Vec<(usize, SharedString)>) {
    let mut items: Vec<Item> = Vec::new();
    let mut sections = Vec::new();
    if q.trim().is_empty() {
        let lately = crate::emoji::lately();
        if !lately.is_empty() {
            sections.push((0, "Used Lately".into()));
            items.extend(lately.into_iter().map(Item::Emoji));
        }
        for (group, list) in crate::emoji::groups() {
            sections.push((items.len(), group.into()));
            items.extend(list.into_iter().map(Item::Emoji));
        }
    } else {
        let found = crate::emoji::find(q);
        if !found.is_empty() {
            sections.push((0, "Emoji and Symbols".into()));
        }
        items.extend(found.into_iter().map(Item::Emoji));
    }
    (items, sections)
}

/// Switch Window, from its shortcut: while its keys are held (and once
/// they're let go, before the windows are listed: then it switches as
/// soon as they are), and how many more times it was pressed till then.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Switching {
    #[default]
    No,
    Held,
    LetGo,
}

pub struct QuickPanel {
    mode: Mode,
    /// Switch Window's pictures of windows, by number (None: none could be
    /// taken), and those being taken; the files go when the panel does.
    pictures: HashMap<u32, Option<PathBuf>>,
    picturing: HashSet<u32>,
    switching: Switching,
    /// Switch Window's shortcut pressed again before the windows were
    /// listed: how many windows on from the one picked first.
    steps: usize,
    /// The app in front when it came up (what Return pastes into).
    front: Option<String>,
    /// The repo you're in as it came up: its window's folder's, or the
    /// editor's file's, or the Finder2 window in front's.
    repo: Option<crate::dev::Repo>,
    input: Entity<InputState>,
    items: Vec<Item>,
    /// Where each section starts in `items`, and its heading.
    sections: Vec<(usize, SharedString)>,
    /// The items under their sections' headings, as listed.
    lines: Vec<Line>,
    list: ListState,
    selected: usize,
    /// ⌘K: the selected result's actions are shown, this one selected.
    acting: Option<usize>,
    /// A command that asks first, asked: a second Return does it.
    confirming: Option<usize>,
    /// The apps running (a dot under their icons, as the Dock shows).
    running: HashSet<PathBuf>,
    /// Finder2's own app, for its icon in the corner.
    me: Option<Entry>,
    error: Option<SharedString>,
    /// What's shown after what's typed, in gray (see `hint`).
    hint: Option<String>,
    /// What was last looked for (the field's text then).
    searched: String,
    /// The text selected in the app you were in, if it's been taken (see
    /// `show_selected`): what to do with it is listed, till you type.
    selection: Option<String>,
    origin: Option<(AnyWindowHandle, WeakEntity<Finder>)>,
    palette: Palette,
    /// Set up as a panel (no window buttons) and shown (`appear`).
    native: bool,
    /// Its window's NSView.
    view: *mut std::ffi::c_void,
    height: f32,
    task: Task<()>,
    /// Extensions' pages open, the last shown (see `pages`); the next
    /// one's id; the action asked about, on the row `confirming`.
    pages: Vec<pages::Page>,
    next_page: u64,
    ext_confirm: usize,
    /// What was typed in the search before the first page, for coming
    /// back to it.
    searched_before: String,
    _subscriptions: Vec<Subscription>,
}

impl QuickPanel {
    fn new(origin: Option<(AnyWindowHandle, WeakEntity<Finder>)>, mode: Mode, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // (The languages apps are found by, looked up here, on the main
        // thread: see `macos::mac_languages`.)
        macos::mac_languages();
        // (No placeholder of its own: see `PLACEHOLDER`.)
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |i, cx| i.focus(window, cx));
        // (Shown once the list's in, or by `REVEAL_BY` whatever it is.)
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REVEAL_BY).await;
            this.update(cx, |this, cx| this.appear(cx)).ok();
        })
        .detach();
        let subscriptions = vec![
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.find(cx);
                    this.typed_on_page(window, cx);
                }
                InputEvent::PressEnter { secondary, .. } => match this.acting {
                    Some(at) => {
                        if let Some(&(act, ..)) = this.actions(cx).get(at) {
                            this.perform(act, window, cx);
                        }
                    }
                    None => this.choose(this.selected, *secondary, window, cx),
                },
                _ => {}
            }),
            // Clicked away: gone, as Spotlight goes.
            // (And as an input method composes: the field doesn't say it's
            // changed till it's done, "めも" till Return; Spotlight shows
            // what it finds as it goes.)
            cx.observe(&input, |this, input, cx| {
                if input.read(cx).value() != this.searched {
                    this.find(cx);
                }
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.close(window, cx);
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.palette = Palette::for_window(window);
                cx.notify();
            }),
        ];
        // The folder you're in: the window's it was opened from; else the
        // document's in the app in front (asked off the main thread: an app
        // can be slow to say), else F2's window in front.
        let opened_in = origin.as_ref().and_then(|(_, finder)| finder.upgrade()).map(|f| f.read(cx).cwd().to_path_buf());
        if opened_in.is_none() {
            let front_folder = crate::browser::front_folder(cx);
            cx.spawn(async move |this, cx| {
                let repo = cx.background_executor().spawn(async move { macos::front_document().or(front_folder).as_deref().and_then(crate::dev::repo_at) }).await;
                if repo.is_some() {
                    this.update(cx, |this, cx| {
                        this.repo = repo;
                        if !this.searched.trim().is_empty() {
                            this.find(cx);
                        }
                    })
                    .ok();
                }
            })
            .detach();
        }
        let view = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
            Ok(RawWindowHandle::AppKit(appkit)) => appkit.ns_view.as_ptr(),
            _ => std::ptr::null_mut(),
        };
        // (The list as it was last time, with nothing typed: found again.)
        let seed = (mode == Mode::Search).then(|| cx.try_global::<Seed>().and_then(|seed| seed.0.clone())).flatten();
        let mut this = QuickPanel {
            mode,
            pictures: HashMap::new(),
            picturing: HashSet::new(),
            switching: Switching::No,
            steps: 0,
            front: macos::front_app().map(|(name, _)| name),
            repo: opened_in.as_deref().and_then(crate::dev::repo_at),
            input,
            items: Vec::new(),
            sections: Vec::new(),
            lines: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(ROW * 4.)),
            selected: 0,
            acting: None,
            confirming: None,
            running: HashSet::new(),
            me: std::env::current_exe().ok().and_then(|exe| exe.ancestors().nth(3).filter(|p| p.extension().is_some_and(|e| e == "app")).and_then(|p| Entry::from_path(p.to_path_buf()))),
            error: None,
            hint: None,
            searched: String::new(),
            selection: None,
            origin,
            palette: Palette::for_window(window),
            native: false,
            view,
            height: 0.,
            task: Task::ready(()),
            pages: Vec::new(),
            next_page: 0,
            ext_confirm: 0,
            searched_before: String::new(),
            _subscriptions: subscriptions,
        };
        if let Some((items, sections, running)) = seed {
            (this.items, this.sections, this.running) = (items, sections, running);
            this.lay_out();
        }
        // (Extensions' rows found by what's typed, kept fresh.)
        if Settings::get(cx).extensions_on() {
            for ext in crate::extensions::all().0 {
                for spec in ext.manifest.commands.iter().filter(|s| (s.root || s.keyword.is_some()) && s.search == crate::extensions::Search::F2) {
                    crate::extensions::freshen(ext.clone(), spec.clone());
                }
            }
        }
        this.find(cx);
        this
    }

    /// Look for what's typed (or, with nothing typed, list everything).
    fn find(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Extension {
            return self.find_on_page(cx);
        }
        let Some(search) = cx.global::<crate::browser::Shared>().search.clone() else {
            self.error = Some("Search is off: fsearch didn't start.".into());
            return cx.notify();
        };
        let text = self.input.read(cx).value().to_string();
        self.searched = text.clone();
        self.hint = if self.mode == Mode::Search { hint(&crate::apps::plain_width(&text)) } else { None };
        let extra = extra(cx, self.repo.clone(), self.selection.clone());
        let mode = self.mode;
        // (Kept, to come up with next time: see `Seed`.)
        let seeds = mode == Mode::Search && text.is_empty() && self.selection.is_none();
        self.task = cx.spawn(async move |this, cx| {
            // (Full-width letters and digits as plain: "１００ ｕｓｄ".)
            let (found, running) = cx.background_executor().spawn(async move { (results(&crate::apps::plain_width(&text), mode, &search, extra), running()) }).await;
            // What's from the internet (a price, the air), fetched now the
            // rest's ready: shown with it if it's quick, else shown waiting
            // and put in once it's here.
            let (found, later) = match found {
                Ok((mut items, mut sections)) => {
                    let fetches = waiting(&items);
                    let mut later = None;
                    if !fetches.is_empty() {
                        let fetching = cx.background_executor().spawn(async move { fetches.into_iter().map(|(at, fetch)| (at, fetch.rows())).collect::<Vec<_>>() });
                        let soon = cx.background_executor().timer(std::time::Duration::from_millis(150));
                        match futures::future::select(fetching, soon).await {
                            futures::future::Either::Left((fetched, _)) => fill(&mut items, &mut sections, fetched),
                            futures::future::Either::Right((_, fetching)) => later = Some(fetching),
                        }
                    }
                    (Ok((items, sections)), later)
                }
                Err(e) => (Err(e), None),
            };
            this.update(cx, |this, cx| {
                // (A page opened meanwhile, as Quick Open came up for it:
                // what was found for its search isn't the page's.)
                if this.mode != mode {
                    return this.appear(cx);
                }
                match found {
                    Ok((items, sections)) => {
                        if seeds {
                            let height = cx.try_global::<Seed>().map_or(0., |seed| seed.1);
                            cx.set_global(Seed(Some((items.clone(), sections.clone(), running.clone())), height));
                        }
                        (this.items, this.sections, this.running, this.error) = (items, sections, running, None);
                        this.lay_out();
                        // (Half a question's row isn't picked: the one after it is.)
                        let unfinished = matches!(this.items.first(), Some(Item::Unfinished { .. })) && this.items.len() > 1;
                        (this.selected, this.acting, this.confirming) = (usize::from(unfinished), None, None);
                        // (The windows: the one before the window you're in, as
                        // ⌘Tab picks the app before; on, if its shortcut was
                        // pressed again meanwhile.)
                        if this.mode == Mode::Windows && this.searched.trim().is_empty() && this.items.len() > 1 {
                            this.selected = (1 + std::mem::take(&mut this.steps)) % this.items.len();
                        }
                    }
                    // (Half a query, like "ext:": what was found stays.)
                    Err(e) => this.error = Some(e.into()),
                }
                this.appear(cx);
                cx.notify();
            })
            .ok();
            if let Some(fetching) = later {
                let fetched = fetching.await;
                this.update(cx, |this, cx| this.fetched(fetched, cx)).ok();
            }
            // (A translation to work out, now the rest is shown.)
            let pending = this.update(cx, |this, _| {
                this.items.iter().enumerate().find_map(|(i, item)| match item {
                    Item::Card(card) => card.pending.clone().map(|p| (i, p)),
                    _ => None,
                })
            });
            if let Ok(Some((ix, pending))) = pending {
                // (Once typing stops: a keystroke before drops this.)
                cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
                match pending {
                    Pending::Translate(text, from, to) => {
                        let outcome = cx.background_executor().spawn(async move { crate::translate::run(&text, from, to) }).await;
                        this.update(cx, |this, cx| this.translated(ix, outcome, from, to, cx)).ok();
                    }
                    Pending::Weather(place, at) => {
                        let forecast = cx.background_executor().spawn(async move { crate::weather::fetch(&place, at) }).await;
                        this.update(cx, |this, cx| this.forecast(ix, forecast, cx)).ok();
                    }
                }
            }
        });
    }

    /// The lines shown, from the results and their sections: each
    /// section's heading, then its items, a line each (emoji: in rows).
    fn lay_out(&mut self) {
        self.lines.clear();
        let mut starts = self.sections.clone();
        if starts.first().is_none_or(|(at, _)| *at > 0) && !self.items.is_empty() {
            starts.insert(0, (0, "".into()));
        }
        for (n, (from, title)) in starts.iter().enumerate() {
            let from = *from;
            let to = starts.get(n + 1).map_or(self.items.len(), |s| s.0);
            if !title.is_empty() {
                self.lines.push(Line::Heading(title.clone()));
            }
            if matches!(self.items.get(from), Some(Item::Emoji(_))) {
                let mut at = from;
                while at < to {
                    self.lines.push(Line::Cells(at, (at + COLUMNS).min(to)));
                    at += COLUMNS;
                }
            } else {
                let items = &self.items;
                self.lines.extend((from..to).map(|i| match &items[i] {
                    Item::Card(card) => Line::Card(i, card.height()),
                    _ => Line::Item(i),
                }));
            }
        }
        self.list.reset(self.lines.len());
    }

    /// What was fetched from the internet, in its waiting rows' places
    /// (the row picked kept picked).
    fn fetched(&mut self, fetched: Vec<(usize, Vec<Item>)>, cx: &mut Context<Self>) {
        let picked = self.selected;
        let moved: usize = fetched.iter().filter(|(at, rows)| *at < picked && !rows.is_empty()).map(|(_, rows)| rows.len() - 1).sum();
        fill(&mut self.items, &mut self.sections, fetched);
        self.lay_out();
        self.selected = (picked + moved).min(self.items.len().saturating_sub(1));
        cx.notify();
    }

    /// The weather's card, with the weather (or why not).
    fn forecast(&mut self, ix: usize, forecast: Result<crate::weather::Forecast, String>, cx: &mut Context<Self>) {
        let Some(Item::Card(card)) = self.items.get_mut(ix) else { return };
        if card.pending.take().is_none() {
            return;
        }
        match forecast {
            Ok(f) => *card = weather_card(&f),
            Err(why) => {
                card.text = "No weather to be had".into();
                card.under = why;
            }
        }
        let height = card.height();
        if let Some(line) = self.lines.iter().position(|l| matches!(l, Line::Card(i, _) if *i == ix)) {
            self.lines[line] = Line::Card(ix, height);
            self.list.splice(line..line + 1, 1);
        }
        cx.notify();
    }

    /// The translation's card, translated (or why not).
    fn translated(&mut self, ix: usize, outcome: crate::translate::Outcome, from: &crate::translate::Language, to: &crate::translate::Language, cx: &mut Context<Self>) {
        use crate::translate::Outcome;
        let Some(Item::Card(card)) = self.items.get_mut(ix) else { return };
        if card.pending.take().is_none() {
            return;
        }
        match outcome {
            Outcome::Done(text) => {
                card.text = text.replace('\n', " ");
                card.copy = Some((text, "Copy Translation"));
            }
            Outcome::NotDownloaded => {
                card.text = format!("Download {} and {} to translate here", from.name, to.name);
                card.under = "In System Settings > General > Language & Region > Translation Languages (once, then it's all on this Mac).".into();
                card.link = Some(("x-apple.systempreferences:com.apple.Localization-Settings.extension".into(), "Open Language & Region"));
                card.opens = true;
            }
            Outcome::Failed(why) => {
                card.text = "Couldn't translate it".into();
                card.under = why;
            }
        }
        let height = card.height();
        if let Some(line) = self.lines.iter().position(|l| matches!(l, Line::Card(i, _) if *i == ix)) {
            self.lines[line] = Line::Card(ix, height);
            self.list.splice(line..line + 1, 1);
        }
        cx.notify();
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let (at, n) = match self.acting {
            Some(at) => (at, self.actions(cx).len()),
            None => (self.selected, self.items.len()),
        };
        if n == 0 {
            return;
        }
        let to = match (self.acting, self.items.get(self.selected)) {
            // (In the emoji's grid: a row up or down.)
            (None, Some(Item::Emoji(_))) => self.row_step(by),
            _ => (at as isize + by).rem_euclid(n as isize) as usize,
        };
        // (Past half a question's row: it's not to be picked.)
        let to = match (self.acting, self.items.get(to)) {
            (None, Some(Item::Unfinished { .. })) if n > 1 => (to as isize + by.signum()).rem_euclid(n as isize) as usize,
            _ => to,
        };
        match &mut self.acting {
            Some(at) => *at = to,
            None => {
                self.selected = to;
                self.confirming = None;
                self.reveal();
            }
        }
        cx.notify();
    }

    /// The emoji a row up or down (`by`) from the one picked: in the same
    /// column, or the row's last.
    fn row_step(&self, by: isize) -> usize {
        let rows: Vec<(usize, usize)> = self.lines.iter().filter_map(|l| if let Line::Cells(a, b) = *l { Some((a, b)) } else { None }).collect();
        let Some(row) = rows.iter().position(|(a, b)| (*a..*b).contains(&self.selected)) else { return self.selected };
        let column = self.selected - rows[row].0;
        let to = (row as isize + by).rem_euclid(rows.len() as isize) as usize;
        let (a, b) = rows[to];
        (a + column).min(b - 1)
    }

    /// The emoji beside the one picked (`by` 1: right, -1: left).
    fn cell_step(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.items.len() as isize;
        if n > 0 {
            self.selected = (self.selected as isize + by).rem_euclid(n) as usize;
            self.reveal();
            cx.notify();
        }
    }

    /// Switch Window's shortcut pressed again: the next window (or, if
    /// they're not listed yet, once they are).
    fn next_window(&mut self, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            self.steps += 1;
        } else {
            self.step(1, cx);
        }
    }

    /// Show `mode`'s list instead, the field emptied.
    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        if mode != Mode::Extension {
            self.pages.clear();
        }
        (self.switching, self.steps) = (Switching::No, 0);
        (self.acting, self.confirming) = (None, None);
        self.input.update(cx, |i, cx| {
            i.set_value("", window, cx);
            i.focus(window, cx);
        });
        self.find(cx);
        cx.notify();
    }

    /// Scroll the selected result into view (a section's first with its
    /// heading). Worked out from the lines' heights, which are known.
    fn reveal(&self) {
        let Some(line) = self.lines.iter().position(|l| l.holds(self.selected)) else { return };
        let first = if line > 0 && matches!(self.lines[line - 1], Line::Heading(_)) { line - 1 } else { line };
        let top_of = |line: usize| self.lines[..line].iter().map(Line::height).sum::<f32>();
        let scrolled = self.list.logical_scroll_top();
        let now = top_of(scrolled.item_ix.min(self.lines.len())) + f32::from(scrolled.offset_in_item);
        let (top, bottom, view) = (top_of(first), top_of(line) + self.lines[line].height(), self.list_height());
        let to = if top < now {
            top
        } else if bottom > now + view {
            bottom - view
        } else {
            return;
        };
        // (Back into a line and how far into it.)
        let (mut item_ix, mut above) = (0, 0.);
        while item_ix < self.lines.len() && above + self.lines[item_ix].height() <= to {
            above += self.lines[item_ix].height();
            item_ix += 1;
        }
        self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(to - above) });
    }

    /// Gone, and the app it took focus from given it back (unless you've
    /// clicked into another).
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_keeping_focus(window, cx);
        macos::panel_give_back();
    }

    /// Gone, F2 keeping the focus it took: what was chosen brings a window
    /// forward, F2's or an app's (else the app you were in came forward
    /// first, then that).
    fn close_keeping_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cx.try_global::<Up>().and_then(|up| up.0.as_ref()).is_some_and(|(_, panel)| panel.entity_id() == cx.entity_id()) {
            cx.set_global(Up(None));
        }
        window.remove_window();
    }

    /// Return on a result: work out the sum's answer (copied), run the
    /// command (asking first if it can't be undone), open the app, or go to
    /// the file or folder: open it (a folder in Finder2, in the window it
    /// was opened from if any), or with `reveal` show it in its folder.
    fn choose(&mut self, ix: usize, reveal: bool, window: &mut Window, cx: &mut Context<Self>) {
        // (A page to read: its first action.)
        if self.items.get(ix).is_none() && self.reading() {
            return self.choose_ext(reveal, window, cx);
        }
        match self.items.get(ix) {
            // (Nothing to do yet: it's still being typed, or fetched.)
            None | Some(Item::Unfinished { .. } | Item::Waiting { .. }) => {}
            Some(Item::Answer { copied, .. }) => {
                macos::copy_text(copied);
                self.close(window, cx);
            }
            Some(Item::Card(card)) => {
                // (⌘Return: the other.)
                let opens = card.opens != reveal || card.copy.is_none();
                match (opens, &card.link, &card.copy) {
                    (true, Some((link, _)), _) => {
                        cx.open_url(link);
                        self.close_keeping_focus(window, cx);
                    }
                    (_, _, Some(_)) => {
                        copy_card(card);
                        self.close(window, cx);
                    }
                    _ => {}
                }
            }
            Some(Item::Command(Command { run: commands::Run::Mode(mode), .. })) => {
                let mode = *mode;
                self.set_mode(mode, window, cx);
            }
            Some(Item::Ext(_)) => self.choose_ext(reveal, window, cx),
            Some(Item::Command(Command { run: commands::Run::Extension { ext, command }, key, .. })) => {
                if let Some(key) = key {
                    recent::chose(Chosen::Command(key.clone()));
                }
                let Some(ext) = crate::extensions::by_id(ext) else { return };
                // (Its keyword typed, and more: what's after it, on its page.)
                let keyword = ext.spec(command).and_then(|s| s.keyword.clone()).map(|k| k.to_lowercase());
                let typed = self.searched.trim();
                let after = keyword.and_then(|k| typed.to_lowercase().starts_with(&format!("{k} ")).then(|| typed[k.len() + 1..].trim().to_string()));
                let from = std::sync::Arc::new(pages::Origin { ext, command: command.clone(), args: serde_json::Value::Null });
                self.open_page(from, after.unwrap_or_default(), window, cx);
            }

            Some(Item::Command(Command { run: run @ (commands::Run::Colors | commands::Run::Resize | commands::Run::Store), key, .. })) => {
                if let Some(key) = key {
                    recent::chose(Chosen::Command(key.clone()));
                }
                let (ext, command, args) = match run {
                    commands::Run::Colors => (crate::colors::extension(), "colors", serde_json::Value::Null),
                    commands::Run::Store => (crate::store::extension(), "browse", serde_json::Value::Null),
                    _ => (crate::resize::extension(), "resize", crate::resize::args(&crate::resize::selected(cx))),
                };
                let from = std::sync::Arc::new(pages::Origin { ext, command: command.into(), args });
                self.open_page(from, String::new(), window, cx);
            }
            Some(Item::Command(Command { run: commands::Run::Fill(text), .. })) => {
                let text = text.clone();
                self.input.update(cx, |i, cx| i.set_value(text, window, cx));
                self.find(cx);
            }
            Some(Item::Command(Command { run: commands::Run::Snippet(text), .. })) => {
                let (text, back) = crate::snippets::resolve(text);
                let paste = !reveal && Settings::get(cx).paste_on_return;
                self.close(window, cx);
                if paste {
                    paste_in_front(text, back, cx);
                } else {
                    // (⌘Return, or no pasting: copied.)
                    macos::copy_text(&text);
                }
            }
            Some(Item::Command(Command { run: commands::Run::UseSelection, .. })) => {
                if !macos::accessibility(false) {
                    macos::accessibility(true);
                    return crate::shots::note("Allow Accessibility".into(), "Quick Open needs it to see what's selected in another app.".into(), None, cx);
                }
                if let Some(text) = macos::selected_text(macos::panel_took_from()) {
                    self.selection = Some(text);
                    self.input.update(cx, |i, cx| i.set_value("", window, cx));
                    return self.find(cx);
                }
                // (The app doesn't say what's selected: out of its way, its
                // keyboard given back, it's copied with ⌘C; then Quick Open
                // again, with it.)
                self.close(window, cx);
                cx.spawn(async move |_, cx| {
                    macos::given_back(cx.background_executor()).await;
                    cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
                    let pid = macos::front_pid();
                    let taken = cx.background_executor().spawn(async move { take_selection(pid) }).await;
                    cx.update(|cx| match taken {
                        Some(text) => show_with_selection(text, cx),
                        None => crate::shots::note("Nothing Selected".into(), "Select some text in an app, then try again.".into(), None, cx),
                    });
                })
                .detach();
            }
            Some(Item::Command(Command { run: commands::Run::PasteText(text), .. })) => {
                let text = text.clone();
                self.paste_back(text, window, cx);
            }
            Some(Item::Command(Command { run: commands::Run::PastePlain, .. })) => {
                self.close(window, cx);
                paste_plain(cx);
            }
            Some(Item::Repo(repo)) => {
                let repo = repo.clone();
                self.close_keeping_focus(window, cx);
                match (&repo.web, reveal) {
                    (Some(web), false) => cx.open_url(web),
                    _ => {
                        if let Some(entry) = Entry::from_path(repo.root.clone()) {
                            self.go(entry, reveal, cx);
                        }
                    }
                }
            }
            Some(Item::Clip(clip)) => {
                let clip = clip.clone();
                // (⌘Return: only copied.)
                self.deliver(|| crate::clips::copy(&clip), reveal, window, cx);
            }
            Some(Item::Emoji(e)) => {
                let text = e.text;
                crate::emoji::used(text);
                self.deliver(
                    move || {
                        macos::copy_text(text);
                        crate::clips::ignore_this();
                        true
                    },
                    reveal,
                    window,
                    cx,
                );
            }
            Some(Item::Command(command)) => {
                if command.confirm.is_some() && self.confirming != Some(ix) {
                    self.confirming = Some(ix);
                    return cx.notify();
                }
                if let Some(key) = &command.key {
                    recent::chose(Chosen::Command(key.clone()));
                }
                let run = command.run.clone();
                self.close_keeping_focus(window, cx);
                // (After this panel is gone; and, unless it brings F2
                // forward, the app you were in given its place back.)
                cx.defer(move |cx| {
                    if !commands::run(run, cx) {
                        macos::panel_give_back();
                    }
                });
            }
            Some(Item::App(entry) | Item::File(entry)) => {
                let entry = entry.clone();
                self.close_keeping_focus(window, cx);
                self.go(entry, reveal, cx);
            }
        }
    }

    /// Put it on the clipboard (`copy`), then, unless `copy_only` or
    /// Settings say not to, paste it into the app in front: ⌘V, once the
    /// panel's gone (with Accessibility; without, macOS is asked to allow
    /// it, and it's only copied).
    fn deliver(&mut self, copy: impl FnOnce() -> bool, copy_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        let copied = copy();
        let paste = !copy_only && Settings::get(cx).paste_on_return;
        self.close(window, cx);
        if !copied || !paste {
            return;
        }
        if macos::accessibility(false) {
            cx.spawn(async move |_, cx| {
                macos::given_back(cx.background_executor()).await;
                cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
                macos::post_paste();
            })
            .detach();
        } else {
            macos::accessibility(true);
            crate::shots::note(
                "Copied".into(),
                format!("To paste straight into the app, allow {} in System Settings > Privacy & Security > Accessibility.", crate::APP_NAME),
                Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
                cx,
            );
        }
    }

    fn go(&self, entry: Entry, reveal: bool, cx: &mut App) {
        if let Some(search) = &cx.global::<crate::browser::Shared>().search {
            search.opened(&entry.path);
        }
        recent::chose(Chosen::Path(entry.path.clone()));
        let folder = entry.is_dir && !entry.is_bundle();
        if !reveal && !folder {
            // (An app, or a file in its app, comes forward: not Finder2.)
            return cx.open_with_system(&entry.path);
        }
        let path = entry.path.clone();
        let origin = self.origin.clone();
        cx.defer(move |cx| {
            let shown = origin.and_then(|(handle, finder)| {
                let finder = finder.upgrade()?;
                handle
                    .update(cx, |_, window, cx| {
                        finder.update(cx, |f, cx| if reveal { f.reveal(path.clone(), window, cx) } else { f.go_place(path.clone(), window, cx) });
                        window.activate_window();
                    })
                    .ok()
            });
            if shown.is_none() {
                match path.parent().filter(|_| reveal) {
                    Some(parent) => crate::browser::open(parent.to_path_buf(), Some(path), cx),
                    None => crate::browser::open(path, None, cx),
                }
            }
            cx.activate(true);
        });
    }

    /// The running app at `path`, if it is.
    fn running(path: &Path) -> Option<i32> {
        macos::running_apps().into_iter().find(|(_, _, p)| p == path).map(|(pid, ..)| pid)
    }

    /// Paste `text` into the app you were in, over what's selected there
    /// (once it has its keyboard back); without Accessibility, copied.
    fn paste_back(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        if macos::accessibility(false) {
            cx.spawn(async move |_, cx| {
                macos::given_back(cx.background_executor()).await;
                cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
                cx.background_executor().spawn(async move { crate::snippets::paste_in(text, 0) }).await;
            })
            .detach();
        } else {
            macos::copy_text(&text);
            macos::accessibility(true);
        }
    }

    /// What can be done with the selected result: the action, its title,
    /// its shortcut.
    fn actions(&self, cx: &App) -> Vec<(Act, String, &'static str)> {
        let tool = |kind| tools::chosen(kind, Settings::get(cx)).map(|t| t.name);
        match self.items.get(self.selected) {
            None if self.mode == Mode::Extension => self.page_actions(),
            None => Vec::new(),
            Some(Item::Ext(_)) => self.page_actions(),
            Some(Item::Answer { .. }) => vec![(Act::Copy, "Copy Answer".into(), "↩")],
            Some(Item::Unfinished { .. } | Item::Waiting { .. }) => Vec::new(),
            Some(Item::Card(card)) => {
                let copy = card.copy.as_ref().map(|(_, name)| (Act::CopyIt, name.to_string()));
                let open = card.link.as_ref().map(|(_, name)| (Act::OpenLink, name.to_string()));
                let (first, second) = if card.opens { (open, copy) } else { (copy, open) };
                let first = first.map(|(act, name)| (act, name, "↩"));
                let second = second.map(|(act, name)| (act, name, "⌘↩"));
                first.into_iter().chain(second).collect()
            }
            Some(Item::Repo(repo)) => {
                let mut acts = Vec::new();
                if repo.web.is_some() {
                    acts.push((Act::OpenLink, "Open on GitHub".into(), "↩"));
                }
                if let Some(name) = tool(Kind::Editor) {
                    acts.push((Act::Editor, format!("Open in {name}"), "⌘E"));
                }
                if let Some(name) = tool(Kind::Terminal) {
                    acts.push((Act::Terminal, format!("Open in {name}"), "⌘T"));
                }
                acts.push((Act::Reveal, format!("Show in {}", crate::APP_NAME), "⌘↩"));
                if let Some(branch) = &repo.branch {
                    acts.push((Act::CopyBranch, format!("Copy Branch Name ({branch})"), ""));
                }
                acts.push((Act::CopyPath, "Copy Path".into(), "⇧⌘C"));
                acts
            }
            Some(item @ (Item::Clip(_) | Item::Emoji(_))) => {
                let paste = Settings::get(cx).paste_on_return;
                let mut acts = Vec::new();
                if paste {
                    acts.push((Act::Paste, format!("Paste to {}", self.front.as_deref().unwrap_or("the App")), "↩"));
                }
                acts.push((Act::CopyIt, "Copy to Clipboard".into(), if paste { "⌘↩" } else { "↩" }));
                if self.mode == Mode::Screenshots {
                    let folder = crate::shots::folder(cx);
                    let folder = folder.file_name().map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned());
                    acts.push((Act::EditShot, "Edit".into(), "⌘E"));
                    acts.push((Act::PinShot, "Pin to Screen".into(), ""));
                    acts.push((Act::SaveShot, format!("Save to {folder}"), "⌘S"));
                    acts.push((Act::Forget, "Delete Screenshot".into(), ""));
                    return acts;
                }
                if let Item::Clip(clip) = item {
                    match &clip.what {
                        crate::clips::What::Link(_) => acts.push((Act::OpenLink, "Open Link".into(), "")),
                        crate::clips::What::Files(_) => acts.push((Act::OpenLink, format!("Show in {}", crate::APP_NAME), "")),
                        _ => {}
                    }
                    if clip.pinned {
                        acts.push((Act::Unpin, "Unpin".into(), ""));
                    } else {
                        acts.push((Act::Pin, "Pin (Keep for Good)".into(), ""));
                    }
                    acts.push((Act::Forget, "Delete from History".into(), ""));
                }
                acts
            }
            Some(Item::Command(c)) => {
                let mut acts = vec![(Act::Run, c.title.clone(), "↩")];
                if self.shortcut_for().is_some() {
                    acts.push((Act::SetShortcut, "Set Shortcut…".into(), ""));
                }
                acts
            }
            Some(Item::App(e)) => {
                let mut acts = vec![(Act::Open, "Open".into(), "↩"), (Act::Reveal, format!("Show in {}", crate::APP_NAME), "⌘↩")];
                if Self::running(&e.path).is_some() {
                    acts.push((Act::Quit, "Quit".into(), ""));
                    acts.push((Act::ForceQuit, "Force Quit".into(), ""));
                }
                acts.push((Act::SetShortcut, "Set Shortcut…".into(), ""));
                acts.push((Act::CopyPath, "Copy Path".into(), "⇧⌘C"));
                if crate::uninstall::refused(&e.path).is_none() {
                    acts.push((Act::Uninstall, "Uninstall…".into(), ""));
                }
                acts
            }
            Some(Item::File(e)) => {
                let mut acts = vec![(Act::Open, "Open".into(), "↩"), (Act::Reveal, format!("Show in {}", crate::APP_NAME), "⌘↩")];
                if let Some(name) = tool(Kind::Terminal) {
                    acts.push((Act::Terminal, format!("Open in {name}"), "⌘T"));
                }
                if let Some(name) = tool(Kind::Editor) {
                    acts.push((Act::Editor, format!("Open in {name}"), "⌘E"));
                }
                if !e.is_dir || e.is_bundle() {
                    acts.push((Act::QuickLook, "Quick Look".into(), "⌘Y"));
                }
                if !e.is_dir && crate::quick_actions::is_image(&e.path) {
                    acts.push((Act::Wallpaper, "Set as Desktop Picture".into(), ""));
                }
                acts.push((Act::AirDrop, "AirDrop".into(), ""));
                if !e.is_dir {
                    acts.push((Act::Email, "Email".into(), ""));
                } else if !e.is_bundle() {
                    acts.push((Act::SetShortcut, "Set Shortcut…".into(), ""));
                }
                // (Make Smaller, Convert…: what a photo, a PDF, a video can be.)
                for action in crate::quick_actions::for_items(std::slice::from_ref(e)) {
                    if !matches!(action, crate::quick_actions::QuickAction::RotateLeft | crate::quick_actions::QuickAction::RotateRight) {
                        acts.push((Act::Convert(action), action.title().into(), ""));
                    }
                }
                acts.push((Act::CopyPath, "Copy Path".into(), "⇧⌘C"));
                acts.push((Act::Trash, format!("Move to {}", macos::bin()), ""));
                acts
            }
        }
    }

    /// Do `act` with the selected result.
    /// What the selected result's shortcut would be for, in Settings'
    /// Keyboard: a command's key, or an app's or a folder's "open:<path>".
    fn shortcut_for(&self) -> Option<String> {
        match self.items.get(self.selected)? {
            Item::Command(c) => c.key.clone().filter(|k| !k.starts_with("url:") && commands::by_key(k).is_some()),
            Item::App(e) | Item::File(e) if e.is_dir => Some(format!("open:{}", e.path.display())),
            _ => None,
        }
    }

    fn perform(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.selected;
        let path = self.items.get(ix).and_then(Item::path);
        self.acting = None;
        match act {
            Act::Ext(i) => self.perform_ext(i, window, cx),
            Act::Again | Act::ExtOutput | Act::ExtForget | Act::ExtSettings if self.mode == Mode::Extension => self.perform_page(act, window, cx),
            Act::Again | Act::ExtOutput | Act::ExtForget | Act::ExtSettings => {}
            Act::SetShortcut => {
                if let Some(what) = self.shortcut_for() {
                    self.close_keeping_focus(window, cx);
                    cx.defer(move |cx| {
                        crate::settings_window::open_shortcut(what, cx);
                        cx.activate(true);
                    });
                }
            }
            // (A card: what it says, not Return's.)
            Act::CopyIt | Act::OpenLink if matches!(self.items.get(ix), Some(Item::Card(_))) => {
                if let Some(Item::Card(card)) = self.items.get(ix) {
                    match act {
                        Act::OpenLink => card.link.as_ref().map(|(link, _)| cx.open_url(link)),
                        _ => Some(copy_card(card)),
                    };
                    self.close(window, cx);
                }
            }
            Act::Open | Act::Copy | Act::Run | Act::Paste => self.choose(ix, false, window, cx),
            Act::CopyIt => self.choose(ix, true, window, cx),
            Act::CopyBranch => {
                if let Some(Item::Repo(crate::dev::Repo { branch: Some(branch), .. })) = self.items.get(ix) {
                    macos::copy_text(branch);
                    self.close(window, cx);
                }
            }
            Act::OpenLink if matches!(self.items.get(ix), Some(Item::Repo(_))) => self.choose(ix, false, window, cx),
            Act::OpenLink => {
                if let Some(Item::Clip(clip)) = self.items.get(ix) {
                    match &clip.what {
                        crate::clips::What::Link(url) => cx.open_url(url),
                        crate::clips::What::Files(paths) => {
                            if let Some(first) = paths.first().cloned() {
                                crate::browser::open(first.parent().map(Path::to_path_buf).unwrap_or_default(), Some(first), cx);
                                cx.activate(true);
                            }
                        }
                        _ => {}
                    }
                    self.close(window, cx);
                }
            }
            Act::Wallpaper => {
                let Some(path) = path else { return };
                self.close(window, cx);
                match macos::set_wallpaper(&path) {
                    Ok(()) => crate::shots::note("Desktop Picture Set".into(), path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), None, cx),
                    Err(why) => crate::shots::note("Couldn't Set the Desktop Picture".into(), why, None, cx),
                }
            }
            Act::AirDrop | Act::Email => {
                let Some(path) = path else { return };
                self.close(window, cx);
                if !macos::share_file(&path, act == Act::Email) {
                    let how = if act == Act::Email { "by email" } else { "by AirDrop" };
                    crate::shots::note(format!("Can't Send It {how}"), "macOS says it can't be sent that way here.".into(), None, cx);
                }
            }
            Act::Pin | Act::Unpin => {
                if let Some(Item::Clip(clip)) = self.items.get(ix) {
                    crate::clips::set_pinned(clip.id, act == Act::Pin);
                    self.acting = None;
                    self.find(cx);
                }
            }
            Act::Forget => {
                if let Some(Item::Clip(clip)) = self.items.get(ix) {
                    match clip.image().filter(|p| crate::shots::is_kept(p)) {
                        Some(shot) => {
                            let _ = std::fs::remove_file(shot);
                        }
                        None => crate::clips::delete(clip.id),
                    }
                    self.find(cx);
                }
            }
            Act::Uninstall => {
                if let Some(path) = path {
                    self.close(window, cx);
                    crate::uninstall::open(path, cx);
                }
            }
            Act::Convert(crate::quick_actions::QuickAction::Resize) => {
                let Some(path) = path else { return };
                let from = std::sync::Arc::new(pages::Origin { ext: crate::resize::extension(), command: "resize".into(), args: crate::resize::args(&[path]) });
                self.open_page(from, String::new(), window, cx);
            }
            Act::Convert(crate::quick_actions::QuickAction::CopyText) => {
                let Some(path) = path else { return };
                self.close(window, cx);
                crate::quick_actions::copy_text(vec![path], cx);
            }
            Act::Convert(action) => {
                let Some(path) = path else { return };
                self.close(window, cx);
                cx.spawn(async move |_, cx| {
                    let made = cx.background_executor().spawn(async move { crate::quick_actions::run(action, &[path]) }).await;
                    cx.update(|cx| match made {
                        Ok(made) => {
                            let name = made.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            crate::shots::note(format!("{}: Done", action.title()), name, None, cx);
                            if let Some(first) = made.first() {
                                crate::browser::show(first.parent().map(Path::to_path_buf), false, cx);
                            }
                        }
                        Err(e) => crate::shots::note(format!("Couldn't {}", action.title().to_lowercase()), e.to_string(), None, cx),
                    });
                })
                .detach();
            }
            Act::EditShot | Act::PinShot | Act::SaveShot => {
                let Some(shot) = self.items.get(ix).and_then(|item| if let Item::Clip(clip) = item { clip.image() } else { None }) else { return };
                self.close(window, cx);
                match act {
                    Act::EditShot => crate::shots::edit(shot, cx),
                    Act::PinShot => crate::shots::pin(shot, cx),
                    _ => match crate::shots::save_copy(&shot, cx) {
                        Ok(to) => crate::shots::note("Saved".into(), to.display().to_string(), None, cx),
                        Err(e) => crate::shots::note("Couldn't Save".into(), e, None, cx),
                    },
                }
            }
            Act::Reveal => self.choose(ix, true, window, cx),
            Act::Terminal | Act::Editor => {
                let kind = if act == Act::Terminal { Kind::Terminal } else { Kind::Editor };
                if let (Some(path), Some(tool)) = (path, tools::chosen(kind, Settings::get(cx))) {
                    self.close(window, cx);
                    recent::chose(Chosen::Path(path.clone()));
                    tools::open(&tool, kind, &[path]);
                }
            }
            Act::QuickLook => {
                if let Some(path) = path {
                    self.close_keeping_focus(window, cx);
                    // (Quick Look's panel is Finder2's: it comes forward for it.)
                    cx.activate(true);
                    macos::quicklook_show(&[path], 0);
                }
            }
            Act::CopyPath => {
                if let Some(path) = path {
                    macos::copy_text(&path.to_string_lossy());
                    self.close(window, cx);
                }
            }
            Act::Trash => {
                if let Some(path) = path {
                    match macos::trash(&path) {
                        Ok(to) => crate::finder::remember_trashed(&[(path, to)]),
                        Err(e) => self.error = Some(e.into()),
                    }
                    self.find(cx);
                }
            }
            Act::Quit | Act::ForceQuit => {
                if let Some(pid) = path.as_deref().and_then(Self::running) {
                    macos::quit_app(pid, act == Act::ForceQuit);
                }
                self.close(window, cx);
            }
        }
        cx.notify();
    }

    /// What's shown after what's typed, in gray: a hint, or the rest of the
    /// result picked.
    fn ghost(&self, window: &mut Window, cx: &mut App) -> Option<String> {
        if self.mode != Mode::Search || self.acting.is_some() {
            return None;
        }
        // (Not after what an input method's still composing.)
        if self.input.update(cx, |input, cx| input.marked_text_range(window, cx)).is_some() {
            return None;
        }
        let typed = self.input.read(cx).value();
        if typed.is_empty() {
            return None;
        }
        self.hint.clone().or_else(|| self.items.get(self.selected).and_then(|item| completion(item, &typed)))
    }

    /// Tab: the hint taken into the field. Whether there was one.
    fn take_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(hint) = self.ghost(window, cx) else { return false };
        self.hint = None;
        let text = format!("{}{hint}", self.input.read(cx).value());
        self.input.update(cx, |i, cx| i.set_value(text, window, cx));
        self.find(cx);
        true
    }

    /// ⌘K, ⌘T, ⌘E, ⌘Y, ⇧⌘C: the action, without the list. Whether it was one.
    fn shortcut(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = key.modifiers;

        // (Across the emoji's grid.)
        if self.acting.is_none() && matches!(self.items.get(self.selected), Some(Item::Emoji(_))) && !m.platform && !m.control && !m.alt && !m.shift {
            match key.key.as_str() {
                "left" => return { self.cell_step(-1, cx); true },
                "right" => return { self.cell_step(1, cx); true },
                _ => {}
            }
        }
        if !m.platform || m.control || m.alt {
            return false;
        }
        let act = match (key.key.as_str(), m.shift) {
            ("k", false) => {
                let none = if self.mode == Mode::Extension { self.actions(cx).is_empty() } else { self.items.is_empty() };
                self.acting = if self.acting.is_some() || none { None } else { Some(0) };
                cx.notify();
                return true;
            }
            (",", false) if self.mode == Mode::Extension => Act::ExtSettings,
            ("t", false) => Act::Terminal,
            ("r", false) => Act::Again,
            ("e", false) if self.mode == Mode::Screenshots => Act::EditShot,
            ("s", false) if self.mode == Mode::Screenshots => Act::SaveShot,
            ("e", false) => Act::Editor,
            ("y", false) => Act::QuickLook,
            ("c", true) => Act::CopyPath,
            _ => return false,
        };
        if self.actions(cx).iter().any(|(a, ..)| *a == act) {
            self.perform(act, window, cx);
        }
        true
    }

    /// The results' list's height: theirs, or at most `LIST`.
    fn list_height(&self) -> f32 {
        self.lines.iter().map(Line::height).sum::<f32>().min(LIST)
    }

    /// The height that fits what's shown.
    /// What's said in place of a list, if there's none.
    fn note(&self, cx: &App) -> Option<SharedString> {
        let typed = !self.input.read(cx).value().trim().is_empty();
        if self.mode == Mode::Extension {
            return self.page_note(typed);
        }
        match (&self.error, self.items.is_empty(), typed, self.mode) {
            (Some(e), ..) => Some(e.clone()),
            (None, true, _, Mode::Clipboard) => Some(match crate::clips::blocked() {
                Some(why) => format!("{why} Allow it in System Settings > Privacy & Security > Paste from Other Apps.").into(),
                None if !Settings::get(cx).clipboard_history => "The clipboard's history is off: Settings > Clipboard turns it on.".into(),
                None if typed => "Nothing copied matches.".into(),
                None => "Nothing copied yet: what's copied from now on shows up here.".into(),
            }),
            (None, true, typed, Mode::Screenshots) => Some(if typed {
                "No screenshot taken then.".into()
            } else {
                "No screenshots yet: the ones you take show up here (the last 50).".into()
            }),
            (None, true, false, Mode::Windows) => Some("No windows open.".into()),
            (None, true, true, _) => Some("Nothing by that name.".into()),
            (None, true, false, _) => Some("Type an app, a command, a sum, or a few letters of a file's name.".into()),
            _ => None,
        }
    }

    /// Shown, once what it shows is in (or `REVEAL_BY` has passed): made a
    /// panel, sized to it and drawn, and only then put on screen, so it
    /// comes up whole (not a frame of a window with no material, then the
    /// material, then the list, as it did).
    fn appear(&mut self, cx: &mut Context<Self>) {
        if self.view.is_null() || std::mem::replace(&mut self.native, true) {
            return;
        }
        let height = self.fitting(self.note(cx).is_some(), cx);
        self.height = height;
        if self.mode == Mode::Search && self.searched.is_empty() && self.selection.is_none() {
            cx.default_global::<Seed>().1 = height;
        }
        let view = self.view;
        // (A task, not now: setting it up resizes it, which calls back into
        // the window. Resized, or what it shows changed since it was made,
        // it's drawn again as it's shown; else what was drawn then is.)
        cx.spawn_with_priority(Priority::High, async move |_| {
            macos::panel_prepare(view, RADIUS as f64, height as f64);
            macos::panel_present(view);
            macos::panel_take_focus(view);
        })
        .detach();
    }

    fn fitting(&self, note: bool, cx: &App) -> f32 {
        let list = match (self.acting, self.mode) {
            (Some(_), _) => HEADING + self.actions(cx).len() as f32 * ACTION,
            // (The clipboard's preview and the emoji's grid want the room.)
            (None, Mode::Clipboard | Mode::Emoji | Mode::Screenshots) => LIST,
            (None, Mode::Windows) if self.previews() => LIST,
            (None, Mode::Extension) if self.previews() || self.reading() => LIST,
            (None, Mode::Search | Mode::Windows | Mode::Extension) => self.list_height(),
        };
        // (And the list's padding, and the lines.)
        field_height(cx) + list + if note { NOTE } else { 0. } + FOOTER + 8. + 4.
    }

    fn title(item: &Item) -> SharedString {
        match item {
            Item::Answer { shown, .. } => shown.clone().into(),
            Item::Card(card) => card.text.clone().into(),
            Item::Command(c) => c.title.clone().into(),
            // An app by its name ("Safari", not "Safari.app").
            // (Its name as Finder shows it: see `named`.)
            Item::App(e) if !e.name.ends_with(".app") => e.name.clone(),
            Item::App(e) => e.path.file_stem().map_or(e.name.clone(), |s| s.to_string_lossy().into_owned().into()),
            Item::File(e) => e.name.clone(),
            Item::Clip(clip) => clip.title().into(),
            Item::Emoji(e) => capitalized(e.name).into(),
            Item::Repo(repo) => repo.name.clone().into(),
            Item::Unfinished { typed, .. } => typed.clone().into(),
            Item::Waiting { asked, .. } => asked.clone().into(),
            Item::Ext(e) => e.row.title.clone().into(),
        }
    }


    /// What Return does with the selected result, for the footer.
    fn primary(&self, cx: &App) -> SharedString {
        let thing: &'static str = match self.items.get(self.selected) {
            Some(Item::Ext(_)) => return self.ext_actions().first().map_or(SharedString::default(), |a| a.title.clone().into()),
            None if self.reading() => return self.ext_actions().first().map_or(SharedString::default(), |a| a.title.clone().into()),
            None => "",
            Some(Item::Clip(_) | Item::Emoji(_)) if Settings::get(cx).paste_on_return => {
                return format!("Paste to {}", self.front.as_deref().unwrap_or("the App")).into();
            }
            Some(Item::Clip(_) | Item::Emoji(_)) => "Copy to Clipboard",
            Some(Item::Answer { .. }) => "Copy Answer",
            Some(Item::Card(card)) => {
                let (first, second) = if card.opens { (&card.link, &card.copy) } else { (&card.copy, &card.link) };
                return first.as_ref().or(second.as_ref()).map_or("", |(_, name)| name).into();
            }
            Some(Item::Command(Command { run: commands::Run::Snippet(_), .. })) if Settings::get(cx).paste_on_return => {
                return format!("Paste to {}", self.front.as_deref().unwrap_or("the App")).into();
            }
            Some(Item::Command(Command { run: commands::Run::Snippet(_), .. })) => "Copy Snippet",
            Some(Item::Command(Command { run: commands::Run::Fill(_), .. })) => "Type What to Look For",
            Some(Item::Command(Command { run: commands::Run::Quit { .. }, kind: "Quit App", .. })) => "Quit App",
            Some(Item::Command(Command { run: commands::Run::CleanKeyboard(_), .. })) => "Turn Keys Off",
            Some(Item::Command(Command { run: commands::Run::Music(_, what), .. })) => match what {
                crate::music::Do::Play(_) | crate::music::Do::Resume => "Play",
                crate::music::Do::PlayPause => "Play/Pause",
                crate::music::Do::Next => "Next Track",
                crate::music::Do::Previous => "Previous Track",
                crate::music::Do::Love => "Love",
            },
            Some(Item::Command(Command { kind: "Search", .. })) => "Search",
            Some(Item::Command(Command { run: commands::Run::Extension { .. } | commands::Run::Colors | commands::Run::Resize | commands::Run::Store, .. })) => "Open Command",
            Some(Item::Command(c)) => match c.kind {
                "Quicklink" => "Open Link",
                "Process" => "Stop It",
                "Website" => "Open Website",
                "Reminders" => "Add Reminder",
                "New Event" => "Add to Calendar",
                "Join" => "Join Call",
                "Event" => "Open Calendar",
                "Timer" if c.title.starts_with("Cancel") => "Cancel Timer",
                "Timer" => "Start Timer",
                "QR Code" => "Type It",
                "Dictionary" => "Open in Dictionary",
                "Translation" => "Open Google Translate",
                "Web Search" => "Search the Web",
                "System Settings" => "Open Settings",
                _ => "Run Command",
            },
            Some(Item::App(_)) => "Open Application",
            Some(Item::File(e)) if e.is_dir && !e.is_bundle() => "Open Folder",
            Some(Item::File(_)) => "Open File",
            Some(Item::Repo(repo)) if repo.web.is_some() => "Open on GitHub",
            Some(Item::Repo(_)) => "Show Repo",
            // (Nothing yet.)
            Some(Item::Unfinished { .. } | Item::Waiting { .. }) => "",
        };
        thing.into()
    }

    /// The selection's highlight: gray, as Raycast's (not the accent's).
    fn highlight(&self, window: &Window) -> Hsla {
        let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        self.palette.label.opacity(if dark { 0.11 } else { 0.07 })
    }

    fn red(window: &Window) -> Hsla {
        let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        rgb(if dark { 0xff453a } else { 0xd70015 }).into()
    }

    fn heading(&self, text: impl Into<SharedString>) -> AnyElement {
        div()
            .w_full()
            .h(px(HEADING))
            .px(px(18.))
            .flex()
            .items_end()
            .pb(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.palette.secondary)
            .child(text.into())
            .into_any_element()
    }

    /// Keys as keycaps: "⌘K" two, "esc" one. (In the system's font, whatever
    /// the panel's: not every font has ↩ and ⌘.)
    fn caps(&self, keys: &str) -> Div {
        let keys: Vec<String> = if keys.len() > 1 && keys.chars().all(|c| c.is_ascii_alphabetic()) {
            vec![keys.into()]
        } else {
            keys.chars().map(String::from).collect()
        };
        self.keycaps(keys)
    }

    /// A shortcut ("shift-cmd-4") as keycaps: ⇧, ⌘, 4 (and "Space" one).
    fn shortcut_caps(&self, shortcut: &str) -> Div {
        let label = crate::hotkeys::label(shortcut);
        let key = label.trim_start_matches(['⌃', '⌥', '⇧', '⌘']);
        let mut keys: Vec<String> = label[..label.len() - key.len()].chars().map(String::from).collect();
        keys.push(key.to_string());
        self.keycaps(keys)
    }

    fn keycaps(&self, keys: Vec<String>) -> Div {
        div().flex().gap(px(3.)).font_family(".SystemUIFont").children(keys.into_iter().map(|key| {
            div()
                .h(px(20.))
                .min_w(px(20.))
                .px(px(5.))
                .rounded(px(5.))
                .bg(self.palette.label.opacity(0.07))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.))
                .text_color(self.palette.secondary)
                .child(key)
        }))
    }

    /// A symbol, white on a tile of `color` lit a little from above (as
    /// System Settings draws its panes').
    fn tile(name: &'static str, color: u32, window: &Window) -> AnyElement {
        let color: Hsla = rgb(color).into();
        let lit = Hsla { l: (color.l + 0.07).min(1.), ..color };
        div()
            .size(px(ICON))
            .flex_shrink_0()
            .rounded(px(5.))
            .bg(linear_gradient(180., linear_color_stop(lit, 0.), linear_color_stop(color, 1.)))
            .flex()
            .items_center()
            .justify_center()
            .child(symbol(name, 11., SEMIBOLD, white(), window))
            .into_any_element()
    }

    fn render_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = self.palette;
        let Some(at) = self.acting else { return Vec::new() };
        let name = self.items.get(self.selected).map(Self::title).or_else(|| self.page_title().map(Into::into)).unwrap_or_default();
        let highlight = self.highlight(window);
        let mut rows = vec![self.heading(format!("Actions for {name}"))];
        for (i, (act, title, key)) in self.actions(cx).into_iter().enumerate() {
            let on = i == at;
            let symbol_name = match act {
                Act::Open | Act::Run => "arrow.up.forward.app",
                Act::Reveal => "folder",
                Act::Terminal => "terminal",
                Act::Editor => "chevron.left.forwardslash.chevron.right",
                Act::QuickLook => "eye",
                Act::CopyPath | Act::Copy => "doc.on.doc",
                Act::Trash => "trash",
                Act::Quit => "xmark.circle",
                Act::ForceQuit => "exclamationmark.octagon",
                Act::Paste => "doc.on.clipboard",
                Act::CopyIt => "doc.on.doc",
                Act::OpenLink => "arrow.up.forward.square",
                Act::Forget => "trash",
                Act::CopyBranch => "arrow.triangle.branch",
                Act::Convert(_) => "wand.and.stars",
                Act::Uninstall => "trash.slash",
                Act::EditShot => "pencil.and.outline",
                Act::PinShot => "pin",
                Act::SaveShot => "square.and.arrow.down",
                Act::Wallpaper => "photo.on.rectangle",
                Act::AirDrop => "airplayaudio",
                Act::Email => "envelope",
                Act::Pin => "pin",
                Act::Unpin => "pin.slash",
                Act::Again => "arrow.clockwise",
                Act::SetShortcut => "command",
                Act::Ext(n) => self.ext_actions().get(n).and_then(|a| a.icon.as_deref()).map_or("arrow.right.circle", crate::extensions::interned),
                Act::ExtOutput => "text.alignleft",
                Act::ExtForget => "key.slash",
                Act::ExtSettings => "gearshape",
            };
            // (What can't be taken back, in red, as Raycast shows it.)
            let destructive = match act {
                Act::Ext(n) => self.ext_actions().get(n).is_some_and(|a| a.destructive),
                _ => matches!(act, Act::Trash | Act::ForceQuit | Act::Forget),
            };
            let text = if destructive { Self::red(window) } else { palette.label };
            let row = div()
                .id(("act", i))
                .h(px(ACTION))
                .px(px(10.))
                .rounded(px(8.))
                .flex()
                .items_center()
                .gap(px(10.))
                .when(on, |d| d.bg(highlight))
                .child(div().size(px(ICON)).flex().items_center().justify_center().child(symbol(symbol_name, 14., MEDIUM, text, window)))
                .child(div().flex_1().text_color(text).child(title))
                .when(!key.is_empty(), |d| d.child(self.caps(key)))
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if this.acting != Some(i) {
                        this.acting = Some(i);
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.perform(act, window, cx)));
            rows.push(div().w_full().px_2().child(row).into_any_element());
        }
        rows
    }

    /// The list's line `ix` (the list asks for those in view).
    fn render_line(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let i = match self.lines.get(ix) {
            Some(Line::Heading(text)) => return self.heading(text.clone()),
            Some(&Line::Item(i)) if matches!(self.items.get(i), Some(Item::Unfinished { .. } | Item::Waiting { .. })) => return self.render_coming(i, window, cx),
            Some(&Line::Item(i)) if matches!(self.items.get(i), Some(Item::Ext(_))) => return self.render_ext_line(i, window, cx),
            Some(&Line::Item(i)) if i < self.items.len() => i,
            Some(&Line::Card(i, height)) if i < self.items.len() => return self.render_card(i, height, window, cx),
            Some(&Line::Cells(from, to)) => return self.render_cells(from, to.min(self.items.len()), window, cx),
            _ => return div().into_any_element(),
        };
        let palette = self.palette;
        let icon_px = (ICON * window.scale_factor()).round() as u32;
        let on = i == self.selected;
        let asking = self.confirming == Some(i);
        let item = &self.items[i];
        let title = Self::title(item);
        // An icon, what's beside the title, and what it is (on the right).
        let (icon, subtitle, kind): (AnyElement, SharedString, &str) = match item {
            Item::Answer { sum, kind, icon, .. } => (Self::tile(icon.0, icon.1, window), sum.clone().into(), *kind),
            Item::Card(_) | Item::Unfinished { .. } | Item::Waiting { .. } | Item::Ext(_) => return div().into_any_element(),
            Item::Command(c) => {
                let icon = match &c.icon {
                    Icon::Tile(name, color) => Self::tile(name, *color, window),
                    Icon::Image(file) => img(file.clone()).size(px(ICON)).rounded(px(4.)).object_fit(ObjectFit::Contain).into_any_element(),
                    Icon::Of(path) => match Entry::from_path(path.clone()) {
                        Some(e) => file_image(cx.global_mut::<PanelIcons>().0.get(&e, icon_px), &e, ICON, palette.accent),
                        None => Self::tile("gearshape.fill", commands::SYSTEM, window),
                    },
                };
                let subtitle = if asking { c.confirm.clone().unwrap_or_default() } else { c.subtitle.clone() };
                (icon, subtitle.into(), c.kind)
            }
            Item::App(e) => (file_image(cx.global_mut::<PanelIcons>().0.get(e, icon_px), e, ICON, palette.accent), "".into(), "Application"),
            Item::File(e) => {
                let kind = if e.is_dir && !e.is_bundle() { "Folder" } else { "File" };
                (file_image(cx.global_mut::<PanelIcons>().0.get(e, icon_px), e, ICON, palette.accent), location(&e.path), kind)
            }
            Item::Clip(clip) => {
                use crate::clips::What;
                let icon = match &clip.what {
                    What::Text(_) => Self::tile("text.alignleft", commands::CLIPBOARD, window),
                    What::Link(_) => Self::tile("link", commands::FINDER2, window),
                    What::Image { .. } => match clip.image() {
                        Some(file) => img(file).size(px(ICON)).rounded(px(4.)).object_fit(ObjectFit::Cover).into_any_element(),
                        None => Self::tile("photo", commands::CLIPBOARD, window),
                    },
                    What::Files(paths) => match paths.first().and_then(|p| Entry::from_path(p.clone())) {
                        Some(e) => file_image(cx.global_mut::<PanelIcons>().0.get(&e, icon_px), &e, ICON, palette.accent),
                        None => Self::tile("doc", commands::CLIPBOARD, window),
                    },
                };
                (icon, "".into(), "")
            }
            Item::Emoji(e) => (div().text_size(px(18.)).child(e.text).into_any_element(), e.group.into(), "Emoji"),
            Item::Repo(repo) => {
                let icon = match Entry::from_path(repo.root.clone()) {
                    Some(e) => file_image(cx.global_mut::<PanelIcons>().0.get(&e, icon_px), &e, ICON, palette.accent),
                    None => Self::tile("arrow.triangle.branch", commands::DEV, window),
                };
                (icon, location(&repo.root), "Repository")
            }
        };
        // (A clip says when on the right.)
        let ago: Option<SharedString> = match item {
            Item::Clip(clip) => Some(crate::clips::ago(clip.at).into()),
            _ => None,
        };
        let running = matches!(item, Item::App(e) if self.running.contains(&e.path));
        // (A list beside a preview is narrow: the clipboard's, the windows'.)
        let windows_row = self.mode == Mode::Windows && self.previews();
        let clip_row = matches!(item, Item::Clip(_)) || windows_row;
        // (Its shortcut, as Settings set it, or the keyword it's typed by.)
        let keys = match item {
            Item::Command(c) => commands::keys(c, Settings::get(cx)).map(|keys| match keys {
                commands::Keys::Shortcut(s) => self.shortcut_caps(&s),
                commands::Keys::Keyword(k) => self.keycaps(vec![k]),
            }),
            Item::App(e) => Settings::get(cx).shortcuts.get(&format!("open:{}", e.path.display())).map(|s| self.shortcut_caps(s)),
            _ => None,
        };
        let red = Self::red(window);
        let row = div()
            .id(("quick", i))
            .h(px(ROW))
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .when(on, |d| d.bg(if asking { red.opacity(0.14) } else { self.highlight(window) }))
            .child(
                div()
                    .relative()
                    .size(px(ICON))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon)
                    // (Running: a dot under it, as the Dock shows.)
                    .when(running, |d| d.child(div().absolute().bottom(px(-5.)).left(px(ICON / 2. - 1.5)).size(px(3.)).rounded_full().bg(palette.secondary))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    // (A clip's list is narrow: its title gives way, not the time.)
                    .child(if clip_row { div().flex_1().min_w_0().truncate().child(title) } else { div().flex_shrink_0().max_w(px(380.)).truncate().child(title) })
                    .when(!clip_row, |d| d.child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(if asking { red } else { palette.secondary }).child(subtitle))),
            )
            .children(keys.map(|k| k.flex_shrink_0()))
            // (The windows' kind: "Window", every one; the app's beside it.)
            .when(!windows_row, |d| d.child(div().flex_shrink_0().text_size(px(13.)).text_color(palette.secondary).child(ago.unwrap_or_else(|| kind.into()))))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.selected != i {
                    this.selected = i;
                    this.confirming = None;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| this.choose(i, e.modifiers().platform, window, cx)));
        // (Full width: the list sizes its lines to what's in them.)
        div().w_full().px_2().child(row).into_any_element()
    }

    /// A row of what's coming: half a question ("18% of …", and what's
    /// still to be typed), or an answer on its way (a shimmer where it'll
    /// be, its icon breathing).
    fn render_coming(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        let on = i == self.selected;
        let (icon, title, subtitle, kind): (AnyElement, AnyElement, SharedString, SharedString) = match &self.items[i] {
            Item::Unfinished { typed, wanted, kind, icon } => {
                let title = div()
                    .flex_shrink_0()
                    .max_w(px(380.))
                    .flex()
                    .items_baseline()
                    .child(div().truncate().child(typed.clone()))
                    .child(div().ml(px(5.)).text_color(placeholder_color(&palette)).child("…"));
                (Self::tile(icon.0, icon.1, window), title.into_any_element(), wanted.clone().into(), kind.clone().into())
            }
            Item::Waiting { asked, kind, icon, .. } => {
                let icon = div()
                    .child(Self::tile(icon.0, icon.1, window))
                    .with_animation(("breathing", i), Animation::new(std::time::Duration::from_millis(1600)).repeat().with_easing(pulsating_between(0.35, 0.9)), |el, t| el.opacity(t));
                // (A light passing over where the answer'll be.)
                let (width, band) = (120., 56.);
                let shine = palette.label.opacity(if dark { 0.16 } else { 0.11 });
                let light = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .w(px(band))
                    .flex()
                    .child(div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(shine.opacity(0.), 0.), linear_color_stop(shine, 1.))))
                    .child(div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(shine, 0.), linear_color_stop(shine.opacity(0.), 1.))))
                    .with_animation(("shine", i), Animation::new(std::time::Duration::from_millis(1300)).repeat().with_easing(ease_in_out), move |el, t| el.left(px(-band + (width + band) * t)));
                let title = div().relative().flex_shrink_0().w(px(width)).h(px(12.)).rounded_full().overflow_hidden().bg(palette.label.opacity(if dark { 0.08 } else { 0.06 })).child(light);
                (icon.into_any_element(), title.into_any_element(), asked.clone().into(), (*kind).into())
            }
            _ => return div().into_any_element(),
        };
        let waiting = matches!(self.items[i], Item::Waiting { .. });
        let row = div()
            .id(("quick", i))
            .h(px(ROW))
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .when(on, |d| d.bg(self.highlight(window)))
            .child(div().size(px(ICON)).flex_shrink_0().flex().items_center().justify_center().child(icon))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(title)
                    .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(palette.secondary).child(subtitle)),
            )
            .child(div().flex_shrink_0().text_size(px(13.)).text_color(palette.secondary).child(kind))
            // (Half a question's row isn't picked; a waiting one is, to be
            // its answer.)
            .when(waiting, |d| {
                d.on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if this.selected != i {
                        this.selected = i;
                        this.confirming = None;
                        cx.notify();
                    }
                }))
            });
        div().w_full().px_2().child(row).into_any_element()
    }

    /// A card: a translation, a word's meaning. (Its text at a set width:
    /// in a flex row, GPUI wraps text wider than the room it ends up with.)
    fn render_card(&mut self, i: usize, height: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let Some(Item::Card(card)) = self.items.get(i) else { return div().into_any_element() };
        let waiting = card.pending.is_some();
        // (The panel, less the list's and the row's sides, the icon and the gap.)
        let column = WIDTH - 16. - 20. - if card.image.is_some() { CARD_IMAGE } else { ICON } - 10.;
        // (Room on its first line for what it is, in the corner.)
        let corner = 90.;
        let under: Vec<AnyElement> = card
            .under_lines()
            .into_iter()
            // (Cut to its lines, and its box to them: GPUI's line_clamp cuts
            // by length, and the last line ran past the edge.)
            .map(|(line, n)| {
                let shown = cut(line, n, CARD_UNDER_PER);
                div().w_full().h(px(n as f32 * 18.)).overflow_hidden().text_size(px(13.)).line_height(px(18.)).text_color(palette.secondary).child(shown).into_any_element()
            })
            .collect();
        let first_line = !card.over.is_empty();
        let row = div()
            .id(("quick", i))
            .relative()
            .h(px(height - 4.))
            .px(px(10.))
            .py(px(10.))
            .rounded(px(8.))
            .flex()
            .items_start()
            .gap(px(10.))
            .overflow_hidden()
            .when(i == self.selected, |d| d.bg(self.highlight(window)))
            .child(match &card.image {
                Some(image) => img(image.clone()).size(px(CARD_IMAGE)).rounded(px(6.)).flex_shrink_0().into_any_element(),
                None => div().mt(px(if first_line { 19. } else { 1. })).child(Self::tile(card.icon.0, card.icon.1, window)).into_any_element(),
            })
            .child(
                div()
                    .w(px(column))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .when(first_line, |d| d.child(div().h(px(18.)).pr(px(corner)).text_size(px(12.)).text_color(palette.secondary).child(card.over.clone())))
                    .child(
                        div()
                            .w_full()
                            .when(!first_line, |d| d.pr(px(corner)))
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .max_h(px(card.text_lines() as f32 * 22.))
                                    .overflow_hidden()
                                    .text_size(px(16.))
                                    .line_height(px(22.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if waiting { palette.secondary } else { palette.label })
                                    .child(cut(&card.text, card.text_lines(), CARD_TEXT_PER)),
                            )
                            .when(!card.aside.is_empty(), |d| d.child(div().flex_shrink_0().text_size(px(13.)).text_color(palette.secondary).child(card.aside.clone()))),
                    )
                    .when(!under.is_empty(), |d| d.child(div().w_full().mt(px(6.)).flex().flex_col().children(under))),
            )
            .child(div().absolute().top(px(10.)).right(px(10.)).text_size(px(13.)).text_color(palette.secondary).child(card.kind))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.selected != i {
                    this.selected = i;
                    this.confirming = None;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| this.choose(i, e.modifiers().platform, window, cx)));
        div().w_full().h(px(height)).px_2().py(px(2.)).child(row).into_any_element()
    }

    /// A row of emoji, `from` to `to`.
    fn render_cells(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let highlight = self.highlight(window);
        // (A little short of the panel: its border, and wide emoji.)
        let width = (WIDTH - 24.) / COLUMNS as f32;
        let cells = (from..to).map(|i| {
            let text = match &self.items[i] {
                Item::Emoji(e) => e.text,
                _ => "",
            };
            div()
                .id(("cell", i))
                .w(px(width))
                .h(px(CELL - 4.))
                .rounded(px(10.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(30.))
                // (Emoji in Apple's font, whatever the panel's.)
                .font_family(".SystemUIFont")
                .when(i == self.selected, |d| d.bg(highlight))
                .child(text)
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if this.selected != i {
                        this.selected = i;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| this.choose(i, e.modifiers().platform, window, cx)))
        });
        div().w_full().h(px(CELL)).px_2().flex().items_center().children(cells).into_any_element()
    }

    /// The clip picked, beside the clipboard's list: all its text, the
    /// image, or the files, and where it's from and when.
    /// Whether the list has the one picked shown beside it: the clipboard's,
    /// the screenshots', the windows' (if it's windows it lists).
    fn previews(&self) -> bool {
        !self.items.is_empty()
            && match self.mode {
                Mode::Clipboard | Mode::Screenshots => true,
                Mode::Windows => self.items.iter().any(|i| matches!(i, Item::Command(Command { run: commands::Run::FocusWindow { .. }, .. }))),
                Mode::Extension => self.page_previews(),
                Mode::Search | Mode::Emoji => false,
            }
    }

    /// Switch Window: the window picked's picture taken, the first time it's
    /// picked (off the main thread).
    fn picture_picked(&mut self, cx: &mut Context<Self>) {
        if self.mode != Mode::Windows || self.acting.is_some() {
            return;
        }
        let Some(Item::Command(Command { run: commands::Run::FocusWindow { id, .. }, .. })) = self.items.get(self.selected) else { return };
        let id = *id;
        if self.pictures.contains_key(&id) || !self.picturing.insert(id) {
            return;
        }
        // (Numbered across panels: GPUI keeps images by their path, so a
        // path used before would show the old picture.)
        static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = TAKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cx.spawn(async move |this, cx| {
            let taken = cx.background_executor().spawn(async move { crate::windows::picture(id, n) }).await;
            let kept = this.update(cx, |this, cx| {
                this.picturing.remove(&id);
                this.pictures.insert(id, taken.clone());
                cx.notify();
            });
            // (The panel gone meanwhile: the picture too.)
            if kept.is_err()
                && let Some(file) = taken
            {
                let _ = std::fs::remove_file(file);
            }
        })
        .detach();
    }

    /// Switch Window's: the window picked as it looks (or, minimized, its
    /// app's icon), its app and its title.
    fn render_window_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let Some(Item::Command(command)) = self.items.get(self.selected) else { return div().flex_1().into_any_element() };
        let commands::Run::FocusWindow { id, .. } = command.run else { return div().flex_1().into_any_element() };
        let (title, subtitle) = (command.title.clone(), command.subtitle.clone());
        let app_path = match &command.icon {
            Icon::Of(path) => Some(path.clone()),
            Icon::Tile(..) | Icon::Image(_) => None,
        };
        let detail = |label: &'static str, value: String| {
            div().flex().justify_between().gap_3().text_size(px(12.)).child(div().text_color(palette.secondary).child(label)).child(div().truncate().child(value))
        };
        let (app, state) = subtitle.split_once(" · ").map_or((subtitle.clone(), None), |(a, s)| (a.to_string(), Some(s.to_string())));
        let icon_px = (96. * window.scale_factor()).round() as u32;
        let shown = match self.pictures.get(&id) {
            Some(Some(file)) => img(file.clone()).w_full().h_full().object_fit(ObjectFit::Contain).into_any_element(),
            // (None could be taken, or it's being taken: its app's icon.)
            _ => match app_path.and_then(Entry::from_path) {
                Some(e) => div().size_full().flex().items_center().justify_center().child(file_image(cx.global_mut::<PanelIcons>().0.get(&e, icon_px), &e, 96., palette.accent)).into_any_element(),
                None => div().into_any_element(),
            },
        };
        let mut rows = vec![("App", app), ("Window", title)];
        if let Some(state) = state {
            rows.push(("Now", state));
        }
        div()
            .flex_1()
            .min_w_0()
            .h(px(LIST + 8.))
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().p_3().overflow_hidden().child(shown))
            .child(div().px_3().py_2().border_t_1().border_color(palette.separator).flex().flex_col().gap_1().children(rows.into_iter().map(|(l, v)| detail(l, v))))
            .into_any_element()
    }

    fn render_preview(&self, _: &mut Window) -> AnyElement {
        use crate::clips::What;
        let palette = self.palette;
        let Some(Item::Clip(clip)) = self.items.get(self.selected) else { return div().flex_1().into_any_element() };
        let detail = |label: &'static str, value: String| {
            div().flex().justify_between().gap_3().text_size(px(12.)).child(div().text_color(palette.secondary).child(label)).child(div().truncate().child(value))
        };
        let (shown, about): (AnyElement, Vec<(&'static str, String)>) = match &clip.what {
            What::Text(text) => {
                let shown: String = text.chars().take(4000).collect();
                let words = text.split_whitespace().count();
                (
                    div().text_size(px(13.)).text_color(palette.label).child(shown).into_any_element(),
                    vec![("Characters", text.chars().count().to_string()), ("Words", words.to_string())],
                )
            }
            What::Link(url) => (div().text_size(px(14.)).text_color(palette.accent).child(url.clone()).into_any_element(), Vec::new()),
            What::Image { w, h, .. } => (
                match clip.image() {
                    Some(file) => img(file).w_full().h(px(LIST - 120.)).object_fit(ObjectFit::Contain).into_any_element(),
                    None => div().into_any_element(),
                },
                vec![("Size", format!("{w} × {h}"))],
            ),
            What::Files(paths) => (
                div().flex().flex_col().gap_1().text_size(px(12.)).children(paths.iter().take(30).map(|p| div().truncate().child(p.display().to_string()))).into_any_element(),
                vec![("Files", paths.len().to_string())],
            ),
        };
        let when = if self.mode == Mode::Screenshots { "Taken" } else { "Copied" };
        let mut rows = vec![("Type", clip.kind().to_string()), (when, crate::clips::ago(clip.at))];
        if let Some(app) = &clip.app {
            rows.insert(0, ("From", app.clone()));
        }
        rows.extend(about);
        div()
            .flex_1()
            .min_w_0()
            .h(px(LIST + 8.))
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().p_3().overflow_hidden().child(shown))
            .child(div().px_3().py_2().border_t_1().border_color(palette.separator).flex().flex_col().gap_1().children(rows.into_iter().map(|(l, v)| detail(l, v))))
            .into_any_element()
    }

    /// The footer: Finder2's icon, then what Return does and what else
    /// there is, with their keys, as Raycast's.
    fn render_footer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let icon_px = (16. * window.scale_factor()).round() as u32;
        let (first, then): ((SharedString, &str), Option<(&str, &str)>) = match (self.acting, self.confirming.is_some()) {
            (Some(at), _) => {
                let title = self.actions(cx).get(at).map(|(_, title, _)| title.clone()).unwrap_or_default();
                ((title.into(), "↩"), Some(("Back", "esc")))
            }
            (None, true) => (("Confirm".into(), "↩"), Some(("Cancel", "esc"))),
            (None, false) => {
                let some = if self.mode == Mode::Extension { !self.actions(cx).is_empty() } else { !self.items.is_empty() };
                ((self.primary(cx), "↩"), some.then_some(("Actions", "⌘K")))
            }
        };
        let me = self.me.clone().map(|me| div().opacity(0.85).child(file_image(cx.global_mut::<PanelIcons>().0.get(&me, icon_px), &me, 16., palette.secondary)));
        // (In the emoji's grid: the name of the one picked.)
        let named = match self.items.get(self.selected) {
            Some(Item::Emoji(e)) if self.acting.is_none() => Some(div().text_color(palette.secondary).child(capitalized(e.name))),
            _ => None,
        };
        // (An extension's page: what it says it's doing, while it is.)
        // (Not while it's what's said in place of the list.)
        let shown = self.acting.is_none() && (!self.items.is_empty() || self.reading());
        let named = named.or_else(|| self.page_loading().and_then(|l| l.0).filter(|_| shown).map(|what| div().min_w_0().truncate().text_color(palette.secondary).child(what)));
        let me = if named.is_some() { None } else { me };
        div()
            .h(px(FOOTER))
            .flex_shrink_0()
            .pl(px(18.))
            .pr(px(10.))
            .flex()
            .items_center()
            .gap_2()
            .border_t_1()
            .border_color(palette.separator)
            .text_size(px(12.))
            .children(me)
            .children(named)
            .child(div().flex_1())
            .when(!first.0.is_empty(), |d| d.child(div().font_weight(FontWeight::MEDIUM).text_color(palette.label).child(first.0)).child(self.caps(first.1)))
            .when_some(then, |d, (title, keys)| {
                d.child(div().mx_1().w(px(1.)).h(px(14.)).bg(palette.separator))
                    .child(div().text_color(palette.secondary).child(title))
                    .child(self.caps(keys))
            })
            .into_any_element()
    }
}

impl Drop for QuickPanel {
    fn drop(&mut self) {
        for file in self.pictures.values().flatten() {
            let _ = std::fs::remove_file(file);
        }
    }
}

impl Render for QuickPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.picture_picked(cx);
        // (Switch Window's keys let go before the windows were listed: now
        // they are, it switches.)
        if self.switching == Switching::LetGo && !self.items.is_empty() {
            self.switching = Switching::No;
            cx.defer_in(window, |this, window, cx| this.choose(this.selected, false, window, cx));
        }
        let palette = self.palette;
        let note = self.note(cx);
        // Once shown (`appear`), sized to what it shows. (After this frame,
        // in a task, not `defer`: that can still run while the window is
        // being drawn, when GPUI can't take in its new size.)
        let height = self.fitting(note.is_some(), cx);
        if self.native && (height - self.height).abs() > 0.5 {
            self.height = height;
            let view = self.view;
            cx.spawn(async move |_, _| {
                macos::panel_fit(view, height as f64);
            })
            .detach();
        }
        let body = if self.acting.is_some() {
            div().py_1().flex().flex_col().children(self.render_actions(window, cx)).into_any_element()
        } else if self.reading() {
            self.render_reading(window)
        } else if self.previews() {
            // The list, and beside it the one picked, all of it.
            let lines = list(self.list.clone(), cx.processor(|this, ix, window, cx| this.render_line(ix, window, cx)));
            div()
                .flex()
                .child(div().w(px(CLIPS_WIDTH)).flex_shrink_0().py_1().child(lines.w_full().h(px(LIST))))
                .child(div().w(px(1.)).h(px(LIST + 8.)).bg(palette.separator))
                .child(match self.mode {
                    Mode::Windows => self.render_window_preview(window, cx),
                    Mode::Extension => self.render_ext_preview(window),
                    _ => self.render_preview(window),
                })
                .into_any_element()
        } else {
            let lines = list(self.list.clone(), cx.processor(|this, ix, window, cx| this.render_line(ix, window, cx)));
            let height = if matches!(self.mode, Mode::Search | Mode::Windows | Mode::Extension) { self.list_height() } else { LIST.min(self.lines.iter().map(Line::height).sum()) };
            div().py_1().child(lines.w_full().h(px(height))).into_any_element()
        };
        let footer = self.render_footer(window, cx);
        let (family, text_size) = font(Settings::get(cx));
        // (gpui-kit's input sets its lines 20 px tall, whatever the size:
        // bigger text lost its descenders.)
        let line = (text_size * 1.3).round();
        let empty = self.input.read(cx).value().is_empty();
        let ghost = self.ghost(window, cx);
        // (Something on its way from the internet: a light along the
        // field's edge, as long as it takes.)
        let loading = self.acting.is_none() && self.items.iter().any(|item| matches!(item, Item::Waiting { .. } | Item::Card(Card { pending: Some(_), .. })));
        // (An extension's page working on it: the light, or how far along.)
        let page_loading = self.page_loading();
        // (None yet, at 0: the light, till it's under way.)
        let progress = page_loading.as_ref().and_then(|l| l.1).map(|p| p.clamp(0., 1.)).filter(|p| *p > 0.);
        let loading = loading || (page_loading.is_some() && progress.is_none());
        let placeholder: SharedString = match self.mode {
            Mode::Search => PLACEHOLDER.into(),
            Mode::Clipboard => "Search what's been copied…".into(),
            Mode::Emoji => "Search emoji and symbols…".into(),
            Mode::Screenshots => "Search screenshots by date…".into(),
            Mode::Windows => "Search windows…".into(),
            Mode::Extension => self.page_placeholder().unwrap_or_default().into(),
        };
        // The material behind (blurred, tinted as the system tints popovers),
        // under the sidebar's color, enough of it that a white window behind
        // doesn't wash a dark panel out to gray (as Raycast keeps its dark).
        let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        let card = palette.sidebar.opacity(if dark { 0.68 } else { 0.5 });
        div()
            .size_full()
            // Switch Window's keys let go: the window picked, as ⌘Tab's.
            .on_modifiers_changed(cx.listener(|this, e: &ModifiersChangedEvent, window, cx| {
                if this.switching == Switching::Held && !e.modifiers.modified() {
                    if this.items.is_empty() {
                        this.switching = Switching::LetGo;
                    } else {
                        this.switching = Switching::No;
                        this.choose(this.selected, false, window, cx);
                    }
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                this.step(1, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                this.step(-1, cx);
                cx.stop_propagation();
            }))
            // (⌘, on an extension's page: its settings, not F2's. A key
            // bound in the app's keymap is an action before it's a key.)
            .capture_action(cx.listener(|this, _: &crate::finder::OpenSettings, window, cx| {
                if this.mode == Mode::Extension && this.page_has_settings() {
                    this.perform(Act::ExtSettings, window, cx);
                    cx.stop_propagation();
                }
            }))
            // (Tab is the field's indent, there: taken before it's a key.)
            .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                this.take_hint(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                // (Out of ⌘K's list, or the question, or the clipboard's or
                // emoji's list, first.)
                if this.acting.take().is_some() || this.confirming.take().is_some() {
                    cx.notify();
                } else if this.mode == Mode::Extension {
                    this.pop_page(window, cx);
                } else if this.mode != Mode::Search {
                    this.set_mode(Mode::Search, window, cx);
                } else {
                    this.close(window, cx);
                }
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if this.shortcut(&e.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(px(RADIUS))
                    .bg(card)
                    .border_1()
                    .border_color(palette.glass_border)
                    .text_color(palette.label)
                    .text_size(px(14.))
                    .font_family(family)
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(field_height(cx)))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .relative()
                            .px(px(18.))
                            .border_b_1()
                            .border_color(palette.separator)
                            .when(loading, |d| d.child(loading_light(palette.accent)))
                            .when_some(progress, |d, p| d.child(div().absolute().left_0().bottom(px(-1.)).h(px(2.)).w(relative(p)).bg(palette.accent)))
                            // (Under the field, which is clear: clicks go to it.)
                            .when(empty, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .left(px(18. + if self.mode == Mode::Search { 0. } else { BACK + BACK_GAP }))
                                        // (Clear of the list's name, on the right.)
                                        .when(self.mode != Mode::Search, |d| d.right(px(270.)).overflow_hidden().whitespace_nowrap())
                                        .top_0()
                                        .bottom_0()
                                        .flex()
                                        .items_center()
                                        .text_size(px(text_size))
                                        .line_height(px(line))
                                        .text_color(placeholder_color(&palette))
                                        .child(placeholder),
                                )
                            })
                            // The hint, after what's typed: the two as one line,
                            // what's typed clear (the field shows it), so the
                            // hint starts where it ends.
                            .when_some(ghost.clone(), |d, hint| {
                                let typed = self.input.read(cx).value().to_string();
                                let clear = HighlightStyle { color: Some(transparent_black()), ..Default::default() };
                                d.child(
                                    div()
                                        .absolute()
                                        .left(px(18.))
                                        .top_0()
                                        .bottom_0()
                                        .flex()
                                        .items_center()
                                        .text_size(px(text_size))
                                        .line_height(px(line))
                                        .text_color(placeholder_color(&palette))
                                        .child(StyledText::new(format!("{typed}{hint}")).with_highlights([(0..typed.len(), clear)])),
                                )
                            })
                            // Back to the search, as esc goes.
                            .when(self.mode != Mode::Search, |d| {
                                d.child(
                                    div()
                                        .id("back")
                                        .size(px(BACK))
                                        .mr(px(BACK_GAP))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(7.))
                                        .bg(palette.glass_selected)
                                        .hover(|s| s.opacity(0.75))
                                        .active(|s| s.opacity(0.55))
                                        .role(Role::Button)
                                        .aria_label("Back")
                                        .child(symbol("arrow.left", 13., SEMIBOLD, palette.secondary, window))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            if this.mode == Mode::Extension {
                                                this.pop_page(window, cx);
                                            } else {
                                                this.set_mode(Mode::Search, window, cx);
                                            }
                                        })),
                                )
                            })
                            .child(
                                div().flex_1().child(
                                    Input::new(&self.input)
                                        .appearance(false)
                                        .font_family(family)
                                        .text_size(px(text_size))
                                        .line_height(px(line))
                                        .px_0()
                                        .py_0()
                                        .h(px(line + 8.)),
                                ),
                            )
                            .when(ghost.is_some(), |d| d.child(self.caps("tab")))
                            // Which list it is, and the way back.
                            .when(self.mode != Mode::Search, |d| {
                                let name: SharedString = match self.mode {
                                    Mode::Clipboard => "Clipboard History".into(),
                                    Mode::Screenshots => "Screenshots".into(),
                                    Mode::Windows => "Windows".into(),
                                    Mode::Extension => self.page_title().unwrap_or_default().into(),
                                    _ => "Emoji & Symbols".into(),
                                };
                                d.child(div().flex().items_center().gap_2().text_size(px(12.)).text_color(palette.secondary).child(div().max_w(px(200.)).truncate().child(name)).child(self.caps("esc")))
                            }),
                    )
                    .child(body)
                    .when_some(note, |d, note| d.child(div().h(px(NOTE)).px(px(18.)).flex().items_center().text_color(palette.secondary).child(note)))
                    .child(div().flex_1())
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, completion, cut, hint};
    use crate::commands::{self, Command, Icon};
    use crate::listing::Entry;

    #[test]
    fn completions() {
        let app = Item::App(Entry::from_path("/System/Applications/Calculator.app".into()).unwrap());
        assert_eq!(completion(&app, "calc").as_deref(), Some("ulator"));
        assert_eq!(completion(&app, "c"), None, "a letter's too little");
        let command = |title: &str, kind, run| Item::Command(Command { title: title.into(), subtitle: String::new(), kind, icon: Icon::Tile("x", 0), run, confirm: None, key: None });
        assert_eq!(completion(&command("Quit Safari", "Command", commands::Run::Lock), "quit sa").as_deref(), Some("fari"));
        assert_eq!(completion(&command("Translate", "Command", commands::Run::Fill("translate ".into())), "trans").as_deref(), Some("late "));
        assert_eq!(completion(&command("Search the web for “se”", "Web Search", commands::Run::Lock), "se"), None);
        let folder = Item::File(Entry::from_path("/Applications".into()).unwrap());
        assert_eq!(completion(&folder, "/Appl").as_deref(), Some("ications/"));
        assert_eq!(completion(&folder, "/Applications").as_deref(), Some("/"), "into it");
        assert_eq!(completion(&folder, "Appl"), None, "a folder, by its path only");
    }

    /// Korean, Chinese and Japanese are cut at half as many characters:
    /// they're twice as wide.
    #[test]
    fn cuts() {
        assert_eq!(cut("short", 1, 10), "short");
        assert_eq!(cut("one two three four", 1, 10), "one two…");
        assert_eq!(cut("하늘은 왜 파래요", 1, 12), "하늘은 왜…");
        assert_eq!(cut("空がなぜ青いのか", 1, 10), "空がなぜ…");
    }

    #[test]
    fn hints() {
        assert_eq!(hint("time").as_deref(), Some(" in tokyo"));
        assert_eq!(hint("Time i").as_deref(), Some("n tokyo"));
        assert_eq!(hint("time in reyk").as_deref(), Some("javík"));
        assert_eq!(hint("time in jap").as_deref(), Some("an"));
        assert_eq!(hint("translate hi to jap").as_deref(), Some("anese"));
        assert_eq!(hint("remind").as_deref(), Some(" me to call mom tomorrow at 5pm"));
        assert_eq!(hint("tim"), None, "not till the word's typed");
        assert_eq!(hint("what"), None, "WhatsApp, say");
        assert_eq!(hint("time in tokyo"), None, "nothing left to show");
        assert_eq!(hint("5 km").as_deref(), Some(" in mi"));
        assert_eq!(hint("30 c").as_deref(), Some(" to f"));
        assert_eq!(hint("5 km i").as_deref(), Some("n mi"));
        assert_eq!(hint("2 gb in ").as_deref(), Some("mb"));
        assert_eq!(hint("5 kg in po").as_deref(), Some("und"));
        assert_eq!(hint("4k"), None, "a screen's, not kelvin");
        assert_eq!(hint("100 usd").as_deref(), Some(" in eur"));
        assert_eq!(hint("€20 i").as_deref(), Some("n usd"));
        assert_eq!(hint("100 usd in k").as_deref(), Some("rw"));
        assert_eq!(hint("3pm lon").as_deref(), Some("don"));
        assert_eq!(hint("3pm in tok").as_deref(), Some("yo"));
        assert!(hint("3pm").is_some_and(|h| h.starts_with(" in ")));
        assert!(hint("3pm london").is_some_and(|h| h.starts_with(" in ")));
        assert_eq!(hint("remind me to call mom ").as_deref(), Some("tomorrow at 9am"));
        assert_eq!(hint("remind me to call mom tomorrow ").as_deref(), Some("at 9am"));
        assert_eq!(hint("translate hello ").as_deref(), Some("to spanish"));
        assert_eq!(hint("translate hello to ").as_deref(), Some("spanish"));
        assert_eq!(hint("photos from").as_deref(), Some(" last week"));
        assert_eq!(hint("1password"), None);
        assert_eq!(hint("timer").as_deref(), Some(" 10 min"));
        assert_eq!(hint("keep awake").as_deref(), Some(" for 1 hour"));
        assert_eq!(hint("time").as_deref(), Some(" in tokyo"), "not the timer's");
    }
}

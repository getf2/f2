//! Shortcuts you give what Quick Open does, from any app: a command (Lock
//! Screen, Toggle Dark Mode, Translate…), an app or a folder, a quicklink,
//! a snippet. Settings keep them (`shortcuts`) by what they're for: a
//! command's key (`commands::Command::key`), "open:<path>",
//! "quicklink:<name>", "snippet:<name>". (Quick Open's own, its
//! clipboard's, emoji's and selection's, the screenshots' and the windows'
//! are kept where they were; Settings' Keyboard shows them all.)

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gpui::{App, Global};

use crate::commands::{self, Command, Icon, Run};
use crate::settings::Settings;

/// Their hot keys' ids, from this on (see `hotkeys`), and how many there
/// can be.
pub const FIRST: u32 = 128;
const MOST: usize = 384;

/// What each id from `FIRST` is for, as last set.
static BOUND: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Those another app has: by what they're for.
#[derive(Default)]
pub struct Taken(pub HashSet<String>);

impl Global for Taken {}

/// Take the shortcuts Settings have (again when they change).
pub fn init(cx: &mut App) {
    let mut registered: Option<BTreeMap<String, String>> = None;
    let mut register = move |cx: &mut App| {
        let wanted = Settings::get(cx).shortcuts.clone();
        if registered.as_ref() == Some(&wanted) {
            return;
        }
        let mut bound = BOUND.lock().unwrap();
        for i in 0..bound.len() {
            crate::hotkeys::set(FIRST + i as u32, None);
        }
        bound.clear();
        let mut taken = HashSet::new();
        for (what, shortcut) in wanted.iter().take(MOST) {
            if !crate::hotkeys::set(FIRST + bound.len() as u32, Some(shortcut)) {
                eprintln!("{what}'s shortcut {shortcut} couldn't be had");
                taken.insert(what.clone());
            }
            bound.push(what.clone());
        }
        drop(bound);
        cx.set_global(Taken(taken));
        // (Switch Window's: the windows listed once now, so the first time
        // it's pressed isn't the slow one.)
        if wanted.contains_key("windows") {
            crate::quick_panel::warm_windows();
        }
        registered = Some(wanted);
    };
    register(cx);
    cx.observe_global::<Settings>(register).detach();
}

/// A shortcut was pressed (see `hotkeys`): whether it's one of these.
pub fn pressed(id: u32, cx: &mut App) -> bool {
    let what = id.checked_sub(FIRST).and_then(|i| BOUND.lock().unwrap().get(i as usize).cloned());
    match what {
        Some(what) => {
            run(&what, cx);
            true
        }
        None => false,
    }
}

/// Do what a shortcut's for.
pub fn run(what: &str, cx: &mut App) {
    let settings = Settings::get(cx);
    if let Some(path) = what.strip_prefix("open:") {
        open(Path::new(path), cx);
    } else if let Some(name) = what.strip_prefix("quicklink:") {
        let Some(link) = settings.quicklinks.iter().find(|l| l.name == name).cloned() else { return };
        if link.link.contains("{query}") || link.link.contains("{argument}") {
            // (It wants what to look for: Quick Open, its keyword typed.)
            let keyword = link.keyword.trim();
            crate::quick_panel::show_query(if keyword.is_empty() { link.name } else { format!("{keyword} ") }, cx);
        } else {
            commands::run(Run::Quicklink(link.link), cx);
        }
    } else if let Some(name) = what.strip_prefix("snippet:") {
        if let Some(snippet) = settings.snippets.iter().find(|s| s.name == name) {
            let text = snippet.text.clone();
            crate::quick_panel::paste_snippet(&text, cx);
        }
    } else if let Some(mut command) = commands::by_key(what) {
        // (As things are: Keep Awake stops it, once it's on.)
        commands::live(&mut command);
        if command.confirm.is_some() {
            return crate::quick_panel::show_asking(command, cx);
        }
        match command.run {
            // (It wants what's typed after: Quick Open, with its words.)
            Run::Fill(text) => crate::quick_panel::show_query(text, cx),
            Run::UseSelection => crate::quick_panel::show_selected(cx),
            Run::PastePlain => crate::quick_panel::paste_plain(cx),
            run => _ = commands::run(run, cx),
        }
    }
}

/// An app: opened, or hidden if it's in front already (pressed again, it
/// goes). A folder: in a window.
fn open(path: &Path, cx: &mut App) {
    if path.extension().is_some_and(|e| e == "app")
        && let Some((_, front)) = crate::macos::front_app()
        && front == path
    {
        let pid = crate::macos::front_pid();
        if pid > 0 && pid != std::process::id() as i32 && crate::macos::hide_app(pid) {
            return;
        }
    }
    commands::run(Run::Quicklink(path.to_string_lossy().into_owned()), cx);
}

/// One line in Settings' list: what it's for (Settings' key, or one of
/// the shortcuts kept elsewhere: "quick-open", "clipboard", "shot:Area"…),
/// its name, what it does, its icon.
pub struct Line {
    pub what: String,
    pub title: String,
    pub about: String,
    pub icon: Icon,
}

/// Its lines, in sections, as Settings have them: Quick Open's, the apps
/// and folders given shortcuts (and `adding`, to give one), quicklinks,
/// snippets, then every command.
pub fn lines(settings: &Settings, adding: &[PathBuf]) -> Vec<(&'static str, Vec<Line>)> {
    let line = |what: String, title: String, about: String, icon: Icon| Line { what, title, about, icon };
    let (commands, panes) = commands::every();
    let mut sections: Vec<(&'static str, Vec<Line>)> = vec![
        ("Quick Open", vec![line("quick-open".into(), "Quick Open".into(), "Find apps, files and answers".into(), Icon::Tile("magnifyingglass", commands::FINDER2))]),
        ("Apps & Folders", Vec::new()),
        ("Quicklinks", Vec::new()),
        ("Snippets", Vec::new()),
        ("System", Vec::new()),
        ("Music & Sound", Vec::new()),
        ("Screenshots", Vec::new()),
        ("Windows", Vec::new()),
        ("Look Up & Ask", Vec::new()),
        (crate::APP_NAME, Vec::new()),
        ("Developer", Vec::new()),
        ("System Settings", Vec::new()),
    ];
    let mut add = |section: &str, l: Line| {
        if let Some((_, lines)) = sections.iter_mut().find(|(s, _)| *s == section) {
            lines.push(l);
        }
    };
    // (The apps and folders with shortcuts, and any being given one.)
    let mut paths: Vec<PathBuf> = settings.shortcuts.keys().filter_map(|k| k.strip_prefix("open:")).map(PathBuf::from).collect();
    paths.extend(adding.iter().filter(|p| !paths.contains(p)).cloned().collect::<Vec<_>>());
    paths.sort_by_key(|p| name(p).to_lowercase());
    for path in paths {
        let about = crate::finder::items::location(&path).to_string();
        add("Apps & Folders", line(format!("open:{}", path.display()), name(&path), about, Icon::Of(path)));
    }
    for l in &settings.quicklinks {
        let icon = if l.link.contains("://") { Icon::Tile("link", commands::QUICKLINKS) } else { Icon::Of(crate::finder::expand(&l.link)) };
        add("Quicklinks", line(format!("quicklink:{}", l.name), l.name.clone(), l.link.clone(), icon));
    }
    for s in &settings.snippets {
        let first: String = s.text.lines().next().unwrap_or("").chars().take(60).collect();
        add("Snippets", line(format!("snippet:{}", s.name), s.name.clone(), first, Icon::Tile("text.quote", commands::SNIPPETS)));
    }
    for c in commands.into_iter().chain(panes) {
        let Some(key) = c.key.clone() else { continue };
        // (Not what the section says already: "System", "F2".)
        let about = match &c.run {
            Run::Fill(typed) => format!("Quick Open, with “{}” typed", typed.trim_end()),
            _ if ["System", "Window", "Screenshot", "Screen Recording", "Developer", "Music", "Sound", crate::APP_NAME].contains(&c.subtitle.as_str()) => String::new(),
            _ => c.subtitle.clone(),
        };
        add(section(&key, &c), line(key, c.title, about, c.icon));
    }
    sections.retain(|(name, lines)| !lines.is_empty() || *name == "Apps & Folders");
    sections
}

/// The section a command's in.
fn section(key: &str, c: &Command) -> &'static str {
    let prefixed = |prefixes: &[&str]| prefixes.iter().any(|p| key.starts_with(p));
    if ["clipboard", "emoji", "screenshots", "selection", "fix-spelling", "paste-plain", "clear-clipboard"].contains(&key) {
        "Quick Open"
    } else if prefixed(&["shot:"]) {
        "Screenshots"
    } else if prefixed(&["window:"]) || key == "windows" {
        "Windows"
    } else if prefixed(&["dev:"]) {
        "Developer"
    } else if prefixed(&["settings:"]) {
        "System Settings"
    } else if prefixed(&["media:", "music:", "sound:", "screen:", "ask:now-playing", "ask:play", "ask:output"]) {
        "Music & Sound"
    } else if matches!(c.run, Run::Fill(_)) {
        "Look Up & Ask"
    } else if c.subtitle == "System" || prefixed(&["toggle:"]) || ["clean-keyboard", "awake", "speed-test", "pomodoro"].contains(&key) {
        "System"
    } else {
        crate::APP_NAME
    }
}

/// An app's or a folder's name, as Finder shows it ("Safari", not
/// "Safari.app").
fn name(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
    name.strip_suffix(".app").map(str::to_string).unwrap_or(name)
}

/// Settings' shortcuts for a quicklink or a snippet follow it as it's
/// renamed (`kind`: "quicklink", "snippet"), and go with it (`to` None).
pub fn renamed(settings: &mut Settings, kind: &str, from: &str, to: Option<&str>) {
    if let Some(shortcut) = settings.shortcuts.remove(&format!("{kind}:{from}"))
        && let Some(to) = to
    {
        settings.shortcuts.insert(format!("{kind}:{to}"), shortcut);
    }
}

//! Uninstall App: an app and what it leaves in your Library when it's only
//! dragged to the Trash (its support files, caches, preferences, container,
//! saved state, logs, a login agent), found by its bundle identifier and
//! name, each with its size and ticked; all to the Trash in one go (Put
//! Back works). Not Apple's own apps, nor F2, nor an app that's running.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};

use futures::StreamExt;
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::icons::Icons;
use crate::listing::{Entry, human_size};
use crate::macos;
use crate::style::{Palette, SEMIBOLD, glass, symbol};

/// Something the app left, and what it takes.
#[derive(Clone, Debug)]
pub struct Leftover {
    pub path: PathBuf,
    pub bytes: u64,
}

/// An app's bundle identifier and name, from its Info.plist.
fn identity(app: &Path) -> Option<(String, String)> {
    let out = std::process::Command::new("/usr/bin/plutil").args(["-convert", "json", "-o", "-"]).arg(app.join("Contents/Info.plist")).output().ok()?;
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let id = info["CFBundleIdentifier"].as_str()?.to_string();
    let name = info["CFBundleName"].as_str().map(str::to_string).unwrap_or_else(|| app.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    Some((id, name))
}

/// Why `app` can't be uninstalled here, if it can't.
pub fn refused(app: &Path) -> Option<String> {
    let Some((id, name)) = identity(app) else { return Some("That isn't an app F2 can read.".into()) };
    if id.starts_with("com.apple.") || app.starts_with("/System") {
        return Some(format!("{name} comes with macOS: it can't be uninstalled."));
    }
    if id == crate::takeover::BUNDLE_ID || id.starts_with("dev.newfinder.") {
        return Some(format!("{} can't uninstall itself: drag it to the Trash.", crate::APP_NAME));
    }
    None
}

fn bytes_of(path: &Path) -> u64 {
    let Ok(meta) = path.symlink_metadata() else { return 0 };
    if meta.is_dir() { crate::usage::walk(path, &AtomicU64::new(0), &AtomicBool::new(false)).map_or(0, |n| n.bytes) } else { crate::usage::on_disk_of(&meta) }
}

/// What `app` leaves in `home`'s Library (yours; a test's own folder), the
/// biggest first.
pub fn leftovers(app: &Path, home: &Path) -> Vec<Leftover> {
    let Some((id, name)) = identity(app) else { return Vec::new() };
    let lib = home.join("Library");
    let mut found: Vec<PathBuf> = Vec::new();
    // Named for the app: its bundle identifier anywhere, its name only
    // where apps file things under it (and not a short, common one).
    let by_name = name.chars().count() >= 4;
    let named = [
        ("Application Support", true),
        ("Caches", true),
        ("Containers", false),
        ("Saved Application State", false),
        ("HTTPStorages", false),
        ("WebKit", false),
        ("Logs", true),
        ("Application Scripts", false),
        ("Cookies", false),
    ];
    for (dir, names_too) in named {
        found.push(lib.join(dir).join(&id));
        found.push(lib.join(dir).join(format!("{id}.savedState")));
        found.push(lib.join(dir).join(format!("{id}.binarycookies")));
        if names_too && by_name {
            found.push(lib.join(dir).join(&name));
        }
    }
    // Preferences and login agents: "<id>.plist" and "<id>.something.plist".
    for dir in ["Preferences", "LaunchAgents"] {
        for entry in std::fs::read_dir(lib.join(dir)).into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file == format!("{id}.plist") || (file.starts_with(&format!("{id}.")) && file.ends_with(".plist")) {
                found.push(entry.path());
            }
        }
    }
    // Shared with its extensions: "<team>.<id>" (and the like).
    for entry in std::fs::read_dir(lib.join("Group Containers")).into_iter().flatten().flatten() {
        let dir = entry.file_name().to_string_lossy().into_owned();
        if dir.ends_with(&format!(".{id}")) || dir == id {
            found.push(entry.path());
        }
    }
    let mut seen = HashSet::new();
    let mut leftovers: Vec<Leftover> =
        found.into_iter().filter(|p| p.symlink_metadata().is_ok() && seen.insert(p.clone())).map(|path| Leftover { bytes: bytes_of(&path), path }).collect();
    leftovers.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    leftovers
}

/// Your home (or, in a test copy with NEW_FINDER_SETTINGS, F2_CLEANUP_ROOT:
/// a folder of its own).
fn home() -> PathBuf {
    match std::env::var_os("F2_CLEANUP_ROOT").filter(|_| std::env::var_os("NEW_FINDER_SETTINGS").is_some()) {
        Some(root) => PathBuf::from(root),
        None => std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from),
    }
}

/// Uninstall `app`: its window (or why it can't be).
pub fn open(app: PathBuf, cx: &mut App) {
    if let Some(why) = refused(&app) {
        crate::shots::note("Can't Uninstall".into(), why, None, cx);
        return;
    }
    let bounds = Bounds::centered(None, size(px(460.), px(500.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some("Uninstall".into()), ..Default::default() }),
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    let _ = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Uninstaller::new(app, window, cx)));
    cx.activate(true);
}

struct Uninstaller {
    focus: FocusHandle,
    palette: Palette,
    app: Entry,
    app_bytes: Option<u64>,
    leftovers: Option<Vec<Leftover>>,
    ticked: HashSet<PathBuf>,
    /// Why it couldn't, or that it's running.
    said: Option<SharedString>,
    /// Going to the Trash (Finder maybe asking for a password meanwhile).
    busy: bool,
    icons: Icons,
    _task: Task<()>,
}

impl Uninstaller {
    fn new(app: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (icons, mut finished) = Icons::new();
        cx.spawn(async move |this, cx| {
            while let Some(done) = finished.next().await {
                let alive = this.update(cx, |this, cx| {
                    for evicted in this.icons.finish(done) {
                        cx.drop_image(evicted, None);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let task = cx.spawn({
            let app = app.clone();
            async move |this, cx| {
                let found = cx.background_executor().spawn(async move { (bytes_of(&app), leftovers(&app, &home())) }).await;
                this.update(cx, |this, cx| {
                    this.app_bytes = Some(found.0);
                    this.ticked = found.1.iter().map(|l| l.path.clone()).collect();
                    this.leftovers = Some(found.1);
                    cx.notify();
                })
                .ok();
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Uninstaller {
            focus,
            palette: Palette::for_window(window),
            app: Entry::from_path(app.clone()).unwrap_or_else(|| Entry { name: app.file_name().unwrap_or_default().to_string_lossy().into_owned().into(), path: app, is_dir: true, ..Default::default() }),
            app_bytes: None,
            leftovers: None,
            ticked: HashSet::new(),
            said: None,
            busy: false,
            icons,
            _task: task,
        }
    }

    fn name(&self) -> String {
        self.app.name.trim_end_matches(".app").to_string()
    }

    /// The app (and what's ticked) to the Trash, off the main thread; the
    /// window closes then. An app that isn't yours to move (an
    /// administrator installed it) goes with what it leaves through Finder,
    /// which asks for an administrator's password or Touch ID once.
    fn uninstall(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let app = self.app.path.clone();
        if let Some((pid, ..)) = macos::running_apps().into_iter().find(|(_, _, p)| *p == app) {
            // (Quit first: a second click while it says so.)
            if self.said.is_none() {
                self.said = Some(format!("{} is open: Uninstall quits it first.", self.name()).into());
                return cx.notify();
            }
            macos::quit_app(pid, false);
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
        let mut paths = vec![app];
        paths.extend(self.leftovers.iter().flatten().filter(|l| self.ticked.contains(&l.path)).map(|l| l.path.clone()));
        let freed = self.app_bytes.unwrap_or(0) + self.leftovers.iter().flatten().filter(|l| self.ticked.contains(&l.path)).map(|l| l.bytes).sum::<u64>();
        let name = self.name();
        self.busy = true;
        self.said = Some(format!("Moving {name} to the {}…", macos::bin()).into());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move {
                    // (The app first: what it leaves only goes once it has.)
                    let results = match macos::try_trash(&paths[0]) {
                        Ok(to) => std::iter::once(Ok(to)).chain(macos::trash_all(&paths[1..])).collect(),
                        Err((_, true)) => macos::finder_trash(&paths),
                        Err((e, false)) => vec![Err(e)],
                    };
                    paths.into_iter().zip(results).collect::<Vec<_>>()
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                let mut trashed = Vec::new();
                for (i, (path, result)) in done.into_iter().enumerate() {
                    match result {
                        Ok(to) => trashed.push((path, to)),
                        Err(e) if i == 0 => {
                            this.said = Some(format!("{name} couldn't be moved to the {}: {e}", macos::bin()).into());
                            return cx.notify();
                        }
                        Err(_) => {}
                    }
                }
                crate::finder::remember_trashed(&trashed);
                crate::shots::note(format!("Uninstalled {name}"), format!("{} moved to the {}, {} freed", crate::listing::count(trashed.len(), "item"), macos::bin(), human_size(freed)), None, cx);
                window.remove_window();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Uninstaller {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let icon = self.icons.get(&self.app, (56. * window.scale_factor()).round() as u32);
        let total = self.app_bytes.map(|app| app + self.leftovers.iter().flatten().filter(|l| self.ticked.contains(&l.path)).map(|l| l.bytes).sum::<u64>());
        let lib = home().join("Library");
        let rows: Vec<AnyElement> = match &self.leftovers {
            None => vec![div().p_3().text_color(palette.secondary).child("Looking for what it leaves…").into_any_element()],
            Some(found) if found.is_empty() => vec![div().p_3().text_color(palette.secondary).child("It leaves nothing else in your Library.").into_any_element()],
            Some(found) => found
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let on = self.ticked.contains(&l.path);
                    let path = l.path.clone();
                    let place = l.path.strip_prefix(&lib).map_or_else(|_| l.path.display().to_string(), |p| format!("Library/{}", p.display()));
                    div()
                        .id(("leftover", i))
                        .w_full()
                        .px_2()
                        .py(px(6.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .hover(|d| d.bg(palette.glass_selected))
                        .child(
                            div()
                                .size(px(16.))
                                .flex_shrink_0()
                                .rounded(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 10., SEMIBOLD, white(), window)))
                                .when(!on, |d| d.border_1().border_color(palette.tertiary)),
                        )
                        .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).child(place))
                        .child(div().flex_shrink_0().text_size(px(12.)).text_color(palette.secondary).child(human_size(l.bytes)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.ticked.remove(&path) {
                                this.ticked.insert(path.clone());
                            }
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect(),
        };
        div()
            .key_context("Utility")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.))
            .child(
                div()
                    .p_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(crate::finder::items::file_image(icon, &self.app, 56., palette.accent))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(17.)).font_weight(FontWeight::BOLD).child(format!("Uninstall {}?", self.name())))
                            .child(div().text_size(px(12.)).text_color(palette.secondary).child(match self.app_bytes {
                                Some(bytes) => format!("The app takes {}; below, what it leaves in your Library.", human_size(bytes)),
                                None => "Working out what it takes…".into(),
                            })),
                    ),
            )
            .child(div().id("leftovers").flex_1().min_h_0().overflow_y_scroll().mx_3().p_1().rounded_lg().border_1().border_color(palette.separator).children(rows))
            .child(
                div()
                    .p_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(palette.secondary).child(match (&self.said, total) {
                        (Some(said), _) => said.to_string(),
                        (None, Some(total)) => format!("{} to free · Put Back works", human_size(total)),
                        (None, None) => String::new(),
                    }))
                    .child(glass(28., &palette).id("cancel").px_4().child("Cancel").on_click(|_, window, _| window.remove_window()))
                    .child(
                        glass(28., &palette)
                            .id("uninstall")
                            .px_4()
                            .bg(rgb(0xff453a))
                            .text_color(white())
                            .when(self.leftovers.is_none() || self.busy, |d| d.opacity(0.5))
                            .child(if self.said.as_ref().is_some_and(|s| s.contains("is open")) { "Quit and Uninstall" } else { "Move to Trash" })
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.leftovers.is_some() {
                                    this.uninstall(window, cx)
                                }
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{leftovers, refused};

    #[test]
    fn finds_what_an_app_leaves() {
        let home = std::env::temp_dir().join(format!("f2-uninstall-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let app = home.join("Applications/Fakeapp.app");
        std::fs::create_dir_all(app.join("Contents")).unwrap();
        std::fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.example.fakeapp</string><key>CFBundleName</key><string>Fakeapp</string></dict></plist>"#,
        )
        .unwrap();
        let lib = home.join("Library");
        for dir in ["Application Support/Fakeapp", "Application Support/com.example.fakeapp", "Caches/com.example.fakeapp", "Containers/com.example.fakeapp", "Group Containers/ABCDE12345.com.example.fakeapp", "Application Support/Other"] {
            std::fs::create_dir_all(lib.join(dir)).unwrap();
            std::fs::write(lib.join(dir).join("data"), vec![0u8; 5000]).unwrap();
        }
        std::fs::create_dir_all(lib.join("Preferences")).unwrap();
        for file in ["com.example.fakeapp.plist", "com.example.fakeapp.helper.plist", "com.example.other.plist"] {
            std::fs::write(lib.join("Preferences").join(file), "x").unwrap();
        }
        let found: Vec<String> = leftovers(&app, &home).iter().map(|l| l.path.strip_prefix(&lib).unwrap().display().to_string()).collect();
        let mut found = found;
        found.sort();
        assert_eq!(
            found,
            [
                "Application Support/Fakeapp",
                "Application Support/com.example.fakeapp",
                "Caches/com.example.fakeapp",
                "Containers/com.example.fakeapp",
                "Group Containers/ABCDE12345.com.example.fakeapp",
                "Preferences/com.example.fakeapp.helper.plist",
                "Preferences/com.example.fakeapp.plist",
            ]
        );
        assert_eq!(refused(&app), None);
        assert!(refused(std::path::Path::new("/System/Applications/Calculator.app")).is_some());
        let _ = std::fs::remove_dir_all(&home);
    }
}

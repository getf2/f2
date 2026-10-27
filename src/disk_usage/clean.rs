//! Disk Usage's Clean Up: Free Up Space (what's safe to let go of, a card
//! each: installers, old downloads, device backups, apps' caches, the
//! Trash, large files, copies), Large Files (100 MB and more on the
//! startup disk, the biggest first), Duplicates (files in your home folder
//! that are copies of one another), each to tick and move to the Trash;
//! and System Data (what macOS's Storage settings lump together under that
//! name, taken apart: Xcode's, developer caches, logs, software updates to
//! move to the Trash; simulator runtimes to delete; snapshots, swap,
//! temporary files and Docker's disk, said how they go). See
//! `crate::cleanup` for what's found, and what's left out.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::DiskUsage;
use crate::cleanup::{Copies, Found, Kind, Part, PartAction, Progress};
use crate::listing::{date_label, human_size, thousands};
use crate::macos;
use crate::style::{MEDIUM, SEMIBOLD, glass, symbol};

/// A line of the list: two lines of text high, a file's or a heading's.
const LINE: f32 = 40.;

/// Which of Clean Up's pages: Free Up Space's cards, or a list.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum List {
    Space,
    Large,
    Copies,
    /// System Data's cards.
    System,
    Of(Kind),
}

/// A line: a file (or folder) of a list, a set of copies' heading, or one
/// of the copies.
#[derive(Clone, Copy)]
enum Line {
    File(usize),
    Set(usize),
    Copy(usize, usize),
}

#[derive(Default)]
pub(super) struct Clean {
    /// Large Files' and Free Up Space's kinds' lists, as last found.
    files: HashMap<List, (Vec<Found>, Duration)>,
    /// Lists being found now.
    finding: HashSet<List>,
    /// What the Trash holds (None inside: it can't be seen).
    trash: Option<Option<u64>>,
    /// Empty Trash asked once: a second click empties it.
    emptying: bool,
    copies: Option<(Vec<Copies>, Duration)>,
    /// System Data's parts that aren't files to tick, as last found.
    system: Option<(Vec<Part>, Duration)>,
    /// A part's Delete asked once (a second click deletes it), and one
    /// being deleted.
    confirm: Option<usize>,
    deleting: Option<usize>,
    /// The page a kind's list goes back to: Free Up Space's, or System
    /// Data's.
    back: Option<List>,
    progress: Option<Arc<Progress>>,
    checked: HashSet<PathBuf>,
    /// What the last Move to Trash did, or why it couldn't.
    said: Option<SharedString>,
    task: Option<Task<()>>,
    /// Lists being found (each its own, so they're found side by side).
    tasks: Vec<Task<()>>,
    scroll: UniformListScrollHandle,
}

impl Drop for Clean {
    fn drop(&mut self) {
        // (A duplicates search still going: it stops when the window does.)
        if let Some(progress) = &self.progress {
            progress.stop.store(true, Ordering::Relaxed);
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// Where Clean Up looks: the startup disk for large files, your home folder
/// for copies and the rest. (A test copy, NEW_FINDER_SETTINGS set, can be
/// kept to a folder of its own with F2_CLEANUP_ROOT: away from those macOS
/// asks about.)
fn roots() -> (PathBuf, PathBuf) {
    match std::env::var_os("F2_CLEANUP_ROOT").filter(|_| std::env::var_os("NEW_FINDER_SETTINGS").is_some()) {
        Some(root) => (PathBuf::from(&root), PathBuf::from(root)),
        None => (PathBuf::from("/"), home()),
    }
}

/// APFS clones of one another: they share their blocks, so deleting one
/// frees next to nothing.
fn clones(set: &Copies) -> bool {
    set.freed < set.len / 10
}

/// When it was last opened, to the day: "Opened today", "Opened Dec 24,
/// 2025", "Never opened".
fn opened_label(opened: Option<i64>) -> SharedString {
    use chrono::{Days, Local, TimeZone};
    let Some(t) = opened.and_then(|secs| Local.timestamp_opt(secs, 0).single()) else { return "Never opened".into() };
    let today = Local::now().date_naive();
    if t.date_naive() == today {
        "Opened today".into()
    } else if today.checked_sub_days(Days::new(1)) == Some(t.date_naive()) {
        "Opened yesterday".into()
    } else {
        t.format("Opened %b %-d, %Y").to_string().into()
    }
}

/// Where a file is, as Finder writes it under your home: "~/Movies".
fn location(path: &Path) -> String {
    let parent = path.parent().unwrap_or(path);
    match parent.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => parent.display().to_string(),
    }
}

impl DiskUsage {
    /// Show one of Clean Up's pages (finding what's in it the first time;
    /// Free Up Space: what's in each card's, but the copies).
    pub(super) fn open_list(&mut self, list: List, cx: &mut Context<Self>) {
        self.list = Some(list);
        self.clean.said = None;
        self.clean.emptying = false;
        self.clean.confirm = None;
        if matches!(list, List::Space | List::System) {
            self.clean.back = Some(list);
        }
        match list {
            List::System => {
                for list in Kind::SYSTEM.map(List::Of) {
                    if !self.clean.files.contains_key(&list) && !self.clean.finding.contains(&list) {
                        self.find(list, cx);
                    }
                }
                if self.clean.system.is_none() && !self.clean.finding.contains(&List::System) {
                    self.find_system(cx);
                }
            }
            List::Space => {
                for list in std::iter::once(List::Large).chain(Kind::ALL.map(List::Of)) {
                    if !self.clean.files.contains_key(&list) && !self.clean.finding.contains(&list) {
                        self.find(list, cx);
                    }
                }
                self.find_trash(cx);
            }
            List::Copies if self.clean.copies.is_none() && self.clean.progress.is_none() => self.find_copies(cx),
            List::Large | List::Of(_) if !self.clean.files.contains_key(&list) && !self.clean.finding.contains(&list) => self.find(list, cx),
            _ => {}
        }
        cx.notify();
    }

    /// Find what's in a list (Large Files, or a kind's), off the main thread.
    fn find(&mut self, list: List, cx: &mut Context<Self>) {
        let Some(search) = cx.global::<crate::browser::Shared>().search.clone() else { return };
        self.clean.finding.insert(list);
        self.clean.tasks.push(cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let found = cx
                .background_executor()
                .spawn(async move {
                    let (disk, home) = roots();
                    match list {
                        List::Of(kind) => crate::cleanup::category(&search, kind, &home),
                        _ => crate::cleanup::large(&search, &disk),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                // (What's safe without a look, ticked to begin with.)
                if let List::Of(kind) = list
                    && kind.ticked()
                {
                    this.clean.checked.extend(found.iter().map(|f| f.path.clone()));
                }
                this.clean.finding.remove(&list);
                this.clean.files.insert(list, (found, started.elapsed()));
                cx.notify();
            })
            .ok();
        }));
    }

    /// Find System Data's parts that aren't files to tick, off the main
    /// thread (simctl and tmutil are asked).
    fn find_system(&mut self, cx: &mut Context<Self>) {
        let Some(search) = cx.global::<crate::browser::Shared>().search.clone() else { return };
        self.clean.finding.insert(List::System);
        self.clean.tasks.push(cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let parts = cx.background_executor().spawn(async move { crate::cleanup::system_parts(&search, &roots().1) }).await;
            this.update(cx, |this, cx| {
                this.clean.finding.remove(&List::System);
                this.clean.system = Some((parts, started.elapsed()));
                cx.notify();
            })
            .ok();
        }));
    }

    /// A part's action: open what it opens, or (a second click, Empty
    /// Trash's way) delete it, then look again.
    fn part_action(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(part) = self.clean.system.as_ref().and_then(|(parts, _)| parts.get(ix)).cloned() else { return };
        let Some(action) = part.action.clone() else { return };
        if let PartAction::Open(_, address) = &action {
            return cx.open_url(address);
        }
        if self.clean.deleting.is_some() {
            return;
        }
        if self.clean.confirm != Some(ix) {
            self.clean.confirm = Some(ix);
            return cx.notify();
        }
        (self.clean.confirm, self.clean.deleting) = (None, Some(ix));
        cx.notify();
        self.clean.task = Some(cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn(async move { crate::cleanup::run_part(&action) }).await;
            this.update(cx, |this, cx| {
                this.clean.deleting = None;
                this.clean.said = Some(match done {
                    Ok(()) => format!("Deleted {} ({})", part.title, part.bytes.map(human_size).unwrap_or_default()).into(),
                    Err(e) => format!("{} couldn't be deleted: {e}", part.title).into(),
                });
                this.find_system(cx);
                cx.notify();
                this.task = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(600)).await;
                    this.update(cx, |this, cx| this.refresh(cx)).ok();
                });
            })
            .ok();
        }));
    }

    fn find_trash(&mut self, cx: &mut Context<Self>) {
        let Some(search) = cx.global::<crate::browser::Shared>().search.clone() else { return };
        self.clean.tasks.push(cx.spawn(async move |this, cx| {
            let held = cx.background_executor().spawn(async move { crate::cleanup::trash_bytes(&search, &roots().1) }).await;
            this.update(cx, |this, cx| {
                this.clean.trash = Some(held);
                cx.notify();
            })
            .ok();
        }));
    }

    /// Empty the Trash: asked first (the button says so), then done.
    fn empty_trash(&mut self, cx: &mut Context<Self>) {
        if !self.clean.emptying {
            self.clean.emptying = true;
            return cx.notify();
        }
        self.clean.emptying = false;
        self.clean.task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { crate::finder::erase_trash() }).await;
            this.update(cx, |this, cx| {
                this.clean.said = Some(match result {
                    Ok(()) => {
                        // (Without Full Disk Access Finder empties it, with its own sound.)
                        if macos::full_disk_access() {
                            macos::play(macos::Sound::EmptyTrash);
                        }
                        format!("Emptied the {}", macos::bin()).into()
                    }
                    Err(e) => e.into(),
                });
                this.find_trash(cx);
                this.refresh(cx);
            })
            .ok();
        }));
    }

    fn find_copies(&mut self, cx: &mut Context<Self>) {
        let Some(search) = cx.global::<crate::browser::Shared>().search.clone() else { return };
        if let Some(old) = self.clean.progress.take() {
            old.stop.store(true, Ordering::Relaxed);
        }
        let progress = Arc::new(Progress::default());
        self.clean.progress = Some(progress.clone());
        self.clean.copies = None;
        self.clean.task = Some(cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut search_for = cx.background_executor().spawn({
                let progress = progress.clone();
                async move { crate::cleanup::duplicates(&search, &roots().1, &progress) }
            });
            // (Showing how far it's got, until it's done.)
            let found = loop {
                let tick = cx.background_executor().timer(Duration::from_millis(250));
                match futures::future::select(&mut search_for, tick).await {
                    futures::future::Either::Left((found, _)) => break found,
                    futures::future::Either::Right(_) => {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            };
            this.update(cx, |this, cx| {
                this.clean.progress = None;
                if let Some(found) = found {
                    this.clean.copies = Some((found, started.elapsed()));
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn lines(&self) -> Vec<Line> {
        match self.list {
            Some(list @ (List::Large | List::Of(_))) => (0..self.clean.files.get(&list).map_or(0, |(f, _)| f.len())).map(Line::File).collect(),
            Some(List::Copies) => {
                let mut lines = Vec::new();
                for (s, set) in self.clean.copies.iter().flat_map(|(c, _)| c.iter()).enumerate() {
                    lines.push(Line::Set(s));
                    lines.extend((0..set.files.len()).map(|f| Line::Copy(s, f)));
                }
                lines
            }
            Some(List::Space | List::System) | None => Vec::new(),
        }
    }

    fn file_of(&self, line: Line) -> Option<&Found> {
        match line {
            Line::File(i) => self.clean.files.get(&self.list?)?.0.get(i),
            Line::Copy(s, f) => self.clean.copies.as_ref()?.0.get(s)?.files.get(f),
            Line::Set(_) => None,
        }
    }

    /// The ticked files, and what moving them to the Trash frees (a clone's
    /// shared blocks aside).
    fn ticked(&self) -> (usize, u64) {
        let mut bytes = 0;
        let mut seen = HashSet::new();
        for line in self.lines() {
            if let Some(f) = self.file_of(line)
                && self.clean.checked.contains(&f.path)
                && seen.insert(&f.path)
            {
                bytes += f.own;
            }
        }
        (seen.len(), bytes)
    }

    /// Tick everything in the list shown, or (all ticked) nothing.
    fn tick_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.lines().into_iter().filter_map(|l| self.file_of(l).map(|f| f.path.clone())).collect();
        if paths.iter().all(|p| self.clean.checked.contains(p)) {
            for p in &paths {
                self.clean.checked.remove(p);
            }
        } else {
            self.clean.checked.extend(paths);
        }
        self.clean.said = None;
        cx.notify();
    }

    /// Whether everything in the list shown is ticked.
    fn all_ticked(&self) -> bool {
        let lines = self.lines();
        !lines.is_empty() && lines.into_iter().filter_map(|l| self.file_of(l)).all(|f| self.clean.checked.contains(&f.path))
    }

    /// Tick all copies in each set but the oldest (or, `set`, in that one).
    /// (All of them: not clones of one another, which free nothing.)
    fn keep_oldest(&mut self, set: Option<usize>, cx: &mut Context<Self>) {
        if let Some((copies, _)) = &self.clean.copies {
            let all = set.is_none();
            for (_, c) in copies.iter().enumerate().filter(|(s, c)| set.is_none_or(|only| only == *s) && !(all && clones(c))) {
                self.clean.checked.remove(&c.files[0].path);
                self.clean.checked.extend(c.files[1..].iter().map(|f| f.path.clone()));
            }
        }
        cx.notify();
    }

    /// Move what's ticked to the Trash (off the main thread), then take it
    /// out of the lists, and work out the disk's use again.
    fn trash_ticked(&mut self, cx: &mut Context<Self>) {
        // (What's ticked in the list shown: not what's still ticked in
        // another, out of sight.)
        let mut paths: Vec<PathBuf> = Vec::new();
        for line in self.lines() {
            if let Some(f) = self.file_of(line)
                && self.clean.checked.contains(&f.path)
                && !paths.contains(&f.path)
            {
                paths.push(f.path.clone());
            }
        }
        if paths.is_empty() {
            return;
        }
        let (_, bytes) = self.ticked();
        self.clean.task = Some(cx.spawn(async move |this, cx| {
            let moved = cx
                .background_executor()
                .spawn(async move {
                    // (What isn't yours to move, through Finder: an administrator's password, once.)
                    let results = macos::trash_all(&paths);
                    paths.into_iter().zip(results).map(|(p, r)| (r.map(|to| (p.clone(), to)), p)).collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                let (done, failed): (Vec<_>, Vec<_>) = moved.into_iter().partition(|(r, _)| r.is_ok());
                let trashed: Vec<(PathBuf, PathBuf)> = done.into_iter().filter_map(|(r, _)| r.ok()).collect();
                crate::finder::remember_trashed(&trashed);
                if !trashed.is_empty() {
                    macos::play(macos::Sound::MoveToTrash);
                }
                let gone: HashSet<PathBuf> = trashed.iter().map(|(from, _)| from.clone()).collect();
                let clean = &mut this.clean;
                clean.checked.retain(|p| !gone.contains(p));
                for (files, _) in clean.files.values_mut() {
                    files.retain(|f| !gone.contains(&f.path));
                }
                if let Some((copies, _)) = &mut clean.copies {
                    for set in copies.iter_mut() {
                        set.files.retain(|f| !gone.contains(&f.path));
                    }
                    copies.retain(|set| set.files.len() > 1);
                }
                let bin = macos::bin();
                clean.said = Some(match failed.first() {
                    None => format!("Moved {} ({}) to the {bin}", crate::listing::count(gone.len(), "item"), human_size(bytes)),
                    Some((Err(e), path)) => format!("Moved {}; “{}” couldn't be: {e}", gone.len(), path.file_name().unwrap_or_default().to_string_lossy()),
                    Some(_) => String::new(),
                }
                .into());
                cx.notify();
                // (Once the index has taken it in: the chart, anew.)
                this.task = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(600)).await;
                    this.update(cx, |this, cx| this.refresh(cx)).ok();
                });
            })
            .ok();
        }));
    }

    /// Clean Up's part of the sidebar.
    pub(super) fn render_clean_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        // (A kind's list is its page's: Free Up Space's or System Data's.)
        let page = match self.list {
            Some(List::Of(_)) => Some(self.clean.back.unwrap_or(List::Space)),
            list => list,
        };
        let row = |id: &'static str, list: List, icon: &'static str, title: &'static str, about: String, cx: &mut Context<Self>| {
            let on = page == Some(list);
            div()
                .id(id)
                .px_2()
                .py_2()
                .rounded_lg()
                .flex()
                .items_center()
                .gap_2()
                .when(on, |d| d.bg(palette.glass_selected))
                .hover(|s| s.bg(palette.glass_selected))
                .child(symbol(icon, 13., MEDIUM, palette.accent, window))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(div().text_size(px(11.)).text_color(palette.secondary).truncate().child(about)),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.open_list(list, cx)))
        };
        let large = match self.clean.files.get(&List::Large) {
            Some((files, _)) if files.is_empty() => "None".into(),
            Some((files, _)) => format!("{} · {}", files.len(), human_size(files.iter().map(|f| f.bytes).sum())),
            None => "100 MB and more".into(),
        };
        let copies = match (&self.clean.copies, &self.clean.progress) {
            (_, Some(_)) => "Comparing…".into(),
            (Some((sets, _)), _) => format!("{} to free", human_size(sets.iter().map(|s| s.freed).sum())),
            (None, None) => "In your home folder".into(),
        };
        let space = match self.freeable() {
            Some(bytes) if bytes > 0 => format!("Up to {} to free", human_size(bytes)),
            _ => "What's safe to let go of".into(),
        };
        let system = match self.system_found() {
            Some(bytes) => format!("{} found", human_size(bytes)),
            None => "What it's made of".into(),
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().px_2().pt_3().pb_1().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child("Clean Up"))
            .child(row("clean-space", List::Space, "sparkles", "Free Up Space", space, cx))
            .child(row("clean-large", List::Large, "doc.badge.ellipsis", "Large Files", large, cx))
            .child(row("clean-copies", List::Copies, "doc.on.doc", "Duplicates", copies, cx))
            .child(row("clean-system", List::System, "gearshape.2", "System Data", system, cx))
    }

    /// What Free Up Space's cards add up to, once each kind's found (large
    /// files aside: they're yours to judge).
    fn freeable(&self) -> Option<u64> {
        let kinds: Option<u64> = Kind::ALL.iter().map(|k| self.clean.files.get(&List::Of(*k)).map(|(f, _)| f.iter().map(|f| f.own).sum::<u64>())).sum();
        Some(kinds? + self.clean.trash.flatten().unwrap_or(0) + self.clean.copies.as_ref().map_or(0, |(c, _)| c.iter().map(|s| s.freed).sum()))
    }

    /// A card: what it is, what it takes, and what to do.
    #[allow(clippy::too_many_arguments)]
    fn card(&self, window: &Window, id: SharedString, icon: &'static str, title: String, about: String, size: String, action: AnyElement) -> AnyElement {
        let palette = self.palette;
        div()
            .id(id)
            .p_3()
            .rounded_xl()
            .bg(palette.label.opacity(0.04))
            .border_1()
            .border_color(palette.separator)
            .flex()
            .items_center()
            .gap_3()
            .child(div().size(px(34.)).flex_shrink_0().rounded_lg().bg(palette.accent.opacity(0.14)).flex().items_center().justify_center().child(symbol(icon, 16., SEMIBOLD, palette.accent, window)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_size(px(11.)).text_color(palette.secondary).child(about)),
            )
            .child(div().flex_shrink_0().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(size))
            .child(action)
            .into_any_element()
    }

    /// A card's button.
    fn card_button(&self, id: SharedString, label: String) -> Stateful<Div> {
        glass(26., &self.palette).id(id).flex_shrink_0().px_3().text_size(px(12.)).child(label)
    }

    /// What a kind's list holds, once found: how many, and what moving them
    /// to the Trash frees.
    fn total(&self, list: List) -> Option<(usize, u64)> {
        self.clean.files.get(&list).map(|(f, _)| (f.len(), f.iter().map(|f| f.own).sum::<u64>()))
    }

    /// A kind's card: what it holds, and Review for its list.
    fn kind_card(&self, kind: Kind, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let (title, about, icon) = kind.about();
        let list = List::Of(kind);
        let (size, action) = match self.total(list) {
            None => ("Looking…".to_string(), div().into_any_element()),
            Some((0, _)) => ("None".to_string(), div().into_any_element()),
            Some((n, bytes)) => (
                format!("{} · {}", crate::listing::count(n, "item"), human_size(bytes)),
                self.card_button(format!("review-{kind:?}").into(), "Review".into()).on_click(cx.listener(move |this, _, _, cx| this.open_list(list, cx))).into_any_element(),
            ),
        };
        self.card(window, format!("card-{kind:?}").into(), icon, title.into(), about.into(), size, action)
    }

    /// What System Data's cards add up to, once all are found.
    fn system_found(&self) -> Option<u64> {
        let kinds: Option<u64> = Kind::SYSTEM.iter().map(|k| self.total(List::Of(*k)).map(|(_, bytes)| bytes)).sum();
        Some(kinds? + self.clean.system.as_ref()?.0.iter().filter_map(|p| p.bytes).sum::<u64>())
    }

    /// System Data, taken apart: a card for each kind of files that can go
    /// to the Trash, then each part that goes another way (a simulator
    /// runtime to delete, snapshots macOS deletes itself…), the biggest
    /// first.
    fn render_system(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let mut cards: Vec<AnyElement> = Kind::SYSTEM.iter().map(|k| self.kind_card(*k, window, cx)).collect();
        let parts = self.clean.system.as_ref().map(|(parts, _)| parts.clone());
        match &parts {
            None => cards.push(div().px_1().py_2().text_size(px(12.)).text_color(palette.secondary).child("Looking at simulators, snapshots, swap and temporary files…").into_any_element()),
            Some(parts) => {
                for (ix, part) in parts.iter().enumerate() {
                    let size = part.bytes.map_or_else(|| "Can't be seen".to_string(), human_size);
                    let action = match &part.action {
                        None => div().into_any_element(),
                        Some(PartAction::Open(label, _)) => {
                            self.card_button(format!("part-{ix}").into(), (*label).into()).on_click(cx.listener(move |this, _, _, cx| this.part_action(ix, cx))).into_any_element()
                        }
                        Some(_) => {
                            let (label, red) = match (self.clean.deleting, self.clean.confirm) {
                                (Some(d), _) if d == ix => ("Deleting…", false),
                                (_, Some(c)) if c == ix => ("Click to Delete", true),
                                _ => ("Delete", false),
                            };
                            self.card_button(format!("part-{ix}").into(), label.into())
                                .when(red, |d| d.text_color(rgb(0xff453a)))
                                .when(self.clean.deleting.is_some_and(|d| d != ix), |d| d.opacity(0.4))
                                .on_click(cx.listener(move |this, _, _, cx| this.part_action(ix, cx)))
                                .into_any_element()
                        }
                    };
                    cards.push(self.card(window, format!("part-card-{ix}").into(), part.icon, part.title.clone(), part.about.clone(), size, action));
                }
            }
        }
        let headline = match self.system_found() {
            Some(bytes) => format!("{} of System Data found", human_size(bytes)),
            None => "Taking System Data apart…".into(),
        };
        let status: SharedString = self.clean.said.clone().unwrap_or_else(|| "What macOS's Storage settings lump together as System Data, each part on its own, and how it's let go of".into());
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child(headline))
                    .child(div().text_size(px(12.)).text_color(palette.secondary).child("Files to review and move to the Trash first; then what goes another way")),
            )
            .child(div().id("system-cards").flex_1().min_h_0().overflow_y_scroll().px_4().py_2().flex().flex_col().gap_2().children(cards))
            .child(div().px_3().py_1().border_t_1().border_color(palette.separator).text_size(px(11.)).text_color(palette.secondary).truncate().child(status))
            .into_any_element()
    }

    /// Free Up Space: a card for each kind of thing that's safe to let go
    /// of, with what it holds and what to do.
    fn render_space(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let total = |list: List| self.total(list);
        let mut cards: Vec<AnyElement> = Kind::ALL.iter().map(|k| self.kind_card(*k, window, cx)).collect();
        let card = |id: SharedString, icon: &'static str, title: String, about: String, size: String, action: AnyElement| self.card(window, id, icon, title, about, size, action);
        let button = |id: SharedString, label: String| self.card_button(id, label);
        let bin = macos::bin();
        let (trash_size, trash_action) = match self.clean.trash {
            None => ("Looking…".to_string(), div().into_any_element()),
            Some(None) => ("Can't be seen".to_string(), div().into_any_element()),
            Some(Some(0)) => ("Empty".to_string(), div().into_any_element()),
            Some(Some(bytes)) => (
                human_size(bytes),
                button("empty-trash".into(), if self.clean.emptying { "Click to Empty".into() } else { format!("Empty {bin}") })
                    .when(self.clean.emptying, |d| d.text_color(rgb(0xff453a)))
                    .on_click(cx.listener(|this, _, _, cx| this.empty_trash(cx)))
                    .into_any_element(),
            ),
        };
        let trash_about = if self.clean.trash == Some(None) {
            format!("Give {} Full Disk Access to see what's in it", crate::APP_NAME)
        } else {
            "Emptied, it's gone for good".to_string()
        };
        cards.push(card("card-trash".into(), "trash", bin.into(), trash_about, trash_size, trash_action));
        let (large_size, large_action) = match total(List::Large) {
            None => ("Looking…".to_string(), div().into_any_element()),
            Some((0, _)) => ("None".to_string(), div().into_any_element()),
            Some((n, bytes)) => (
                format!("{} · {}", crate::listing::count(n, "file"), human_size(bytes)),
                button("review-large".into(), "Review".into()).on_click(cx.listener(|this, _, _, cx| this.open_list(List::Large, cx))).into_any_element(),
            ),
        };
        cards.push(card("card-large".into(), "doc.badge.ellipsis", "Large Files".into(), "100 MB and more: yours to judge".into(), large_size, large_action));
        let (copies_size, copies_label) = match (&self.clean.copies, &self.clean.progress) {
            (_, Some(_)) => ("Comparing…".to_string(), "Show"),
            (Some((sets, _)), _) => (human_size(sets.iter().map(|s| s.freed).sum()), "Review"),
            (None, None) => (String::new(), "Look for Copies"),
        };
        let copies_action = button("review-copies".into(), copies_label.into()).on_click(cx.listener(|this, _, _, cx| this.open_list(List::Copies, cx))).into_any_element();
        cards.push(card("card-copies".into(), "doc.on.doc", "Duplicates".into(), "Identical files in your home folder".into(), copies_size, copies_action));

        let headline = match self.freeable() {
            Some(bytes) => format!("Up to {} can be freed", human_size(bytes)),
            None => "Looking for what's safe to let go of…".into(),
        };
        let status: SharedString = self.clean.said.clone().unwrap_or_else(|| "Nothing's deleted until you say so: what you move to the Trash can be put back".into());
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child(headline))
                    .child(div().text_size(px(12.)).text_color(palette.secondary).child("What's on this Mac that's safe to let go of, each kind on its own")),
            )
            .child(div().id("space-cards").flex_1().min_h_0().overflow_y_scroll().px_4().py_2().flex().flex_col().gap_2().children(cards))
            .child(div().px_3().py_1().border_t_1().border_color(palette.separator).text_size(px(11.)).text_color(palette.secondary).truncate().child(status))
            .into_any_element()
    }

    /// The page: Free Up Space's cards, or a list with its heading and the
    /// bar under it.
    pub(super) fn render_clean(&mut self, list: List, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match list {
            List::Space => return self.render_space(window, cx),
            List::System => return self.render_system(window, cx),
            _ => {}
        }
        let palette = self.palette;
        let (title, about) = match list {
            List::Large => ("Large Files", "Files of 100 MB and more on the startup disk, the biggest first"),
            List::Copies => ("Duplicates", "Identical files of 1 MB and more in your home folder"),
            List::Of(kind) => (kind.about().0, kind.about().1),
            List::Space | List::System => ("", ""),
        };
        let lines = self.lines();
        let body = if lines.is_empty() {
            let message = match (list, &self.clean.copies, &self.clean.progress) {
                (List::Copies, _, Some(p)) => {
                    let (read, of) = (p.read.load(Ordering::Relaxed), p.to_read.load(Ordering::Relaxed));
                    if of == 0 { "Finding files of the same size…".to_string() } else { format!("Comparing… {} of {} read", human_size(read), human_size(of)) }
                }
                (List::Copies, Some(_), _) => "No copies found.".into(),
                (List::Copies, ..) => "Looking…".into(),
                (_, ..) if !self.clean.files.contains_key(&list) => "Looking…".into(),
                (List::Large, ..) => "No files of 100 MB or more.".into(),
                _ => "Nothing here.".into(),
            };
            div().flex_1().flex().items_center().justify_center().text_color(palette.secondary).child(message).into_any_element()
        } else {
            let lines = Arc::new(lines);
            let shown = lines.clone();
            div()
                .flex_1()
                .min_h_0()
                .child(
                    uniform_list("clean-lines", lines.len(), cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        range.map(|i| this.render_line(shown[i], i, window, cx)).collect()
                    }))
                    .track_scroll(&self.clean.scroll)
                    .size_full()
                    .px_2()
                    .py_1(),
                )
                .into_any_element()
        };
        let (n, bytes) = self.ticked();
        let status: SharedString = match (&self.clean.said, list) {
            (Some(said), _) => said.clone(),
            (None, List::Copies) => match &self.clean.copies {
                Some((sets, took)) => format!(
                    "{} sets of copies · {} can be freed (keeping the oldest of each) · compared in {:.1} s",
                    sets.len(),
                    human_size(sets.iter().map(|s| s.freed).sum()),
                    took.as_secs_f32()
                )
                .into(),
                None => "".into(),
            },
            (None, _) => match self.clean.files.get(&list) {
                Some((files, took)) => format!(
                    "{} · {} · found in {} ms",
                    crate::listing::count(files.len(), "item"),
                    human_size(files.iter().map(|f| f.bytes).sum()),
                    took.as_millis().max(1)
                )
                .into(),
                None => "".into(),
            },
        };
        let button = |id: &'static str, label: String, enabled: bool| {
            glass(26., &palette).id(id).px_3().text_size(px(12.)).when(!enabled, |d| d.opacity(0.4)).child(label)
        };
        let back = matches!(list, List::Of(_));
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(palette.separator)
                    // (A kind's list: back to Free Up Space's cards.)
                    .when(back, |d| {
                        d.child(
                            glass(26., &palette)
                                .id("clean-back")
                                .px_2()
                                .child(symbol("chevron.left", 12., SEMIBOLD, palette.label, window))
                                .on_click(cx.listener(|this, _, _, cx| this.open_list(this.clean.back.unwrap_or(List::Space), cx))),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(div().text_size(px(11.)).text_color(palette.secondary).truncate().child(about)),
                    )
                    .child(
                        glass(26., &palette)
                            .id("clean-again")
                            .px_2()
                            .child(symbol("arrow.clockwise", 12., SEMIBOLD, palette.label, window))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.clean.checked.clear();
                                this.clean.said = None;
                                match list {
                                    List::Copies => this.find_copies(cx),
                                    _ => {
                                        this.clean.files.remove(&list);
                                        this.find(list, cx);
                                    }
                                }
                            })),
                    ),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(palette.separator)
                    .text_size(px(12.))
                    .child(div().flex_1().min_w_0().truncate().text_color(palette.secondary).child(if n == 0 {
                        "Tick what to move to the Trash".to_string()
                    } else {
                        format!("{} ticked · {} to free", thousands(n as u64), human_size(bytes))
                    }))
                    .when(list == List::Copies && self.clean.copies.as_ref().is_some_and(|(s, _)| !s.is_empty()), |d| {
                        d.child(button("keep-oldest", "Tick All but the Oldest".into(), true).on_click(cx.listener(|this, _, _, cx| this.keep_oldest(None, cx))))
                    })
                    .when(list != List::Copies && !self.lines().is_empty(), |d| {
                        let label = if self.all_ticked() { "Untick All" } else { "Tick All" };
                        d.child(button("tick-all", label.into(), true).on_click(cx.listener(|this, _, _, cx| this.tick_all(cx))))
                    })
                    .child(button("trash-ticked", format!("Move to {}", macos::bin()), n > 0).on_click(cx.listener(|this, _, _, cx| this.trash_ticked(cx)))),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(palette.separator)
                    .text_size(px(11.))
                    .text_color(palette.secondary)
                    .truncate()
                    .child(status),
            )
            .into_any_element()
    }

    fn render_line(&mut self, line: Line, i: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        if let Line::Set(s) = line {
            let Some(set) = self.clean.copies.as_ref().and_then(|(c, _)| c.get(s)).cloned() else { return div().into_any_element() };
            let clones = clones(&set);
            let about = if clones {
                "clones of one another: they share their space, so deleting frees next to nothing".to_string()
            } else {
                format!("frees {}", human_size(set.freed))
            };
            return div()
                .id(("clean-set", i))
                .w_full()
                .h(px(LINE))
                .px_2()
                .flex()
                .items_end()
                .pb_1()
                .gap_2()
                .text_size(px(12.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} copies · {} each", set.files.len(), human_size(set.len))))
                .child(div().flex_1().min_w_0().truncate().text_color(if clones { palette.tertiary } else { palette.secondary }).child(about))
                .child(
                    div()
                        .id(("keep-one", i))
                        .px_2()
                        .rounded_md()
                        .text_color(palette.accent)
                        .hover(|d| d.bg(palette.glass_selected))
                        .child("Keep Oldest")
                        .on_click(cx.listener(move |this, _, _, cx| this.keep_oldest(Some(s), cx))),
                )
                .into_any_element();
        }
        let Some(f) = self.file_of(line).cloned() else { return div().into_any_element() };
        let ticked = self.clean.checked.contains(&f.path);
        let oldest = matches!(line, Line::Copy(_, 0));
        let name = f.label.clone().unwrap_or_else(|| f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let (toggle, menu) = (f.path.clone(), f.path.clone());
        // (Large files and downloads: when they were last opened, what says
        // whether they're still used. The rest: when they changed.)
        let when: SharedString = match self.list {
            Some(List::Large | List::Of(Kind::Installers | Kind::OldDownloads)) => opened_label(f.opened),
            _ => date_label(f.modified),
        };
        let tag = |text: &'static str| div().flex_shrink_0().px_1p5().rounded_sm().bg(palette.label.opacity(0.07)).text_size(px(10.)).text_color(palette.secondary).child(text);
        div()
            .id(("clean-file", i))
            .w_full()
            .h(px(LINE))
            .px_2()
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
                    .when(ticked, |d| d.bg(palette.accent).child(symbol("checkmark", 10., SEMIBOLD, white(), window)))
                    .when(!ticked, |d| d.border_1().border_color(palette.tertiary)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_size(px(12.)).child(name))
                    .child(div().truncate().text_size(px(11.)).text_color(palette.secondary).child(location(&f.path))),
            )
            // (In Free Up Space's lists everything is: no need to say.)
            .when(f.app_data && self.list == Some(List::Large), |d| d.child(tag("App data")))
            .when(oldest, |d| d.child(tag("Oldest")))
            .child(div().w(px(150.)).flex_shrink_0().text_size(px(11.)).text_color(palette.secondary).text_right().child(when))
            .child(div().w(px(70.)).flex_shrink_0().text_size(px(12.)).text_right().child(human_size(f.bytes)))
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                // (A double click shows it in F2.)
                if e.click_count() >= 2 {
                    if let Some(parent) = toggle.parent() {
                        crate::browser::open(parent.to_path_buf(), Some(toggle.clone()), cx);
                    }
                    return;
                }
                if !this.clean.checked.remove(&toggle) {
                    this.clean.checked.insert(toggle.clone());
                }
                this.clean.said = None;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, _, cx| this.item_menu(menu.clone(), cx)))
            .into_any_element()
    }
}

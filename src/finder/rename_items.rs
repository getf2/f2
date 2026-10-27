//! Renaming several items at once, as Finder's "Rename…" does with more
//! than one selected: replace text in their names, add text to them, or
//! name them anew with numbers. Shows what the names become as you type,
//! and is one step for undo.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::Finder;
use super::sheets::{Sheet, button, buttons, card};
use super::undo::Op;

/// How many of the new names the sheet lists.
const SHOWN: usize = 5;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Mode {
    Replace,
    Add,
    Format,
}

/// What to do to the names, as typed in the sheet.
#[derive(Clone, Debug)]
struct Edit {
    mode: Mode,
    find: String,
    replace: String,
    add: String,
    format: String,
    /// Add Text and Format: before the name (or the number before the
    /// format's text) instead of after.
    before: bool,
    /// Format: numbers as 00001 (Finder's counter) instead of 1 (its index).
    counter: bool,
    start: u64,
}

/// A name's own part and its extension (".txt", or "" for folders, which
/// are all name, and names without one).
fn split(name: &str, folder: bool) -> (&str, &str) {
    match name.rfind('.').filter(|&d| d > 0 && !folder) {
        Some(d) => name.split_at(d),
        None => (name, ""),
    }
}

/// The new name for an item named `name`, the `index`th renamed (from 0).
fn new_name(edit: &Edit, name: &str, folder: bool, index: u64) -> String {
    let (stem, ext) = split(name, folder);
    match edit.mode {
        Mode::Replace if edit.find.is_empty() => name.to_string(),
        Mode::Replace => name.replace(&edit.find, &edit.replace),
        Mode::Add if edit.before => format!("{}{name}", edit.add),
        Mode::Add => format!("{stem}{}{ext}", edit.add),
        Mode::Format => {
            let n = edit.start + index;
            let n = if edit.counter { format!("{n:05}") } else { n.to_string() };
            if edit.before { format!("{n}{}{ext}", edit.format) } else { format!("{}{n}{ext}", edit.format) }
        }
    }
}

/// What renaming would do.
#[derive(Default)]
struct Plan {
    /// (from, to) for each item whose name changes, in order.
    changes: Vec<(PathBuf, PathBuf)>,
    /// Why it can't be done, if so.
    problem: Option<String>,
    /// Some new names are others' old ones: rename through stand-in names.
    chained: bool,
}

/// Work out the new names of `items` (path, and whether it's a folder that's
/// all name), checking they can all be had.
fn plan(edit: &Edit, items: &[(PathBuf, bool)]) -> Plan {
    let mut plan = Plan::default();
    for (i, (path, folder)) in items.iter().enumerate() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let new = new_name(edit, &name, *folder, i as u64);
        if new == name {
            continue;
        }
        // (As text: in a path, a "/" would just start another part.)
        let problem = if new.trim().is_empty() {
            Some("A name can't be empty.")
        } else if new.contains(['/', ':']) {
            Some("Names can't contain “/” or “:”.")
        } else if new.starts_with('.') {
            Some("Names can't start with “.”: those are hidden.")
        } else {
            None
        };
        if let Some(problem) = problem {
            return Plan { problem: Some(problem.into()), ..plan };
        }
        plan.changes.push((path.clone(), path.with_file_name(new)));
    }
    plan.problem = check(&mut plan);
    plan
}

fn check(plan: &mut Plan) -> Option<String> {
    // Most Macs' disks don't tell "a" from "A": count those as one name.
    let mut taken = HashSet::new();
    for (_, to) in &plan.changes {
        let name = to.file_name().unwrap_or_default().to_string_lossy();
        if !taken.insert((to.parent().map(Path::to_path_buf), name.to_lowercase())) {
            return Some(format!("More than one item would be named “{name}”."));
        }
    }
    // A new name already on disk is fine if it's one being renamed away.
    let id = |p: &Path| p.symlink_metadata().ok().map(|m| (m.dev(), m.ino()));
    let moving: HashMap<_, usize> = plan.changes.iter().enumerate().filter_map(|(i, (from, _))| Some((id(from)?, i))).collect();
    for (i, (_, to)) in plan.changes.iter().enumerate() {
        match id(to).map(|key| moving.get(&key)) {
            None => {}
            // Itself, with only the case changed.
            Some(Some(&j)) if j == i => {}
            Some(Some(_)) => plan.chained = true,
            Some(None) => {
                let name = to.file_name().unwrap_or_default().to_string_lossy();
                return Some(format!("The name “{name}” is already taken."));
            }
        }
    }
    None
}

/// Rename as planned, all or nothing: if one fails, those done go back.
fn apply(plan: &Plan) -> Result<Op, String> {
    let mut done = Op::default();
    let step = |from: &Path, to: &Path, done: &mut Op| -> io::Result<()> {
        std::fs::rename(from, to)?;
        done.moved.push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    };
    let result = (|| {
        if plan.chained {
            // Out of each other's way first, under hidden stand-in names.
            let stand_ins: Vec<PathBuf> = (0..plan.changes.len())
                .map(|i| plan.changes[i].0.with_file_name(format!(".{}-rename-{}-{i}", crate::APP_NAME, std::process::id())))
                .collect();
            for ((from, _), stand_in) in plan.changes.iter().zip(&stand_ins) {
                step(from, stand_in, &mut done)?;
            }
            for ((_, to), stand_in) in plan.changes.iter().zip(&stand_ins) {
                step(stand_in, to, &mut done)?;
            }
        } else {
            for (from, to) in &plan.changes {
                step(from, to, &mut done)?;
            }
        }
        Ok::<(), io::Error>(())
    })();
    match result {
        Ok(()) => Ok(done),
        Err(e) => {
            let undone = done.revert().err().map(|u| format!(" Some items kept their new names: {u}")).unwrap_or_default();
            Err(format!("The items couldn't be renamed: {e}.{undone}"))
        }
    }
}

pub(super) struct RenameItems {
    /// The items in the order shown, and whether each is a folder that's
    /// all name (not a package like an app, whose extension stays).
    items: Vec<(PathBuf, bool)>,
    mode: Mode,
    before: bool,
    counter: bool,
    find: Entity<InputState>,
    replace: Entity<InputState>,
    add: Entity<InputState>,
    format: Entity<InputState>,
    start: Entity<InputState>,
    plan: Plan,
    _subscriptions: Vec<Subscription>,
}

impl RenameItems {
    fn edit(&self, cx: &App) -> Result<Edit, String> {
        let text = |input: &Entity<InputState>| input.read(cx).value().to_string();
        let start = text(&self.start);
        Ok(Edit {
            mode: self.mode,
            find: text(&self.find),
            replace: text(&self.replace),
            add: text(&self.add),
            format: text(&self.format),
            before: self.before,
            counter: self.counter,
            start: start.trim().parse().map_err(|_| "Numbers start at a whole number, like 1.".to_string())?,
        })
    }

    fn update_plan(&mut self, cx: &App) {
        self.plan = match self.edit(cx) {
            Ok(edit) => plan(&edit, &self.items),
            Err(problem) => Plan { problem: Some(problem), ..Plan::default() },
        };
    }

    /// The field the keyboard starts in for the mode.
    fn first_field(&self) -> &Entity<InputState> {
        match self.mode {
            Mode::Replace => &self.find,
            Mode::Add => &self.add,
            Mode::Format => &self.format,
        }
    }
}

impl Finder {
    /// "Rename…" with several items selected.
    pub(super) fn open_rename_items(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let items: Vec<(PathBuf, bool)> =
            self.rows.iter().filter(|e| paths.contains(&e.path)).map(|e| (e.path.clone(), e.is_dir && !e.is_bundle())).collect();
        if items.len() < 2 {
            if let Some((path, _)) = items.into_iter().next() {
                self.start_rename(path, window, cx);
            }
            return;
        }
        let field = |placeholder: &'static str, value: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let find = field("Text to find", "", window, cx);
        let replace = field("Replace it with", "", window, cx);
        let add = field("Text to add", "", window, cx);
        let format = field("Like “Photo ”", "", window, cx);
        let start = field("1", "1", window, cx);
        let subscriptions = [&find, &replace, &add, &format, &start]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        if let Some(Sheet::RenameItems(r)) = &mut this.overlay {
                            r.update_plan(cx);
                        }
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.finish_rename_items(window, cx),
                    _ => {}
                })
            })
            .collect();
        find.update(cx, |i, cx| i.focus(window, cx));
        let mut sheet = RenameItems {
            items,
            mode: Mode::Replace,
            before: false,
            counter: false,
            find,
            replace,
            add,
            format,
            start,
            plan: Plan::default(),
            _subscriptions: subscriptions,
        };
        sheet.update_plan(cx);
        self.overlay = Some(Sheet::RenameItems(sheet));
        cx.notify();
    }

    fn set_rename_items(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut RenameItems)) {
        let Some(Sheet::RenameItems(r)) = &mut self.overlay else { return };
        let mode = r.mode;
        f(r);
        if r.mode != mode {
            r.first_field().update(cx, |i, cx| i.focus(window, cx));
        }
        r.update_plan(cx);
        cx.notify();
    }

    /// The sheet's Rename (or Return in it).
    pub(super) fn finish_rename_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Sheet::RenameItems(r)) = &self.overlay else { return };
        if r.plan.problem.is_some() {
            return;
        }
        let Some(Sheet::RenameItems(r)) = self.overlay.take() else { return };
        window.focus(&self.focus, cx);
        cx.notify();
        if r.plan.changes.is_empty() {
            return;
        }
        let renamed: Vec<PathBuf> = r.plan.changes.iter().map(|(_, to)| to.clone()).collect();
        let folders: Vec<PathBuf> = renamed.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
        match apply(&r.plan) {
            Ok(op) => {
                self.pending_select = renamed;
                self.record(op);
            }
            Err(e) => self.show_notice(e, cx),
        }
        self.reload_folders(&folders, cx);
    }

    pub(super) fn render_rename_items(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let palette = self.palette;
        let Some(Sheet::RenameItems(r)) = &self.overlay else { return div() };
        let segment = |id: &'static str, label: &'static str, on: bool, f: fn(&mut RenameItems), cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .py(px(3.))
                .rounded_md()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| this.set_rename_items(window, cx, f)))
        };
        let label = |text: &'static str| div().w(px(96.)).flex_shrink_0().text_size(px(12.)).text_color(palette.secondary).child(text);
        let field = |text: &'static str, input: &Entity<InputState>| div().flex().items_center().child(label(text)).child(div().flex_1().child(Input::new(input).small()));
        let row = || div().flex().items_center().gap_1();
        let where_ = |cx: &mut Context<Self>| {
            row()
                .child(label("Where"))
                .child(segment("after", "After Name", !r.before, |r| r.before = false, cx))
                .child(segment("before", "Before Name", r.before, |r| r.before = true, cx))
        };
        let fields = match r.mode {
            Mode::Replace => div().flex().flex_col().gap_2().child(field("Find", &r.find)).child(field("Replace with", &r.replace)),
            Mode::Add => div().flex().flex_col().gap_2().child(field("Add", &r.add)).child(where_(cx)),
            Mode::Format => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    row()
                        .child(label("Numbers"))
                        .child(segment("index", "1, 2, 3", !r.counter, |r| r.counter = false, cx))
                        .child(segment("counter", "00001, 00002", r.counter, |r| r.counter = true, cx)),
                )
                .child(where_(cx))
                .child(field("Text", &r.format))
                .child(div().flex().items_center().child(label("Start at")).child(div().w(px(80.)).child(Input::new(&r.start).small()))),
        };
        let name = |p: &Path| p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let changes = &r.plan.changes;
        let preview = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p_2()
            .rounded_md()
            .bg(palette.glass)
            .text_size(px(12.))
            .when(changes.is_empty(), |d| d.child(div().text_color(palette.tertiary).child("No names change yet.")))
            .children(changes.iter().take(SHOWN).map(|(from, to)| {
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().min_w_0().truncate().text_color(palette.secondary).child(name(from)))
                    .child(div().text_color(palette.tertiary).child("→"))
                    .child(div().flex_1().min_w_0().truncate().child(name(to)))
            }))
            .when(changes.len() > SHOWN, |d| {
                d.child(div().text_color(palette.tertiary).child(format!("and {} more", changes.len() - SHOWN)))
            });
        let problem = r.plan.problem.clone();
        let ready = problem.is_none() && !changes.is_empty();
        let title = format!("Rename {} Items", r.items.len());
        let mode = r.mode;
        card(&palette)
            .w(px(480.))
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(
                row()
                    .child(segment("replace", "Replace Text", mode == Mode::Replace, |r| r.mode = Mode::Replace, cx))
                    .child(segment("add", "Add Text", mode == Mode::Add, |r| r.mode = Mode::Add, cx))
                    .child(segment("format", "Format", mode == Mode::Format, |r| r.mode = Mode::Format, cx)),
            )
            .child(fields)
            .child(preview)
            .when_some(problem, |d, problem| d.child(div().text_size(px(12.)).text_color(rgb(0xff453a)).child(problem)))
            .child(
                buttons()
                    .child(button("cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx)))
                    .child(
                        button("rename", "Rename", true, &palette, cx, |this, window, cx| this.finish_rename_items(window, cx))
                            .when(!ready, |d| d.opacity(0.5)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    // (Not super::*: that brings in gpui's own `test`.)
    use super::{Edit, Mode, Path, PathBuf, apply, new_name, plan};

    fn edit(mode: Mode) -> Edit {
        Edit {
            mode,
            find: String::new(),
            replace: String::new(),
            add: String::new(),
            format: String::new(),
            before: false,
            counter: false,
            start: 1,
        }
    }

    #[test]
    fn names() {
        let replace = Edit { find: "IMG".into(), replace: "Photo".into(), ..edit(Mode::Replace) };
        assert_eq!(new_name(&replace, "IMG_01 IMG.jpg", false, 0), "Photo_01 Photo.jpg");
        // Extensions too, as Finder does.
        let ext = Edit { find: ".jpeg".into(), replace: ".jpg".into(), ..edit(Mode::Replace) };
        assert_eq!(new_name(&ext, "a.jpeg", false, 0), "a.jpg");
        assert_eq!(new_name(&edit(Mode::Replace), "a.txt", false, 0), "a.txt");

        let add = Edit { add: " final".into(), ..edit(Mode::Add) };
        assert_eq!(new_name(&add, "report.pdf", false, 0), "report final.pdf");
        assert_eq!(new_name(&add, "v1.2 notes", true, 0), "v1.2 notes final");
        assert_eq!(new_name(&add, ".env", false, 0), ".env final");
        let before = Edit { add: "old ".into(), before: true, ..edit(Mode::Add) };
        assert_eq!(new_name(&before, "report.pdf", false, 0), "old report.pdf");

        let format = Edit { format: "Photo ".into(), start: 5, ..edit(Mode::Format) };
        assert_eq!(new_name(&format, "IMG_9.HEIC", false, 2), "Photo 7.HEIC");
        assert_eq!(new_name(&format, "Some.app", false, 0), "Photo 5.app");
        let counter = Edit { format: " copy".into(), counter: true, before: true, ..edit(Mode::Format) };
        assert_eq!(new_name(&counter, "x.txt", false, 0), "00001 copy.txt");
    }

    fn scratch(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rename-items-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            std::fs::write(dir.join(f), f).unwrap();
        }
        dir
    }

    fn listed(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn renames_through_each_others_names() {
        // 1 → 2 and 2 → 3: each new name is the next one's old name.
        let dir = scratch("chain", &["File1.txt", "File2.txt"]);
        let items = vec![(dir.join("File1.txt"), false), (dir.join("File2.txt"), false)];
        let p = plan(&Edit { format: "File".into(), start: 2, ..edit(Mode::Format) }, &items);
        assert!(p.problem.is_none() && p.chained);
        let op = apply(&p).unwrap();
        assert_eq!(listed(&dir), ["File2.txt", "File3.txt"]);
        assert_eq!(std::fs::read_to_string(dir.join("File3.txt")).unwrap(), "File2.txt");
        // Undo puts all of it back.
        op.revert().unwrap();
        assert_eq!(listed(&dir), ["File1.txt", "File2.txt"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_names_in_use() {
        let dir = scratch("taken", &["a.txt", "b.txt", "keep.txt"]);
        let items = vec![(dir.join("a.txt"), false), (dir.join("b.txt"), false)];
        // "a" → "keep": taken by an item not being renamed.
        let p = plan(&Edit { find: "a".into(), replace: "keep".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.unwrap().contains("already taken"));
        // "a" → "b" while "b" stays: it isn't moving out of the way.
        let p = plan(&Edit { find: "a".into(), replace: "b".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.unwrap().contains("already taken"));
        // Only "a" changes; "b" is left be.
        let p = plan(&Edit { find: "a".into(), replace: "c".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.is_none() && p.changes.len() == 1);
        let p = plan(&Edit { add: "x".into(), before: true, ..edit(Mode::Add) }, &items);
        assert!(p.problem.is_none() && p.changes.len() == 2);
        let p = plan(&Edit { find: ".txt".into(), replace: "/".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.unwrap().contains("“/”"));
        let p = plan(&Edit { find: "a.txt".into(), replace: "".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.unwrap().contains("empty"));
        let p = plan(&Edit { add: ".".into(), before: true, ..edit(Mode::Add) }, &items);
        assert!(p.problem.unwrap().contains("hidden"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn case_only_and_duplicates() {
        let dir = scratch("case", &["a.txt", "A2.txt"]);
        let items = vec![(dir.join("a.txt"), false)];
        let p = plan(&Edit { find: "a".into(), replace: "A".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.is_none() && !p.chained);
        apply(&p).unwrap();
        assert!(listed(&dir).contains(&"A.txt".to_string()));
        // Two items to one name, differing only in case.
        let items = vec![(dir.join("A.txt"), false), (dir.join("A2.txt"), false)];
        let p = plan(&Edit { find: "2".into(), replace: "".into(), ..edit(Mode::Replace) }, &items);
        assert!(p.problem.is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

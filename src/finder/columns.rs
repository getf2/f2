//! Column view (⌘3), as Finder's: a column per folder from where you
//! started down to the one with the keyboard, then what's in the selected
//! folder (or a preview of the selected file). → goes into the selected
//! folder's column, ← back out; clicking any item shows it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::dnd::{Dragged, drop_target};
use super::items::file_image;
use super::{Finder, display_name};
use crate::listing::{self, Entry};
use crate::settings::Settings;
use crate::style::{SEMIBOLD, symbol};

const COLUMN_WIDTH: f32 = 230.;
const ROW: f32 = 22.;

/// One column: a folder and what's in it (the keyboard's column shows the
/// live rows instead).
struct Col {
    dir: PathBuf,
    entries: Vec<Entry>,
    /// The item picked in it (on the way to the keyboard's column).
    picked: Option<PathBuf>,
    active: bool,
}

impl Finder {
    /// In column view: show `dir` as the keyboard's column (no history:
    /// these are moves between columns).
    pub(super) fn column_into(&mut self, dir: PathBuf, select: Option<PathBuf>, cx: &mut Context<Self>) {
        // The column being left keeps showing what it held.
        if self.listed.as_ref() == Some(&self.cwd) {
            self.col_cache.insert(self.cwd.clone(), self.rows.clone());
        }
        if !dir.starts_with(&self.col_base) {
            self.col_base = dir.clone();
        }
        // Within the same column, what's picked is all that's selected
        // (else it would join what was).
        if self.listed.as_ref() == Some(&dir) {
            let ix = select.as_ref().and_then(|p| self.rows.iter().position(|e| e.path == *p));
            self.select_only(ix);
        }
        self.cwd = dir;
        self.virtual_folder = None;
        self.load(select, cx);
        cx.notify();
    }

    /// Whether `dir` is already open as a column (so a drag over it
    /// doesn't spring it open again).
    pub(super) fn column_open(&self, dir: &Path) -> bool {
        (self.cwd.starts_with(dir) && dir.starts_with(&self.col_base))
            || (self.chosen.len() == 1 && self.selected_entry().is_some_and(|e| e.path == dir))
    }

    /// → into the selected folder, ← out to the enclosing one.
    pub(super) fn column_step(&mut self, right: bool, _: &mut Window, cx: &mut Context<Self>) {
        if self.flat() {
            return;
        }
        if right {
            let Some(entry) = self.selected_entry().filter(|e| e.is_dir && !e.is_bundle()).cloned() else { return };
            let first = self.column_entries(&entry.path, cx).and_then(|list| list.first().map(|e| e.path.clone()));
            self.column_into(entry.path, first, cx);
        } else if let Some(parent) = self.cwd.parent().map(Path::to_path_buf) {
            if self.cwd == self.col_base {
                self.col_base = parent.clone();
            }
            let from = self.cwd.clone();
            self.column_into(parent, Some(from), cx);
        }
    }

    /// A folder's items for a column, sorted as shown; listed off the main
    /// thread the first time (then `None` until they're in).
    fn column_entries(&mut self, dir: &Path, cx: &mut Context<Self>) -> Option<Vec<Entry>> {
        if let Some(entries) = self.col_cache.get(dir) {
            return Some(entries.clone());
        }
        // (An empty placeholder, so it's asked for once.)
        self.col_cache.insert(dir.to_path_buf(), Vec::new());
        let target = dir.to_path_buf();
        let (show_hidden, sort, folders_on_top) = (self.show_hidden, self.sort, Settings::get(cx).folders_on_top);
        cx.spawn(async move |this, cx| {
            let key = target.clone();
            let sizes = Default::default();
            let entries = cx
                .background_executor()
                .spawn(async move {
                    let mut entries: Vec<Entry> =
                        listing::list_dir(&target).unwrap_or_default().into_iter().filter(|e| show_hidden || !e.hidden).collect();
                    listing::sort(&mut entries, sort.0, sort.1, folders_on_top, &sizes);
                    entries
                })
                .await;
            this.update(cx, |this, cx| {
                this.col_cache.insert(key, entries);
                this.watch_git(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        None
    }

    pub(super) fn render_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.cwd.starts_with(&self.col_base) {
            self.col_base = self.cwd.clone();
        }
        // The folders from the first column down to the keyboard's.
        let mut dirs: Vec<PathBuf> = self.cwd.ancestors().take_while(|p| p.starts_with(&self.col_base)).map(Path::to_path_buf).collect();
        dirs.reverse();
        let mut cols = Vec::new();
        for (i, dir) in dirs.iter().enumerate() {
            let active = i + 1 == dirs.len();
            let entries = if active { Vec::new() } else { self.column_entries(dir, cx).unwrap_or_default() };
            let picked = dirs.get(i + 1).cloned();
            cols.push(Col { dir: dir.clone(), entries, picked, active });
        }
        // Then what's in the selected folder, or the selected file's preview.
        let one = (self.chosen.len() == 1).then(|| self.selected_entry().cloned()).flatten();
        let next = match &one {
            Some(e) if e.is_dir && !e.is_bundle() => {
                let entries = self.column_entries(&e.path, cx).unwrap_or_default();
                Some(Col { dir: e.path.clone(), entries, picked: None, active: false })
            }
            _ => None,
        };
        let seen = (self.cwd.clone(), one.as_ref().map(|e| e.path.clone()));
        let mut children: Vec<AnyElement> = cols.into_iter().map(|col| self.render_column(col, window, cx)).collect();
        match (next, one) {
            (Some(col), _) => children.push(self.render_column(col, window, cx)),
            (None, Some(file)) => children.push(self.render_column_preview(&file, window)),
            _ => {}
        }
        // Bring the last columns into view when moving along.
        if self.col_seen.as_ref() != Some(&seen) {
            self.col_seen = Some(seen);
            self.col_scroll.scroll_to_item(children.len().saturating_sub(1));
        }
        div()
            .id("columns")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .overflow_x_scroll()
            .track_scroll(&self.col_scroll)
            .children(children)
            .into_any_element()
    }

    fn render_column(&mut self, col: Col, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let focused = self.focus.is_focused(window);
        let icon_px = (16. * window.scale_factor()).round() as u32;
        let show_all = Settings::get(cx).show_all_extensions;
        let dir = col.dir.clone();
        let selection = if col.active { self.dragged_selection() } else { None };
        let entries: Vec<(Entry, bool)> = if col.active {
            self.rows.iter().enumerate().map(|(ix, e)| (e.clone(), self.is_chosen(ix))).collect()
        } else {
            col.entries.iter().map(|e| (e.clone(), col.picked.as_ref() == Some(&e.path))).collect()
        };
        let rows: Vec<AnyElement> = entries
            .into_iter()
            .enumerate()
            .map(|(ix, (entry, on))| {
                let shown = self.icons.get(&entry, icon_px);
                let (bg, text) = match (on, col.active && focused) {
                    (true, true) => (Some(palette.selection), palette.selected_text),
                    (true, false) => (Some(palette.selection_inactive), palette.label),
                    _ => (None, palette.label),
                };
                let folder = entry.is_dir && !entry.is_bundle();
                let (path, dir) = (entry.path.clone(), dir.clone());
                let active = col.active;
                // (The keyboard's column lists the rows, in order.)
                let row_ix = active.then_some(ix);
                // Its items drag the selection, as in the list; the others
                // just themselves.
                let dragged = match row_ix {
                    Some(ix) => self.dragged_from(ix, &selection, window),
                    None => Dragged {
                        paths: Arc::new([entry.path.clone()]),
                        icon: shown.as_ref().map(|s| s.image.clone()),
                        name: entry.name.clone(),
                        palette,
                        lead: None,
                        grab: Point::default(),
                    },
                };
                let target = path.clone();
                if folder {
                    self.on_screen.push(target.clone());
                }
                let row = div()
                    .id(SharedString::from(format!("col-{}-{ix}", dir.display())))
                    .h(px(ROW))
                    .mx_1()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .relative()
                    .rounded(px(5.))
                    .when_some(bg, |d, bg| d.bg(bg))
                    .text_color(text)
                    .when(entry.hidden || super::transfer::is_cut(&entry.path), |d| d.opacity(0.5))
                    .role(Role::ListItem)
                    .aria_label(entry.name.clone())
                    .when(folder, |d| d.children(self.spot_motion(&super::motion::Spot::Folder(target.clone()), 5.)))
                    .child(file_image(shown, &entry, 16., palette.accent))
                    .child(div().flex_1().min_w_0().truncate().child(entry.display_name(show_all)))
                    .child(super::items::tag_dots(&entry, palette.background))
                    .children(super::items::git_badge(
                        self.git_marks.get(&entry.path).copied(),
                        entry.is_dir,
                        (on && active && focused).then_some(text),
                    ))
                    .when(folder, |d| d.child(symbol("chevron.right", 9., SEMIBOLD, if on && active && focused { text } else { palette.tertiary }, window)))
                    // (So the press isn't taken as one on empty space, which
                    // would clear the selection and with it this column.)
                    .on_mouse_down(MouseButton::Left, self.on_item_press(cx))
                    .when_some(row_ix, |d, ix| d.on_mouse_down(MouseButton::Right, self.on_item_menu(ix, cx)))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        window.focus(&this.focus, cx);
                        if active {
                            if let Some(ix) = this.rows.iter().position(|e| e.path == path) {
                                this.select_only(Some(ix));
                                if event.click_count() >= 2 {
                                    this.open_selected(window, cx);
                                }
                            }
                            cx.notify();
                        } else {
                            // An item in another column: its column gets the
                            // keyboard, with it selected.
                            this.column_into(dir.clone(), Some(path.clone()), cx);
                        }
                    }))
                    .on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
                    .external_drag_payload(super::dnd::files_for_other_apps);
                let dim = if entry.hidden || super::transfer::is_cut(&entry.path) { 0.5 } else { 1. };
                if folder { self.item_motion(&target, dim, drop_target(row, &target, palette.selection.opacity(0.35), cx)) } else { self.item_motion(&target, dim, row) }
            })
            .collect();
        let empty = rows.is_empty();
        let (here, active) = (col.dir.clone(), col.active);
        let column = div()
            .id(SharedString::from(format!("column-{}", col.dir.display())))
            // Empty space in another column: that column gets the keyboard,
            // nothing in it selected (as in Finder).
            .when(!active, |d| {
                d.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        // (An item in it took the press: leave the flag for
                        // the space under the columns, which clears it.)
                        if this.pressed_item {
                            return;
                        }
                        this.pressed_item = true;
                        window.focus(&this.focus, cx);
                        this.column_into(here.clone(), None, cx);
                    }),
                )
            })
            .w(px(COLUMN_WIDTH))
            .flex_shrink_0()
            .h_full()
            .py_1()
            .border_r_1()
            .border_color(palette.separator)
            .overflow_y_scroll()
            .role(Role::List)
            .aria_label(display_name(&col.dir))
            .children(rows)
            .when(empty, |d| d.child(div().pt_4().flex().justify_center().text_color(palette.tertiary).text_size(px(12.)).child("Empty")));
        // Dropped on a column's empty space: into its folder.
        drop_target(column, &col.dir, palette.selection.opacity(0.12), cx).into_any_element()
    }

    /// The last column when a file is selected: Finder's preview of it.
    fn render_column_preview(&mut self, entry: &Entry, window: &mut Window) -> AnyElement {
        let palette = self.palette;
        let shown = self.icons.get(entry, (200. * window.scale_factor()).round() as u32);
        let details = format!("{} · {}", entry.kind_label(), entry.size_label());
        div()
            .w(px(COLUMN_WIDTH + 40.))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .child(div().size(px(200.)).flex().items_center().justify_center().child(file_image(shown, entry, 180., palette.accent)))
            .child(div().max_w_full().text_center().font_weight(FontWeight::SEMIBOLD).child(entry.name.clone()))
            .child(div().text_size(px(11.)).text_color(palette.secondary).child(details))
            .child(div().text_size(px(11.)).text_color(palette.secondary).child(format!("Modified {}", entry.mtime_label())))
            .into_any_element()
    }
}

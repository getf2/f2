//! The files: Finder's list (columns you choose, resize and reorder; striped
//! rows; folders that expand in place; groups), the icon view (any icon
//! size), and the column view's place among them.

use std::ops::Range;
use std::path::Path;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Icon, Sizable as _};

use super::dnd::drop_target;
use super::motion::Spot;
use super::menu::Target;
use super::{Finder, Line, View, home};
use crate::icons::Shown;
use crate::listing::{Entry, date_label, human_size};
use crate::macos::{self, MenuItem};
use crate::settings::{Column, Settings};
use crate::style::{MEDIUM, SEMIBOLD, symbol, tag_color};

pub(super) const ROW_HEIGHT: f32 = 20.;
const HEADER_HEIGHT: f32 = 28.;
/// Rows are rounded and stand in this far from the sides (Finder's 10).
const ROW_INSET: f32 = 10.;
const DISCLOSURE: f32 = 16.;
const LIST_ICON: f32 = 16.;
/// The narrowest the list's Name column gets: columns after it are left out
/// first (see `fitting_columns`).
const NAME_MIN: f32 = 180.;
const LOCATION_WIDTH: f32 = 220.;
pub(super) const GRID_PADDING: f32 = 8.;
/// Grouped icon views aren't virtualized; past this many, they aren't
/// grouped.
const GROUPED_GRID_LIMIT: usize = 3000;

/// An icon view cell for icons of `icon` points.
pub(super) fn cell_width(icon: f32) -> f32 {
    (icon + 44.).max(90.)
}

pub(super) fn cell_height(icon: f32) -> f32 {
    icon + 48.
}

/// For when macOS has no icon to give.
fn fallback_icon(entry: &Entry) -> IconName {
    if entry.link.is_some() {
        IconName::Globe
    } else if entry.is_bundle() {
        IconName::AppWindow
    } else if entry.is_dir {
        IconName::Folder
    } else {
        IconName::File
    }
}

/// The file's icon or thumbnail, `size` points square.
pub fn file_image(shown: Option<Shown>, entry: &Entry, size: f32, color: Hsla) -> AnyElement {
    match shown {
        Some(shown) if entry.link.is_none() => {
            img(shown.image).size(px(size)).flex_shrink_0().object_fit(ObjectFit::Contain).into_any_element()
        }
        _ => Icon::new(fallback_icon(entry)).size(px(size)).text_color(color).into_any_element(),
    }
}

/// A folder's path, home written as ~.
fn tilde(dir: &Path) -> SharedString {
    match dir.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()).into(),
        Err(_) => dir.display().to_string().into(),
    }
}

/// Where an item is (a search result, say): its folder.
pub(crate) fn location(path: &Path) -> SharedString {
    tilde(path.parent().unwrap_or(path))
}

/// The name field of an item being renamed; clicks in it stay in it.
fn rename_field(input: &Entity<InputState>, height: f32) -> Div {
    div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(Input::new(input).small().h(px(height)))
}

/// An item's tags, as Finder's small colored dots.
pub(super) fn tag_dots(entry: &Entry, ring: Hsla) -> impl IntoElement + use<> {
    div().flex().flex_shrink_0().children(entry.tags.iter().filter_map(|(_, color)| tag_color(*color)).enumerate().map(|(i, color)| {
        div().size(px(9.)).rounded_full().bg(color).border_1().border_color(ring).when(i > 0, |d| d.ml(px(-3.)))
    }))
}

/// Git's mark, as editors show it: a letter for a file (Modified, Added,
/// Untracked...), a dot for a folder with changes inside. `on` is the
/// color to use on a selection.
pub(super) fn git_badge(status: Option<u8>, dir: bool, on: Option<Hsla>) -> Option<AnyElement> {
    let (letter, color) = match fsearch::git::word(status?)? {
        "conflict" => ("!", rgb(0xff453a)),
        "untracked" => ("U", rgb(0x30d158)),
        "added" => ("A", rgb(0x30d158)),
        "modified" => ("M", rgb(0xff9f0a)),
        "renamed" => ("R", rgb(0x0a84ff)),
        "deleted" => ("D", rgb(0xff453a)),
        "staged" => ("S", rgb(0x30d158)),
        _ => return None,
    };
    let color = on.unwrap_or(color.into());
    let badge = if dir {
        div().size(px(7.)).rounded_full().bg(color)
    } else {
        div().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(color).child(letter)
    };
    Some(badge.flex_shrink_0().into_any_element())
}

/// iCloud's badge for items not (yet) here.
fn cloud_badge(entry: &Entry, color: Hsla, window: &Window) -> Option<AnyElement> {
    let name = match entry.cloud? {
        macos::Cloud::NotDownloaded => "icloud.and.arrow.down",
        macos::Cloud::Downloading => "arrow.down.circle",
        macos::Cloud::Uploading => "icloud.and.arrow.up",
        macos::Cloud::Current => return None,
    };
    Some(symbol(name, 11., MEDIUM, color, window))
}

/// A list column being dragged to a new place.
#[derive(Clone)]
struct DraggedColumn(Column);

impl Render for DraggedColumn {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().px_2().py_1().rounded_md().bg(black().opacity(0.6)).text_color(white()).text_size(px(12.)).child(self.0.title())
    }
}

impl Finder {
    pub(super) fn render_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // (Filled in again as the rows are drawn: see `watch_git`.)
        self.on_screen.clear();
        if self.virtual_folder == Some(super::places::Virtual::AirDrop) && !self.searching {
            return self.render_airdrop(window, cx);
        }
        if self.connecting.is_some() {
            return self.render_connecting(window, cx);
        }
        let palette = self.palette;
        let loading = !self.flat() && self.listed.as_ref() != Some(&self.cwd);
        let empty = match (&self.error, self.flat()) {
            (Some(e), _) => Some(e.clone()),
            // (Column view keeps its columns while the next one loads.)
            (None, false) if loading && self.view != View::Columns => Some("Loading…".into()),
            (None, true) if self.rows.is_empty() && self.virtual_folder.is_some() && !self.searching => Some("Looking…".into()),
            (None, true) if self.rows.is_empty() => Some("No results".into()),
            (None, false) if self.rows.is_empty() && !self.listing.is_empty() => Some("Only hidden items here".into()),
            (None, false) if self.rows.is_empty() && self.view != View::Columns => Some("Folder is empty".into()),
            _ => None,
        };
        let view = if self.flat() && self.view == View::Columns { View::List } else { self.view };
        let columns = view == View::Columns && empty.is_none();
        let list = match empty {
            Some(message) => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px_6()
                .text_center()
                .text_color(palette.tertiary)
                .child(message)
                .into_any_element(),
            None if view == View::Columns => self.render_columns(window, cx),
            None if view == View::Icons => self.render_icons(window, cx),
            None if view == View::Gallery => self.render_gallery(window, cx),
            None => {
                let lines = if self.lines.is_empty() { self.rows.len() } else { self.lines.len() };
                // Finder's stripes go on past the last row, to the bottom
                // (from the item after the last one, as the rows have them).
                let row_height = (self.text_size + 7.).max(ROW_HEIGHT);
                let next = match self.lines.last() {
                    Some(Line::Item(ix)) => ix + 1,
                    _ => lines,
                };
                let scroll = self.scroll.clone();
                let stripes = canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        // (Cut off at the list's bottom: the last one, part
                        // of it, not over the path bar.)
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            let mut top = bounds.top() + scroll.0.borrow().base_handle.offset().y + px(lines as f32 * row_height);
                            let mut n = next;
                            while top < bounds.bottom() {
                                if n % 2 == 0 {
                                    let row = Bounds::new(point(bounds.left() + px(ROW_INSET), top), size(bounds.size.width - px(2. * ROW_INSET), px(row_height)));
                                    window.paint_quad(fill(row, palette.stripe).corner_radii(px(6.)));
                                }
                                top += px(row_height);
                                n += 1;
                            }
                        });
                    },
                )
                .absolute()
                .size_full();
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(stripes)
                    .child(
                        uniform_list("rows", lines, cx.processor(Self::render_rows))
                            .track_scroll(&self.scroll)
                            .size_full()
                            .pb_1(),
                    )
                    .vertical_scrollbar(&self.scroll)
                    .into_any_element()
            }
        };
        // The folder's empty space: a click there selects nothing, a drop
        // there lands in the folder, a right-click is the folder's menu.
        let space = div()
            .id("space")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .role(Role::List)
            .aria_label(self.title(cx))
            .child(list)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if !std::mem::take(&mut this.pressed_item) {
                        this.select_only(None);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, window, cx| this.show_menu(Target::Background, window, cx)));
        // (Columns take drops themselves, each into its folder.)
        let space = if self.flat() || columns { space } else { drop_target(space, &self.cwd.clone(), palette.selection.opacity(0.12), cx) };
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .when(view == View::List, |d| d.child(self.render_header(window, cx)))
            .child(space)
            // A list column being resized follows the mouse.
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some((col, start_x, start_width)) = this.resizing {
                    if e.pressed_button != Some(MouseButton::Left) {
                        this.resizing = None;
                        return;
                    }
                    let width = (start_width + start_x - f32::from(e.position.x)).clamp(60., 500.);
                    Settings::update(cx, |s| {
                        if let Some(c) = s.columns.iter_mut().find(|(c, _)| *c == col) {
                            c.1 = width;
                        }
                    });
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.resizing = None))
            .into_any_element()
    }

    /// Found items get a Location column; the Trash's don't (they're all
    /// in a Trash).
    fn shows_location(&self) -> bool {
        self.flat() && self.virtual_folder != Some(super::places::Virtual::Trash)
    }

    /// A press on an item, so the empty space under it doesn't clear the
    /// selection (and, the window in the background, it stays there while
    /// the item's dragged off to another app).
    pub(super) fn on_item_press(&self, cx: &mut Context<Self>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        cx.listener(|this, _, _, _| {
            this.pressed_item = true;
            crate::macos::item_pressed();
        })
    }

    /// Right-click (or control-click) on an item: select it and open its
    /// menu.
    pub(super) fn on_item_menu(&self, ix: usize, cx: &mut Context<Self>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            if e.button == MouseButton::Left && !e.modifiers.control {
                return;
            }
            cx.stop_propagation();
            // The menu is for the selection if it's in it, else just for it.
            if !this.is_chosen(ix) {
                this.select_only(Some(ix));
            }
            window.focus(&this.focus, cx);
            this.show_menu(Target::Selection, window, cx);
            cx.notify();
        })
    }

    /// The columns' menu: which ones show.
    fn show_columns_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown: Vec<Column> = Settings::get(cx).columns.iter().map(|(c, _)| *c).collect();
        let items: Vec<MenuItem> = Column::ALL
            .iter()
            .enumerate()
            .map(|(i, c)| MenuItem { title: Some(c.title().into()), tag: i as i32, enabled: true, checked: shown.contains(c), ..Default::default() })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let Some(tag) = macos::popup_menu(&items) else { return };
            this.update(cx, |this, cx| this.toggle_column(Column::ALL[tag as usize], cx)).ok();
        })
        .detach();
    }

    /// The list's columns that fit beside a Name column NAME_MIN wide: the
    /// rest, from the right, are left out while the pane's that narrow.
    fn fitting_columns(&self, cx: &App) -> Vec<(Column, f32)> {
        let location = if self.shows_location() { LOCATION_WIDTH } else { 0. };
        let mut room = self.list_width - NAME_MIN - location - (DISCLOSURE + LIST_ICON + 6.) - 2. * (ROW_INSET + 4.);
        Settings::get(cx).columns.iter().copied().take_while(|(_, width)| {
            room -= width;
            room >= 0.
        }).collect()
    }

    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let columns = self.fitting_columns(cx);
        let (key, ascending) = match (self.flat(), self.search_sort) {
            (true, Some(s)) => (Some(s.0), s.1),
            (true, None) => (None, true),
            (false, _) => (Some(self.sort.0), self.sort.1),
        };
        let arrow = |k| (key == Some(k)).then(|| symbol(if ascending { "chevron.up" } else { "chevron.down" }, 9., SEMIBOLD, palette.secondary, window));
        let name = div()
            .id("h-name")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .pl(px(DISCLOSURE + LIST_ICON + 6.))
            .pr_2()
            .child(div().flex_1().truncate().child("Name"))
            .children(arrow(crate::listing::SortKey::Name))
            .on_click(cx.listener(|this, _, _, cx| this.set_sort(crate::listing::SortKey::Name, cx)));
        let cells: Vec<AnyElement> = columns
            .iter()
            .map(|&(col, width)| {
                let sort = col.sort_key();
                div()
                    .id(SharedString::from(format!("h-{col:?}")))
                    .w(px(width))
                    .flex_shrink_0()
                    .h_full()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_l_1()
                    .border_color(palette.separator)
                    .pl_2()
                    .pr_2()
                    .child(div().flex_1().truncate().child(col.title()))
                    .children(sort.and_then(arrow))
                    // Drag the edge to resize; the heading, to reorder.
                    .child(
                        div()
                            .id(SharedString::from(format!("resize-{col:?}")))
                            .absolute()
                            .left(px(-3.))
                            .top_0()
                            .h_full()
                            .w(px(6.))
                            .cursor_col_resize()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.resizing = Some((col, f32::from(e.position.x), width));
                                }),
                            ),
                    )
                    .on_drag(DraggedColumn(col), |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<DraggedColumn>(move |s, _, _, _| s.bg(palette.glass_selected))
                    .on_drop(cx.listener(move |_, d: &DraggedColumn, _, cx| {
                        Settings::update(cx, |s| {
                            if let Some(from) = s.columns.iter().position(|(c, _)| *c == d.0) {
                                let item = s.columns.remove(from);
                                let to = s.columns.iter().position(|(c, _)| *c == col).unwrap_or(s.columns.len());
                                s.columns.insert(to, item);
                            }
                        })
                    }))
                    .when_some(sort, |d, k| d.on_click(cx.listener(move |this, _, _, cx| this.set_sort(k, cx))))
                    .into_any_element()
            })
            .collect();
        div()
            .id("header")
            .h(px(HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(ROW_INSET + 4.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.secondary)
            .border_b_1()
            .border_color(palette.separator)
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, window, cx| this.show_columns_menu(window, cx)))
            .child(name)
            .children(cells)
            .when(self.shows_location(), |d| d.child(div().w(px(LOCATION_WIDTH)).flex_shrink_0().border_l_1().border_color(palette.separator).pl_2().child("Location")))
    }

    fn render_rows(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = self.palette;
        let icon_px = (LIST_ICON * window.scale_factor()).round() as u32;
        let focused = self.focus.is_focused(window);
        let selection = self.dragged_selection();
        let columns = self.fitting_columns(cx);
        let show_all = Settings::get(cx).show_all_extensions;
        let row_height = (self.text_size + 7.).max(ROW_HEIGHT);
        let mut rows = Vec::new();
        for line in range {
            let ix = if self.lines.is_empty() {
                line
            } else {
                match self.lines.get(line) {
                    Some(Line::Item(ix)) => *ix,
                    Some(Line::Header(title)) => {
                        rows.push(
                            div()
                                .id(("group", line))
                                .w_full()
                                .h(px(row_height))
                                .px(px(ROW_INSET + 4.))
                                .child(
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_end()
                                        .pb(px(2.))
                                        .border_b_1()
                                        .border_color(palette.separator)
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(palette.secondary)
                                        .child(title.clone()),
                                )
                                .into_any_element(),
                        );
                        continue;
                    }
                    None => continue,
                }
            };
            if ix >= self.rows.len() {
                continue;
            }
            let dragged = self.dragged_from(ix, &selection, window);
            let entry = &self.rows[ix];
            let selected = self.is_chosen(ix);
            // Selected neighbors join into one rounded block.
            let (joined_above, joined_below) = (selected && ix > 0 && self.is_chosen(ix - 1), selected && self.is_chosen(ix + 1));
            let (text, meta) = match (selected, focused) {
                (true, true) => (palette.selected_text, palette.selected_text),
                _ => (palette.label, palette.secondary),
            };
            let expandable = !self.flat() && entry.is_dir && !entry.is_bundle();
            let expanded = expandable && self.expanded.contains(&entry.path);
            let shown = self.icons.get(entry, icon_px);
            let renaming = self.renaming.as_ref().filter(|r| r.path == entry.path).map(|r| r.input.clone());
            let disclosure = div()
                .id(("disclosure", ix))
                .w(px(DISCLOSURE))
                .h_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .when(expandable, |d| {
                    d.child(symbol(if expanded { "chevron.down" } else { "chevron.right" }, 9., SEMIBOLD, meta, window))
                        // On press, as Finder does; and not a click on the row.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.set_expanded(ix, !expanded, cx);
                                cx.stop_propagation();
                            }),
                        )
                });
            let name = div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(6.))
                .pl(px(entry.depth as f32 * DISCLOSURE))
                .child(disclosure)
                .child(file_image(shown, entry, LIST_ICON, palette.accent))
                .child(match renaming {
                    Some(input) => rename_field(&input, row_height - 2.).flex_1().min_w(px(120.)).into_any_element(),
                    None => div().flex_shrink(1.).min_w(px(40.)).truncate().child(entry.display_name(show_all)).into_any_element(),
                })
                .child(tag_dots(entry, palette.background))
                .children(git_badge(self.git_marks.get(&entry.path).copied(), entry.is_dir, (selected && focused).then_some(text)))
                .children(cloud_badge(entry, meta, window))
                .when_some(entry.detail.clone(), |d, line| d.child(div().flex_1().min_w_0().truncate().text_color(meta).child(line)));
            let cells: Vec<AnyElement> = columns
                .iter()
                .map(|&(col, width)| {
                    let text: SharedString = match col {
                        Column::DateModified => entry.mtime_label(),
                        Column::DateCreated => date_label(entry.created),
                        Column::DateAdded if entry.added > 0 => date_label(entry.added),
                        Column::DateLastOpened if entry.opened > 0 => date_label(entry.opened),
                        Column::DateAdded | Column::DateLastOpened => "--".into(),
                        Column::Size => match (entry.is_dir, self.sizes.get(&entry.path)) {
                            (true, Some(bytes)) => human_size(*bytes).into(),
                            _ => entry.size_label(),
                        },
                        Column::Kind => entry.kind_label(),
                        Column::Tags => entry.tags.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>().join(", ").into(),
                    };
                    let cell = div().w(px(width)).flex_shrink_0().pl_2().pr_2().truncate();
                    if col == Column::Size { cell.flex().justify_end().child(text) } else { cell.child(text) }.into_any_element()
                })
                .collect();
            let folder = entry.is_dir && !entry.is_bundle() && entry.link.is_none();
            let path = entry.path.clone();
            if folder {
                self.on_screen.push(path.clone());
            }
            let row = div()
                .id(ix)
                .w_full()
                .h(px(row_height))
                .px(px(ROW_INSET))
                .role(Role::ListItem)
                .aria_label(entry.name.clone())
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .items_center()
                        .px_1()
                        .when(!joined_above, |d| d.rounded_t(px(6.)))
                        .when(!joined_below, |d| d.rounded_b(px(6.)))
                        .when(ix % 2 == 0, |d| d.bg(palette.stripe))
                        .when(selected, |d| d.bg(if focused { palette.selection } else { palette.selection_inactive }))
                        .when(entry.hidden || super::transfer::is_cut(&entry.path), |d| d.opacity(0.5))
                        .text_color(text)
                        .text_size(px(self.text_size))
                        .when(folder, |d| d.children(self.spot_motion(&Spot::Folder(path.clone()), 6.)))
                        .child(name)
                        .child(div().flex_shrink_0().flex().text_color(meta).children(cells))
                        .when(self.shows_location(), |d| {
                            d.child(div().w(px(LOCATION_WIDTH)).flex_shrink_0().pl_2().text_color(meta).text_ellipsis_middle().child(location(&entry.path)))
                        }),
                )
                .on_click(self.on_item_click(ix, cx))
                .on_mouse_down(MouseButton::Left, self.on_item_press(cx))
                .on_mouse_down(MouseButton::Right, self.on_item_menu(ix, cx))
                .on_mouse_down(MouseButton::Left, self.on_item_menu(ix, cx))
                .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
                .external_drag_payload(super::dnd::files_for_other_apps);
            // A folder takes what's dropped on it (and springs open).
            let row = if folder { drop_target(row, &path, palette.selection.opacity(0.35), cx) } else { row };
            rows.push(self.item_motion(&path, 1., row));
        }
        rows
    }

    /// Icon view: a grid, virtualized (a row of icons per item); grouped,
    /// a plain scroll with a heading over each group.
    fn render_icons(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.free_layout_on() {
            return self.render_free(window, cx);
        }
        let columns = self.columns.max(1);
        if self.lines.is_empty() || self.rows.len() > GROUPED_GRID_LIMIT {
            self.grid_row_of = (0..self.rows.len()).map(|ix| ix / columns).collect();
            let rows = self.rows.len().div_ceil(columns);
            return div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(uniform_list("grid", rows, cx.processor(Self::render_grid)).track_scroll(&self.scroll).size_full().pt_2())
                .vertical_scrollbar(&self.scroll)
                .into_any_element();
        }
        // Grouped: each group from a new row, under its heading.
        let palette = self.palette;
        let mut groups: Vec<(SharedString, Vec<usize>)> = Vec::new();
        for line in &self.lines {
            match line {
                Line::Header(title) => groups.push((title.clone(), Vec::new())),
                Line::Item(ix) if self.rows[*ix].depth == 0 => {
                    if let Some(g) = groups.last_mut() {
                        g.1.push(*ix)
                    }
                }
                Line::Item(_) => {}
            }
        }
        let mut children = Vec::new();
        for (title, members) in groups {
            children.push(
                div()
                    .mx(px(GRID_PADDING + 4.))
                    .mt_3()
                    .pb(px(2.))
                    .border_b_1()
                    .border_color(palette.separator)
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.secondary)
                    .child(title)
                    .into_any_element(),
            );
            for chunk in members.chunks(columns) {
                let cells: Vec<AnyElement> = chunk.iter().map(|&ix| self.render_cell(ix, window, cx)).collect();
                children.push(div().flex().px(px(GRID_PADDING)).children(cells).into_any_element());
            }
        }
        div().id("grouped-icons").flex_1().min_h_0().overflow_y_scroll().pb_2().children(children).into_any_element()
    }

    fn render_grid(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let columns = self.columns.max(1);
        let height = cell_height(self.icon_size);
        range
            .map(|row| {
                let cells: Vec<AnyElement> =
                    (row * columns..((row + 1) * columns).min(self.rows.len())).map(|ix| self.render_cell(ix, window, cx)).collect();
                div().h(px(height)).flex().px(px(GRID_PADDING)).children(cells).into_any_element()
            })
            .collect()
    }

    pub(super) fn render_cell(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let (icon, width) = (self.icon_size, cell_width(self.icon_size));
        let icon_px = (icon * window.scale_factor()).round() as u32;
        let focused = self.focus.is_focused(window);
        let show_all = Settings::get(cx).show_all_extensions;
        let selection = self.dragged_selection();
        let dragged = self.dragged_from(ix, &selection, window);
        let entry = &self.rows[ix];
        let selected = self.is_chosen(ix);
        let shown = self.icons.get(entry, icon_px);
        let renaming = self.renaming.as_ref().filter(|r| r.path == entry.path).map(|r| r.input.clone());
        let name = match renaming {
            Some(input) => rename_field(&input, 20.).w(px(width - 8.)).into_any_element(),
            None => div()
                .max_w(px(width - 8.))
                .px_1()
                .rounded_sm()
                .text_size(px(self.text_size - 1.))
                .text_center()
                .line_clamp(2)
                .text_ellipsis()
                .overflow_hidden()
                .when(selected && focused, |d| d.bg(palette.selection).text_color(palette.selected_text))
                .when(selected && !focused, |d| d.bg(palette.selection_inactive))
                .child(entry.display_name(show_all))
                .into_any_element(),
        };
        let (folder, path) = (entry.is_dir && !entry.is_bundle() && entry.link.is_none(), entry.path.clone());
        if folder {
            self.on_screen.push(path.clone());
        }
        let dim = if entry.hidden || super::transfer::is_cut(&entry.path) { 0.5 } else { 1. };
        let cell = div()
            .id(ix)
            .w(px(width))
            .h(px(cell_height(icon)))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .pt_2()
            .role(Role::ListItem)
            .aria_label(entry.name.clone())
            .when(entry.hidden || super::transfer::is_cut(&entry.path), |d| d.opacity(0.5))
            .child(
                div()
                    .relative()
                    .size(px(icon + 12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .when(selected, |d| d.bg(palette.glass_selected))
                    .when(folder, |d| d.children(self.spot_motion(&Spot::Folder(path.clone()), 8.)))
                    .child(file_image(shown, entry, icon, palette.accent)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(tag_dots(entry, palette.background))
                    .children(git_badge(self.git_marks.get(&entry.path).copied(), entry.is_dir, None))
                    .children(cloud_badge(entry, palette.secondary, window))
                    .child(name),
            )
            .on_click(self.on_item_click(ix, cx))
            .on_mouse_down(MouseButton::Left, self.on_item_press(cx))
            .on_mouse_down(MouseButton::Right, self.on_item_menu(ix, cx))
            .on_mouse_down(MouseButton::Left, self.on_item_menu(ix, cx))
            // (Where it was taken hold of: freely arranged, it lands so.)
            .on_drag(dragged, |dragged, grab, _, cx| cx.new(|_| super::dnd::Dragged { grab, ..dragged.clone() }))
            .external_drag_payload(super::dnd::files_for_other_apps);
        let cell = if folder { drop_target(cell, &path, palette.selection.opacity(0.25), cx) } else { cell };
        self.item_motion(&path, dim, cell)
    }
}

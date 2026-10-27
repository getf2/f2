//! Everything around the files: the sidebar, the toolbar (and the search
//! scope bar under it while searching), the tab bar, the path bar and the
//! status bar.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::Input;

use super::dnd::{Dragged, drop_target};
use super::motion::Spot;
use super::menu::{Command, Target};
use super::places::{Goes, Place, Virtual};
use super::sheets::Sheet;
use super::{Finder, GoBack, GoForward, GoUp, SIDEBAR_INSET, SIDEBAR_WIDTH, View, display_name};
use crate::listing::{Entry, count, human_size};
use crate::macos;
use crate::settings::{Settings, Tool};
use crate::style::{MEDIUM, REGULAR, SEMIBOLD, glass, symbol, tag_color};

pub(super) const TOOLBAR_HEIGHT: f32 = 52.;
const CAPSULE: f32 = 34.;
/// Room at the top of the sidebar for the window's traffic lights.
const TRAFFIC_LIGHTS: f32 = 46.;
const SEARCH_WIDTH: f32 = 220.;
/// The folder's name in the toolbar gets at least this.
const TITLE_MIN: f32 = 48.;

/// A favorite being dragged in the sidebar, to reorder.
#[derive(Clone)]
struct DraggedFavorite {
    ix: usize,
    name: SharedString,
}

impl Render for DraggedFavorite {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().px_2().py_1().rounded_md().bg(black().opacity(0.6)).text_color(white()).text_size(px(12.)).child(self.name.clone())
    }
}

impl Finder {
    /// An empty part of the toolbar: drags the window, double-click zooms.
    fn drag_area(&self, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(id)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| this.drag_from = Some(e.position)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.drag_from = None))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, _| {
                // A few points of travel first, so a click stays a click.
                if let Some(from) = this.drag_from
                    && (e.position - from).magnitude() > 3.
                {
                    this.drag_from = None;
                    window.start_window_move();
                }
            }))
            .on_click(|e, window, _| {
                if e.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
    }

    fn is_current(&self, place: &Place) -> bool {
        if self.searching {
            return false;
        }
        match &place.goes {
            Goes::Folder(path) => self.virtual_folder.is_none() && self.connecting.is_none() && *path == self.cwd,
            Goes::Virtual(v) => self.virtual_folder.as_ref() == Some(v),
            Goes::Server(dest) => {
                self.connecting.as_ref().is_some_and(|c| c.dest.host == dest.host && c.dest.port == dest.port)
                    || self.connecting.is_none() && self.remote_here().is_some_and(|h| h.dest.host == dest.host && h.dest.port == dest.port)
            }
        }
    }

    fn sidebar_item(&self, id: (&'static str, usize), place: &Place, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let current = self.is_current(place);
        let (goes, menu_place) = (place.goes.clone(), place.clone());
        let eject_path = place.folder().cloned();
        let icon = if place.color > 0 {
            div().size(px(11.)).rounded_full().bg(tag_color(place.color).unwrap_or(palette.tertiary)).into_any_element()
        } else {
            symbol(place.symbol, 14., REGULAR, palette.label, window)
        };
        // (Where it pulses when something's dropped on it.)
        let spot = match (&place.goes, place.folder()) {
            (_, Some(path)) => Some(Spot::Folder(path.clone())),
            (Goes::Virtual(Virtual::Trash), _) => Some(Spot::Trash),
            (Goes::Virtual(Virtual::AirDrop), _) => Some(Spot::AirDrop),
            _ => None,
        };
        let row = div()
            .id(id)
            .relative()
            .mx_2()
            .px_2()
            .h(px(28.))
            .flex()
            .items_center()
            .gap_2()
            .rounded_lg()
            .role(Role::Link)
            .aria_label(place.name.clone())
            .when(current, |d| d.bg(palette.glass_selected))
            .children(spot.and_then(|spot| self.spot_motion(&spot, 8.)))
            .child(div().w(px(18.)).flex().justify_center().child(icon))
            .child(div().flex_1().min_w_0().truncate().child(place.name.clone()))
            .when(place.ejectable, |d| {
                let eject_path = eject_path.clone();
                d.child(
                    div()
                        .id((id.0, id.1 + 1000))
                        .p_1()
                        .rounded_md()
                        .role(Role::Button)
                        .aria_label(format!("Eject {}", place.name))
                        .hover(|s| s.bg(palette.glass_selected))
                        .child(symbol("eject", 11., MEDIUM, palette.secondary, window))
                        // On press, and not a click on the place.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                if let Some(path) = eject_path.clone() {
                                    this.eject(path, window, cx);
                                }
                            }),
                        ),
                )
            })
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, window, cx| this.show_menu(Target::Place(menu_place.clone()), window, cx)));
        // A favorite can be dragged to another place among them.
        let row = match place.favorite {
            Some(ix) => {
                let dragged = DraggedFavorite { ix, name: place.name.clone() };
                row.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<DraggedFavorite>(move |s, _, _, _| s.border_t_2().border_color(palette.accent))
                    .on_drop(cx.listener(move |this, d: &DraggedFavorite, _, cx| this.move_favorite(d.ix, ix, cx)))
            }
            None => row,
        };
        // Files dropped on a folder go there; on the Trash, to the Trash.
        let row = match (&place.goes, place.folder()) {
            (_, Some(path)) => drop_target(row, &path.clone(), palette.selection.opacity(0.35), cx),
            (Goes::Virtual(Virtual::Trash), _) => row
                .drag_over::<Dragged>(move |s, _, _, _| s.bg(palette.selection.opacity(0.35)))
                .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(palette.selection.opacity(0.35)))
                .on_drop(cx.listener(|this, d: &Dragged, _, cx| this.drop_in_trash(d.paths.to_vec(), cx)))
                .on_drop(cx.listener(|this, d: &ExternalPaths, _, cx| this.drop_in_trash(d.paths().to_vec(), cx))),
            // On AirDrop: sent with AirDrop.
            (Goes::Virtual(Virtual::AirDrop), _) => row
                .drag_over::<Dragged>(move |s, _, _, _| s.bg(palette.selection.opacity(0.35)))
                .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(palette.selection.opacity(0.35)))
                .on_drop(cx.listener(|this, d: &Dragged, window, cx| {
                    this.land(Spot::AirDrop, cx);
                    this.send_airdrop(d.paths.to_vec(), window, cx)
                }))
                .on_drop(cx.listener(|this, d: &ExternalPaths, window, cx| {
                    this.land(Spot::AirDrop, cx);
                    this.send_airdrop(d.paths().to_vec(), window, cx)
                })),
            _ => row,
        };
        row.on_click(cx.listener(move |this, _, window, cx| this.go(goes.clone(), window, cx))).into_any_element()
    }

    pub(super) fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        if !Settings::get(cx).show_sidebar {
            return div().into_any_element();
        }
        let section = |title: &'static str| {
            div().px_4().pt_3().pb_1().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(title)
        };
        let favorites: Vec<_> = self.favorites.iter().enumerate().map(|(i, p)| self.sidebar_item(("favorite", i), p, window, cx)).collect();
        let (icloud, locations): (Vec<&Place>, Vec<&Place>) = self.locations.iter().partition(|p| p.symbol == "icloud");
        let icloud: Vec<_> = icloud.into_iter().enumerate().map(|(i, p)| self.sidebar_item(("icloud", i), p, window, cx)).collect();
        let locations: Vec<_> = locations.into_iter().enumerate().map(|(i, p)| self.sidebar_item(("location", i), p, window, cx)).collect();
        let tags: Vec<_> =
            super::places::tag_places(cx).iter().enumerate().map(|(i, p)| self.sidebar_item(("tag", i), p, window, cx)).collect();
        // Servers only once there's one (~/.ssh/config's, or one added: by
        // Connect to Server, say), so it's not there for those who don't use SSH.
        let servers: Vec<_> = if Settings::get(cx).sidebar.servers {
            super::servers::server_places(cx).iter().enumerate().map(|(i, p)| self.sidebar_item(("server", i), p, window, cx)).collect()
        } else {
            Vec::new()
        };
        // Folders dropped on "Favorites" are added there.
        let favorites_title = section("Favorites")
            .id("favorites-title")
            .drag_over::<Dragged>(move |s, _, _, _| s.text_color(palette.accent))
            .drag_over::<ExternalPaths>(move |s, _, _, _| s.text_color(palette.accent))
            .on_drop(cx.listener(|this, d: &Dragged, _, cx| {
                let folders: Vec<PathBuf> = d.paths.iter().filter(|p| p.is_dir()).cloned().collect();
                this.add_to_sidebar(folders, cx);
            }))
            .on_drop(cx.listener(|this, d: &ExternalPaths, _, cx| {
                let folders: Vec<PathBuf> = d.paths().iter().filter(|p| p.is_dir()).cloned().collect();
                this.add_to_sidebar(folders, cx);
            }));

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .pl(px(SIDEBAR_INSET))
            .py(px(SIDEBAR_INSET))
            .child(
                div()
                    .id("sidebar")
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(px(16.))
                    .bg(palette.sidebar)
                    .border_1()
                    .border_color(palette.sidebar_border)
                    .role(Role::List)
                    .aria_label("Sidebar")
                    .child(self.drag_area("sidebar-top", cx).h(px(TRAFFIC_LIGHTS)).flex_shrink_0())
                    .child(
                        div()
                            .id("sidebar-places")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(favorites_title)
                            .children(favorites)
                            .when(!icloud.is_empty(), |d| d.child(section("iCloud")).children(icloud))
                            .child(section("Locations"))
                            .children(locations)
                            .when(!servers.is_empty(), |d| {
                                d.child(
                                    section("Servers")
                                        .flex()
                                        .items_center()
                                        .child(div().flex_1())
                                        .child(
                                            div()
                                                .id("add-server")
                                                .mr(px(-6.))
                                                .px_1()
                                                .rounded_md()
                                                .cursor_pointer()
                                                .role(Role::Button)
                                                .aria_label("Add Server")
                                                .hover(|s| s.bg(palette.glass_selected))
                                                .child(symbol("plus", 11., MEDIUM, palette.secondary, window))
                                                .on_click(cx.listener(|this, _, window, cx| this.open_add_server(None, window, cx))),
                                        ),
                                )
                                .children(servers)
                            })
                            .when(!tags.is_empty(), |d| d.child(section("Tags")).children(tags)),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .flex()
                            .flex_col()
                            .text_size(px(11.))
                            .text_color(palette.tertiary)
                            .when(!self.index_status.is_empty(), |d| d.child(self.index_status.clone()))
                            .when(!macos::full_disk_access(), |d| {
                                d.child(
                                    div()
                                        .id("full-disk-access")
                                        .text_color(palette.accent)
                                        .cursor_pointer()
                                        .child("Turn On Full Disk Access…")
                                        .on_click(|_, _, cx| cx.open_url(macos::FULL_DISK_ACCESS_SETTINGS)),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn tool_button(&self, id: &'static str, name: &'static str, label: &'static str, window: &Window) -> Stateful<Div> {
        let palette = self.palette;
        glass(CAPSULE, &palette)
            .id(id)
            .w(px(CAPSULE + 4.))
            .justify_center()
            .role(Role::Button)
            .aria_label(label)
            .child(symbol(name, 14., MEDIUM, palette.label, window))
            .active(|s| s.opacity(0.6))
    }

    pub(super) fn render_toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let settings = Settings::get(cx);
        let tools = settings.toolbar.clone();
        let sidebar_hidden = !settings.show_sidebar;
        // (Narrow, as a pane of two is: tools that don't fit are left out,
        // the least needed first, as Finder tucks them away. Back and
        // forward, the views and search stay.)
        let query_open = self.query_focused(window, cx) || !self.query.read(cx).value().is_empty() || self.search_tips;
        let traffic_lights = sidebar_hidden && self.side != Some(crate::browser::Side::Right);
        let mut room = self.toolbar_width - 16. - if traffic_lights { 80. } else { 0. } - TITLE_MIN;
        for (tool, width) in [(Tool::BackForward, 77.), (Tool::View, 150.), (Tool::Search, if query_open { SEARCH_WIDTH } else { CAPSULE + 4. })] {
            if tools.contains(&tool) {
                room -= width + 8.;
            }
        }
        let kept: Vec<Tool> = [Tool::Up, Tool::Action, Tool::Share, Tool::Tags, Tool::Group, Tool::NewFolder, Tool::GetInfo, Tool::Delete]
            .into_iter()
            .filter(|t| tools.contains(t))
            .filter(|_| {
                room -= CAPSULE + 4. + 8.;
                room >= 0.
            })
            .collect();
        let has = |t: Tool| tools.contains(&t) && (matches!(t, Tool::BackForward | Tool::View | Tool::Search) || kept.contains(&t));
        let chevron = |name, enabled: bool, window: &Window| {
            symbol(name, 15., MEDIUM, if enabled { palette.label } else { palette.tertiary }, window)
        };
        let nav = glass(CAPSULE, &palette)
            .child(
                div()
                    .id("back")
                    .w(px(38.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .role(Role::Button)
                    .aria_label("Back")
                    .child(chevron("chevron.left", !self.back.is_empty(), window))
                    .active(|s| s.opacity(0.6))
                    .on_click(cx.listener(|this, _, window, cx| this.go_back(&GoBack, window, cx))),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(palette.separator))
            .child(
                div()
                    .id("forward")
                    .w(px(38.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .role(Role::Button)
                    .aria_label("Forward")
                    .child(chevron("chevron.right", !self.forward.is_empty(), window))
                    .active(|s| s.opacity(0.6))
                    .on_click(cx.listener(|this, _, window, cx| this.go_forward(&GoForward, window, cx))),
            );

        let segment = |id, name, label: &'static str, view: View, window: &Window, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(36.))
                .h(px(CAPSULE - 8.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .role(Role::RadioButton)
                .aria_label(label)
                .when(self.view == view, |d| d.bg(palette.glass_selected))
                .child(symbol(name, 14., MEDIUM, palette.label, window))
                .on_click(cx.listener(move |this, _, _, cx| this.set_view(view, cx)))
        };
        let views = glass(CAPSULE, &palette)
            .px(px(3.))
            .child(segment("as-icons", "square.grid.2x2", "as Icons", View::Icons, window, cx))
            .child(segment("as-list", "list.bullet", "as List", View::List, window, cx))
            .child(segment("as-columns", "rectangle.split.3x1", "as Columns", View::Columns, window, cx))
            .child(segment("as-gallery", "squares.below.rectangle", "as Gallery", View::Gallery, window, cx));

        // (Open while its tips are, which a click in leaves it focused for.)
        let search = if query_open {
            glass(CAPSULE, &palette)
                .w(px(SEARCH_WIDTH))
                .pl_3()
                .pr_1()
                .gap_1()
                .child(symbol("magnifyingglass", 13., MEDIUM, palette.secondary, window))
                .child(div().flex_1().child(Input::new(&self.query).appearance(false).small().cleanable(true)))
                .child(
                    div()
                        .id("search-tips-button")
                        .size(px(22.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .when(self.search_tips, |d| d.bg(palette.glass_selected))
                        .hover(|s| s.bg(palette.glass_selected))
                        .role(Role::Button)
                        .aria_label("Search Tips")
                        .child(symbol("questionmark.circle", 13., MEDIUM, if self.search_tips { palette.accent } else { palette.secondary }, window))
                        // On press, leaving the keyboard in the field.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.toggle_search_tips(cx);
                            }),
                        ),
                )
                .into_any_element()
        } else {
            self.tool_button("search", "magnifyingglass", "Search", window)
                .on_click(cx.listener(|this, _, window, cx| this.focus_search(window, cx)))
                .into_any_element()
        };

        div()
            .id("toolbar")
            .h(px(TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .pl_2()
            .pr_3()
            .role(Role::Toolbar)
            // With the sidebar hidden, leave room for the traffic lights
            // (not in the right pane: they're on the left).
            .when(traffic_lights, |d| d.pl(px(80.)))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| this.toggle_sheet(Sheet::Toolbar, cx)))
            .when(has(Tool::BackForward), |d| d.child(nav))
            .when(has(Tool::Up), |d| {
                let can = self.virtual_folder.is_none() && self.cwd.parent().is_some();
                d.child(
                    glass(CAPSULE, &palette)
                        .id("up")
                        .w(px(CAPSULE + 4.))
                        .justify_center()
                        .role(Role::Button)
                        .aria_label("Enclosing Folder")
                        .child(chevron("chevron.up", can, window))
                        .active(|s| s.opacity(0.6))
                        .on_click(cx.listener(|this, _, window, cx| this.go_up(&GoUp, window, cx))),
                )
            })
            .child(
                self.drag_area("title", cx)
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .pl_2()
                    .child(div().truncate().text_size(px(15.)).font_weight(FontWeight::BOLD).child(self.title(cx))),
            )
            .when(has(Tool::View), |d| d.child(views))
            .when(has(Tool::Group), |d| {
                d.child(self.tool_button("group", "square.grid.3x1.below.line.grid.1x2", "Group", window).on_click(
                    cx.listener(|this, _, window, cx| this.show_group_menu(window, cx)),
                ))
            })
            .when(has(Tool::Share), |d| {
                d.child(self.tool_button("share", "square.and.arrow.up", "Share", window).on_click(cx.listener(|this, _, window, cx| this.show_share_menu(window, cx))))
            })
            .when(has(Tool::Tags), |d| {
                d.child(self.tool_button("tags", "tag", "Edit Tags", window).on_click(cx.listener(|this, _, window, cx| this.show_tags_menu(window, cx))))
            })
            .when(has(Tool::Action), |d| {
                d.child(self.tool_button("action", "ellipsis.circle", "Action", window).on_click(cx.listener(|this, _, window, cx| this.show_action_menu(window, cx))))
            })
            .when(has(Tool::NewFolder), |d| {
                d.child(self.tool_button("new-folder", "folder.badge.plus", "New Folder", window).on_click(cx.listener(|this, _, window, cx| {
                    if !this.flat() {
                        this.run(Command::NewFolder, window, cx)
                    }
                })))
            })
            .when(has(Tool::Delete), |d| {
                d.child(self.tool_button("delete", "trash", "Delete", window).on_click(cx.listener(|this, _, window, cx| {
                    let paths = this.selection_paths();
                    if !paths.is_empty() {
                        this.run(Command::Trash(paths), window, cx)
                    }
                })))
            })
            .when(has(Tool::GetInfo), |d| {
                d.child(self.tool_button("get-info", "info.circle", "Get Info", window).on_click(cx.listener(|this, _, window, cx| {
                    let mut entries = this.selection();
                    if entries.is_empty() {
                        entries.push(super::menu::folder_entry(&this.cwd));
                    }
                    this.run(Command::GetInfo(entries), window, cx)
                })))
            })
            .when(has(Tool::Search), |d| d.child(search))
    }

    /// While searching (and once the field's clicked): where to look
    /// ("Search: This Mac | "folder""), by name or inside files too, the
    /// filters (Kind, Date, Size), Save as a Smart Folder; and how fast it
    /// was. In a narrow pane, the filters on a line of their own.
    pub(super) fn render_scope_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let pill = |id: &'static str, label: SharedString, on: bool, cx: &mut Context<Self>, f: fn(&mut Finder, &mut Context<Finder>)| {
            div()
                .id(id)
                .px_2()
                .h(px(22.))
                .flex()
                .items_center()
                .rounded_md()
                .when(on, |d| d.bg(palette.glass_selected).text_color(palette.label))
                .when(!on, |d| d.text_color(palette.secondary))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| f(this, cx)))
        };
        let folder: SharedString = format!("“{}”", display_name(&self.cwd)).into();
        let narrow = self.toolbar_width < 900.;
        let mut chips = Some(div().flex().flex_shrink_0().gap(px(6.)).children(self.render_filter_chips(window, cx)));
        let bar = div()
            .h(px(32.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px_4()
            .text_size(px(12.))
            .when(!narrow, |d| d.border_b_1().border_color(palette.separator))
            .child(div().mr_1().text_color(palette.secondary).child("Search:"))
            // (Where, picked, is where it looks instead.)
            .child(pill("scope-mac", "This Mac".into(), self.everywhere && self.filters.place.is_none(), cx, |t, cx| t.set_scope(true, cx)))
            .child(pill("scope-folder", folder, !self.everywhere && self.filters.place.is_none(), cx, |t, cx| t.set_scope(false, cx)))
            .child(div().w(px(1.)).h(px(14.)).mx_2().bg(palette.separator))
            .child(pill("by-name", "Name".into(), !self.search_contents, cx, |t, cx| t.set_search_contents(false, cx)))
            .child(pill("by-contents", "Contents".into(), self.search_contents, cx, |t, cx| t.set_search_contents(true, cx)))
            // (The filters after Name and Contents.)
            .when(!narrow, |d| d.child(div().w(px(1.)).h(px(14.)).mx_2().bg(palette.separator)).children(chips.take()))
            .child(div().flex_1().min_w_0())
            .child(div().min_w_0().truncate().text_size(px(11.)).text_color(palette.tertiary).child(self.timing.clone()))
            .when(self.searching, |d| {
                d.child(
                    glass(22., &palette)
                        .id("save-search")
                        .ml_2()
                        .px_3()
                        .text_size(px(11.))
                        .child("Save")
                        .on_click(cx.listener(|this, _, _, cx| this.save_search(cx))),
                )
            });
        let bar = if narrow {
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .text_size(px(12.))
                .border_b_1()
                .border_color(palette.separator)
                .child(bar)
                .child(div().h(px(30.)).px_4().pb(px(4.)).flex().items_center().children(chips))
        } else {
            bar
        };
        // (A press here leaves the keyboard in the search field: with
        // nothing typed yet, the bar is there only while the field has it,
        // so picking This Mac or Contents first closed it.)
        bar.on_mouse_down(MouseButton::Left, cx.listener(|_, _, window, _| window.prevent_default())).into_any_element()
    }

    /// In the Trash: Finder's bar with its name and an Empty button.
    pub(super) fn render_trash_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let empty = self.rows.is_empty();
        div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .text_size(px(12.))
            .border_b_1()
            .border_color(palette.separator)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(macos::bin()))
            .child(div().flex_1())
            .child(
                glass(24., &palette)
                    .id("empty-trash")
                    .px_4()
                    .role(Role::Button)
                    .aria_label(format!("Empty {}", macos::bin()))
                    .when(empty, |d| d.opacity(0.5))
                    .child("Empty")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !empty {
                            this.ask_empty_trash(cx)
                        }
                    })),
            )
    }

    /// Over an archive looked inside: that it's a copy, and Extract.
    pub(super) fn render_archive_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette;
        let (archive, _) = crate::archives::of(&self.cwd)?;
        let name = archive.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bar = div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .text_size(px(12.))
            .border_b_1()
            .border_color(palette.separator)
            .child(div().flex_shrink_0().font_weight(FontWeight::SEMIBOLD).child(format!("Inside “{name}”")))
            .child(div().flex_1().min_w_0().truncate().text_color(palette.secondary).child("A copy: changes here aren't saved into the archive."))
            .child(
                glass(24., &palette)
                    .id("extract-archive")
                    .flex_shrink_0()
                    .px_4()
                    .role(Role::Button)
                    .aria_label("Extract")
                    .child("Extract")
                    .on_click(cx.listener(move |this, _, window, cx| this.extract_beside(archive.clone(), window, cx))),
            );
        Some(bar.into_any_element())
    }

    /// Save the search as a Smart Folder (in ~/Library/Saved Searches, as
    /// Finder does) and add it to the sidebar.
    fn save_search(&mut self, cx: &mut Context<Self>) {
        let text = self.query.read(cx).value().trim().to_string();
        if text.is_empty() && !self.filters.any() {
            return;
        }
        let dir = super::home().join("Library/Saved Searches");
        let _ = std::fs::create_dir_all(&dir);
        // (Named by what's typed, else by the filters: "PDFs, Past 7 Days".)
        let said = if text.is_empty() { self.filters.said() } else { text.clone() };
        let name: String = said.chars().filter(|c| !matches!(c, '/' | ':')).collect();
        let file = crate::fileops::free_name(&dir, &name, "savedSearch");
        let scope = self.filters.scope().or_else(|| (!self.everywhere).then(|| self.cwd.clone()));
        match crate::macos::write_saved_search(&file, &super::places::raw_query(&text, &self.filters), scope.as_deref()) {
            Ok(()) => {
                self.add_to_sidebar(vec![file], cx);
                self.show_notice(format!("Saved “{name}” as a Smart Folder in the sidebar."), cx);
            }
            Err(e) => self.show_notice(e, cx),
        }
    }

    /// Where the selected item (or the folder) is, Macintosh HD down, each
    /// folder clickable (and a place to drop files).
    pub(super) fn render_path_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let show_all = Settings::get(cx).show_all_extensions;
        // Found items: where the selected one is (else nothing).
        let target = match self.selected_entry() {
            _ if self.connecting.is_some() => PathBuf::new(),
            Some(e) => e.path.clone(),
            None if self.flat() => PathBuf::new(),
            None => self.cwd.clone(),
        };
        let selected = self.selected_entry().cloned().filter(|e| e.link.is_none());
        let mut segments: Vec<Entry> = super::path_folders(&target)
            .into_iter()
            .map(|path| Entry { name: display_name(&path), path, is_dir: true, ..Default::default() })
            .collect();
        if let (Some(last), Some(selected)) = (segments.last_mut(), selected) {
            *last = selected;
        }
        if target.as_os_str().is_empty() {
            segments.clear();
        }
        // (Connecting: the server, by its name.)
        if let Some(c) = &self.connecting {
            segments = vec![Entry { name: c.dest.host.clone().into(), path: crate::remote::local_of(&c.dest, std::path::Path::new("/")), is_dir: true, ..Default::default() }];
        }
        let px_size = (14. * window.scale_factor()).round() as u32;
        let count = segments.len();
        let mut children = Vec::new();
        for (i, entry) in segments.into_iter().enumerate() {
            if i > 0 {
                children.push(symbol("chevron.right", 8., SEMIBOLD, palette.tertiary, window));
            }
            let shown = self.icons.get(&entry, px_size);
            let path = entry.path.clone();
            let folder = entry.is_dir && !entry.is_bundle();
            let segment = div()
                .id(("path", i))
                .relative()
                .flex()
                .items_center()
                .gap_1()
                .px_1()
                .min_w(px(28.))
                .when(i + 1 < count, |d| d.flex_shrink(1.))
                .when(i + 1 == count, |d| d.flex_shrink_0())
                .rounded_sm()
                .when(folder, |d| d.children(self.spot_motion(&Spot::Folder(path.clone()), 4.)))
                .child(super::items::file_image(shown, &entry, 14., palette.secondary))
                .child(div().truncate().child(entry.display_name(show_all)))
                .when(folder, |d| {
                    let target = path.clone();
                    d.on_click(cx.listener(move |this, _, window, cx| this.go_place(target.clone(), window, cx)))
                });
            let segment = if folder { drop_target(segment, &path, palette.selection.opacity(0.35), cx) } else { segment };
            children.push(segment.into_any_element());
        }
        div()
            .h(px(26.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(palette.secondary)
            .border_t_1()
            .border_color(palette.separator)
            .children(children)
    }

    /// Finder's status bar: "21 items, 64.5 GB available", plus how long the
    /// listing took; or, for a while, what just went wrong.
    pub(super) fn render_status(&self) -> impl IntoElement {
        let palette = self.palette;
        let mut text = if let Some(notice) = &self.notice {
            notice.to_string()
        } else if let Some(c) = &self.connecting {
            if c.error.is_some() { "Not connected".to_string() } else { format!("Connecting to {}…", c.dest.host) }
        } else if self.virtual_folder == Some(Virtual::AirDrop) && !self.searching {
            String::new()
        } else if self.flat() {
            if self.chosen.len() > 1 { format!("{} of {} selected", self.chosen.len(), self.rows.len()) } else { count(self.rows.len(), "item") }
        } else if self.listed.as_ref() != Some(&self.cwd) {
            String::new()
        } else {
            let shown = self.rows.iter().filter(|e| e.depth == 0).count();
            let hidden = self.listing.len().saturating_sub(shown);
            let mut text = if self.chosen.len() > 1 {
                format!("{} of {} selected", self.chosen.len(), self.rows.len())
            } else {
                count(shown, "item")
            };
            if hidden > 0 && self.chosen.len() <= 1 {
                text += &format!(" ({hidden} hidden)");
            }
            if let Some(bytes) = self.available {
                text += &format!(", {} available", human_size(bytes));
            }
            text
        };
        if self.notice.is_none() && !self.searching && !self.timing.is_empty() {
            text += &format!(" · {}", self.timing);
        }
        div()
            .h(px(22.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.))
            .text_color(if self.notice.is_some() { palette.label } else { palette.secondary })
            .border_t_1()
            .border_color(palette.separator)
            .child(div().truncate().px_3().child(text))
    }

    /// Finder's tab bar, under the toolbar once the window has two tabs:
    /// one equal segment per tab, the current one raised; × closes, +
    /// adds.
    pub(super) fn render_tab_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette;
        // (This pane's tabs.)
        let side = self.side.unwrap_or(crate::browser::Side::Left);
        let browser = self.browser.upgrade()?;
        let (tabs, active) = browser.read(cx).tabs(side)?;
        if tabs.len() < 2 {
            return None;
        }
        let titles: Vec<SharedString> =
            tabs.iter().map(|t| if t.entity_id() == cx.entity_id() { self.title(cx) } else { t.read(cx).title(cx) }).collect();
        let segments: Vec<AnyElement> = titles
            .into_iter()
            .enumerate()
            .map(|(ix, title)| {
                let (on, switch, close) = (ix == active, browser.downgrade(), browser.downgrade());
                div()
                    .id(("tab", ix))
                    .group("tab")
                    .flex_1()
                    .min_w_0()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_1()
                    .rounded_md()
                    .text_size(px(12.))
                    .role(Role::Tab)
                    .aria_label(title.clone())
                    .when(on, |d| d.bg(palette.glass_selected).text_color(palette.label))
                    .when(!on, |d| d.text_color(palette.secondary))
                    .child(
                        div()
                            .id(("close-tab", ix))
                            .size(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .invisible()
                            .group_hover("tab", |s| s.visible())
                            .hover(|s| s.bg(palette.glass_selected))
                            .child(symbol("xmark", 8., SEMIBOLD, palette.secondary, window))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                close.update(cx, |b, cx| b.close_tab(side, ix, window, cx)).ok();
                            }),
                    )
                    .child(div().flex_1().min_w_0().flex().justify_center().child(div().truncate().child(title)))
                    .child(div().size(px(16.)))
                    .on_click(move |_, window, cx| {
                        switch.update(cx, |b, cx| b.activate(side, ix, window, cx)).ok();
                    })
                    .into_any_element()
            })
            .collect();
        let add = browser.downgrade();
        Some(
            div()
                .id("tab-bar")
                .h(px(32.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .border_b_1()
                .border_color(palette.separator)
                .role(Role::TabList)
                .children(segments)
                .child(
                    div()
                        .id("new-tab")
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .role(Role::Button)
                        .aria_label("New Tab")
                        .hover(|s| s.bg(palette.glass_selected))
                        .child(symbol("plus", 11., MEDIUM, palette.secondary, window))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let path = this.cwd.clone();
                            add.update(cx, |b, cx| b.open_tab(side, path, window, cx)).ok();
                        })),
                )
                .into_any_element(),
        )
    }
}

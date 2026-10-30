//! Windows and their tabs. Each window is a `Browser` holding tabs, each a
//! `Finder`; the tab bar shows under the toolbar once there are two, as in
//! Finder. (In-window tabs: macOS's own would sit over the custom toolbar.)
//! A window can show two panes, as file managers like Total Commander do:
//! its tab on the left, with the sidebar, and another beside it; Tab goes
//! between them, F5 copies to the other one and F6 moves there. Each pane
//! has its own tabs; ⌘T, ⌘W and ⌃⇥ are the pane's with the keyboard.
//! A window remembers its frame for the next one, and takes files other
//! apps only promise (Mail, Photos).

use std::collections::HashMap;
use std::ffi::c_char;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::finder::Finder;
use crate::search::Search;
use crate::settings::Settings;
use crate::windows::Frame;

actions!(
    browser,
    [NewWindow, NewTab, CloseTab, NextTab, PrevTab, ToggleDualPane, SwitchPane, SwapPanes, OpenInOtherPane, CopyToOtherPane, MoveToOtherPane]
);

const CONTEXT: &str = "Browser";
/// With two panes (for the keys that only work then).
const SPLIT: &str = "Browser split";
/// The line between the panes.
const DIVIDER: f32 = 1.;

/// Which of a window's two panes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

/// How wide a pane is: the window, or its side of it (the sidebar and
/// half the rest; the other half).
pub fn pane_width(window: &Window, side: Option<Side>, sidebar_shown: bool) -> f32 {
    let whole = f32::from(window.viewport_size().width);
    let sidebar = if sidebar_shown { crate::finder::SIDEBAR_WIDTH } else { 0. };
    let half = ((whole - sidebar - DIVIDER) / 2.).floor();
    match side {
        None => whole,
        Some(Side::Left) => sidebar + half,
        Some(Side::Right) => whole - sidebar - DIVIDER - half,
    }
}

/// What every window shares.
pub struct Shared {
    pub search: Option<Arc<Search>>,
}

impl Global for Shared {}

/// The windows, by the token their promised files come with.
#[derive(Default)]
struct Browsers(HashMap<u64, WeakEntity<Browser>>);

impl Global for Browsers {}

static PROMISED: OnceLock<UnboundedSender<(u64, Vec<PathBuf>)>> = OnceLock::new();

extern "C" fn on_promised(token: u64, paths: *const c_char, _x: f64, _y: f64) {
    if let Some(tx) = PROMISED.get() {
        let _ = tx.unbounded_send((token, crate::macos::split_paths(paths)));
    }
}

static TO_TRASH: OnceLock<UnboundedSender<(u64, Vec<PathBuf>)>> = OnceLock::new();

/// Dragged out onto the Dock's Trash, which leaves trashing them to the app
/// they came from.
extern "C" fn on_dragged_to_trash(token: u64, paths: *const c_char) {
    if let Some(tx) = TO_TRASH.get() {
        let _ = tx.unbounded_send((token, crate::macos::split_paths(paths)));
    }
}

pub fn init(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewWindow, None),
        KeyBinding::new("cmd-t", NewTab, ctx),
        KeyBinding::new("cmd-w", CloseTab, ctx),
        KeyBinding::new("ctrl-tab", NextTab, ctx),
        KeyBinding::new("ctrl-shift-tab", PrevTab, ctx),
        KeyBinding::new("cmd-shift-]", NextTab, ctx),
        KeyBinding::new("cmd-shift-[", PrevTab, ctx),
        // (Not ⌘\, which 1Password takes for its autofill.)
        KeyBinding::new("cmd-alt-s", ToggleDualPane, ctx),
        KeyBinding::new("cmd-alt-u", SwapPanes, Some("split")),
        KeyBinding::new("cmd-alt-o", OpenInOtherPane, ctx),
        KeyBinding::new("tab", SwitchPane, Some("split")),
        // (⌃⌘C and ⌃⌘M first, as the menus show a key's first: F5 and F6
        // take fn on a Mac's keyboard.)
        KeyBinding::new("ctrl-cmd-c", CopyToOtherPane, Some("split")),
        KeyBinding::new("ctrl-cmd-m", MoveToOtherPane, Some("split")),
        KeyBinding::new("f5", CopyToOtherPane, Some("split")),
        KeyBinding::new("f6", MoveToOtherPane, Some("split")),
    ]);
    cx.on_action(|_: &NewWindow, cx| open(crate::finder::new_window_folder(cx), None, cx));
    cx.set_global(Browsers::default());
    // Promised files arrive here (from AppKit), for the window they were
    // dropped on.
    let (tx, mut rx) = unbounded();
    let _ = PROMISED.set(tx);
    cx.spawn(async move |cx| {
        while let Some((token, paths)) = rx.next().await {
            cx.update(|cx| {
                let browser = cx.global::<Browsers>().0.get(&token).and_then(|b| b.upgrade());
                if let Some(browser) = browser {
                    let tab = browser.read(cx).current();
                    tab.update(cx, |finder, cx| finder.receive_files(paths, cx));
                }
            });
        }
    })
    .detach();
    let (tx, mut rx) = unbounded();
    let _ = TO_TRASH.set(tx);
    cx.spawn(async move |cx| {
        while let Some((token, paths)) = rx.next().await {
            cx.update(|cx| {
                let browser = cx.global::<Browsers>().0.get(&token).and_then(|b| b.upgrade());
                if let Some(browser) = browser {
                    let tab = browser.read(cx).showing(&paths, cx);
                    tab.update(cx, |finder, cx| finder.dragged_to_trash(paths, cx));
                }
            });
        }
    })
    .detach();
}

/// A new window at `path`, with `select` selected in it.
pub fn open(path: PathBuf, select: Option<PathBuf>, cx: &mut App) {
    // As big as the last one; each new one steps down and right of it, all
    // of it on the screen.
    let step = 24. * (cx.windows().len() % 8) as f32;
    let last = match Settings::get(cx).window_frame {
        Some([x, y, w, h]) if w >= 720. && h >= 400. => Frame { x, y, w, h },
        _ => {
            let b = Bounds::centered(None, size(px(1100.), px(700.)), cx);
            Frame { x: b.origin.x.into(), y: b.origin.y.into(), w: b.size.width.into(), h: b.size.height.into() }
        }
    };
    let at = crate::windows::place_new(last, step, &crate::macos::screens());
    let bounds = Bounds { origin: point(px(at.x), px(at.y)), size: size(px(at.w), px(at.h)) };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(720.), px(400.))),
        // Finder's unified toolbar: no title bar, the traffic lights in the
        // sidebar, and the toolbar dragging the window itself.
        titlebar: Some(TitlebarOptions {
            title: Some(crate::APP_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(22.), px(20.))),
        }),
        app_owns_titlebar_drag: true,
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Browser::new(path, select, false, window, cx)));
    if let Err(e) = opened {
        eprintln!("couldn't open a window: {e}");
    }
}

/// A window for a folder laid out for Finder (a disk image's: see
/// `disk_images`): `content` points under an ordinary title bar, no
/// toolbar or sidebar, its icons over its picture. Centered on the screen.
pub fn open_laid_out(path: PathBuf, content: (f32, f32), cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(content.0), px(content.1)), cx))),
        titlebar: Some(TitlebarOptions { title: Some(crate::finder::display_name(&path).into()), appears_transparent: false, traffic_light_position: None }),
        is_resizable: true,
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Browser::new(path, None, true, window, cx)));
    if let Err(e) = opened {
        eprintln!("couldn't open a window: {e}");
    }
}

/// The window laid out for `path` (see `open_laid_out`), brought to the
/// front, if one's open.
pub fn bring_forward(path: &std::path::Path, cx: &mut App) -> bool {
    let browsers: Vec<Entity<Browser>> = cx.global::<Browsers>().0.values().filter_map(|b| b.upgrade()).collect();
    let Some(browser) = browsers.into_iter().find(|b| b.read(cx).laid_out && b.read(cx).current().read(cx).folder() == path) else { return false };
    let handle = browser.read(cx).window;
    handle.update(cx, |_, window, _| window.activate_window()).is_ok()
}

/// Show a folder (or, with `search`, the search field, for the whole Mac)
/// in the window in front, as Finder's Go menu does; in a new one if none
/// is open.
pub fn show(path: Option<PathBuf>, search: bool, cx: &mut App) {
    let front = front_browser(cx);
    let opened = front.is_none();
    if opened {
        open(path.clone().unwrap_or_else(|| crate::finder::new_window_folder(cx)), None, cx);
    }
    if let Some(browser) = front.or_else(|| front_browser(cx)) {
        let handle = browser.read(cx).window;
        handle
            .update(cx, |_, window, cx| {
                let tab = browser.read(cx).current();
                tab.update(cx, |finder, cx| {
                    if let Some(path) = path.filter(|_| !opened) {
                        finder.go_place(path, window, cx);
                    }
                    if search {
                        finder.search_this_mac(window, cx);
                    }
                });
                window.activate_window();
            })
            .ok();
    }
    cx.activate(true);
}

/// The Trash, in the window in front (a new one if none is open).
pub fn show_trash(cx: &mut App) {
    if front_browser(cx).is_none() {
        open(crate::finder::new_window_folder(cx), None, cx);
    }
    if let Some(browser) = front_browser(cx) {
        let handle = browser.read(cx).window;
        handle
            .update(cx, |_, window, cx| {
                let tab = browser.read(cx).current();
                tab.update(cx, |finder, cx| finder.go_trash(window, cx));
                window.activate_window();
            })
            .ok();
    }
    cx.activate(true);
}

/// Tidy Up a folder (the Desktop, Downloads), shown in the window in front
/// so ⌘Z there puts it back.
pub fn tidy_up(path: PathBuf, cx: &mut App) {
    show(Some(path), false, cx);
    let Some(browser) = front_browser(cx) else { return };
    let tab = browser.read(cx).current();
    // (Once it's there: the folder's listed in the window first.)
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(300)).await;
        cx.update(|cx| tab.update(cx, |finder, cx| finder.tidy_up(cx)));
    })
    .detach();
}

/// Show two panes in the window in front, or one again (from Quick Open).
pub fn toggle_dual_pane(cx: &mut App) {
    let Some(browser) = front_browser(cx) else { return };
    let handle = browser.read(cx).window;
    handle.update(cx, |_, window, cx| browser.update(cx, |this, cx| this.toggle_split(window, cx))).ok();
    cx.activate(true);
}

/// The folder shown in the window in front (its tab's), if any.
pub(crate) fn front_folder(cx: &App) -> Option<PathBuf> {
    let browser = front_browser(cx)?;
    let browser = browser.read(cx);
    Some(browser.current().read(cx).cwd().to_path_buf())
}

/// The folder shown in the window in front, and what's selected there (for
/// Resize…: the images it's of).
pub(crate) fn front_selection(cx: &App) -> Option<(PathBuf, Vec<PathBuf>)> {
    let browser = front_browser(cx)?;
    let finder = browser.read(cx).current();
    let finder = finder.read(cx);
    Some((finder.cwd().to_path_buf(), finder.selected_paths()))
}

/// `paths` shown selected in the window in front (in a new one if none is
/// open): the first one's folder, the others there selected too.
pub fn reveal_all(paths: Vec<PathBuf>, cx: &mut App) {
    let Some(first) = paths.first().cloned() else { return };
    if front_browser(cx).is_none() {
        let Some(parent) = first.parent() else { return };
        open(parent.to_path_buf(), Some(first), cx);
    }
    if let Some(browser) = front_browser(cx) {
        let handle = browser.read(cx).window;
        handle
            .update(cx, |_, window, cx| {
                let tab = browser.read(cx).current();
                tab.update(cx, |finder, cx| finder.reveal_all(paths, window, cx));
                window.activate_window();
            })
            .ok();
    }
    cx.activate(true);
}

/// Files just made beside what's selected in the window in front
/// (Resize…): selected there in place, an expanded folder's too, and
/// undoable. Only if they're out of its sight does it go to their folder.
pub fn made(paths: Vec<PathBuf>, cx: &mut App) {
    let Some(browser) = front_browser(cx) else { return reveal_all(paths, cx) };
    let handle = browser.read(cx).window;
    handle
        .update(cx, |_, window, cx| {
            let tab = browser.read(cx).current();
            if !tab.update(cx, |finder, cx| finder.made(paths.clone(), cx)) {
                tab.update(cx, |finder, cx| finder.reveal_all(paths, window, cx));
            }
            window.activate_window();
        })
        .ok();
    cx.activate(true);
}

/// A server (over SSH: see `remote`), connected to in the window in
/// front (a new one if none is open), which says so meanwhile.
pub fn open_server(dest: crate::sftp::Dest, cx: &mut App) {
    if front_browser(cx).is_none() {
        open(crate::finder::new_window_folder(cx), None, cx);
    }
    if let Some(browser) = front_browser(cx) {
        let handle = browser.read(cx).window;
        handle
            .update(cx, |_, window, cx| {
                let tab = browser.read(cx).current();
                tab.update(cx, |finder, cx| finder.connect_server(dest, None, window, cx));
                window.activate_window();
            })
            .ok();
    }
    cx.activate(true);
}

/// Add Server, in the window in front (a new one if none is open).
pub fn add_server(cx: &mut App) {
    if front_browser(cx).is_none() {
        open(crate::finder::new_window_folder(cx), None, cx);
    }
    if let Some(browser) = front_browser(cx) {
        let handle = browser.read(cx).window;
        handle
            .update(cx, |_, window, cx| {
                let tab = browser.read(cx).current();
                tab.update(cx, |finder, cx| finder.open_add_server(None, window, cx));
                window.activate_window();
            })
            .ok();
    }
    cx.activate(true);
}

/// Every tab showing one of `dirs` lists it again (a server's folder
/// changed: no events come from there).
pub(crate) fn reload_everywhere(dirs: Vec<PathBuf>, cx: &mut App) {
    let browsers: Vec<Entity<Browser>> = cx.global::<Browsers>().0.values().filter_map(|b| b.upgrade()).collect();
    for browser in browsers {
        let finders: Vec<Entity<Finder>> =
            [Side::Left, Side::Right].into_iter().filter_map(|side| browser.read(cx).tabs(side).map(|(tabs, _)| tabs.to_vec())).flatten().collect();
        for finder in finders {
            finder.update(cx, |finder, cx| finder.reload_dirs(&dirs, cx));
        }
    }
}

/// Say `message` in the front window's status bar (for a while).
pub(crate) fn notice(message: String, cx: &mut App) {
    if let Some(browser) = front_browser(cx) {
        let finder = browser.read(cx).current();
        finder.update(cx, |finder, cx| finder.say(message, cx));
    }
}

/// The browser window nearest the front.
fn front_browser(cx: &App) -> Option<Entity<Browser>> {
    let browsers: Vec<Entity<Browser>> = cx.global::<Browsers>().0.values().filter_map(|b| b.upgrade()).collect();
    let order = cx.window_stack().unwrap_or_default().into_iter().chain(cx.windows());
    order.into_iter().find_map(|w| browsers.iter().find(|b| b.read(cx).window == w).cloned())
}

/// A pane's tabs, and which is showing.
struct Pane {
    tabs: Vec<Entity<Finder>>,
    active: usize,
}

impl Pane {
    fn current(&self) -> Entity<Finder> {
        self.tabs[self.active].clone()
    }
}

pub struct Browser {
    window: AnyWindowHandle,
    /// The pane on the left: the only one, unless there are two.
    left: Pane,
    /// The second pane, beside it.
    right: Option<Pane>,
    /// Accepting promised files: set up once the window is on screen.
    promises: bool,
    /// The frame last seen, and the pending save of it.
    frame: Option<Bounds<Pixels>>,
    save_frame: Task<()>,
    /// Laid out as its folder's maker set it for Finder (see
    /// `open_laid_out`): one tab, one pane, its size not kept for others.
    laid_out: bool,
}

impl Browser {
    fn new(path: PathBuf, select: Option<PathBuf>, laid_out: bool, window: &mut Window, cx: &mut Context<Self>) -> Browser {
        let first = Self::make_tab(path, select, window, cx);
        if laid_out {
            first.update(cx, |finder, cx| finder.lay_out_as_made(cx));
        }
        let token = cx.entity_id().as_u64();
        let me = cx.entity().downgrade();
        cx.global_mut::<Browsers>().0.insert(token, me);
        let mut this = Browser {
            window: Window::window_handle(window),
            left: Pane { tabs: vec![first], active: 0 },
            right: None,
            promises: false,
            frame: None,
            save_frame: Task::ready(()),
            laid_out,
        };
        if Settings::get(cx).dual_pane && !laid_out {
            this.set_split(true, window, cx);
        }
        this.activate(Side::Left, 0, window, cx);
        this
    }

    fn make_tab(path: PathBuf, select: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Entity<Finder> {
        let search = cx.try_global::<Shared>().and_then(|s| s.search.clone());
        let browser = cx.entity().downgrade();
        cx.new(|cx| Finder::new(search, path, select, browser, window, cx))
    }

    /// The tab shown on the left: the window's, for what comes from
    /// outside it (a folder sent, the Go menu of the menu bar item).
    pub fn current(&self) -> Entity<Finder> {
        self.left.current()
    }

    /// The tab showing the folder `paths` are in (the one they were
    /// dragged from), else `current`.
    fn showing(&self, paths: &[PathBuf], cx: &App) -> Entity<Finder> {
        let folder = paths.first().and_then(|p| p.parent());
        let tabs = [Some(&self.left), self.right.as_ref()].into_iter().flatten().map(Pane::current);
        tabs.into_iter().find(|tab| folder == Some(tab.read(cx).folder())).unwrap_or_else(|| self.current())
    }

    fn pane(&self, side: Side) -> Option<&Pane> {
        match side {
            Side::Left => Some(&self.left),
            Side::Right => self.right.as_ref(),
        }
    }

    fn pane_mut(&mut self, side: Side) -> Option<&mut Pane> {
        match side {
            Side::Left => Some(&mut self.left),
            Side::Right => self.right.as_mut(),
        }
    }

    /// The pane with the keyboard (the left one, if there's one).
    fn focused(&self, window: &Window, cx: &App) -> Side {
        if self.right.as_ref().is_some_and(|right| right.current().read(cx).has_focus(window, cx)) { Side::Right } else { Side::Left }
    }

    /// A tab for `path` in a pane, after its current one, and switch to it.
    pub fn open_tab(&mut self, side: Side, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let tab = Self::make_tab(path, None, window, cx);
        if self.right.is_some() {
            tab.update(cx, |finder, cx| finder.set_side(Some(side), cx));
        }
        let Some(pane) = self.pane_mut(side) else { return };
        let ix = pane.active + 1;
        pane.tabs.insert(ix, tab);
        self.activate(side, ix, window, cx);
    }

    /// Close a pane's tab: its last closes the window, or the right pane.
    pub fn close_tab(&mut self, side: Side, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_mut(side) else { return };
        if pane.tabs.len() <= 1 {
            return match side {
                Side::Left => window.remove_window(),
                Side::Right => self.toggle_split(window, cx),
            };
        }
        pane.tabs.remove(ix);
        let next = if ix < pane.active || pane.active >= pane.tabs.len() { pane.active.saturating_sub(1) } else { pane.active };
        let next = next.min(pane.tabs.len() - 1);
        self.activate(side, next, window, cx);
    }

    pub fn activate(&mut self, side: Side, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_mut(side) else { return };
        pane.active = ix.min(pane.tabs.len().saturating_sub(1));
        let tab = pane.current();
        tab.update(cx, |finder, cx| finder.became_active(window, cx));
        cx.notify();
    }

    /// Tell each tab which pane it's in (None: there's one).
    fn place_tabs(&self, cx: &mut Context<Self>) {
        let split = self.right.is_some();
        for tab in &self.left.tabs {
            tab.update(cx, |finder, cx| finder.set_side(split.then_some(Side::Left), cx));
        }
        for tab in self.right.iter().flat_map(|pane| &pane.tabs) {
            tab.update(cx, |finder, cx| finder.set_side(Some(Side::Right), cx));
        }
    }

    /// Show a second pane (at the folder of the first), or stop.
    fn set_split(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if on && self.right.is_none() {
            let right = Self::make_tab(self.left.current().read(cx).folder().to_path_buf(), None, window, cx);
            self.right = Some(Pane { tabs: vec![right], active: 0 });
        } else if !on && let Some(right) = self.right.take() {
            if right.current().read(cx).has_focus(window, cx) {
                self.left.current().update(cx, |finder, cx| finder.became_active(window, cx));
            }
        }
        self.place_tabs(cx);
        cx.notify();
    }

    /// Two panes, or one (and new windows the same).
    fn toggle_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let on = self.right.is_none();
        self.set_split(on, window, cx);
        Settings::update(cx, |s| s.dual_pane = on);
    }

    /// The left pane goes right, and the right one left (tabs and all).
    fn swap(&mut self, cx: &mut Context<Self>) {
        let Some(right) = self.right.take() else { return };
        self.right = Some(std::mem::replace(&mut self.left, right));
        self.place_tabs(cx);
        cx.notify();
    }

    /// Show `path` in the other pane than `from` (making it, if there's
    /// one pane).
    pub fn show_in_other(&mut self, from: Side, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.right.is_none() {
            self.toggle_split(window, cx);
        }
        let to = if from == Side::Left { Side::Right } else { Side::Left };
        if let Some(pane) = self.pane(to) {
            pane.current().update(cx, |finder, cx| finder.go_place(path, window, cx));
        }
    }

    /// A server connected to in the pane beside `from` (two panes shown,
    /// if they weren't).
    pub fn connect_in_other(&mut self, from: Side, dest: crate::sftp::Dest, window: &mut Window, cx: &mut Context<Self>) {
        if self.right.is_none() {
            self.toggle_split(window, cx);
        }
        let to = if from == Side::Left { Side::Right } else { Side::Left };
        if let Some(pane) = self.pane(to) {
            pane.current().update(cx, |finder, cx| finder.connect_server(dest, None, window, cx));
        }
    }

    /// The folder shown in a pane, if things can be put in it (None: one
    /// pane, or it's showing found items).
    pub fn pane_folder(&self, side: Side, cx: &App) -> Option<PathBuf> {
        self.right.as_ref()?;
        self.pane(side)?.current().read(cx).drop_folder()
    }

    /// The pane with the keyboard, and the other one (None: one pane).
    fn panes(&self, window: &Window, cx: &App) -> Option<(Entity<Finder>, Entity<Finder>)> {
        let (left, right) = (self.left.current(), self.right.as_ref()?.current());
        Some(if right.read(cx).has_focus(window, cx) { (right, left) } else { (left, right) })
    }

    fn switch_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, other)) = self.panes(window, cx) {
            other.update(cx, |finder, cx| finder.became_active(window, cx));
        }
    }

    /// What's selected in the pane with the keyboard, into the other's
    /// folder.
    fn send_to_other(&mut self, how: crate::fileops::How, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.panes(window, cx) {
            let dir = to.read(cx).drop_folder();
            from.update(cx, |finder, cx| finder.send_selection(dir, how, cx));
        }
    }

    /// A pane's tabs and which is showing, for its tab bar.
    pub fn tabs(&self, side: Side) -> Option<(&[Entity<Finder>], usize)> {
        let pane = self.pane(side)?;
        Some((&pane.tabs, pane.active))
    }

    fn step(&mut self, side: Side, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.pane(side) else { return };
        let n = pane.tabs.len() as isize;
        let ix = ((pane.active as isize + by).rem_euclid(n)) as usize;
        self.activate(side, ix, window, cx);
    }

    /// Remember the window's frame (a second after it stops changing).
    fn track_frame(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.laid_out {
            return;
        }
        let bounds = window.bounds();
        if self.frame == Some(bounds) {
            return;
        }
        let first = self.frame.is_none();
        self.frame = Some(bounds);
        if first {
            return;
        }
        self.save_frame = cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update(|cx| {
                let frame = [f32::from(bounds.origin.x), f32::from(bounds.origin.y), f32::from(bounds.size.width), f32::from(bounds.size.height)];
                Settings::update(cx, |s| s.window_frame = Some(frame));
            });
        });
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.promises
            && let Ok(handle) = window.window_handle()
            && let RawWindowHandle::AppKit(appkit) = handle.as_raw()
        {
            self.promises = true;
            crate::macos::accept_promises(appkit.ns_view.as_ptr(), cx.entity_id().as_u64(), on_promised);
            crate::macos::drag_out(appkit.ns_view.as_ptr(), cx.entity_id().as_u64(), on_dragged_to_trash);
        }
        self.track_frame(window, cx);
        let left = self.left.current();
        // (Tabs: of the pane with the keyboard.)
        let root = div()
            .key_context(if self.right.is_some() { SPLIT } else { CONTEXT })
            .size_full()
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                // (Laid out: a new window, ordinary, at its folder.)
                if this.laid_out {
                    let path = this.left.current().read(cx).folder().to_path_buf();
                    return open(path, None, cx);
                }
                let side = this.focused(window, cx);
                let Some(pane) = this.pane(side) else { return };
                let path = pane.current().read(cx).folder().to_path_buf();
                this.open_tab(side, path, window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                // (Quick Look up, or F2's preview: ⌘W closes it, not the
                // window, as in Finder.)
                if crate::macos::quicklook_visible() {
                    cx.spawn(async move |_, _| crate::macos::quicklook_close()).detach();
                    return;
                }
                let side = this.focused(window, cx);
                if let Some(finder) = this.pane(side).map(|pane| pane.current())
                    && finder.update(cx, |finder, cx| finder.close_peek(cx))
                {
                    return;
                }
                let Some(ix) = this.pane(side).map(|pane| pane.active) else { return };
                this.close_tab(side, ix, window, cx)
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.step(this.focused(window, cx), 1, window, cx)))
            .on_action(cx.listener(|this, _: &PrevTab, window, cx| this.step(this.focused(window, cx), -1, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleDualPane, window, cx| {
                if !this.laid_out {
                    this.toggle_split(window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &SwitchPane, window, cx| this.switch_pane(window, cx)))
            .on_action(cx.listener(|this, _: &SwapPanes, _, cx| this.swap(cx)))
            .on_action(cx.listener(|this, _: &OpenInOtherPane, window, cx| {
                let side = this.focused(window, cx);
                let Some(pane) = this.pane(side) else { return };
                let path = pane.current().read(cx).chosen_folder();
                this.show_in_other(side, path, window, cx)
            }))
            .on_action(cx.listener(|this, _: &CopyToOtherPane, window, cx| this.send_to_other(crate::fileops::How::Copy, window, cx)))
            .on_action(cx.listener(|this, _: &MoveToOtherPane, window, cx| this.send_to_other(crate::fileops::How::Move, window, cx)));
        let Some(right) = self.right.as_ref().map(Pane::current) else { return root.child(left) };
        let left_width = pane_width(window, Some(Side::Left), Settings::get(cx).show_sidebar);
        root.flex()
            .child(div().w(px(left_width)).h_full().flex_shrink_0().child(left))
            .child(div().w(px(DIVIDER)).h_full().flex_shrink_0().bg(crate::style::Palette::for_window(window).separator))
            .child(div().flex_1().min_w_0().h_full().child(right))
    }
}

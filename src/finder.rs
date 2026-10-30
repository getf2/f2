//! The browser window's content, laid out like Finder on macOS 26: a
//! sidebar of places, a toolbar, the files as icons, a list (folders expand
//! in place) or columns, a preview pane, right-click menus, a path bar and
//! a status bar. This file holds the state and the moves between folders;
//! the rest is split by part.

mod actions;
mod airdrop;
mod chrome;
mod columns;
mod gallery;
mod install;
mod dnd;
pub(crate) mod items;
mod keys;
mod menu;
mod motion;
mod places;
pub(crate) use places::{erase_trash, favorite_list, is_trashed, remember_trashed};
pub(crate) use sheets::{completions, expand};
pub(crate) use undo::revert_outside;
mod rename_items;
mod preview;
mod peek;
mod arrange;
mod customize;
mod filters;
mod search_tips;
mod selection;
mod servers;
mod sheets;
mod transfer;
mod undo;

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::*;
use gpui_kit::component::Theme;
use gpui_kit::component::input::{InputEvent, InputState};

pub use actions::*;
pub use items::file_image;
pub use menu::TAGS as TAG_COLORS;
pub(crate) use crate::settings::ViewKind as View;
use crate::browser::{Browser, Side};
use crate::icons::{self, Icons};
use crate::listing::{self, Entry, SortKey};
use crate::macos;
use crate::search::Search;
use crate::settings::{Column, FolderView, GroupBy, Settings};
use crate::style::Palette;
use menu::Command;
use places::{Place, Virtual};

const SEARCH_LIMIT: usize = 1000;
/// The sidebar column: its panel and the inset around it (none on the
/// right: the files meet it, as Finder's do).
pub(crate) const SIDEBAR_WIDTH: f32 = 188.;
const SIDEBAR_INSET: f32 = 8.;
const PREVIEW_PANE_WIDTH: f32 = 260.;

/// iCloud Drive's folder.
fn icloud() -> PathBuf {
    home().join("Library/Mobile Documents/com~apple~CloudDocs")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// Where a new window opens.
pub fn new_window_folder(cx: &App) -> PathBuf {
    Settings::get(cx).new_window_folder()
}

/// A folder's name as Finder shows it ("Macintosh HD" for /).
pub(crate) fn display_name(path: &Path) -> SharedString {
    // (An archive looked inside: its own name, not its cache folder's.)
    if let Some((archive, root)) = crate::archives::of(path)
        && root == path
    {
        return archive.file_name().map_or_else(|| "Archive".into(), |n| n.to_string_lossy().into_owned().into());
    }
    // (A server's: the folder you'd log in to there, by the server's name.)
    if let Some(here) = crate::remote::here(path)
        && here.path == Path::new("/")
    {
        return here.server.into();
    }
    path.file_name().map_or("Macintosh HD".into(), |n| n.to_string_lossy().into_owned().into())
}

/// The folders down to `path`, for the path bar: inside an archive, those
/// down to the archive, then its unpacked folders.
fn path_folders(path: &Path) -> Vec<PathBuf> {
    let down = |p: &Path| p.ancestors().map(Path::to_path_buf).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>();
    // (A server's folder: from the server's top, not the cache's.)
    if let Some(here) = crate::remote::here(path) {
        let top = crate::remote::local_of(&here.dest, Path::new("/"));
        return down(path).into_iter().filter(|p| p.starts_with(&top)).collect();
    }
    match crate::archives::of(path) {
        Some((archive, root)) => {
            let mut folders = archive.parent().map_or_else(Vec::new, down);
            folders.extend(down(path).into_iter().filter(|p| p.starts_with(&root)));
            folders
        }
        None => down(path),
    }
}

/// What's in /Volumes, to notice mounts and ejects.
fn volume_names() -> Vec<OsString> {
    let mut names: Vec<_> = std::fs::read_dir("/Volumes").into_iter().flatten().flatten().map(|v| v.file_name()).collect();
    names.sort();
    names
}

/// Where the window is: a folder, or a list of found items (Recents, a
/// tag, a Smart Folder, the network).
#[derive(Clone, PartialEq)]
enum Loc {
    Folder(PathBuf),
    Virtual(Virtual),
}

/// A line of the list: a group's heading, or an item (index into `rows`).
#[derive(Clone)]
enum Line {
    Header(SharedString),
    Item(usize),
}

/// An item's name being edited in place.
struct Renaming {
    path: PathBuf,
    input: Entity<InputState>,
    _subscription: Subscription,
}

pub struct Finder {
    /// The window this is a tab of.
    browser: WeakEntity<Browser>,
    /// Which pane it is, in a window showing two (see `Browser`).
    side: Option<Side>,
    /// How wide the files are shown (the pane, less its sidebar and preview
    /// pane), and the toolbar (less the sidebar), as last drawn.
    list_width: f32,
    toolbar_width: f32,
    search: Option<Arc<Search>>,
    focus: FocusHandle,
    query: Entity<InputState>,
    /// The folder shown (in column view, the column with the keyboard).
    cwd: PathBuf,
    /// Found items shown instead of a folder.
    virtual_folder: Option<Virtual>,
    back: Vec<Loc>,
    forward: Vec<Loc>,
    /// Everything in `listed`, hidden entries too.
    listing: Vec<Entry>,
    /// The folder `listing` holds: not `cwd` while a new one loads.
    listed: Option<PathBuf>,
    /// Folders expanded in the list, and what's in them.
    expanded: HashSet<PathBuf>,
    children: HashMap<PathBuf, Vec<Entry>>,
    /// What's shown: `listing` (and expanded folders) filtered and sorted,
    /// or found items.
    rows: Vec<Entry>,
    /// The list's lines when grouped (else one per row).
    lines: Vec<Line>,
    /// In icon view, which grid row each item is on (for scrolling).
    grid_row_of: Vec<usize>,
    searching: bool,
    /// Search the whole disk rather than inside `cwd`.
    everywhere: bool,
    /// Search with Spotlight, inside files too (else names, with fsearch).
    search_contents: bool,
    /// The search field's cheat sheet is open.
    search_tips: bool,
    /// The search's filters, picked in the bar under the toolbar (Kind,
    /// Date, Size).
    filters: filters::Filters,
    /// Icon view's icons where they've been put (Sort By: None), and where
    /// that is, by name: their centers (see `arrange`).
    free: bool,
    placed: HashMap<String, (f32, f32)>,
    /// What's behind the icons, as the folder's View Options say (see
    /// `arrange::Backdrop`).
    backdrop: Option<arrange::Backdrop>,
    /// Shown as its maker laid it out for Finder (a disk image's window:
    /// see `disk_images`): no toolbar, sidebar or bars, and folders opened
    /// from it open in windows of their own.
    laid_out: bool,
    /// The disk image the volume shown is of, with an app to install (see
    /// `install`).
    image: Option<install::Image>,
    free_scroll: ScrollHandle,
    /// A drag over the freely arranged icons: where it is, and where their
    /// space starts (both in the window).
    drag_at: Option<(Point<Pixels>, Point<Pixels>)>,
    /// Results sorted by a column (else as found, best first).
    search_sort: Option<(SortKey, bool)>,
    /// The item the keyboard is on (and the preview and path bar show).
    selected: Option<usize>,
    /// Everything selected, by path; and where a ⇧ range starts.
    chosen: HashSet<PathBuf>,
    anchor: Option<PathBuf>,
    /// An item took this mouse press (so the empty space under it didn't).
    pressed_item: bool,
    history: undo::History,
    view: View,
    sort: (SortKey, bool),
    group: GroupBy,
    icon_size: f32,
    text_size: f32,
    show_hidden: bool,
    scroll: UniformListScrollHandle,
    /// Icons per row in icon view, as last laid out.
    columns: usize,
    /// Column view: the leftmost column's folder, and the other columns'
    /// listings.
    col_base: PathBuf,
    col_cache: HashMap<PathBuf, Vec<Entry>>,
    col_scroll: ScrollHandle,
    /// Gallery view's strip of thumbnails, and the item it last brought
    /// into view.
    gallery_scroll: ScrollHandle,
    gallery_shown: Option<usize>,
    /// The column and selection last scrolled into view.
    col_seen: Option<(PathBuf, Option<PathBuf>)>,
    /// Folders' sizes, worked out when Settings ask for them.
    sizes: HashMap<PathBuf, u64>,
    /// Git's status of what's shown, by path (see `watch_git`).
    git_marks: HashMap<PathBuf, u8>,
    git_task: Task<()>,
    /// The folders drawn in the last frame (rows, icons, columns): the
    /// repos among them are the ones whose status is read (`watch_git`).
    on_screen: Vec<PathBuf>,
    /// Folders to size again, a pass coming.
    resize_pending: HashSet<PathBuf>,
    sizing: Arc<AtomicBool>,
    overlay: Option<sheets::Sheet>,
    /// A copy or move asking what to do about a taken name.
    conflicts: Option<transfer::Conflicts>,
    /// Copies and moves under way.
    jobs: Vec<transfer::Job>,
    next_job: usize,
    ticking: bool,
    renaming: Option<Renaming>,
    /// Select these once they show up (made, moved, renamed, put back, or
    /// the next after trashed ones); and start renaming this one (a new
    /// folder).
    pending_select: Vec<PathBuf>,
    pending_rename: Option<PathBuf>,
    /// Typing selects by name: what's been typed, and when.
    typed: (String, Instant),
    /// What Quick Look shows now (to follow the selection), and whether
    /// it's on its way up (not visible yet, but not closed either).
    ql_shown: Vec<PathBuf>,
    ql_opening: bool,
    /// F2's own preview (Space), while it's up, and the looks it's read.
    peek: Option<peek::Peek>,
    peeks: peek::Cache,
    /// The preview pane's More Info, for the item it shows.
    preview_info: Option<(PathBuf, Vec<(String, String)>)>,
    /// A drag waiting over a folder to spring it open.
    spring: Option<(PathBuf, Instant)>,
    /// What's pulsing, fading out or in just now (see `motion`).
    motions: motion::Motions,
    icons: Icons,
    palette: Palette,
    favorites: Vec<Place>,
    locations: Vec<Place>,
    volumes_seen: Vec<OsString>,
    /// Free space on the volume of the folder shown.
    available: Option<u64>,
    /// Where a press in the toolbar started: moving from there drags the
    /// window.
    drag_from: Option<Point<Pixels>>,
    /// A list column being resized: which, and where the drag started.
    resizing: Option<(Column, f32, f32)>,
    error: Option<SharedString>,
    /// What just went wrong, in the status bar for a while.
    notice: Option<SharedString>,
    notice_task: Task<()>,
    timing: SharedString,
    index_status: SharedString,
    title_dirty: bool,
    /// A server being connected to, shown meanwhile (see `servers`).
    connecting: Option<servers::Connecting>,
    next_connect: u64,
    list_task: Task<()>,
    search_task: Task<()>,
    /// The search again for changes on disk (apart: never in the way of
    /// what's typed).
    refresh_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Finder {
    /// The folder shown.
    pub(crate) fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// What's selected.
    pub(crate) fn selected_paths(&self) -> Vec<PathBuf> {
        self.selection_paths()
    }
}

impl Finder {
    pub fn new(
        search: Option<Arc<Search>>,
        start: PathBuf,
        select: Option<PathBuf>,
        browser: WeakEntity<Browser>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions = vec![
            cx.subscribe_in(&query, window, |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.run_search(cx),
                InputEvent::PressEnter { .. } => this.open_selected(window, cx),
                // The search field folds back into its button when left empty.
                InputEvent::Focus | InputEvent::Blur => cx.notify(),
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.palette = Palette::for_window(window);
                Theme::sync_system_appearance(Some(window), cx);
                cx.notify();
            }),
            // Settings changed (in another window): show them.
            cx.observe_global::<Settings>(|this, cx| {
                this.favorites = places::favorites(cx);
                this.locations = places::locations(cx);
                if !this.flat() {
                    this.refresh_in_place(cx);
                }
                cx.notify();
            }),
        ];

        let focus = cx.focus_handle();
        let (icons, finished) = Icons::new();
        let fv = Settings::get(cx).view_for(&start);
        let everywhere = Settings::get(cx).search_this_mac;

        let mut this = Finder {
            browser,
            side: None,
            list_width: 1000.,
            toolbar_width: 1000.,
            search,
            focus,
            query,
            col_base: start.clone(),
            cwd: start,
            virtual_folder: None,
            back: Vec::new(),
            forward: Vec::new(),
            listing: Vec::new(),
            listed: None,
            expanded: HashSet::new(),
            children: HashMap::new(),
            rows: Vec::new(),
            lines: Vec::new(),
            grid_row_of: Vec::new(),
            searching: false,
            everywhere,
            search_contents: false,
            search_tips: false,
            filters: Default::default(),
            free: false,
            placed: HashMap::new(),
            backdrop: None,
            laid_out: false,
            image: None,
            free_scroll: ScrollHandle::new(),
            drag_at: None,
            search_sort: None,
            selected: None,
            chosen: HashSet::new(),
            anchor: None,
            pressed_item: false,
            history: undo::History::default(),
            view: fv.view,
            sort: (fv.sort, fv.ascending),
            group: fv.group,
            icon_size: fv.icon_size,
            text_size: fv.text_size,
            show_hidden: false,
            scroll: UniformListScrollHandle::new(),
            columns: 1,
            col_cache: HashMap::new(),
            col_scroll: ScrollHandle::new(),
            gallery_scroll: ScrollHandle::new(),
            gallery_shown: None,
            col_seen: None,
            sizes: HashMap::new(),
            resize_pending: HashSet::new(),
            git_marks: HashMap::new(),
            git_task: Task::ready(()),
            on_screen: Vec::new(),
            sizing: Arc::new(AtomicBool::new(false)),
            overlay: None,
            conflicts: None,
            jobs: Vec::new(),
            next_job: 0,
            ticking: false,
            renaming: None,
            pending_select: Vec::new(),
            pending_rename: None,
            typed: (String::new(), Instant::now()),
            ql_shown: Vec::new(),
            ql_opening: false,
            peek: None,
            peeks: peek::Cache::default(),
            preview_info: None,
            spring: None,
            motions: Default::default(),
            icons,
            palette: Palette::for_window(window),
            favorites: places::favorites(cx),
            locations: places::locations(cx),
            volumes_seen: volume_names(),
            available: None,
            drag_from: None,
            resizing: None,
            error: None,
            notice: None,
            notice_task: Task::ready(()),
            timing: "".into(),
            index_status: "".into(),
            title_dirty: true,
            connecting: None,
            next_connect: 0,
            list_task: Task::ready(()),
            search_task: Task::ready(()),
            refresh_task: Task::ready(()),
            _subscriptions: subscriptions,
        };
        if let Some(search) = &this.search {
            search.opened(&this.cwd);
        }
        this.read_layout(cx);
        this.look_for_image(cx);
        this.load(select, cx);
        this.receive_icons(finished, cx);
        this.watch_disk(cx);
        this.poll_index_status(cx);
        // Once, ask for Full Disk Access: else macOS asks about each guarded
        // folder as it's opened.
        if !macos::full_disk_access() && !Settings::get(cx).asked_full_disk_access {
            Settings::update(cx, |s| s.asked_full_disk_access = true);
            this.overlay = Some(sheets::Sheet::FullDiskAccess);
        }
        this
    }

    /// Shown as its maker laid it out for Finder (see `laid_out`).
    pub fn lay_out_as_made(&mut self, cx: &mut Context<Self>) {
        self.laid_out = true;
        self.read_layout(cx);
        self.look_for_image(cx);
        cx.notify();
    }

    /// Its tab was switched to (or its window opened).
    pub fn became_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.title_dirty = true;
        cx.notify();
    }

    /// The folder shown.
    pub fn folder(&self) -> &Path {
        &self.cwd
    }

    /// Files dropped by another app that only promised them (Mail, Photos):
    /// they land in the folder shown.
    pub fn receive_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let dir = self.cwd.clone();
        self.transfer(paths, dir, crate::fileops::How::Move, cx);
    }

    /// Shown as one of a window's two panes (or not).
    pub fn set_side(&mut self, side: Option<Side>, cx: &mut Context<Self>) {
        self.side = side;
        cx.notify();
    }

    /// Whether the keyboard's here (in its list, or its search field).
    pub fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// The folder files can be put in here: not found items, the Trash, nor
    /// an archive looked inside.
    pub fn drop_folder(&self) -> Option<PathBuf> {
        (!self.flat() && !self.in_trash() && crate::archives::of(&self.cwd).is_none()).then(|| self.cwd.clone())
    }

    /// The folder to show elsewhere: the one selected, else this one.
    pub fn chosen_folder(&self) -> PathBuf {
        match self.selection().as_slice() {
            [one] if one.is_dir && !one.is_bundle() => one.path.clone(),
            _ => self.cwd.clone(),
        }
    }

    /// Show `path` in the other pane (making it, if there's one).
    fn show_in_other_pane(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.side.unwrap_or(Side::Left);
        if let Some(browser) = self.browser.upgrade() {
            // (On the next turn, once nothing's being updated: the browser
            // updates both panes, this one too. Deferred, from the
            // right-click menu, this one was still mid-update, and F2
            // crashed.)
            let browser = browser.downgrade();
            cx.spawn_in(window, async move |_, cx| {
                browser.update_in(cx, |browser, window, cx| browser.show_in_other(side, path, window, cx)).ok();
            })
            .detach();
        }
    }

    /// Connect to a server in the other pane (see `show_in_other_pane`).
    fn connect_in_other_pane(&mut self, dest: crate::sftp::Dest, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.side.unwrap_or(Side::Left);
        if let Some(browser) = self.browser.upgrade() {
            let browser = browser.downgrade();
            cx.spawn_in(window, async move |_, cx| {
                browser.update_in(cx, |browser, window, cx| browser.connect_in_other(side, dest, window, cx)).ok();
            })
            .detach();
        }
    }

    /// The other pane's folder, in a window showing two (to extract into).
    fn other_pane_folder(&self, cx: &App) -> Option<PathBuf> {
        let side = self.side?;
        let browser = self.browser.upgrade()?;
        browser.read(cx).pane_folder(if side == Side::Left { Side::Right } else { Side::Left }, cx)
    }

    /// Copy or move what's selected into `dir`, the other pane's folder
    /// (None: it isn't showing one).
    pub fn send_selection(&mut self, dir: Option<PathBuf>, how: crate::fileops::How, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.selection().into_iter().map(|e| e.path).collect();
        match dir {
            _ if paths.is_empty() => self.show_notice("Select what to copy or move first.", cx),
            None => self.show_notice("The other pane isn't showing a folder to put them in.", cx),
            Some(dir) => self.transfer(paths, dir, how, cx),
        }
    }

    /// Showing found items (a search, Recents, a tag...) rather than a
    /// folder: no expanding, a Location column.
    fn flat(&self) -> bool {
        self.searching || self.virtual_folder.is_some()
    }

    /// Open `path` in a new tab of this window.
    fn open_in_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(browser) = self.browser.upgrade() {
            let side = self.side.unwrap_or(Side::Left);
            browser.update(cx, |browser, cx| browser.open_tab(side, path, window, cx));
        }
    }

    // Navigation

    fn here(&self) -> Loc {
        match &self.virtual_folder {
            Some(v) => Loc::Virtual(v.clone()),
            None => Loc::Folder(self.cwd.clone()),
        }
    }

    fn navigate(&mut self, path: PathBuf, select: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if path == self.cwd && !self.flat() {
            return;
        }
        // (Laid out as made: anywhere else is in a window of its own, as a
        // toolbarless window of Finder's opens one.)
        if self.laid_out {
            return crate::browser::open(path, select, cx);
        }
        let here = self.here();
        if here != Loc::Folder(path.clone()) {
            self.back.push(here);
            self.forward.clear();
        }
        self.go_to(Loc::Folder(path), select, window, cx);
    }

    /// Show `loc` (history aside).
    fn go_to(&mut self, loc: Loc, select: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        // (Gone elsewhere while connecting: that's let be.)
        self.connecting = None;
        self.clear_query(window, cx);
        match loc {
            Loc::Folder(path) => {
                if let Some(search) = &self.search {
                    search.opened(&path);
                }
                self.virtual_folder = None;
                self.cwd = path.clone();
                self.col_base = path;
                self.apply_folder_view(cx);
                self.look_for_image(cx);
                Settings::update(cx, |s| s.visited(&self.cwd));
                self.load(select, cx);
            }
            Loc::Virtual(v) => self.show_virtual(v, cx),
        }
        cx.notify();
    }

    fn go_back(&mut self, _: &GoBack, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev) = self.back.pop() {
            let from = self.here();
            let select = match &from {
                Loc::Folder(p) => Some(p.clone()),
                Loc::Virtual(_) => None,
            };
            self.forward.push(from);
            self.go_to(prev, select, window, cx);
        }
    }

    fn go_forward(&mut self, _: &GoForward, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.forward.pop() {
            self.back.push(self.here());
            self.go_to(next, None, window, cx);
        }
    }

    fn go_up(&mut self, _: &GoUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.virtual_folder.is_some() || self.remote_here().is_some_and(|h| h.path == Path::new("/")) {
            return;
        }
        if let Some(parent) = self.cwd.parent().map(Path::to_path_buf) {
            let from = self.cwd.clone();
            self.navigate(parent, Some(from), window, cx);
        }
    }

    /// List these folders again, if they're shown here.
    pub(crate) fn reload_dirs(&mut self, dirs: &[PathBuf], cx: &mut Context<Self>) {
        self.reload_folders(dirs, cx);
    }

    /// Say `message` in the status bar, for a while.
    pub(crate) fn say(&mut self, message: String, cx: &mut Context<Self>) {
        self.show_notice(message, cx);
    }

    /// The Trash (Quick Open's Trash command).
    pub(crate) fn go_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_virtual(Virtual::Trash, window, cx);
    }

    /// Show `path` in its folder, selected.
    pub(crate) fn reveal(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = path.parent().map(Path::to_path_buf) {
            self.navigate(parent, Some(path), window, cx);
            window.focus(&self.focus, cx);
        }
    }

    /// Show `paths` selected, in the first one's folder (the others there
    /// too).
    pub(crate) fn reveal_all(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        // (As listed: /tmp's are /private/tmp's.)
        let paths: Vec<PathBuf> = paths.into_iter().map(|p| std::fs::canonicalize(&p).unwrap_or(p)).collect();
        let Some(first) = paths.first().cloned() else { return };
        let Some(parent) = first.parent().map(Path::to_path_buf) else { return };
        if parent == self.cwd && !self.flat() {
            self.load(Some(first), cx);
        } else {
            self.navigate(parent, Some(first), window, cx);
        }
        self.select_when_listed(paths);
        window.focus(&self.focus, cx);
    }

    pub(crate) fn go_place(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(path, None, window, cx);
        window.focus(&self.focus, cx);
    }

    /// Show found items in place of a folder (Recents, a tag...).
    fn go_virtual(&mut self, v: Virtual, window: &mut Window, cx: &mut Context<Self>) {
        if self.virtual_folder.as_ref() == Some(&v) {
            return;
        }
        self.back.push(self.here());
        self.forward.clear();
        self.go_to(Loc::Virtual(v), None, window, cx);
        window.focus(&self.focus, cx);
    }

    /// How this folder is shown, as remembered (or the default).
    fn apply_folder_view(&mut self, cx: &mut Context<Self>) {
        let fv = Settings::get(cx).view_for(&self.cwd);
        // (Column view stays as you go deeper into its columns.)
        if self.view != View::Columns || fv.view != View::List {
            self.view = fv.view;
        }
        self.sort = (fv.sort, fv.ascending);
        self.group = fv.group;
        self.icon_size = fv.icon_size;
        self.text_size = fv.text_size;
        // (Where its icons are: as Finder put them, or as moved here.)
        self.read_layout(cx);
    }

    /// Remember how this folder is shown.
    fn save_folder_view(&mut self, cx: &mut App) {
        if self.virtual_folder.is_some() {
            return;
        }
        let fv = self.folder_view();
        let dir = self.cwd.clone();
        Settings::update(cx, |s| {
            if s.folder_views.len() > 2000 {
                s.folder_views.clear();
            }
            s.folder_views.insert(dir, fv);
        });
    }

    fn folder_view(&self) -> FolderView {
        FolderView {
            view: self.view,
            sort: self.sort.0,
            ascending: self.sort.1,
            group: self.group,
            icon_size: self.icon_size,
            text_size: self.text_size,
            free: Some(self.free),
        }
    }

    /// Whether the list needs Last Opened dates (slow: from Spotlight).
    fn wants_opened(&self, cx: &App) -> bool {
        self.sort.0 == SortKey::LastOpened || Settings::get(cx).columns.iter().any(|(c, _)| *c == Column::DateLastOpened)
    }

    /// List `cwd` off the main thread, then select `select` if it's there.
    fn load(&mut self, select: Option<PathBuf>, cx: &mut Context<Self>) {
        let dir = self.cwd.clone();
        self.title_dirty = true;
        if self.listed.as_ref() != Some(&dir) {
            // Don't show the last folder's files under this one's name: a
            // consent-gated folder can take as long as the macOS prompt.
            self.listing.clear();
            self.expanded.clear();
            self.children.clear();
            // (What waited to be selected was in the folder left behind.)
            self.pending_select.retain(|p| p.starts_with(&dir));
            self.error = None;
            self.overlay = None;
            self.renaming = None;
            self.preview_info = None;
            self.icons.forget_queued();
            if !self.flat() {
                self.rows.clear();
                self.lines.clear();
                self.select_only(None);
                self.timing = "".into();
            }
        }
        let opened = self.wants_opened(cx);
        self.list_task = cx.spawn(async move |this, cx| {
            let listed = dir.clone();
            let (result, available, elapsed) = cx
                .background_executor()
                .spawn(async move {
                    let started = Instant::now();
                    let result = if opened { listing::list_dir_opened(&dir) } else { listing::list_dir(&dir) };
                    let elapsed = started.elapsed();
                    let available = if crate::remote::is_remote(&dir) { None } else { macos::available_bytes(&dir) };
                    (result, available, elapsed)
                })
                .await;
            this.update(cx, |this, cx| {
                let fresh = this.listed.as_ref() != Some(&listed);
                this.listed = Some(listed);
                this.available = available;
                match result {
                    Ok(entries) => {
                        this.listing = entries;
                        this.error = None;
                    }
                    Err(e) => {
                        this.listing.clear();
                        this.error = Some(places::explain(&this.cwd, e).into());
                    }
                }
                if !this.flat() {
                    let picked = this.show_listing(select.as_deref(), cx);
                    // A new folder opens at its top (or its selection); a
                    // refresh stays where it's scrolled to (something
                    // changed in the folder: a download came in, say), but
                    // for what an operation just made, brought into view.
                    match this.selected {
                        Some(ix) if fresh => this.scroll_to(ix, ScrollStrategy::Center),
                        Some(ix) if picked => this.scroll_to(ix, ScrollStrategy::Nearest),
                        None if fresh => this.scroll.scroll_to_item(0, ScrollStrategy::Top),
                        _ => {}
                    }
                    this.timing = format!("listed in {:.1} ms", elapsed.as_secs_f64() * 1e3).into();
                }
                this.watch_git(cx);
                if Settings::get(cx).calculate_sizes {
                    this.work_out_sizes(cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Lay out `listing` as rows. What waits to be selected becomes the
    /// selection once it's here; else what's selected stays (what's still
    /// here of it), with the keyboard on `select`.
    /// (Whether what waited to be selected now is.)
    fn show_listing(&mut self, select: Option<&Path>, cx: &App) -> bool {
        let (rows, lines) = self.flatten_all(cx);
        self.rows = rows;
        self.lines = lines;
        self.motions.listed();
        self.keep_selection(select)
    }

    /// Select what an operation left behind once it's listed: what's here
    /// (or in a folder open in the list), not what went out of sight,
    /// which would wait to be selected when its folder's next opened.
    pub(super) fn select_when_listed(&mut self, paths: Vec<PathBuf>) {
        let shown = |p: &PathBuf| self.flat() || p.parent().is_some_and(|d| d == self.cwd || self.expanded.contains(d));
        self.pending_select = paths.into_iter().filter(shown).collect();
    }

    /// After the rows changed: select what waited for it (then true), or
    /// keep what's still there.
    fn keep_selection(&mut self, select: Option<&Path>) -> bool {
        let present: HashSet<&PathBuf> = self.rows.iter().map(|e| &e.path).collect();
        let pending: Vec<PathBuf> = self.pending_select.iter().filter(|p| present.contains(p)).cloned().collect();
        if let Some(first) = pending.first() {
            self.selected = self.rows.iter().position(|e| e.path == *first);
            self.anchor = Some(first.clone());
            self.chosen = pending.into_iter().collect();
            self.pending_select.clear();
            return true;
        }
        self.chosen.retain(|p| present.contains(p));
        self.selected = select.and_then(|p| self.rows.iter().position(|e| e.path == p));
        match self.selected {
            Some(ix) => {
                self.chosen.insert(self.rows[ix].path.clone());
            }
            None => self.selected = self.rows.iter().position(|e| self.chosen.contains(&e.path)),
        }
        false
    }

    /// The rows (each expanded folder followed by what's in it) and, when
    /// grouped, the list's lines with the groups' headings.
    fn flatten_all(&self, cx: &App) -> (Vec<Entry>, Vec<Line>) {
        let folders_on_top = Settings::get(cx).folders_on_top;
        let mut level: Vec<Entry> = self.listing.iter().filter(|e| self.show_hidden || !e.hidden).cloned().collect();
        listing::sort(&mut level, self.sort.0, self.sort.1, folders_on_top, &self.sizes);
        let mut rows = Vec::new();
        let mut lines = Vec::new();
        if self.group == GroupBy::None || self.view == View::Columns {
            for entry in level {
                self.push_with_children(entry, 0, &mut rows, folders_on_top);
            }
            return (rows, lines);
        }
        for (title, members) in places::group(level, self.group, &self.sizes) {
            lines.push(Line::Header(title));
            for entry in members {
                let start = rows.len();
                self.push_with_children(entry, 0, &mut rows, folders_on_top);
                lines.extend((start..rows.len()).map(Line::Item));
            }
        }
        (rows, lines)
    }

    fn push_with_children(&self, mut entry: Entry, depth: usize, rows: &mut Vec<Entry>, folders_on_top: bool) {
        entry.depth = depth;
        let inside = (self.view == View::List && self.expanded.contains(&entry.path)).then(|| self.children.get(&entry.path)).flatten();
        rows.push(entry);
        if let Some(inside) = inside {
            let mut level: Vec<Entry> = inside.iter().filter(|e| self.show_hidden || !e.hidden).cloned().collect();
            listing::sort(&mut level, self.sort.0, self.sort.1, folders_on_top, &self.sizes);
            for child in level {
                self.push_with_children(child, depth + 1, rows, folders_on_top);
            }
        }
    }

    /// Re-lay the rows (after a sort, a grouping...), keeping the selection
    /// in view.
    fn refresh_rows(&mut self, cx: &mut Context<Self>) {
        self.relayout(true, cx)
    }

    /// Re-lay the rows where they are (a folder opened in the list, sizes
    /// or settings changed): not scrolled to the selection, which can be
    /// far off (below a folder opened above it), but to what an operation's
    /// just made, if that's what's selected now.
    fn refresh_in_place(&mut self, cx: &mut Context<Self>) {
        self.relayout(false, cx)
    }

    fn relayout(&mut self, follow: bool, cx: &mut Context<Self>) {
        if self.flat() {
            if let Some((key, ascending)) = self.search_sort {
                let folders_on_top = Settings::get(cx).folders_on_top;
                listing::sort(&mut self.rows, key, ascending, folders_on_top, &self.sizes);
            }
            cx.notify();
            return;
        }
        let keep = self.selected_entry().map(|e| e.path.clone());
        let picked = self.show_listing(keep.as_deref(), cx);
        if let Some(ix) = self.selected.filter(|_| follow || picked) {
            self.scroll_to(ix, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let ascending_first = matches!(key, SortKey::Name | SortKey::Kind);
        if self.flat() {
            self.search_sort = Some(match self.search_sort {
                Some((k, asc)) if k == key => (key, !asc),
                _ => (key, ascending_first),
            });
            let keep = self.selected_entry().map(|e| e.path.clone());
            self.refresh_rows(cx);
            self.keep_selection(keep.as_deref());
            return;
        }
        self.sort = if self.sort.0 == key { (key, !self.sort.1) } else { (key, ascending_first) };
        // (Sorted: in icon view, out of the order they were put in.)
        self.free = false;
        self.save_folder_view(cx);
        if key == SortKey::LastOpened && self.listing.iter().all(|e| e.opened == 0) {
            let keep = self.selected_entry().map(|e| e.path.clone());
            self.listed = None;
            return self.load(keep, cx);
        }
        self.refresh_rows(cx);
    }

    fn set_group(&mut self, group: GroupBy, cx: &mut Context<Self>) {
        self.group = group;
        self.save_folder_view(cx);
        self.refresh_rows(cx);
    }

    fn toggle_hidden(&mut self, _: &ToggleHidden, _: &mut Window, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.refresh_rows(cx);
    }

    /// Expand or collapse the folder at `ix` in the list.
    fn set_expanded(&mut self, ix: usize, open: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.rows.get(ix) else { return };
        if self.flat() || self.view != View::List || !entry.is_dir || entry.is_bundle() {
            return;
        }
        let path = entry.path.clone();
        if open == self.expanded.contains(&path) {
            return;
        }
        if !open {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path.clone());
            if !self.children.contains_key(&path) {
                return self.load_children(path, cx);
            }
        }
        self.refresh_in_place(cx);
    }

    fn load_children(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let target = dir.clone();
            let entries = cx.background_executor().spawn(async move { listing::list_dir(&target) }).await;
            this.update(cx, |this, cx| {
                if this.expanded.contains(&dir) {
                    this.children.insert(dir, entries.unwrap_or_default());
                    this.refresh_in_place(cx);
                    this.watch_git(cx);
                    if Settings::get(cx).calculate_sizes {
                        this.work_out_sizes(cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// The folders shown: the folder's own, and those in the folders
    /// expanded in the list.
    fn shown_folders(&self) -> impl Iterator<Item = &PathBuf> {
        let expanded = self.expanded.iter().filter_map(|d| self.children.get(d)).flatten();
        self.listing.iter().chain(expanded).filter(|e| e.is_dir).map(|e| &e.path)
    }

    /// Git badges for what's shown, if it's in a repo or holds some: their
    /// status read in the background (only these repos'), then kept up as
    /// it changes.
    fn watch_git(&mut self, cx: &mut Context<Self>) {
        if crate::remote::is_remote(&self.cwd) {
            return;
        }
        let Some(search) = self.search.clone() else { return };
        if self.flat() || self.virtual_folder.is_some() {
            self.git_marks.clear();
            self.git_task = Task::ready(());
            return;
        }
        let expanded = self.expanded.iter().filter_map(|d| self.children.get(d)).flatten();
        let columns = self.col_cache.values().flatten();
        let entries: Vec<&Entry> = self.listing.iter().chain(expanded).chain(columns).collect();
        let shown: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
        let subfolders: Vec<PathBuf> = entries.iter().filter(|e| e.is_dir && !e.is_bundle()).map(|e| e.path.clone()).collect();
        let folders: Vec<PathBuf> = std::iter::once(self.cwd.clone()).chain(self.col_cache.keys().cloned()).collect();
        self.git_task = cx.spawn(async move |this, cx| {
            // The repos: the one this is in, and those it holds, read as
            // they come into view (a folder of sixty repos reads the twenty
            // on screen, not all sixty at once: git status and log each).
            let s = search.clone();
            let (folders, held) = cx
                .background_executor()
                .spawn(async move {
                    let folders: Vec<PathBuf> = folders.into_iter().filter(|dir| s.git_repo(dir).is_some()).collect();
                    let held: HashSet<PathBuf> = subfolders.into_iter().filter(|p| p.join(".git").exists()).collect();
                    (folders, held)
                })
                .await;
            if folders.is_empty() && held.is_empty() {
                this.update(cx, |this, cx| {
                    if !this.git_marks.is_empty() {
                        this.git_marks.clear();
                        cx.notify();
                    }
                })
                .ok();
                return;
            }
            let mut last = None;
            for round in 0.. {
                // (Those on screen now: read if they aren't yet, and kept
                // read while they're shown.)
                let Ok(showing) = this.update(cx, |this, _| this.on_screen.iter().filter(|p| held.contains(*p)).cloned().collect::<Vec<_>>())
                else {
                    break;
                };
                let (s, paths, folders) = (search.clone(), shown.clone(), folders.clone());
                let marks = cx
                    .background_executor()
                    .spawn(async move {
                        for dir in folders.iter().chain(&showing) {
                            s.git_repo(dir);
                        }
                        s.git_marks(&paths)
                    })
                    .await;
                if last.as_ref() != Some(&marks) {
                    let shown = this.update(cx, |this, cx| {
                        this.git_marks = marks.clone();
                        cx.notify();
                    });
                    if shown.is_err() {
                        break;
                    }
                    last = Some(marks);
                }
                // (Quickly at first, while git reads the status.)
                let wait = if round < 10 { 300 } else { 1500 };
                cx.background_executor().timer(Duration::from_millis(wait)).await;
            }
        });
    }

    /// Sizes came in: show them, sorted by them if that's the order.
    fn sizes_changed(&mut self, cx: &mut Context<Self>) {
        if self.sort.0 == SortKey::Size || self.group == GroupBy::Size {
            self.refresh_in_place(cx);
        }
        cx.notify();
    }

    /// Add up the folders' sizes (Settings > Calculate all sizes): from the
    /// index, all at once, where it holds them; the rest by walking them,
    /// showing them as they come.
    fn work_out_sizes(&mut self, cx: &mut Context<Self>) {
        if crate::remote::is_remote(&self.cwd) {
            return;
        }
        self.sizing.store(true, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        self.sizing = stop.clone();
        let folders: Vec<PathBuf> = self.shown_folders().filter(|p| !self.sizes.contains_key(*p)).cloned().collect();
        let search = self.search.clone();
        cx.spawn(async move |this, cx| {
            let indexed = match search {
                Some(search) if !folders.is_empty() => {
                    let dirs = folders.clone();
                    cx.background_executor().spawn(async move { search.folder_sizes(&dirs) }).await
                }
                _ => vec![None; folders.len()],
            };
            let mut walk = Vec::new();
            let alive = this.update(cx, |this, cx| {
                for (dir, size) in folders.into_iter().zip(indexed) {
                    match size {
                        Some((bytes, _)) => _ = this.sizes.insert(dir, bytes),
                        None => walk.push(dir),
                    }
                }
                this.sizes_changed(cx);
            });
            if alive.is_err() || stop.load(Ordering::Relaxed) {
                return;
            }
            for dir in walk {
                let flag = stop.clone();
                let target = dir.clone();
                let (bytes, _) =
                    cx.background_executor().spawn(async move { crate::fileops::folder_size(&target, &flag) }).await;
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let alive = this.update(cx, |this, cx| {
                    this.sizes.insert(dir, bytes);
                    this.sizes_changed(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Size these folders again after a change, if the index holds them
    /// (walking them again on every change would never end): once a second
    /// at most, as some (~/Library) always change.
    fn update_sizes(&mut self, folders: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(search) = self.search.clone() else { return };
        let coming = !self.resize_pending.is_empty();
        self.resize_pending.extend(folders);
        if coming {
            return;
        }
        cx.spawn(async move |this, cx| {
            // (After fsearch has taken the change in, which it hears of first.)
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let Ok(folders) = this.update(cx, |this, _| this.resize_pending.drain().collect::<Vec<PathBuf>>()) else { return };
            let dirs = folders.clone();
            let sizes = cx.background_executor().spawn(async move { search.folder_sizes(&dirs) }).await;
            this.update(cx, |this, cx| {
                let mut changed = false;
                for (dir, size) in folders.into_iter().zip(sizes) {
                    if let Some((bytes, _)) = size
                        && this.sizes.get(&dir) != Some(&bytes)
                    {
                        this.sizes.insert(dir, bytes);
                        changed = true;
                    }
                }
                if changed {
                    this.sizes_changed(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // Search

    fn run_search(&mut self, cx: &mut Context<Self>) {
        self.search_for(false, cx);
    }

    /// The search again, for changes on disk: only what changed in the
    /// results changes, and the timing stays that of what was typed.
    pub(super) fn refresh_search(&mut self, cx: &mut Context<Self>) {
        self.search_for(true, cx);
    }

    fn search_for(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let typed = self.query.read(cx).value().trim().to_string();
        if !refresh {
            self.refresh_task = Task::ready(());
        }
        // (Nothing typed, but a filter picked, is a search too: "PDFs from
        // the past week".)
        if typed.is_empty() && !self.filters.any() {
            self.search_task = Task::ready(());
            self.refresh_task = Task::ready(());
            if self.searching {
                self.searching = false;
                self.title_dirty = true;
                self.search_sort = None;
                let keep = self.selected_entry().map(|e| e.path.clone());
                match self.virtual_folder.clone() {
                    Some(v) => self.show_virtual(v, cx),
                    None => {
                        self.show_listing(keep.as_deref(), cx);
                    }
                }
                self.timing = "".into();
                cx.notify();
            }
            return;
        }
        let started = Instant::now();
        // (A server's folder: not searched yet.)
        if !self.everywhere && self.remote_here().is_some() {
            self.search_task = Task::ready(());
            self.searching = true;
            self.rows.clear();
            self.lines.clear();
            self.timing = "".into();
            self.error = Some("Searching on a server isn't here yet: pick This Mac to search the Mac.".into());
            cx.notify();
            return;
        }
        // (Where, picked, over This Mac or the folder: for fsearch it's its
        // in:, for Spotlight its scope.)
        let scope = (!self.everywhere).then(|| self.cwd.clone());
        if self.search_contents {
            let scope = self.filters.scope().or(scope);
            let text = if self.filters.any() { places::raw_query(&typed, &self.filters) } else { typed };
            let task = cx.spawn(async move |this, cx| {
                let found = cx.background_executor().spawn(async move { places::spotlight(&text, scope.as_deref()) }).await;
                this.update(cx, |this, cx| {
                    if refresh {
                        return this.refresh_found(found, "result", cx);
                    }
                    this.timing = format!("{} · Spotlight {:.0} ms", listing::count(found.len(), "result"), started.elapsed().as_secs_f64() * 1e3)
                        .into();
                    this.show_found(found, cx);
                })
                .ok();
            });
            if refresh { self.refresh_task = task } else { self.search_task = task }
            return;
        }
        let Some(search) = self.search.clone() else {
            self.error = Some("fsearch failed to start; see the terminal".into());
            cx.notify();
            return;
        };
        if !refresh {
            search.warm();
        }
        let text = filters::full_query(&typed, &self.filters);
        let task = cx.spawn(async move |this, cx| {
            let s = search.clone();
            let result =
                cx.background_executor().spawn(async move { s.run(&text, scope.as_deref(), SEARCH_LIMIT) }).await;
            let retry = this
                .update(cx, |this, cx| match result {
                    Ok(outcome) if refresh => {
                        this.refresh_found(outcome.entries, if outcome.content { "file" } else { "result" }, cx);
                        false
                    }
                    // (What was found stays.)
                    Err(_) if refresh => false,
                    Ok(outcome) => {
                        let what = if outcome.content { "file" } else { "result" };
                        this.timing = match outcome.listed {
                            // (Not in the index: looked through there and then.)
                            Some(items) => format!(
                                "{} · {} items here looked through · {:.0} ms",
                                listing::count(outcome.entries.len(), what),
                                listing::thousands(items),
                                outcome.elapsed.as_secs_f64() * 1e3,
                            ),
                            None => format!(
                                "{} · fsearch {:.2} ms · keystroke to list {:.2} ms",
                                listing::count(outcome.entries.len(), what),
                                outcome.elapsed.as_secs_f64() * 1e3,
                                started.elapsed().as_secs_f64() * 1e3,
                            ),
                        }
                        .into();
                        this.show_found(outcome.entries, cx);
                        false
                    }
                    Err(e) => {
                        this.searching = true;
                        this.rows.clear();
                        this.lines.clear();
                        // (Not the last query's count beside what's wrong.)
                        this.timing = "".into();
                        this.error = Some(e.into());
                        cx.notify();
                        !search.ready()
                    }
                })
                .unwrap_or(false);
            // The very first index is still being built: ask again shortly.
            if retry {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                this.update(cx, |this, cx| this.run_search(cx)).ok();
            }
        });
        if refresh { self.refresh_task = task } else { self.search_task = task }
    }

    /// Search results found again: left as they are if they're the same,
    /// else changed without moving the selection or the scroll.
    fn refresh_found(&mut self, mut found: Vec<Entry>, what: &str, cx: &mut Context<Self>) {
        if !self.searching {
            return;
        }
        if let Some((key, ascending)) = self.search_sort {
            listing::sort(&mut found, key, ascending, Settings::get(cx).folders_on_top, &self.sizes);
        }
        let same = |(a, b): (&Entry, &Entry)| a.path == b.path && a.mtime == b.mtime && a.size == b.size;
        if found.len() == self.rows.len() && found.iter().zip(&self.rows).all(same) {
            return;
        }
        // The count changes; how long the typed search took doesn't.
        if let Some((_, took)) = self.timing.split_once(" · ") {
            self.timing = format!("{} · {took}", listing::count(found.len(), what)).into();
        }
        self.rows = found;
        self.keep_selection(None);
        cx.notify();
    }

    /// Show search results.
    fn show_found(&mut self, found: Vec<Entry>, cx: &mut Context<Self>) {
        self.searching = true;
        self.title_dirty = true;
        self.error = None;
        self.icons.forget_queued();
        self.rows = found;
        self.lines.clear();
        if let Some((key, ascending)) = self.search_sort {
            listing::sort(&mut self.rows, key, ascending, Settings::get(cx).folders_on_top, &self.sizes);
        }
        self.select_only((!self.rows.is_empty()).then_some(0));
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn clear_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |q, cx| q.set_value("", window, cx));
        self.filters = Default::default();
        self.searching = false;
        self.search_task = Task::ready(());
        self.refresh_task = Task::ready(());
        self.search_sort = None;
        self.timing = "".into();
        self.title_dirty = true;
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keep = self.selected_entry().map(|e| e.path.clone());
        let was_searching = self.searching;
        self.clear_query(window, cx);
        if was_searching {
            // Stay on the result you picked if it's in this folder.
            match self.virtual_folder.clone() {
                Some(v) => self.show_virtual(v, cx),
                None => {
                    self.show_listing(keep.as_deref(), cx);
                }
            }
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn set_scope(&mut self, everywhere: bool, cx: &mut Context<Self>) {
        self.everywhere = everywhere;
        // (This Mac or the folder, picked: over Where.)
        self.filters.place = None;
        self.run_search(cx);
        cx.notify();
    }

    fn set_search_contents(&mut self, contents: bool, cx: &mut Context<Self>) {
        self.search_contents = contents;
        self.run_search(cx);
        cx.notify();
    }

    /// The menu bar item's Search This Mac…
    pub(crate) fn search_this_mac(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_scope(true, cx);
        self.focus_search(window, cx);
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |q, cx| q.focus(window, cx));
        cx.notify();
    }

    fn query_focused(&self, window: &Window, cx: &App) -> bool {
        self.query.read(cx).focus_handle(cx).is_focused(window)
    }

    // Selection and opening

    fn selected_entry(&self) -> Option<&Entry> {
        self.selected.and_then(|ix| self.rows.get(ix))
    }

    fn select_by(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() as isize - 1;
        let ix = match self.selected {
            Some(ix) => (ix as isize + delta).clamp(0, last),
            None if delta > 0 => 0,
            None => last,
        } as usize;
        self.select_only(Some(ix));
        self.scroll_to(ix, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Up or down a row: one item in the list, a row of them in icon view.
    fn select_row(&mut self, rows: isize, cx: &mut Context<Self>) {
        // (Freely arranged: the nearest icon above or below.)
        if self.free_layout_on() {
            return self.select_toward(0, rows.signum() as i32, cx);
        }
        // (Gallery: ↑ ↓ as ← →, one item along the strip.)
        let step = if self.view == View::Icons { self.columns.max(1) as isize } else { 1 };
        self.select_by(rows * step, cx);
    }

    /// ← and →: across in icon view; in the list, collapse and expand (or
    /// go to the enclosing folder's row); in columns, a column over.
    fn select_side(&mut self, right: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.free_layout_on() {
            return self.select_toward(if right { 1 } else { -1 }, 0, cx);
        }
        match self.view {
            View::Icons => return self.select_by(if right { 1 } else { -1 }, cx),
            View::Columns => return self.column_step(right, window, cx),
            View::Gallery => return self.select_by(if right { 1 } else { -1 }, cx),
            View::List => {}
        }
        let Some(ix) = self.selected else { return };
        let entry = &self.rows[ix];
        if right || self.expanded.contains(&entry.path) {
            return self.set_expanded(ix, right, cx);
        }
        if entry.depth > 0
            && let Some(parent) = self.rows[..ix].iter().rposition(|e| e.depth < entry.depth)
        {
            self.select_only(Some(parent));
            self.scroll_to(parent, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Scroll so the item at `ix` shows (in icon view, its row; grouped,
    /// its line).
    fn scroll_to(&self, ix: usize, strategy: ScrollStrategy) {
        if self.free_layout_on() {
            return self.scroll_free_to(ix);
        }
        let item = match self.view {
            View::Icons => self.grid_row_of.get(ix).copied().unwrap_or(ix / self.columns.max(1)),
            _ if !self.lines.is_empty() => {
                self.lines.iter().position(|l| matches!(l, Line::Item(i) if *i == ix)).unwrap_or(ix)
            }
            _ => ix,
        };
        self.scroll.scroll_to_item(item, strategy);
    }

    fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.col_base = self.cwd.clone();
        self.icons.forget_queued();
        self.save_folder_view(cx);
        // Icon view and columns show only the folder's own items.
        let keep = self.selected_entry().map(|e| e.path.clone());
        if !self.flat() {
            self.show_listing(keep.as_deref(), cx);
        }
        match self.selected {
            Some(ix) => self.scroll_to(ix, ScrollStrategy::Center),
            None => self.scroll.scroll_to_item(0, ScrollStrategy::Top),
        }
        cx.notify();
    }

    /// Space, ⌘Y: Quick Look the selection, or close it. (The panel is
    /// shown from a task, not this event: taking key focus calls back into
    /// GPUI, which mustn't be mid-update then.)
    fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        // (A server's file: copied here first, then shown.)
        if self.peek.is_none() && !macos::quicklook_visible() && self.peek_remote(cx) {
            return;
        }
        self.toggle_preview_now(cx)
    }

    fn toggle_preview_now(&mut self, cx: &mut Context<Self>) {
        // F2's own, up: put away. One item F2 draws itself: F2's, at once.
        if self.close_peek(cx) {
            return;
        }
        if !macos::quicklook_visible() && self.peeks_here() {
            return self.open_peek(cx);
        }
        if macos::quicklook_visible() {
            self.ql_shown.clear();
            cx.spawn(async move |_, _| macos::quicklook_close()).detach();
            return;
        }
        let paths = self.quicklook_items();
        if !paths.is_empty() {
            self.ql_shown = paths.clone();
            self.ql_opening = true;
            cx.spawn(async move |this, cx| {
                macos::quicklook_show(&paths, 0);
                this.update(cx, |this, _| this.ql_opening = false).ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// What Quick Look shows: the selected items, the one under the
    /// keyboard first.
    fn quicklook_items(&self) -> Vec<PathBuf> {
        let mut paths = self.selection_paths();
        if let Some(current) = self.selected_entry().map(|e| e.path.clone())
            && let Some(at) = paths.iter().position(|p| *p == current)
        {
            paths.rotate_left(at);
        }
        paths
    }

    /// Escape: answer a question, stop renaming, close a sheet, else clear
    /// the search.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_peek(cx) {
            // (F2's preview put away.)
        } else if macos::quicklook_visible() {
            self.toggle_preview(cx);
        } else if self.conflicts.is_some() {
            self.resolve(None, cx);
        } else if self.renaming.is_some() {
            self.finish_rename(false, window, cx);
        } else if self.overlay.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.search_tips {
            self.toggle_search_tips(cx);
        } else if !self.searching && transfer::uncut() {
            // (What was cut isn't any more.)
            cx.notify();
        } else {
            self.clear_search(window, cx);
        }
    }

    /// Run a menu command on the selected items, as its shortcut does.
    fn on_selected(&mut self, command: impl FnOnce(Vec<PathBuf>) -> Command, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selection_paths();
        if !paths.is_empty() {
            self.run(command(paths), window, cx);
        }
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries = self.selection();
        self.open_entries(entries, window, cx);
    }

    /// A click on a row or an icon: select it (⌘ adds or removes it, ⇧
    /// selects up to it); a double-click opens it.
    fn on_item_click(&self, ix: usize, cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, event: &ClickEvent, window, cx| {
            let m = event.modifiers();
            let folder = this.rows.get(ix).filter(|e| e.is_dir && !e.is_bundle()).map(|e| e.path.clone());
            if m.platform && event.click_count() >= 2 && let Some(folder) = folder {
                return this.open_in_tab(folder, window, cx);
            }
            if m.platform {
                this.toggle_chosen(ix);
            } else if m.shift {
                this.extend_to(ix);
            } else {
                this.select_only(Some(ix));
                if event.click_count() >= 2 {
                    this.open_selected(window, cx);
                }
            }
            if this.renaming.is_none() {
                window.focus(&this.focus, cx);
            }
            cx.notify();
        })
    }

    // Background feeds

    /// Show icons and thumbnails as the workers finish them.
    fn receive_icons(&mut self, mut finished: UnboundedReceiver<icons::Done>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while let Some(first) = finished.next().await {
                let mut batch = vec![first];
                while let Ok(done) = finished.try_recv() {
                    batch.push(done);
                }
                let alive = this.update(cx, |this, cx| {
                    for done in batch {
                        for evicted in this.icons.finish(done) {
                            cx.drop_image(evicted, None);
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Reload the folder, any expanded in it (or shown as a column), or the
    /// search when FSEvents says something there changed. fsearch already
    /// follows FSEvents for its index; this rides on the same stream.
    fn watch_disk(&mut self, cx: &mut Context<Self>) {
        let Some(search) = self.search.clone() else { return };
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<(PathBuf, bool)>();
        search.on_change(move |dir, deep| {
            let _ = tx.unbounded_send((dir.to_path_buf(), deep));
        });
        cx.spawn(async move |this, cx| {
            while let Some(first) = rx.next().await {
                // Coalesce a burst into one refresh. Search results refresh
                // once a second at most: somewhere, the disk always changes.
                let searching = this.read_with(cx, |this, _| this.searching).unwrap_or(false);
                cx.background_executor().timer(Duration::from_millis(if searching { 1000 } else { 100 })).await;
                let mut changes = vec![first];
                while let Ok(c) = rx.try_recv() {
                    changes.push(c);
                }
                let alive = this.update(cx, |this, cx| {
                    let touched =
                        |dir: &Path| changes.iter().any(|(d, deep)| d == dir || (*deep && dir.starts_with(d)));
                    if this.searching {
                        let stale = if this.search_contents {
                            // Spotlight finds new files once it has indexed
                            // them: here, just changes to what it found.
                            this.rows.iter().any(|e| e.path.parent().is_some_and(touched))
                        } else {
                            this.everywhere || changes.iter().any(|(d, _)| d.starts_with(&this.cwd))
                        };
                        if stale {
                            this.refresh_search(cx);
                        }
                        return;
                    }
                    if this.virtual_folder.is_some() {
                        return;
                    }
                    if touched(&this.cwd) {
                        let keep = this.selected_entry().map(|e| e.path.clone());
                        this.load(keep, cx);
                    }
                    let stale: Vec<PathBuf> = this.expanded.iter().filter(|d| touched(d)).cloned().collect();
                    for dir in stale {
                        this.load_children(dir, cx);
                    }
                    let stale_columns: Vec<PathBuf> = this.col_cache.keys().filter(|d| touched(d)).cloned().collect();
                    for dir in stale_columns {
                        this.col_cache.remove(&dir);
                    }
                    // Sizes of folders something changed in: again now if
                    // shown, else when they next are.
                    let on_screen: HashSet<&PathBuf> = this.shown_folders().collect();
                    let (shown, hidden): (Vec<PathBuf>, Vec<PathBuf>) = this
                        .sizes
                        .keys()
                        .filter(|f| changes.iter().any(|(d, _)| d.starts_with(f)))
                        .cloned()
                        .partition(|f| on_screen.contains(f));
                    for dir in hidden {
                        this.sizes.remove(&dir);
                    }
                    if !shown.is_empty() {
                        this.update_sizes(shown, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_index_status(&mut self, cx: &mut Context<Self>) {
        let search = self.search.clone();
        if search.is_none() {
            self.index_status = "fsearch unavailable".into();
        }
        cx.spawn(async move |this, cx| {
            loop {
                // (Only while search is getting ready, the first time: how many
                // files it knows is nothing to most people.)
                let status: Option<SharedString> = search.as_ref().map(|s| if s.ready() { SharedString::default() } else { "Getting search ready…".into() });
                let alive = this.update(cx, |this, cx| {
                    if let Some(status) = status
                        && this.index_status != status
                    {
                        this.index_status = status;
                        cx.notify();
                    }
                    // Something mounted or ejected (/Volumes isn't followed
                    // by FSEvents without Full Disk Access).
                    let volumes = volume_names();
                    if volumes != this.volumes_seen {
                        this.volumes_seen = volumes;
                        this.locations = places::locations(cx);
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        })
        .detach();
    }

    /// The toolbar's title: the folder, the found items, or the search.
    pub fn title(&self, cx: &App) -> SharedString {
        if self.searching {
            let typed = self.query.read(cx).value().trim().to_string();
            match (typed.is_empty(), self.filters.any()) {
                (true, _) => self.filters.said().into(),
                (false, true) => format!("Searching “{typed}”: {}", self.filters.said()).into(),
                (false, false) => format!("Searching “{typed}”").into(),
            }
        } else if let Some(c) = &self.connecting {
            c.dest.host.clone().into()
        } else if let Some(v) = &self.virtual_folder {
            v.title()
        } else {
            display_name(&self.cwd)
        }
    }
}

impl Render for Finder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.title_dirty {
            self.title_dirty = false;
            window.set_window_title(&self.title(cx));
        }
        if let Some(path) = self.pending_rename.take_if(|p| self.rows.iter().any(|e| e.path == *p)) {
            self.start_rename(path, window, cx);
        }
        self.sync_quicklook(cx);
        if crate::info::inspector_open(cx) {
            crate::info::follow_selection(self.selection(), cx);
        }
        let palette = self.palette;
        if self.laid_out {
            return self.render_laid_out(window, cx);
        }
        let settings = Settings::get(cx);
        // (Two panes: no room for the preview pane.)
        let (show_path_bar, show_status_bar, show_preview_pane) =
            (settings.show_path_bar, settings.show_status_bar, settings.show_preview_pane && self.side.is_none());
        let pane = if show_preview_pane { PREVIEW_PANE_WIDTH } else { 0. };
        let sidebar = if settings.show_sidebar && self.side != Some(Side::Right) { SIDEBAR_WIDTH } else { 0. };
        let width = crate::browser::pane_width(window, self.side, settings.show_sidebar);
        self.list_width = width - sidebar - pane;
        self.toolbar_width = width - sidebar;
        let grid_width = self.list_width - 2. * items::GRID_PADDING;
        self.columns = ((grid_width / items::cell_width(self.icon_size)).floor() as usize).max(1);

        let content = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(window, cx))
            .children(self.render_tab_bar(window, cx))
            // (While searching; and as soon as the field's clicked, so the
            // filters are there to pick before anything's typed.)
            .children((self.searching || self.filters.any() || self.query_focused(window, cx)).then(|| self.render_scope_bar(window, cx).into_any_element()))
            .children((!self.searching && self.in_trash()).then(|| self.render_trash_bar(cx).into_any_element()))
            .children((!self.searching && !self.flat()).then(|| self.render_archive_bar(cx)).flatten())
            .children((!self.searching).then(|| self.render_install_bar(window, cx)).flatten())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.render_files(window, cx))
                    .children(show_preview_pane.then(|| self.render_preview_pane(window, cx))),
            )
            .children(self.render_jobs(window))
            .children(show_path_bar.then(|| self.render_path_bar(window, cx).into_any_element()))
            .children(show_status_bar.then(|| self.render_status().into_any_element()))
            .children(self.render_search_tips(window, cx))
            .children(self.render_peek(window, cx))
            .children(self.render_sheet(window, cx))

            .children(self.render_conflict(window, cx));

        let root = div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.));
        // (The right pane: the left one's sidebar is the window's.)
        let sidebar = (self.side != Some(Side::Right)).then(|| self.render_sidebar(window, cx));
        self.with_actions(root, cx).children(sidebar).child(content).into_any_element()
    }
}

impl Finder {
    /// Laid out as made (see `laid_out`): the icons over their picture, and
    /// under them the bar to install its app; what's said, over them.
    fn render_laid_out(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        self.list_width = f32::from(window.viewport_size().width);
        self.toolbar_width = self.list_width;
        self.columns = (((self.list_width - 2. * items::GRID_PADDING) / items::cell_width(self.icon_size)).floor() as usize).max(1);
        // (Over the icons: above the bar, if there's one.)
        let above = if self.image.is_some() { crate::disk_images::INSTALL_BAR } else { 0. };
        let notice = self.notice.clone().map(|notice| {
            div()
                .absolute()
                .bottom(px(12. + above))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(div().max_w(px(420.)).px_3().py(px(6.)).rounded(px(8.)).bg(palette.sidebar).border_1().border_color(palette.separator).text_size(px(12.)).child(notice))
        });
        let content = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().flex().child(self.render_files(window, cx)))
            .children(self.render_install_bar(window, cx))
            .children(self.render_jobs(window))
            .children(notice)
            .children(self.render_peek(window, cx))
            .children(self.render_sheet(window, cx))
            .children(self.render_conflict(window, cx));
        let root = div().key_context(CONTEXT).track_focus(&self.focus).size_full().flex().bg(palette.background).text_color(palette.label).text_size(px(13.));
        self.with_actions(root, cx).child(content).into_any_element()
    }
}

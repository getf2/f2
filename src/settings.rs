//! What's remembered between launches: Settings (as Finder's), each
//! folder's view, the sidebar's favorites, the toolbar, the list's columns,
//! the window's frame, recent servers and folders. Kept as JSON in
//! `~/Library/Application Support/NewFinder/settings.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};

use crate::listing::SortKey;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ViewKind {
    Icons,
    List,
    Columns,
    /// A big preview of the one selected, the folder's thumbnails below.
    Gallery,
}

/// What's done with a report of F2 having quit unexpectedly (see
/// `reports`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CrashReports {
    /// Asked at the next start.
    #[default]
    Ask,
    Send,
    Never,
}

/// The keys held to drag a window from anywhere in it (or size it from
/// near an edge): see `windows::drag`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DragKeys {
    Off,
    ControlCommand,
    OptionCommand,
    ControlOption,
}

impl DragKeys {
    pub const ALL: [DragKeys; 4] = [DragKeys::ControlCommand, DragKeys::OptionCommand, DragKeys::ControlOption, DragKeys::Off];

    pub fn label(self) -> &'static str {
        match self {
            DragKeys::Off => "Off",
            DragKeys::ControlCommand => "⌃⌘",
            DragKeys::OptionCommand => "⌥⌘",
            DragKeys::ControlOption => "⌃⌥",
        }
    }

    /// As CGEventFlags has them (none: off).
    pub fn flags(self) -> u64 {
        const CONTROL: u64 = 0x40000;
        const OPTION: u64 = 0x80000;
        const COMMAND: u64 = 0x100000;
        match self {
            DragKeys::Off => 0,
            DragKeys::ControlCommand => CONTROL | COMMAND,
            DragKeys::OptionCommand => OPTION | COMMAND,
            DragKeys::ControlOption => CONTROL | OPTION,
        }
    }
}

/// The screen Quick Open comes up on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum QuickScreen {
    /// The one the pointer's on.
    Pointer,
    /// The one the app in front's window is on.
    ActiveWindow,
    /// The main one (with the menu bar: see System Settings > Displays).
    Main,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GroupBy {
    None,
    Kind,
    DateModified,
    DateAdded,
    Size,
    Tags,
}

/// A column of the list after Name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Column {
    DateModified,
    DateCreated,
    DateAdded,
    DateLastOpened,
    Size,
    Kind,
    Tags,
}

impl Column {
    pub const ALL: [Column; 7] =
        [Column::DateModified, Column::DateCreated, Column::DateAdded, Column::DateLastOpened, Column::Size, Column::Kind, Column::Tags];

    pub fn title(self) -> &'static str {
        match self {
            Column::DateModified => "Date Modified",
            Column::DateCreated => "Date Created",
            Column::DateAdded => "Date Added",
            Column::DateLastOpened => "Date Last Opened",
            Column::Size => "Size",
            Column::Kind => "Kind",
            Column::Tags => "Tags",
        }
    }

    pub fn default_width(self) -> f32 {
        match self {
            Column::Size => 80.,
            Column::Kind => 150.,
            Column::Tags => 110.,
            _ => 170.,
        }
    }

    pub fn sort_key(self) -> Option<SortKey> {
        Some(match self {
            Column::DateModified => SortKey::Modified,
            Column::DateCreated => SortKey::Created,
            Column::DateAdded => SortKey::Added,
            Column::DateLastOpened => SortKey::LastOpened,
            Column::Size => SortKey::Size,
            Column::Kind => SortKey::Kind,
            Column::Tags => return None,
        })
    }
}

/// How a folder is shown.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderView {
    pub view: ViewKind,
    pub sort: SortKey,
    pub ascending: bool,
    pub group: GroupBy,
    /// Icon view's icons, in points.
    pub icon_size: f32,
    /// The list's and icons' text, in points.
    pub text_size: f32,
    /// Icon view's icons where they've been put (Sort By: None); None:
    /// as Finder has the folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free: Option<bool>,
}

impl Default for FolderView {
    fn default() -> Self {
        FolderView { view: ViewKind::List, sort: SortKey::Name, ascending: true, group: GroupBy::None, icon_size: 64., text_size: 13., free: None }
    }
}

/// A sidebar favorite: a folder, named as you like.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Favorite {
    pub path: PathBuf,
    pub name: Option<String>,
}

/// Which sidebar sections and items show (Finder's Settings > Sidebar).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Sidebar {
    pub recents: bool,
    pub airdrop: bool,
    pub icloud: bool,
    pub cloud_storage: bool,
    pub external_disks: bool,
    pub network: bool,
    /// Servers over SSH (shown only when there's one: ~/.ssh/config's or
    /// one added).
    pub servers: bool,
    pub tags: bool,
    pub trash: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar { recents: true, airdrop: true, icloud: true, cloud_storage: true, external_disks: true, network: true, servers: true, tags: true, trash: true }
    }
}

/// Toolbar items, in Finder's order; which show is in `Settings::toolbar`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tool {
    BackForward,
    /// The enclosing folder (Finder has no button for it).
    Up,
    View,
    Group,
    Share,
    Tags,
    Action,
    NewFolder,
    Delete,
    GetInfo,
    Search,
}

impl Tool {
    pub const ALL: [Tool; 11] = [
        Tool::BackForward,
        Tool::Up,
        Tool::View,
        Tool::Group,
        Tool::Share,
        Tool::Tags,
        Tool::Action,
        Tool::NewFolder,
        Tool::Delete,
        Tool::GetInfo,
        Tool::Search,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tool::BackForward => "Back/Forward",
            Tool::Up => "Enclosing Folder",
            Tool::View => "View",
            Tool::Group => "Group",
            Tool::Share => "Share",
            Tool::Tags => "Edit Tags",
            Tool::Action => "Action",
            Tool::NewFolder => "New Folder",
            Tool::Delete => "Delete",
            Tool::GetInfo => "Get Info",
            Tool::Search => "Search",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Finder's Settings.
    pub show_all_extensions: bool,
    /// Where new windows open (None: home).
    pub new_window_folder: Option<PathBuf>,
    /// A window opened as F2 starts (else it starts in the menu bar: its
    /// Dock icon, ⌘N or the menu bar item opens one).
    pub open_window_at_launch: bool,
    pub folders_on_top: bool,
    pub warn_extension_change: bool,
    pub warn_empty_trash: bool,
    /// A soft sound when files are dropped (on a folder, in a window).
    pub drop_sound: bool,
    pub crash_reports: CrashReports,
    /// The newest crash report looked at (a Unix time; 0: none yet).
    pub crashes_seen: i64,
    /// Search the whole Mac (else the folder shown) when a search starts.
    pub search_this_mac: bool,
    /// Typing starts a search (else it selects by name, as in Finder).
    pub typing_searches: bool,
    /// Work out folders' sizes in the list.
    pub calculate_sizes: bool,
    pub sidebar: Sidebar,
    /// The sidebar's Favorites (None: Finder's default ones).
    pub favorites: Option<Vec<Favorite>>,
    pub toolbar: Vec<Tool>,
    /// The Up button's been put in the toolbar (it came after toolbars
    /// were kept: one kept without it gets it, once).
    #[serde(default)]
    pub toolbar_up: bool,
    /// The list's columns after Name, in order, with their widths.
    pub columns: Vec<(Column, f32)>,
    pub default_view: FolderView,
    pub folder_views: HashMap<PathBuf, FolderView>,
    pub show_sidebar: bool,
    pub show_path_bar: bool,
    pub show_status_bar: bool,
    pub show_preview_pane: bool,
    /// The last window's frame: x, y, width, height.
    pub window_frame: Option<[f32; 4]>,
    pub recent_servers: Vec<String>,
    /// Servers over SSH added in F2 (Add Server…, or connected to by
    /// address), and ~/.ssh/config's Hosts left out of the sidebar.
    pub servers: Vec<SavedServer>,
    pub hidden_servers: Vec<String>,
    pub recent_folders: Vec<PathBuf>,
    /// Full Disk Access was asked for (once: after that, the sidebar's
    /// link).
    pub asked_full_disk_access: bool,
    /// Show the windows Finder opens here instead (an experiment: Finder's
    /// Dock icon can't be removed).
    pub take_finder_windows: bool,
    /// The terminal and editor "Open in" uses (bundle identifiers; None:
    /// the first installed, see `tools`).
    pub terminal: Option<String>,
    pub editor: Option<String>,
    /// The shortcut that brings up Quick Open in any app: one of
    /// `quick_panel::SHORTCUTS`, or one recorded ("ctrl-alt-k"); "off".
    pub quick_open_shortcut: String,
    pub quick_open_screen: QuickScreen,
    /// Quick Open's font (see `quick_panel::FONTS`), and its search
    /// field's text size.
    pub quick_open_font: String,
    pub quick_open_text_size: f32,
    /// Keep a history of what's copied, this many at most.
    pub clipboard_history: bool,
    pub clipboard_keep: usize,
    /// Return on a clip or an emoji pastes it into the app in front (with
    /// Accessibility); else it's only copied.
    pub paste_on_return: bool,
    /// Shortcuts (in any app) to Quick Open's clipboard history, and its
    /// emoji ("shift-cmd-v"; None: none).
    pub clipboard_shortcut: Option<String>,
    pub emoji_shortcut: Option<String>,
    /// A shortcut (in any app) to Quick Open with what's selected there: to
    /// translate, define, search or count it.
    pub selection_shortcut: Option<String>,
    /// Window management's shortcuts, by place (`windows::Place::key`),
    /// and the gap round windows it places, in points.
    pub window_shortcuts: std::collections::BTreeMap<String, String>,
    pub window_gap: f32,
    pub window_drag: DragKeys,
    /// Shortcuts (in any app) to anything else Quick Open does: a command
    /// by its key, an app or a folder ("open:<path>"), a quicklink or a
    /// snippet ("quicklink:<name>"). See `keyboard`.
    pub shortcuts: std::collections::BTreeMap<String, String>,
    /// F2's own keys changed from Finder's: by action ("finder::GoBack"),
    /// the keys it has instead (none, maybe). See `keymap`.
    pub window_keys: std::collections::BTreeMap<String, Vec<String>>,
    /// Quick Open's own links (see `Quicklink`).
    pub quicklinks: Vec<Quicklink>,
    /// Text kept to paste, and typed keywords that expand into it (with
    /// `snippet_expansion`).
    pub snippets: Vec<Snippet>,
    pub snippet_expansion: bool,
    /// The welcome's been shown.
    pub onboarded: bool,
    /// Windows show two panes (the last window's choice).
    pub dual_pane: bool,
    /// Windows keep two frames for drawing, not GPUI's three: a full-size
    /// surface less each (see `macos::set_drawables`).
    pub lean_drawing: bool,
    pub shots: Shots,
    /// Extensions in Quick Open (src/extensions.rs, EXTENSIONS.md): on
    /// unless set false, here, by hand. See `extensions_on`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<bool>,
    /// Extensions' settings, by extension and preference (their passwords
    /// are in the keychain), and those turned off.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extension_preferences: std::collections::BTreeMap<String, std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disabled_extensions: Vec<String>,
    /// Those installed from the store (src/store.rs), by id, and the
    /// version each is.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub store_installed: std::collections::BTreeMap<String, String>,
    /// Extensions that are programs (in any language, run as you, not in
    /// F2's sandbox) run: those allowed, by id, in the folder each was in
    /// when it was.
    pub extension_developer_mode: bool,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub allowed_extension_programs: std::collections::BTreeMap<String, String>,
    /// Folder Rules (src/rules.rs), in the order they're run.
    pub rules: Vec<crate::rules::Rule>,
}

/// A server over SSH added in F2: what it's called in the sidebar, and its
/// address ("ssh://me@box.example.com:2222/var/www").
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SavedServer {
    pub name: String,
    pub address: String,
}

/// A link Quick Open opens by name or keyword: a web address or a path,
/// "{query}" in it for what's typed after the keyword ("gh rust").
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Quicklink {
    pub name: String,
    pub keyword: String,
    pub link: String,
}

/// Text to paste: found by name in Quick Open, or typed as its keyword
/// anywhere. "{clipboard}", "{date}", "{time}", "{datetime}", "{day}",
/// "{uuid}" are filled in, and the text cursor goes where "{cursor}" is.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub keyword: String,
    pub text: String,
}

fn quicklinks() -> Vec<Quicklink> {
    let link = |name: &str, keyword: &str, link: &str| Quicklink { name: name.into(), keyword: keyword.into(), link: link.into() };
    vec![
        link("GitHub Search", "gh", "https://github.com/search?q={query}"),
        link("Google Search", "g", "https://www.google.com/search?q={query}"),
        link("Downloads", "dl", "~/Downloads"),
    ]
}

fn snippets() -> Vec<Snippet> {
    let snippet = |name: &str, keyword: &str, text: &str| Snippet { name: name.into(), keyword: keyword.into(), text: text.into() };
    vec![snippet("Today's Date", ";date", "{date}"), snippet("Shrug", ";shrug", "¯\\_(ツ)_/¯")]
}

/// Screenshots: each capture's shortcut ("shift-cmd-4"; None: none), and
/// what happens after one.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Shots {
    pub area: Option<String>,
    pub window: Option<String>,
    pub screen: Option<String>,
    pub text: Option<String>,
    pub color: Option<String>,
    /// How Pick Color copies a color (one of `colors::FORMATS`' ids).
    pub color_format: String,
    pub qr: Option<String>,
    /// Screen recording's shortcuts: the screen the pointer's on, or an
    /// area (again: stop).
    pub record_screen: Option<String>,
    pub record_area: Option<String>,
    /// Scrolling Capture's.
    pub scrolling: Option<String>,
    /// A recording has the microphone's sound in it; shows clicks.
    pub record_audio: bool,
    pub record_clicks: bool,
    /// Put it on the clipboard.
    pub copy: bool,
    /// Show it in the corner (to drag into an app, edit it, save it).
    pub preview: bool,
    /// Save it to `folder` (None: the Desktop) too.
    pub save: bool,
    pub folder: Option<PathBuf>,
    pub sound: bool,
    /// A window's shadow, around it.
    pub shadow: bool,
    /// Screenshots on the Desktop moved into its Screenshots folder.
    pub tidy_desktop: bool,
}

impl Default for Shots {
    fn default() -> Self {
        // (BetterShot's shortcuts.)
        Shots {
            area: Some("shift-cmd-4".into()),
            window: None,
            screen: Some("shift-cmd-3".into()),
            text: Some("shift-cmd-o".into()),
            color: None,
            color_format: "hex".into(),
            qr: None,
            record_screen: None,
            record_area: None,
            scrolling: None,
            record_audio: false,
            record_clicks: false,
            copy: true,
            preview: true,
            save: false,
            folder: None,
            sound: false,
            shadow: true,
            tidy_desktop: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_all_extensions: false,
            new_window_folder: None,
            open_window_at_launch: true,
            folders_on_top: true,
            warn_extension_change: true,
            warn_empty_trash: true,
            drop_sound: true,
            crash_reports: CrashReports::Ask,
            crashes_seen: 0,
            search_this_mac: false,
            typing_searches: false,
            calculate_sizes: false,
            sidebar: Sidebar::default(),
            favorites: None,
            toolbar: vec![Tool::BackForward, Tool::Up, Tool::View, Tool::Group, Tool::Share, Tool::Tags, Tool::Action, Tool::Search],
            toolbar_up: true,
            columns: [Column::DateModified, Column::Size, Column::Kind].map(|c| (c, c.default_width())).to_vec(),
            default_view: FolderView::default(),
            folder_views: HashMap::new(),
            show_sidebar: true,
            show_path_bar: true,
            show_status_bar: true,
            show_preview_pane: false,
            window_frame: None,
            recent_servers: Vec::new(),
            servers: Vec::new(),
            hidden_servers: Vec::new(),
            recent_folders: Vec::new(),
            asked_full_disk_access: false,
            take_finder_windows: false,
            terminal: None,
            editor: None,
            quick_open_shortcut: "cmd-space".into(),
            quick_open_screen: QuickScreen::Pointer,
            quick_open_font: "system".into(),
            quick_open_text_size: 22.,
            clipboard_history: true,
            clipboard_keep: 200,
            paste_on_return: true,
            clipboard_shortcut: None,
            emoji_shortcut: None,
            selection_shortcut: None,
            window_shortcuts: Default::default(),
            window_gap: 0.,
            window_drag: DragKeys::ControlCommand,
            shortcuts: Default::default(),
            window_keys: Default::default(),
            quicklinks: quicklinks(),
            snippets: snippets(),
            snippet_expansion: true,
            onboarded: false,
            dual_pane: false,
            lean_drawing: true,
            shots: Shots::default(),
            extensions: None,
            extension_preferences: Default::default(),
            disabled_extensions: Vec::new(),
            store_installed: Default::default(),
            extension_developer_mode: false,
            allowed_extension_programs: Default::default(),
            rules: Vec::new(),
        }
    }
}

impl Global for Settings {}

impl Settings {
    /// Whether Quick Open has extensions (see `extensions`): on, unless
    /// it's turned off.
    pub fn extensions_on(&self) -> bool {
        self.extensions.unwrap_or(true)
    }
}

/// Where Finder2 keeps what it remembers (settings, the Trash's records,
/// what you open).
pub fn data_dir() -> PathBuf {
    file().parent().map(Path::to_path_buf).unwrap_or_default()
}

fn file() -> PathBuf {
    // (NEW_FINDER_SETTINGS: a folder to keep them in instead, for testing.)
    if let Some(dir) = std::env::var_os("NEW_FINDER_SETTINGS") {
        return PathBuf::from(dir).join("settings.json");
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    home.join("Library/Application Support/NewFinder/settings.json")
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read(file()).ok().and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok()).map(Settings::migrated).unwrap_or_default()
    }

    /// Kept settings brought up to date: the Up button, after Back/Forward
    /// (once: taken out after, it stays out).
    fn migrated(mut self) -> Settings {
        if !self.toolbar_up {
            self.toolbar_up = true;
            if !self.toolbar.contains(&Tool::Up) {
                let at = self.toolbar.iter().position(|t| *t == Tool::BackForward).map_or(0, |i| i + 1);
                self.toolbar.insert(at, Tool::Up);
            }
        }
        self
    }

    pub fn get(cx: &App) -> &Settings {
        cx.global::<Settings>()
    }

    /// Change the settings and save them.
    pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) {
        f(cx.global_mut::<Settings>());
        let settings = cx.global::<Settings>().clone();
        // (Off the main thread: it's a small file, but a disk can be slow.)
        cx.background_executor()
            .spawn(async move {
                let path = file();
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Ok(json) = serde_json::to_vec_pretty(&settings) {
                    let tmp = path.with_extension("json.tmp");
                    if std::fs::write(&tmp, json).is_ok() {
                        let _ = std::fs::rename(tmp, path);
                    }
                }
            })
            .detach();
    }

    /// How `dir` is shown: as it was last, else the default.
    pub fn view_for(&self, dir: &std::path::Path) -> FolderView {
        self.folder_views.get(dir).copied().unwrap_or(self.default_view)
    }

    /// Where new windows open.
    pub fn new_window_folder(&self) -> PathBuf {
        self.new_window_folder.clone().filter(|p| p.is_dir()).unwrap_or_else(|| {
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
        })
    }

    /// Remember a folder visited, for Go > Recent Folders.
    pub fn visited(&mut self, dir: &std::path::Path) {
        self.recent_folders.retain(|p| p != dir);
        self.recent_folders.insert(0, dir.to_path_buf());
        self.recent_folders.truncate(10);
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn up_put_in_once() {
        use super::Tool;
        let read = |json: &str| serde_json::from_str::<Settings>(json).unwrap().migrated();
        assert_eq!(read(r#"{"toolbar": ["BackForward", "View", "Search"]}"#).toolbar, [Tool::BackForward, Tool::Up, Tool::View, Tool::Search]);
        assert_eq!(read(r#"{"toolbar": ["View"]}"#).toolbar, [Tool::Up, Tool::View]);
        assert_eq!(read(r#"{"toolbar": ["View"], "toolbar_up": true}"#).toolbar, [Tool::View], "taken out, it stays out");
        assert_eq!(read("{}").toolbar, Settings::default().toolbar);
    }

    #[test]
    fn drag_keys() {
        use super::DragKeys;
        let read = |json: &str| serde_json::from_str::<Settings>(json).unwrap();
        assert_eq!(read("{}").window_drag, DragKeys::ControlCommand, "on unless turned off");
        assert_eq!(read(r#"{"window_drag": "Off"}"#).window_drag.flags(), 0);
        let flags: Vec<u64> = DragKeys::ALL.iter().map(|k| k.flags()).collect();
        assert!(flags.iter().enumerate().all(|(i, f)| flags.iter().skip(i + 1).all(|g| g != f)), "each its own keys");
        // (⌃ ⌥ ⌘ as CGEventFlags has them.)
        assert_eq!(DragKeys::ControlCommand.flags(), 0x40000 | 0x100000);
    }
}

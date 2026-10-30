//! Right-click menus like Finder's: for the selected items, for the empty
//! space of a folder, and for places in the sidebar; and what their items
//! (and the matching keyboard shortcuts) do. File operations are recorded
//! for undo.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::*;
use gpui_kit::component::input::{InputEvent, InputState};

use super::places::{Goes, Place, Virtual};
use super::sheets::Sheet;
use super::undo::Op;
use crate::tools::{self, Kind};
use super::{Finder, Renaming, View, home};
use crate::fileops::{self, How};
use crate::listing::{Entry, SortKey};
use crate::macos::{self, COMMAND, MenuItem, OPTION, SHIFT};
use crate::quick_actions::{self, QuickAction};
use crate::settings::{GroupBy, Settings};

/// `NSEventModifierFlags` for ⌃ in a menu's key equivalent.
const CONTROL: u64 = 1 << 18;

/// Finder's own tags: name, and color number.
pub const TAGS: [(&str, u8); 7] =
    [("Red", 6), ("Orange", 7), ("Yellow", 5), ("Green", 2), ("Blue", 4), ("Purple", 3), ("Gray", 1)];

/// What was right-clicked.
pub(super) enum Target {
    /// The selected items.
    Selection,
    /// The folder shown, outside any item.
    Background,
    Place(Place),
}

#[derive(Clone)]
pub(super) enum Command {
    Open(Vec<Entry>),
    /// A server in the sidebar: added, its terminal, changed, taken out.
    AddServer,
    ServerTerminal(crate::sftp::Dest),
    ServerInOtherPane(crate::sftp::Dest),
    EditServer(usize),
    RemoveServer(crate::sftp::Dest),
    /// An image as the desktop picture.
    SetWallpaper(PathBuf),
    OpenInTab(PathBuf),
    OpenWith(Vec<PathBuf>, PathBuf),
    /// In the terminal, or the editor, Settings choose.
    OpenIn(Kind, Vec<PathBuf>),
    Trash(Vec<PathBuf>),
    DeleteImmediately(Vec<PathBuf>),
    PutBack(Vec<PathBuf>),
    EmptyTrash,
    GetInfo(Vec<Entry>),
    Rename(PathBuf),
    RenameItems(Vec<PathBuf>),
    /// Folders' color, and a symbol or an emoji on them.
    CustomizeFolders(Vec<PathBuf>),
    Compress(Vec<PathBuf>),
    OpenInOtherPane(PathBuf),
    Uninstall(PathBuf),
    /// Archives, into a folder (None: each beside itself).
    Extract(Vec<PathBuf>, Option<PathBuf>),
    Duplicate(Vec<PathBuf>),
    MakeAlias(Vec<PathBuf>),
    QuickLook,
    Copy(Vec<PathBuf>),
    /// Cut, to be moved where they're pasted.
    Cut(Vec<PathBuf>),
    CopyPath(Vec<PathBuf>),
    /// Add (true) or remove a tag.
    Tag(Vec<PathBuf>, &'static str, bool),
    QuickAction(QuickAction, Vec<PathBuf>),
    Shortcut(String, Vec<PathBuf>),
    Download(Vec<PathBuf>),
    RemoveDownload(Vec<PathBuf>),
    ShowOriginal(PathBuf),
    ShowInEnclosingFolder(PathBuf),
    ShowInFinder(PathBuf),
    Eject(PathBuf),
    AddToSidebar(Vec<PathBuf>),
    RemoveFromSidebar(usize),
    GoTo(Goes),
    NewFolder,
    /// An empty text file.
    NewFile,
    NewFolderWith(Vec<PathBuf>),
    Paste,
    MoveHere,
    ViewAs(View),
    SortBy(SortKey),
    /// Icon view: the icons where they've been put (Sort By: None).
    SortNone,
    /// Freely arranged: each icon in the grid's nearest place, or all lined
    /// up by a key.
    CleanUp(Option<SortKey>),
    GroupBy(GroupBy),
    ViewOptions,
    ToggleHidden,
    TidyUp,
    /// Settings › Folder Rules, with a new one for the folder.
    FolderRules,
}

/// A menu being built: its lines, and what each one you can choose does.
#[derive(Default)]
struct Menu {
    items: Vec<MenuItem>,
    commands: Vec<Command>,
    depth: u8,
}

impl Menu {
    fn add(&mut self, title: impl Into<String>, command: Command) -> &mut MenuItem {
        let tag = self.commands.len() as i32;
        self.commands.push(command);
        self.items.push(MenuItem { title: Some(title.into()), tag, depth: self.depth, enabled: true, ..Default::default() });
        self.items.last_mut().unwrap()
    }

    /// (Never first in a menu, nor two in a row.)
    fn separator(&mut self) {
        if self.items.last().is_some_and(|i| i.title.is_some() && (i.depth == self.depth || self.depth == 0)) {
            self.items.push(MenuItem { depth: self.depth, ..Default::default() });
        }
    }

    fn submenu(&mut self, title: impl Into<String>, build: impl FnOnce(&mut Menu)) {
        self.items.push(MenuItem { title: Some(title.into()), tag: -1, enabled: true, ..Default::default() });
        self.depth = 1;
        build(self);
        self.depth = 0;
    }

    /// Continuity Camera's Import from iPhone or iPad (Take Photo, Scan
    /// Documents, Add Sketch: macOS fills it in), into `folder`.
    fn import(&mut self, folder: PathBuf) {
        let title = Some("Import from iPhone or iPad".into());
        self.items.push(MenuItem { title, import_to: Some(folder), depth: self.depth, enabled: true, ..Default::default() });
    }

    /// The system's Share… item (it brings up the share sheet itself).
    fn share(&mut self, paths: Vec<PathBuf>) {
        let title = Some("Share…".into());
        self.items.push(MenuItem { title, share: paths, depth: self.depth, enabled: true, ..Default::default() });
    }
}

/// A folder as an item, for Get Info on it.
pub(super) fn folder_entry(path: &Path) -> Entry {
    Entry::from_path(path.to_path_buf()).unwrap_or_else(|| Entry {
        name: super::display_name(path),
        path: path.to_path_buf(),
        is_dir: true,
        ..Default::default()
    })
}

/// The terminal's and the editor's names, for "Open in" (None: none
/// installed).
type ToolNames = [Option<String>; 2];

fn tool_names(cx: &App) -> ToolNames {
    [Kind::Terminal, Kind::Editor].map(|kind| tools::chosen(kind, Settings::get(cx)).map(|t| t.name))
}

/// "Open in Ghostty", "Open in Zed" (as Settings choose).
fn open_in_items(m: &mut Menu, tools: &ToolNames, paths: &[PathBuf]) {
    for (kind, name, key) in [(Kind::Terminal, &tools[0], "t"), (Kind::Editor, &tools[1], "e")] {
        if let Some(name) = name {
            m.add(format!("Open in {name}"), Command::OpenIn(kind, paths.to_vec())).key(key, COMMAND | OPTION);
        }
    }
}

fn paths_of(entries: &[Entry]) -> Vec<PathBuf> {
    entries.iter().map(|e| e.path.clone()).collect()
}

pub(super) fn parents(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect()
}

/// Show a built menu at the mouse, then run what's chosen. (Not from inside
/// the event handler: the menu runs its own event loop until it closes,
/// and GPUI must not be mid-update then.)
fn pop_up(menu: Menu, window: &mut Window, cx: &mut Context<Finder>) {
    let Menu { mut items, commands, .. } = menu;
    while items.last().is_some_and(|i| i.title.is_none()) {
        items.pop();
    }
    cx.spawn_in(window, async move |this, cx| {
        // A frame first, so what was right-clicked shows selected under it.
        cx.background_executor().timer(Duration::from_millis(30)).await;
        let Some(command) = macos::popup_menu(&items).and_then(|tag| commands.get(tag as usize)).cloned() else {
            return;
        };
        this.update_in(cx, |this, window, cx| this.run(command, window, cx)).ok();
    })
    .detach();
}

impl Finder {
    /// ⌥⌘T / ⌥⌘E: the selection (or this folder) in the terminal, or the
    /// editor, Settings choose.
    pub(super) fn open_in(&mut self, kind: Kind, cx: &mut Context<Self>) {
        let mut paths = self.selection_paths();
        if paths.is_empty() && !self.flat() && !self.in_trash() {
            paths.push(self.cwd.clone());
        }
        match tools::chosen(kind, Settings::get(cx)) {
            Some(tool) if !paths.is_empty() => tools::open(&tool, kind, &paths),
            Some(_) => {}
            None if kind == Kind::Terminal => self.show_notice("No terminal app was found.", cx),
            None => self.show_notice("No code editor was found (like Zed, Cursor or Visual Studio Code).", cx),
        }
    }

    pub(super) fn show_menu(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let menu = match target {
            Target::Selection => {
                let items = self.selection();
                if items.is_empty() {
                    return;
                }
                if items.iter().any(|e| crate::remote::is_remote(&e.path)) {
                    self.server_item_menu(&items, &tool_names(cx))
                } else {
                    self.item_menu(&items, &tool_names(cx), self.other_pane_folder(cx))
                }
            }
            Target::Background if self.remote_here().is_some() => self.server_background_menu(&tool_names(cx)),
            Target::Background => self.background_menu(&tool_names(cx)),
            Target::Place(place) => self.place_menu(&place, cx),
        };
        pop_up(menu, window, cx);
    }

    /// The toolbar's Action (⋯) menu: the selection's, else the folder's.
    pub(super) fn show_action_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.chosen.is_empty() { Target::Background } else { Target::Selection };
        self.show_menu(target, window, cx);
    }

    /// The toolbar's Group menu.
    pub(super) fn show_group_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut m = Menu::default();
        self.group_items(&mut m);
        pop_up(m, window, cx);
    }

    /// The toolbar's Tags menu, for the selection.
    pub(super) fn show_tags_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selection();
        if items.is_empty() {
            return self.show_notice("Select something to tag.", cx);
        }
        let mut m = Menu::default();
        Self::tag_items(&mut m, &items);
        pop_up(m, window, cx);
    }

    /// The toolbar's Share menu, for the selection.
    pub(super) fn show_share_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selection_paths();
        if paths.is_empty() {
            return self.show_notice("Select something to share.", cx);
        }
        let mut m = Menu::default();
        m.share(paths);
        pop_up(m, window, cx);
    }

    fn tag_items(m: &mut Menu, items: &[Entry]) {
        let paths = paths_of(items);
        for (name, color) in TAGS {
            let all = items.iter().all(|e| e.tags.iter().any(|(t, _)| t == name));
            m.add(name, Command::Tag(paths.clone(), name, !all)).checked(all).swatch(color);
        }
    }

    fn group_items(&self, m: &mut Menu) {
        for (group, title) in [
            (GroupBy::None, "None"),
            (GroupBy::Kind, "Kind"),
            (GroupBy::DateModified, "Date Modified"),
            (GroupBy::DateAdded, "Date Added"),
            (GroupBy::Size, "Size"),
            (GroupBy::Tags, "Tags"),
        ] {
            m.add(title, Command::GroupBy(group)).checked(self.group == group);
        }
    }

    /// The menu for the selected items (one, or several). `other_pane`: the
    /// other pane's folder, in a window showing two.
    fn item_menu(&self, items: &[Entry], tools: &ToolNames, other_pane: Option<PathBuf>) -> Menu {
        let mut m = Menu::default();
        let (first, one, n) = (&items[0], items.len() == 1, items.len());
        let paths = paths_of(items);
        let named = |what: &str| if one { format!("{what} “{}”", first.name) } else { format!("{what} {n} Items") };
        m.add("Open", Command::Open(items.to_vec())).key("o", COMMAND);
        if one && first.is_dir && !first.is_bundle() && first.link.is_none() {
            m.add("Open in New Tab", Command::OpenInTab(first.path.clone()));
            m.add("Open in Other Pane", Command::OpenInOtherPane(first.path.clone()));
        }
        if one && !first.is_dir {
            let apps = macos::apps_for(&first.path);
            if !apps.is_empty() {
                m.submenu("Open With", |m| {
                    for (i, app) in apps.iter().enumerate() {
                        let app_name = app.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let title = if i == 0 { format!("{app_name} (default)") } else { app_name };
                        m.add(title, Command::OpenWith(paths.clone(), app.clone())).icon(app.clone());
                        if i == 0 {
                            m.separator();
                        }
                    }
                });
            }
        }
        if one && first.path.extension().is_some_and(|e| e == "app") && !self.in_trash() && crate::uninstall::refused(&first.path).is_none() {
            m.add("Uninstall…", Command::Uninstall(first.path.clone()));
        }
        if one && first.is_dir && !first.is_bundle() && !self.in_trash() {
            m.import(first.path.clone());
        }
        if one && first.is_bundle() {
            // A package is a folder inside: open it as one.
            m.add("Show Package Contents", Command::Open(vec![folder_entry(&first.path.join("Contents"))]));
        }
        if !self.in_trash() && items.iter().all(|e| e.link.is_none()) {
            m.separator();
            open_in_items(&mut m, tools, &paths);
        }
        m.separator();
        if self.in_trash() {
            m.add("Put Back", Command::PutBack(paths.clone()));
            m.add("Delete Immediately…", Command::DeleteImmediately(paths.clone())).key("\u{8}", COMMAND | OPTION);
            m.add(format!("Empty {}", macos::bin()), Command::EmptyTrash).key("\u{8}", COMMAND | SHIFT);
        } else {
            m.add(format!("Move to {}", macos::bin()), Command::Trash(paths.clone())).key("\u{8}", COMMAND);
            m.add("Delete Immediately…", Command::DeleteImmediately(paths.clone())).key("\u{8}", COMMAND | OPTION).alternate();
        }
        m.separator();
        m.add("Get Info", Command::GetInfo(items.to_vec())).key("i", COMMAND);
        if one {
            m.add("Rename", Command::Rename(first.path.clone()));
        } else {
            m.add("Rename…", Command::RenameItems(paths.clone()));
        }
        m.add(named("Compress"), Command::Compress(paths.clone()));
        if !self.in_trash() && items.iter().all(|e| !e.is_dir && crate::archives::stem(&e.path).is_some()) {
            m.add(named("Extract"), Command::Extract(paths.clone(), None));
            if let Some(other) = other_pane {
                m.add("Extract to Other Pane", Command::Extract(paths.clone(), Some(other)));
            }
        }
        m.add("Duplicate", Command::Duplicate(paths.clone())).key("d", COMMAND);
        m.add(if one { "Make Alias" } else { "Make Aliases" }, Command::MakeAlias(paths.clone()));
        m.add(format!("Quick Look “{}”", first.name), Command::QuickLook).key("y", COMMAND);
        if !self.flat() && n > 1 {
            m.add(format!("New Folder with Selection ({n} Items)"), Command::NewFolderWith(paths.clone())).key("n", COMMAND | CONTROL);
        }
        m.separator();
        m.add(named("Cut"), Command::Cut(paths.clone())).key("x", COMMAND);
        m.add(named("Copy"), Command::Copy(paths.clone())).key("c", COMMAND);
        let as_path = if one { format!("Copy “{}” as Pathname", first.name) } else { format!("Copy {n} Items as Pathnames") };
        m.add(as_path, Command::CopyPath(paths.clone())).key("c", COMMAND | OPTION).alternate();
        m.share(paths.clone());
        m.separator();
        m.submenu("Tags", |m| Self::tag_items(m, items));
        if !self.in_trash() && items.iter().all(|e| super::customize::customizable(&e.path, e.is_dir, e.is_bundle()) && e.link.is_none() && !e.alias) {
            m.add(if one { "Customize Folder…" } else { "Customize Folders…" }, Command::CustomizeFolders(paths.clone()));
        }
        if one && !first.is_dir && quick_actions::is_image(&first.path) {
            m.add("Set Desktop Picture", Command::SetWallpaper(first.path.clone()));
        }
        let actions = quick_actions::for_items(items);
        let shortcuts = quick_actions::shortcuts();
        if !actions.is_empty() || !shortcuts.is_empty() {
            m.submenu("Quick Actions", |m| {
                for action in actions {
                    m.add(action.title(), Command::QuickAction(action, paths.clone()));
                }
                if !shortcuts.is_empty() {
                    m.separator();
                    for name in shortcuts {
                        m.add(name.clone(), Command::Shortcut(name, paths.clone()));
                    }
                }
            });
        }
        if items.iter().any(|e| e.cloud.is_some()) {
            m.separator();
            if items.iter().any(|e| e.cloud == Some(macos::Cloud::NotDownloaded)) {
                m.add("Download Now", Command::Download(paths.clone()));
            }
            if items.iter().any(|e| e.cloud == Some(macos::Cloud::Current)) {
                m.add("Remove Download", Command::RemoveDownload(paths.clone()));
            }
        }
        m.separator();
        if one && first.alias {
            m.add("Show Original", Command::ShowOriginal(first.path.clone())).key("r", COMMAND);
        } else if one && self.flat() {
            m.add("Show in Enclosing Folder", Command::ShowInEnclosingFolder(first.path.clone())).key("r", COMMAND);
        }
        if items.iter().all(|e| e.is_dir && !e.is_bundle() && e.link.is_none()) {
            m.add("Add to Sidebar", Command::AddToSidebar(paths.clone())).key("t", COMMAND | CONTROL);
        }
        if one && first.link.is_none() {
            m.add("Show in Finder", Command::ShowInFinder(first.path.clone()));
        }
        if one && first.is_dir && macos::volume_flags(&first.path).1 {
            m.separator();
            m.add(format!("Eject “{}”", first.name), Command::Eject(first.path.clone())).key("e", COMMAND);
        }
        m
    }

    /// A server's items' menu: what can be done there.
    fn server_item_menu(&self, items: &[Entry], tools: &ToolNames) -> Menu {
        let mut m = Menu::default();
        let (first, one, n) = (&items[0], items.len() == 1, items.len());
        let paths = paths_of(items);
        let named = |what: &str| if one { format!("{what} “{}”", first.name) } else { format!("{what} {n} Items") };
        m.add("Open", Command::Open(items.to_vec())).key("o", COMMAND);
        if one && first.is_dir {
            m.add("Open in New Tab", Command::OpenInTab(first.path.clone()));
            m.add("Open in Other Pane", Command::OpenInOtherPane(first.path.clone()));
        }
        if let Some(name) = &tools[0] {
            m.separator();
            m.add(format!("Open in {name}"), Command::OpenIn(Kind::Terminal, paths.clone())).key("t", COMMAND | OPTION);
        }
        m.separator();
        m.add("Delete Immediately…", Command::DeleteImmediately(paths.clone())).key("\u{8}", COMMAND);
        m.separator();
        if one {
            m.add("Rename", Command::Rename(first.path.clone()));
        }
        m.separator();
        m.add(named("Copy"), Command::Copy(paths.clone())).key("c", COMMAND);
        m.add(if one { "Copy Path" } else { "Copy Paths" }, Command::CopyPath(paths.clone())).key("c", COMMAND | OPTION);
        if items.iter().all(|e| e.is_dir) {
            m.separator();
            m.add("Add to Sidebar", Command::AddToSidebar(paths.clone())).key("t", COMMAND | CONTROL);
        }
        m
    }

    /// A server's folder's menu (its background).
    fn server_background_menu(&self, tools: &ToolNames) -> Menu {
        let mut m = Menu::default();
        m.add("New Folder", Command::NewFolder).key("n", COMMAND | SHIFT);
        m.add("New File", Command::NewFile).key("n", COMMAND | OPTION);
        if let Some(name) = &tools[0] {
            m.separator();
            m.add(format!("Open in {name}"), Command::OpenIn(Kind::Terminal, vec![self.cwd.clone()])).key("t", COMMAND | OPTION);
        }
        m.separator();
        let pasted = macos::pasted_files().len();
        m.add(if pasted > 1 { format!("Paste {pasted} Items") } else { "Paste Item".into() }, Command::Paste).key("v", COMMAND).enabled(pasted > 0);
        m.separator();
        m.submenu("View as", |m| {
            m.add("Icons", Command::ViewAs(View::Icons)).key("1", COMMAND).checked(self.view == View::Icons);
            m.add("List", Command::ViewAs(View::List)).key("2", COMMAND).checked(self.view == View::List);
            m.add("Columns", Command::ViewAs(View::Columns)).key("3", COMMAND).checked(self.view == View::Columns);
        });
        m.add(if self.show_hidden { "Hide Hidden Files" } else { "Show Hidden Files" }, Command::ToggleHidden).key(".", COMMAND | SHIFT);
        m
    }

    fn background_menu(&self, tools: &ToolNames) -> Menu {
        let mut m = Menu::default();
        let real = !self.flat();
        m.add("New Folder", Command::NewFolder).key("n", COMMAND | SHIFT).enabled(real);
        m.add("New File", Command::NewFile).key("n", COMMAND | OPTION).enabled(real && !self.in_trash());
        m.separator();
        if real && !self.in_trash() {
            open_in_items(&mut m, tools, std::slice::from_ref(&self.cwd));
            m.separator();
        }
        m.add("Get Info", Command::GetInfo(vec![folder_entry(&self.cwd)])).key("i", COMMAND).enabled(real);
        m.add("Tidy Up", Command::TidyUp).enabled(real && !self.in_trash());
        m.add("Folder Rules…", Command::FolderRules).enabled(real && !self.in_trash());
        if real && !self.in_trash() {
            m.import(self.cwd.clone());
        }
        m.separator();
        if self.in_trash() {
            m.add(format!("Empty {}", macos::bin()), Command::EmptyTrash).key("\u{8}", COMMAND | SHIFT);
            m.separator();
        }
        let files = macos::pasted_files();
        let pasted = files.len();
        // (Cut: ⌘V moves them.)
        let cut = super::transfer::pasting_cut(&files);
        let can = pasted > 0 && real;
        let (paste, move_here) = if pasted > 1 {
            (format!("Paste {pasted} Items"), format!("Move {pasted} Items Here"))
        } else {
            ("Paste Item".into(), "Move Item Here".into())
        };
        if cut {
            m.add(move_here, Command::Paste).key("v", COMMAND).enabled(can);
        } else {
            m.add(paste, Command::Paste).key("v", COMMAND).enabled(can);
            m.add(move_here, Command::MoveHere).key("v", COMMAND | OPTION).enabled(can).alternate();
        }
        m.separator();
        m.submenu("View as", |m| {
            m.add("Icons", Command::ViewAs(View::Icons)).key("1", COMMAND).checked(self.view == View::Icons);
            m.add("List", Command::ViewAs(View::List)).key("2", COMMAND).checked(self.view == View::List);
            m.add("Columns", Command::ViewAs(View::Columns)).key("3", COMMAND).checked(self.view == View::Columns);
            m.add("Gallery", Command::ViewAs(View::Gallery)).key("4", COMMAND).checked(self.view == View::Gallery);
        });
        m.submenu("Group By", |m| self.group_items(m));
        if self.free_layout_on() {
            m.add("Clean Up", Command::CleanUp(None));
            m.submenu("Clean Up By", |m| {
                for (key, title) in [(SortKey::Name, "Name"), (SortKey::Kind, "Kind"), (SortKey::Modified, "Date Modified"), (SortKey::Size, "Size")] {
                    m.add(title, Command::CleanUp(Some(key)));
                }
            });
        }
        m.submenu("Sort By", |m| {
            // (None: where they've been put, in icon view as Finder has it.)
            if self.view == View::Icons && !self.flat() {
                m.add("None", Command::SortNone).checked(self.free);
                m.separator();
            }
            for (key, title) in [
                (SortKey::Name, "Name"),
                (SortKey::Kind, "Kind"),
                (SortKey::Modified, "Date Modified"),
                (SortKey::Created, "Date Created"),
                (SortKey::Added, "Date Added"),
                (SortKey::LastOpened, "Date Last Opened"),
                (SortKey::Size, "Size"),
            ] {
                m.add(title, Command::SortBy(key)).checked(self.sort.0 == key && !(self.free && self.view == View::Icons));
            }
        });
        m.add("Show View Options", Command::ViewOptions).key("j", COMMAND);
        let hidden = if self.show_hidden { "Hide Hidden Files" } else { "Show Hidden Files" };
        m.add(hidden, Command::ToggleHidden).key(".", COMMAND | SHIFT);
        if real {
            m.separator();
            m.add("Show in Finder", Command::ShowInFinder(self.cwd.clone()));
        }
        m
    }

    fn place_menu(&self, place: &Place, cx: &App) -> Menu {
        let mut m = Menu::default();
        m.add("Open", Command::GoTo(place.goes.clone()));
        if let Some(path) = place.folder() {
            m.add("Open in New Tab", Command::OpenInTab(path.clone()));
            m.add("Open in Other Pane", Command::OpenInOtherPane(path.clone()));
            m.add("Get Info", Command::GetInfo(vec![folder_entry(path)]));
            m.add("Show in Finder", Command::ShowInFinder(path.clone()));
        }
        if place.goes == Goes::Virtual(Virtual::Trash) {
            m.separator();
            m.add(format!("Empty {}", macos::bin()), Command::EmptyTrash);
        }
        if let Some(ix) = place.favorite {
            m.separator();
            m.add("Remove from Sidebar", Command::RemoveFromSidebar(ix));
        }
        if let Goes::Server(dest) = &place.goes {
            m.add("Open in Other Pane", Command::ServerInOtherPane(dest.clone()));
            m.add("Open in Terminal", Command::ServerTerminal(dest.clone()));
            m.separator();
            let saved = super::servers::saved_server(dest, cx);
            if let Some(ix) = saved {
                m.add("Edit…", Command::EditServer(ix));
            }
            m.add("Add Server…", Command::AddServer);
            m.separator();
            m.add("Remove from Sidebar", Command::RemoveServer(dest.clone()));
        }
        if place.ejectable
            && let Some(path) = place.folder()
        {
            m.separator();
            m.add(format!("Eject “{}”", place.name), Command::Eject(path.clone()));
        }
        m
    }

    pub(super) fn run(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        // (On a server: done there, or said it can't be yet.)
        if self.run_remote(&command, window, cx) {
            return;
        }
        match command {
            Command::AddServer => self.open_add_server(None, window, cx),
            Command::EditServer(ix) => self.open_add_server(Some(ix), window, cx),
            Command::RemoveServer(dest) => self.remove_server(&dest, cx),
            Command::ServerInOtherPane(dest) => self.connect_in_other_pane(dest, window, cx),
            Command::ServerTerminal(dest) => {
                if let Some(tool) = tools::chosen(Kind::Terminal, Settings::get(cx))
                    && let Err(e) = crate::remote::terminal(&dest, None, &tool.app)
                {
                    self.show_notice(e, cx);
                }
            }
            Command::Open(entries) => self.open_entries(entries, window, cx),
            Command::OpenInTab(path) => self.open_in_tab(path, window, cx),
            Command::OpenIn(kind, paths) => {
                if let Some(tool) = tools::chosen(kind, Settings::get(cx)) {
                    tools::open(&tool, kind, &paths);
                }
            }
            Command::OpenWith(paths, app) => {
                for path in &paths {
                    macos::open_with(path, &app);
                }
            }
            Command::Trash(paths) => self.trash(&paths, cx),
            Command::DeleteImmediately(paths) => {
                self.overlay = Some(Sheet::DeleteNow(paths));
                cx.notify();
            }
            Command::PutBack(paths) => self.put_back(paths, cx),
            Command::EmptyTrash => self.ask_empty_trash(cx),
            Command::GetInfo(entries) => crate::info::open(entries, cx),
            Command::Rename(path) => self.start_rename(path, window, cx),
            Command::RenameItems(paths) => self.open_rename_items(paths, window, cx),
            Command::CustomizeFolders(paths) => self.open_customize(paths, window, cx),
            Command::OpenInOtherPane(path) => self.show_in_other_pane(path, window, cx),
            Command::Uninstall(app) => crate::uninstall::open(app, cx),
            Command::Compress(paths) => self.in_background(false, cx, move || {
                Ok(Op::created(vec![fileops::compress_all(&paths)?]))
            }),
            Command::Extract(paths, into) => self.in_background(false, cx, move || {
                let mut made = Vec::new();
                for archive in &paths {
                    let dir = match &into {
                        Some(dir) => dir.clone(),
                        None => archive.parent().map(Path::to_path_buf).ok_or_else(|| io::Error::other("no enclosing folder"))?,
                    };
                    made.push(crate::archives::extract(archive, &dir)?);
                }
                Ok(Op::created(made))
            }),
            Command::Duplicate(paths) => self.in_background(false, cx, move || {
                Ok(Op::created(paths.iter().map(|p| fileops::duplicate(p)).collect::<io::Result<_>>()?))
            }),
            Command::MakeAlias(paths) => self.in_background(false, cx, move || {
                let mut made = Vec::new();
                for path in &paths {
                    let alias = fileops::alias_path(path)?;
                    macos::make_alias(path, &alias).map_err(io::Error::other)?;
                    made.push(alias);
                }
                Ok(Op::created(made))
            }),
            Command::QuickLook => self.toggle_preview(cx),
            Command::Copy(paths) => macos::copy_files(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>()),
            Command::Cut(paths) => {
                super::transfer::cut(paths);
                cx.notify();
            }
            Command::CopyPath(paths) => {
                let text: Vec<_> = paths.iter().map(|p| p.to_string_lossy()).collect();
                macos::copy_text(&text.join("\n"));
            }
            Command::Tag(paths, name, add) => self.set_tag(&paths, name, add, cx),
            // (Its text onto the clipboard, and a note of it: nothing's made.)
            Command::QuickAction(quick_actions::QuickAction::CopyText, paths) => quick_actions::copy_text(paths, cx),
            // (A size chosen first, in Quick Open.)
            // (Once this window's done: Quick Open asks it what it shows.)
            Command::QuickAction(quick_actions::QuickAction::Resize, paths) => {
                cx.defer(move |cx| crate::quick_panel::show_native(crate::resize::extension(), "resize", crate::resize::args(&paths), cx))
            }
            Command::QuickAction(action, paths) => self.in_background(false, cx, move || {
                Ok(Op::created(quick_actions::run(action, &paths)?))
            }),
            Command::SetWallpaper(path) => {
                if let Err(e) = macos::set_wallpaper(&path) {
                    self.show_notice(e, cx);
                }
            }
            Command::Shortcut(name, paths) => {
                cx.background_executor().spawn(async move { quick_actions::run_shortcut(&name, &paths) }).detach();
            }
            Command::Download(paths) => {
                for path in &paths {
                    if let Err(e) = macos::download(path) {
                        self.show_notice(e, cx);
                    }
                }
                self.reload_folders(&parents(paths), cx);
            }
            Command::RemoveDownload(paths) => {
                for path in &paths {
                    if let Err(e) = macos::evict(path) {
                        self.show_notice(e, cx);
                    }
                }
                self.reload_folders(&parents(paths), cx);
            }
            Command::ShowOriginal(alias) => match macos::resolve_alias(&alias) {
                Some(original) => {
                    let folder = original.parent().map_or_else(|| original.clone(), Path::to_path_buf);
                    self.navigate(folder, Some(original), window, cx);
                    window.focus(&self.focus, cx);
                }
                None => self.show_notice("The original item can't be found.", cx),
            },
            Command::ShowInEnclosingFolder(path) => {
                if let Some(parent) = path.parent() {
                    self.navigate(parent.to_path_buf(), Some(path.clone()), window, cx);
                    window.focus(&self.focus, cx);
                }
            }
            Command::ShowInFinder(path) => cx.reveal_path(&path),
            Command::Eject(path) => self.eject(path, window, cx),
            Command::AddToSidebar(paths) => self.add_to_sidebar(paths, cx),
            Command::RemoveFromSidebar(ix) => self.remove_favorite(ix, cx),
            Command::GoTo(goes) => self.go(goes, window, cx),
            Command::NewFolder => {
                let dir = self.cwd.clone();
                self.in_background(true, cx, move || Ok(Op::created(vec![fileops::new_folder(&dir)?])));
            }
            Command::NewFile => {
                let dir = self.cwd.clone();
                self.in_background(true, cx, move || Ok(Op::created(vec![fileops::new_file(&dir)?])));
            }
            Command::NewFolderWith(paths) => {
                let dir = self.cwd.clone();
                self.in_background(true, cx, move || {
                    let folder = fileops::new_folder(&dir)?;
                    let mut moved = Vec::new();
                    for path in &paths {
                        let to = folder.join(path.file_name().unwrap_or_default());
                        fileops::move_item(path, &to)?;
                        moved.push((path.clone(), to));
                    }
                    // (Undo puts them back, then trashes the folder.)
                    Ok(Op { moved, created: vec![folder] })
                });
            }
            Command::Paste => {
                let dir = self.cwd.clone();
                let (pasted, cut) = super::transfer::to_paste();
                self.transfer(pasted, dir, if cut { How::Move } else { How::Copy }, cx);
            }
            Command::MoveHere => {
                let dir = self.cwd.clone();
                self.transfer(super::transfer::to_paste().0, dir, How::Move, cx);
            }
            Command::ViewAs(view) => self.set_view(view, cx),
            Command::SortBy(key) => {
                if self.sort.0 != key || self.free {
                    self.set_sort(key, cx);
                }
            }
            Command::SortNone => {
                self.free = true;
                self.save_folder_view(cx);
                cx.notify();
            }
            Command::CleanUp(by) => self.clean_up(by, cx),
            Command::GroupBy(group) => self.set_group(group, cx),
            Command::ViewOptions => self.toggle_sheet(Sheet::ViewOptions, cx),
            Command::TidyUp => self.tidy_up(cx),
            Command::FolderRules => crate::settings_window::open_rules_for(self.cwd.clone(), cx),
            Command::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.refresh_rows(cx);
            }
        }
    }

    /// Go where a sidebar item goes.
    pub(super) fn go(&mut self, goes: Goes, window: &mut Window, cx: &mut Context<Self>) {
        match goes {
            Goes::Folder(path) => self.go_place(path, window, cx),
            Goes::Virtual(v) => self.go_virtual(v, window, cx),
            Goes::Server(dest) => self.connect_server(dest, None, window, cx),
        }
    }

    /// Open items: files in their apps, an alias's original, servers, Smart
    /// Folders, and the (first) folder here.
    pub(super) fn open_entries(&mut self, entries: Vec<Entry>, window: &mut Window, cx: &mut Context<Self>) {
        // (A server's: opened from there.)
        let (on_server, entries): (Vec<Entry>, Vec<Entry>) = entries.into_iter().partition(|e| crate::remote::is_remote(&e.path));
        if !on_server.is_empty() {
            self.open_remote(on_server, window, cx);
        }
        let mut folder = None;
        for entry in entries {
            if let Some(url) = &entry.link {
                cx.open_url(url);
                continue;
            }
            if entry.path.extension().is_some_and(|e| e == "savedSearch") {
                return self.go_virtual(Virtual::Smart(entry.path), window, cx);
            }
            // An archive opens like a folder (a copy of what's in it).
            if !entry.alias && crate::archives::is_archive(&entry.path) {
                return self.open_archive(entry.path, window, cx);
            }
            // A disk image: mounted here, its volume shown as it's laid out.
            if !entry.alias && crate::disk_images::is_disk_image(&entry.path) {
                self.show_notice(format!("Opening “{}”…", entry.name), cx);
                crate::disk_images::open(entry.path, Some(cx.entity().downgrade()), cx);
                continue;
            }
            let target = if entry.alias && !entry.is_dir {
                match macos::resolve_alias(&entry.path) {
                    Some(original) => original,
                    None => {
                        self.show_notice(format!("The original of “{}” can't be found.", entry.name), cx);
                        continue;
                    }
                }
            } else {
                entry.path
            };
            if target.is_dir() && !crate::listing::is_package(&target) {
                folder.get_or_insert(target);
            } else {
                if let Some(search) = &self.search {
                    search.opened(&target);
                }
                cx.open_with_system(&target);
            }
        }
        if let Some(folder) = folder {
            self.go_place(folder, window, cx);
        }
    }

    /// Tidy Up (see `crate::tidy`): the folder's loose files into folders
    /// of their kinds; ⌘Z puts them back.
    pub(crate) fn tidy_up(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.drop_folder() else {
            return self.show_notice("Only a folder can be tidied up.", cx);
        };
        cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn({
                let dir = dir.clone();
                async move { crate::tidy::run(&dir) }
            });
            let done = done.await;
            this.update(cx, |this, cx| match done {
                Ok((moved, _)) if moved.is_empty() => this.show_notice("Nothing loose here to tidy up.", cx),
                Ok((moved, made)) => {
                    let folders: std::collections::BTreeSet<_> = moved.iter().filter_map(|(_, to)| to.parent().map(Path::to_path_buf)).collect();
                    let note = format!(
                        "Tidied {} into {}. ⌘Z puts them back.",
                        crate::listing::count(moved.len(), "item"),
                        crate::listing::count(folders.len(), "folder")
                    );
                    this.record(Op { moved, created: made });
                    this.reload_folders(&[dir.clone()], cx);
                    this.show_notice(note, cx);
                }
                Err(e) => this.show_notice(format!("Couldn't tidy up: {e}"), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Look inside an archive: unpacked (off the main thread) into the
    /// cache, then shown like a folder.
    fn open_archive(&mut self, archive: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(search) = &self.search {
            search.opened(&archive);
        }
        cx.spawn_in(window, async move |this, cx| {
            let unpacked = cx.background_executor().spawn(async move { crate::archives::unpacked(&archive) }).await;
            this.update_in(cx, |this, window, cx| match unpacked {
                Ok(dir) => this.go_place(dir, window, cx),
                Err(e) => this.show_notice(e.to_string(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Extract the archive looked inside, beside it, and show what that
    /// made there.
    pub(super) fn extract_beside(&mut self, archive: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = archive.parent().map(Path::to_path_buf) else { return };
        self.go_place(dir.clone(), window, cx);
        self.in_background(false, cx, move || Ok(Op::created(vec![crate::archives::extract(&archive, &dir)?])));
    }

    /// Run a file operation off the main thread; then record it for undo,
    /// show and select what it left (and, with `rename`, start renaming
    /// it), or say why it failed.
    pub(super) fn in_background(
        &mut self,
        rename: bool,
        cx: &mut Context<Self>,
        op: impl FnOnce() -> io::Result<Op> + Send + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { op() }).await;
            this.update(cx, |this, cx| match result {
                Ok(op) => {
                    let results = op.results();
                    let touched = parents(op.moved.iter().map(|(from, _)| from.clone()).chain(results.iter().cloned()));
                    if rename {
                        this.pending_rename = results.last().cloned();
                    }
                    this.pending_select = if rename { results.last().cloned().into_iter().collect() } else { results };
                    this.record(op);
                    this.reload_folders(&touched, cx);
                }
                Err(e) => this.show_notice(e.to_string(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Files made for what's selected here by something else (Resize…, on
    /// Quick Open's page): left as an operation of its own leaves them,
    /// undoable, and listed and selected where they show, beside their
    /// originals (in a folder expanded in the list too), the window
    /// staying where it is. Whether any shows here.
    pub(crate) fn made(&mut self, made: Vec<PathBuf>, cx: &mut Context<Self>) -> bool {
        // (As listed: /tmp's are /private/tmp's.)
        let made: Vec<PathBuf> = made.into_iter().map(|p| std::fs::canonicalize(&p).unwrap_or(p)).collect();
        let shown = self.flat() || made.iter().any(|p| p.parent().is_some_and(|d| d == self.cwd || self.expanded.contains(d)));
        let touched = parents(made.iter().cloned());
        self.select_when_listed(made.clone());
        self.record(Op::created(made));
        self.reload_folders(&touched, cx);
        shown
    }

    /// Re-list these folders where they show: the folder, or expanded in
    /// it, or as a column (or the found items, if that's what's shown).
    pub(super) fn reload_folders(&mut self, dirs: &[PathBuf], cx: &mut Context<Self>) {
        if self.searching {
            return self.refresh_search(cx);
        }
        if let Some(v) = self.virtual_folder.clone() {
            return self.show_virtual(v, cx);
        }
        let mut seen = std::collections::HashSet::new();
        for dir in dirs.iter().filter(|d| seen.insert(*d)) {
            if *dir == self.cwd {
                let keep = self.selected_entry().map(|e| e.path.clone());
                self.load(keep, cx);
            } else if self.expanded.contains(dir) {
                self.load_children(dir.clone(), cx);
            }
            self.col_cache.remove(dir);
            self.sizes.remove(dir);
        }
    }

    /// Move them to the Trash, off the main thread: those that aren't yours
    /// to move (an administrator's), through Finder, which asks for an
    /// administrator's password or Touch ID once.
    pub(super) fn trash(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        self.trash_with(paths, macos::Sound::MoveToTrash, cx)
    }

    /// Dropped on the Trash in the sidebar.
    pub(super) fn drop_in_trash(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.land(super::motion::Spot::Trash, cx);
        self.trash_with(&paths, macos::Sound::DragToTrash, cx)
    }

    /// Dragged out onto the Dock's Trash, which leaves trashing them to
    /// the app they came from.
    pub(crate) fn dragged_to_trash(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.trash_with(&paths, macos::Sound::DragToTrash, cx)
    }

    /// `trash`, playing `sound` once they're in the Trash. They fade out
    /// first.
    fn trash_with(&mut self, paths: &[PathBuf], sound: macos::Sound, cx: &mut Context<Self>) {
        // (Not those already on their way.)
        let paths: Vec<PathBuf> = paths.iter().filter(|p| !self.motions.is_leaving(p)).cloned().collect();
        if paths.is_empty() {
            return;
        }
        // (A server has no Trash: deleted, once asked.)
        if paths.iter().any(|p| crate::remote::is_remote(p)) {
            self.overlay = Some(Sheet::DeleteNow(paths));
            return cx.notify();
        }
        self.leave(&paths, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(super::motion::LEAVE).await;
            let done = cx
                .background_executor()
                .spawn(async move {
                    let results = macos::trash_all(&paths);
                    paths.into_iter().zip(results).collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| this.trashed(done, sound, cx)).ok();
        })
        .detach();
    }

    /// What `trash` moved (and the first reason one couldn't be, said):
    /// the selection on to the item after, the folders listed again.
    fn trashed(&mut self, done: Vec<(PathBuf, Result<PathBuf, String>)>, sound: macos::Sound, cx: &mut Context<Self>) {
        let mut moved = Vec::new();
        let mut failed = None;
        for (path, result) in done {
            match result {
                Ok(trashed) => moved.push((path, trashed)),
                Err(e) => {
                    // (Still here: shown again.)
                    self.motions.stayed([path]);
                    failed.get_or_insert(e);
                }
            }
        }
        if let Some(e) = failed {
            self.show_notice(e, cx);
        }
        if !moved.is_empty() {
            macos::play(sound);
        }
        super::places::remember_trashed(&moved);
        // The selection moves on to the item after, as in Finder.
        let gone: std::collections::HashSet<&PathBuf> = moved.iter().map(|(from, _)| from).collect();
        let last = self.rows.iter().rposition(|e| gone.contains(&e.path));
        if let Some(ix) = last {
            let next = self.rows[ix + 1..].iter().chain(self.rows[..ix].iter().rev()).find(|e| !gone.contains(&e.path));
            self.pending_select = next.map(|e| e.path.clone()).into_iter().collect();
        }
        let touched = parents(moved.iter().map(|(from, _)| from.clone()));
        if self.flat() {
            // Found items aren't found again: just drop them.
            self.rows.retain(|e| !gone.contains(&e.path));
            self.motions.stayed(gone.iter().map(|p| (*p).clone()));
            let next = std::mem::take(&mut self.pending_select);
            let ix = next.first().and_then(|p| self.rows.iter().position(|e| e.path == *p));
            self.select_only(ix);
            self.record(Op::moved(moved));
            return cx.notify();
        }
        self.record(Op::moved(moved));
        self.reload_folders(&touched, cx);
    }

    pub(super) fn set_tag(&mut self, paths: &[PathBuf], name: &str, add: bool, cx: &mut Context<Self>) {
        for path in paths {
            let mut names: Vec<String> = macos::tags(path).into_iter().map(|(n, _)| n).filter(|n| n != name).collect();
            if add {
                names.push(name.to_string());
            }
            if let Err(e) = macos::set_tags(path, &names) {
                return self.show_notice(e, cx);
            }
        }
        self.pending_select = paths.to_vec();
        if self.flat() {
            for entry in self.rows.iter_mut().filter(|e| paths.contains(&e.path)) {
                entry.tags = macos::tags(&entry.path).into_iter().map(|(n, c)| (n.into(), c)).collect();
            }
            return cx.notify();
        }
        self.reload_folders(&parents(paths.iter().cloned()), cx);
    }

    pub(super) fn start_rename(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.rows.iter().position(|e| e.path == path) else { return };
        self.select_only(Some(ix));
        self.scroll_to(ix, ScrollStrategy::Nearest);
        let entry = &self.rows[ix];
        let name = entry.name.to_string();
        // Finder selects the name without its extension.
        let stem = if entry.is_dir && !entry.is_bundle() { name.len() } else { name.rfind('.').filter(|&d| d > 0).unwrap_or(name.len()) };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_selected_range(0..stem, cx);
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.finish_rename(true, window, cx);
            }
        });
        self.renaming = Some(Renaming { path, input, _subscription: subscription });
        cx.notify();
    }

    /// Rename to what was typed (or, with `commit` false, leave it be).
    pub(super) fn finish_rename(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else { return };
        window.focus(&self.focus, cx);
        cx.notify();
        let name = renaming.input.read(cx).value().trim().to_string();
        let old = renaming.path;
        if !commit || name.is_empty() || old.file_name().is_some_and(|n| n.to_string_lossy() == name) {
            return;
        }
        self.rename_checking_extension(old, name, cx);
    }

    /// Rename now (any question asked).
    pub(super) fn rename_to(&mut self, old: PathBuf, name: String, cx: &mut Context<Self>) {
        if name.contains(['/', ':']) {
            return self.show_notice("Names can't contain “/” or “:”.", cx);
        }
        if crate::remote::is_remote(&old) {
            return self.rename_remote(old, name, cx);
        }
        let new = old.with_file_name(&name);
        // (Changing only the case finds the item itself on most Macs' disks.)
        if new.symlink_metadata().is_ok() && !fileops::same_file(&old, &new) {
            return self.show_notice(format!("The name “{name}” is already taken. Please choose a different name."), cx);
        }
        match std::fs::rename(&old, &new) {
            Ok(()) => {
                self.pending_select = vec![new.clone()];
                // (Freely arranged: it stays where it was, by its new name.)
                self.keep_place(&old, &new);
                self.record(Op::moved(vec![(old, new.clone())]));
                self.reload_folders(&parents([new]), cx);
            }
            Err(e) => self.show_notice(e.to_string(), cx),
        }
    }

    pub(super) fn eject(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // (A disk image's window, laid out: it closes, and the image goes.)
        if self.laid_out && self.cwd.starts_with(&path) {
            window.remove_window();
            return super::install::eject_away(path, None, cx);
        }
        // Step off the volume first: nothing in use can be ejected.
        if self.cwd.starts_with(&path) {
            self.go_place(home(), window, cx);
        }
        let name = super::display_name(&path);
        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let result = cx.background_executor().spawn(async move { macos::eject(&target) }).await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.locations = super::places::locations(cx),
                    Err(e) => this.show_notice(format!("“{name}” couldn't be ejected: {e}"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A message in the status bar for a few seconds (what went wrong).
    pub(super) fn show_notice(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notice = Some(message.into());
        self.notice_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(6)).await;
            this.update(cx, |this, cx| {
                this.notice = None;
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }
}

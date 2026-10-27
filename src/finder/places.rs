//! Where the sidebar goes, and found items shown in place of a folder:
//! Favorites (yours to edit), iCloud, Locations (disks, cloud drives,
//! servers), Tags; Recents, a tag's items, Smart Folders and the network,
//! found with Spotlight and Bonjour. Also the list's groups, and the Trash
//! (Put Back, Empty Trash, Delete Immediately).

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, TimeZone};
use gpui::*;
use serde::{Deserialize, Serialize};

use super::menu::TAGS;
use super::{Finder, display_name, home, icloud};
use crate::listing::{self, Entry};
use crate::macos;
use crate::settings::{Favorite, GroupBy, Settings};

/// Found items shown in place of a folder.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Virtual {
    Recents,
    Tag(SharedString),
    /// A Smart Folder (a `.savedSearch` file).
    Smart(PathBuf),
    /// Servers on the network.
    Network,
    /// What's in your Trashes: the home folder's and each disk's.
    Trash,
    /// Sending files with AirDrop.
    AirDrop,
}

impl Virtual {
    pub(super) fn title(&self) -> SharedString {
        match self {
            Virtual::Recents => "Recents".into(),
            Virtual::Tag(name) => name.clone(),
            Virtual::Smart(path) => path.file_stem().map_or("Smart Folder".into(), |n| n.to_string_lossy().into_owned().into()),
            Virtual::Network => "Network".into(),
            Virtual::Trash => macos::bin().into(),
            Virtual::AirDrop => "AirDrop".into(),
        }
    }
}

/// Where a sidebar item goes.
#[derive(Clone, PartialEq)]
pub(super) enum Goes {
    Folder(PathBuf),
    Virtual(Virtual),
    /// A server, over SSH: connected to (see `servers`).
    Server(crate::sftp::Dest),
}

/// A sidebar item: an SF Symbol, a name, where it goes.
#[derive(Clone)]
pub(super) struct Place {
    pub symbol: &'static str,
    pub name: SharedString,
    pub goes: Goes,
    /// A disk image, removable or network drive: gets an eject button.
    pub ejectable: bool,
    /// A tag's color dot instead of a symbol.
    pub color: u8,
    /// Its index among the Favorites you can edit.
    pub favorite: Option<usize>,
}

impl Place {
    pub(super) fn new(symbol: &'static str, name: impl Into<SharedString>, goes: Goes) -> Place {
        Place { symbol, name: name.into(), goes, ejectable: false, color: 0, favorite: None }
    }

    pub(super) fn folder(&self) -> Option<&PathBuf> {
        match &self.goes {
            Goes::Folder(path) => Some(path),
            _ => None,
        }
    }
}

/// Finder's default Favorites.
fn default_favorites() -> Vec<Favorite> {
    let home = home();
    ["/Applications".into(), home.join("Desktop"), home.join("Downloads"), home.join("Documents"), home]
        .into_iter()
        .map(|path| Favorite { path, name: None })
        .collect()
}

pub(crate) fn favorite_list(cx: &App) -> Vec<Favorite> {
    Settings::get(cx).favorites.clone().unwrap_or_else(default_favorites)
}

fn symbol_for(path: &Path) -> &'static str {
    let home = home();
    match path {
        p if p == Path::new("/Applications") => "a.circle",
        p if p == home => "house",
        p if p == home.join("Desktop") => "menubar.dock.rectangle",
        p if p == home.join("Downloads") => "arrow.down.circle",
        p if p == home.join("Documents") => "doc",
        p if p == home.join("Pictures") => "photo",
        p if p == home.join("Music") => "music.note",
        p if p == home.join("Movies") => "film",
        p if p.extension().is_some_and(|e| e == "savedSearch") => "gearshape",
        _ => "folder",
    }
}

/// The Favorites section: Recents and AirDrop (as Settings say), then
/// yours.
pub(super) fn favorites(cx: &App) -> Vec<Place> {
    let sidebar = &Settings::get(cx).sidebar;
    let mut places = Vec::new();
    if sidebar.recents {
        places.push(Place::new("clock", "Recents", Goes::Virtual(Virtual::Recents)));
    }
    if sidebar.airdrop {
        places.push(Place::new("dot.radiowaves.left.and.right", "AirDrop", Goes::Virtual(Virtual::AirDrop)));
    }
    for (ix, fav) in favorite_list(cx).into_iter().enumerate() {
        let name: SharedString = match &fav.name {
            Some(name) => name.clone().into(),
            None if fav.path.extension().is_some_and(|e| e == "savedSearch") => {
                fav.path.file_stem().unwrap_or_default().to_string_lossy().into_owned().into()
            }
            None => display_name(&fav.path),
        };
        let goes = if fav.path.extension().is_some_and(|e| e == "savedSearch") {
            Goes::Virtual(Virtual::Smart(fav.path.clone()))
        } else {
            Goes::Folder(fav.path.clone())
        };
        places.push(Place { favorite: Some(ix), ..Place::new(symbol_for(&fav.path), name, goes) });
    }
    if sidebar.trash {
        places.push(Place::new("trash", macos::bin(), Goes::Virtual(Virtual::Trash)));
    }
    places
}

/// iCloud and Locations: iCloud Drive, the startup disk, what's mounted,
/// cloud drives, the network.
pub(super) fn locations(cx: &App) -> Vec<Place> {
    let sidebar = Settings::get(cx).sidebar.clone();
    let home = home();
    let mut locations = Vec::new();
    // (Its parent, not the folder: that's consent-gated, so even a stat
    // can raise a privacy prompt.)
    if sidebar.icloud && home.join("Library/Mobile Documents").exists() {
        locations.push(Place::new("icloud", "iCloud Drive", Goes::Folder(icloud())));
    }
    locations.push(Place::new("internaldrive", "Macintosh HD", Goes::Folder("/".into())));
    // The startup disk is in /Volumes too, as a link to /; and a folder left
    // there isn't a volume.
    if sidebar.external_disks {
        for volume in std::fs::read_dir("/Volumes").into_iter().flatten().flatten() {
            let path = volume.path();
            let (mounted, ejectable) = macos::volume_flags(&path);
            if mounted && volume.file_type().is_ok_and(|t| !t.is_symlink()) {
                let symbol = if ejectable { "externaldrive" } else { "internaldrive" };
                let name = volume.file_name().to_string_lossy().into_owned();
                locations.push(Place { ejectable, ..Place::new(symbol, name, Goes::Folder(path)) });
            }
        }
    }
    if sidebar.cloud_storage {
        for provider in std::fs::read_dir(home.join("Library/CloudStorage")).into_iter().flatten().flatten() {
            let name = provider.file_name().to_string_lossy().into_owned();
            // "GoogleDrive-me@example.com" is Google Drive.
            let base = name.split('-').next().unwrap_or(&name);
            let shown = match base {
                "GoogleDrive" => "Google Drive",
                "OneDrive" => "OneDrive",
                "Dropbox" => "Dropbox",
                "Box" => "Box",
                other => other,
            };
            locations.push(Place::new("cloud", shown.to_string(), Goes::Folder(provider.path())));
        }
    }
    if sidebar.network {
        locations.push(Place::new("network", "Network", Goes::Virtual(Virtual::Network)));
    }
    locations
}

/// The Tags section: Finder's seven.
pub(super) fn tag_places(cx: &App) -> Vec<Place> {
    if !Settings::get(cx).sidebar.tags {
        return Vec::new();
    }
    TAGS.iter().map(|(name, color)| Place { color: *color, ..Place::new("circle", *name, Goes::Virtual(Virtual::Tag((*name).into()))) }).collect()
}

/// Paths a Spotlight query finds (in `scopes`, else everywhere), at most
/// `limit`.
fn mdfind(query: &str, scopes: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    let mut command = Command::new("/usr/bin/mdfind");
    for scope in scopes {
        command.arg("-onlyin").arg(scope);
    }
    command.arg(query).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else { return Vec::new() };
    let mut found = Vec::new();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            found.push(PathBuf::from(line));
            if found.len() >= limit {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    found
}

fn entries(paths: Vec<PathBuf>) -> Vec<Entry> {
    paths.into_iter().filter_map(Entry::from_path).filter(|e| !e.hidden).collect()
}

/// Spotlight, by name and inside files (PDFs, documents, text in images),
/// in `scope` or the whole Mac.
pub(super) fn spotlight(text: &str, scope: Option<&Path>) -> Vec<Entry> {
    let scopes: Vec<PathBuf> = scope.map(Path::to_path_buf).into_iter().collect();
    entries(mdfind(text, &scopes, super::SEARCH_LIMIT))
}

/// The Spotlight query a search is saved as (and a Contents search with
/// filters is): its words in names or inside files, and each filter.
pub(super) fn raw_query(text: &str, filters: &super::filters::Filters) -> String {
    let text = text.replace('"', "");
    let words = format!("(kMDItemDisplayName == \"*{text}*\"cd) || (kMDItemTextContent == \"{text}\"cd)");
    if !filters.any() {
        return words;
    }
    let mut all = filters.spotlight();
    if !text.trim().is_empty() {
        all.insert(0, format!("({words})"));
    }
    all.join(" && ")
}

/// Servers on the network (Bonjour: file sharing over SMB and AFP, both
/// browsed at once for a moment).
fn network_servers() -> Vec<Entry> {
    let browsers: Vec<_> = ["_smb._tcp", "_afpovertcp._tcp"].into_iter().map(|service| std::thread::spawn(move || browse(service))).collect();
    let mut servers: Vec<String> = browsers.into_iter().flat_map(|b| b.join().unwrap_or_default()).collect();
    servers.sort();
    servers.dedup();
    servers
        .into_iter()
        .map(|name| Entry {
            link: Some(format!("smb://{}.local", name.replace(' ', "%20")).into()),
            path: PathBuf::from("/Network").join(&name),
            name: name.into(),
            is_dir: true,
            ..Default::default()
        })
        .collect()
}

/// Names of computers offering `service`.
fn browse(service: &'static str) -> Vec<String> {
    let mut servers = Vec::new();
    {
        let Ok(mut child) =
            Command::new("/usr/bin/dns-sd").args(["-B", service, "local."]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        else {
            return servers;
        };
        let out = child.stdout.take();
        let reader = std::thread::spawn(move || {
            let mut names = Vec::new();
            for line in out.into_iter().flat_map(|o| BufReader::new(o).lines().map_while(Result::ok)) {
                // "... Add  3  4 local.  _smb._tcp.  Name With Spaces"
                if line.contains(" Add ")
                    && let Some(at) = line.find(service)
                {
                    names.push(line[at + service.len() + 1..].trim().to_string());
                }
            }
            names
        });
        let deadline = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
        servers.extend(reader.join().unwrap_or_default());
    }
    servers
}

/// What a virtual folder holds (slow: Spotlight, Bonjour).
fn find(v: &Virtual) -> Vec<Entry> {
    match v {
        Virtual::Recents => {
            let found = mdfind(
                "kMDItemLastUsedDate >= $time.today(-30) && kMDItemContentTypeTree != public.folder",
                &[home()],
                2000,
            );
            let mut found: Vec<Entry> = entries(found).into_iter().map(Entry::with_opened).collect();
            found.sort_by(|a, b| b.opened.cmp(&a.opened));
            found.truncate(500);
            found
        }
        Virtual::Tag(name) => entries(mdfind(&format!("kMDItemUserTags == '{}'", name.replace('\'', "")), &[], 2000)),
        Virtual::Smart(path) => match macos::saved_search(path) {
            Some((query, scopes)) => entries(mdfind(&query, &scopes, 2000)),
            None => Vec::new(),
        },
        Virtual::Network => network_servers(),
        Virtual::Trash => trash_items(),
        Virtual::AirDrop => Vec::new(),
    }
}

/// Your Trashes: each disk's, where you can look, and the home folder's,
/// which macOS lets only apps with Full Disk Access list: without it,
/// Finder is asked what's there (macOS asks you once to allow that).
fn trash_items() -> Vec<Entry> {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(home().join(".Trash")) {
        Ok(items) => items.flatten().map(|e| e.path()).collect(),
        Err(_) => macos::finder_trash_items(),
    };
    let uid = unsafe { libc::getuid() };
    for volume in std::fs::read_dir("/Volumes").into_iter().flatten().flatten() {
        let trash = volume.path().join(".Trashes").join(uid.to_string());
        paths.extend(std::fs::read_dir(trash).into_iter().flatten().flatten().map(|e| e.path()));
    }
    let mut items = entries(paths);
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    items
}

impl Finder {
    /// Show what a virtual folder holds (looked up off the main thread).
    pub(super) fn show_virtual(&mut self, v: Virtual, cx: &mut Context<Self>) {
        self.virtual_folder = Some(v.clone());
        self.title_dirty = true;
        self.rows.clear();
        self.lines.clear();
        self.select_only(None);
        self.error = None;
        // (AirDrop is a place to drop files, nothing to look up.)
        if v == Virtual::AirDrop {
            self.timing = "".into();
            self.list_task = Task::ready(());
            return cx.notify();
        }
        self.timing = "Looking…".into();
        let started = Instant::now();
        self.list_task = cx.spawn(async move |this, cx| {
            let target = v.clone();
            let found = cx.background_executor().spawn(async move { find(&target) }).await;
            this.update(cx, |this, cx| {
                if this.virtual_folder.as_ref() != Some(&v) {
                    return;
                }
                this.timing = format!("found in {:.0} ms", started.elapsed().as_secs_f64() * 1e3).into();
                if found.is_empty() {
                    this.error = Some(match v {
                        Virtual::Network => "No computers sharing files were found.".into(),
                        Virtual::Recents => "Nothing opened lately.".into(),
                        Virtual::Trash => format!("The {} is empty.", macos::bin()).into(),
                        _ => "Nothing here.".into(),
                    });
                }
                this.rows = found;
                if let Some((key, ascending)) = this.search_sort {
                    listing::sort(&mut this.rows, key, ascending, false, &this.sizes);
                }
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    /// Add folders (or Smart Folders) to Favorites, after the others.
    pub(super) fn add_to_sidebar(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut list = favorite_list(cx);
        for path in paths {
            if !list.iter().any(|f| f.path == path) {
                list.push(Favorite { path, name: None });
            }
        }
        Settings::update(cx, |s| s.favorites = Some(list));
    }

    pub(super) fn remove_favorite(&mut self, ix: usize, cx: &mut Context<Self>) {
        let mut list = favorite_list(cx);
        if ix < list.len() {
            list.remove(ix);
            Settings::update(cx, |s| s.favorites = Some(list));
        }
    }

    /// Move a favorite to where another one is (dragging in the sidebar).
    pub(super) fn move_favorite(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let mut list = favorite_list(cx);
        if from < list.len() && to < list.len() && from != to {
            let item = list.remove(from);
            list.insert(to, item);
            Settings::update(cx, |s| s.favorites = Some(list));
        }
    }

    /// Whether the folder shown is a Trash.
    pub(super) fn in_trash(&self) -> bool {
        self.virtual_folder == Some(Virtual::Trash)
            || self.cwd == home().join(".Trash") || self.cwd.parent().is_some_and(|p| p.file_name().is_some_and(|n| n == ".Trashes"))
    }

    /// Put Back: items trashed here go back where they were.
    pub(super) fn put_back(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let known = trash_records();
        let mut moved = Vec::new();
        for path in paths {
            let Some(original) = known.get(&path) else {
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.show_notice(format!("{} doesn't know where “{name}” came from (it was trashed elsewhere).", crate::APP_NAME), cx);
                continue;
            };
            if let Some(parent) = original.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match crate::fileops::move_item(&path, original) {
                Ok(()) => moved.push((path, original.clone())),
                Err(e) => self.show_notice(e.to_string(), cx),
            }
        }
        forget_trashed(moved.iter().map(|(t, _)| t.clone()));
        let dirs: Vec<PathBuf> = moved.iter().flat_map(|(a, b)| [a, b]).filter_map(|p| p.parent().map(Into::into)).collect();
        self.record(super::undo::Op::moved(moved));
        self.reload_folders(&dirs, cx);
    }

    /// Empty the Trash, asking first (as Settings say).
    pub(super) fn ask_empty_trash(&mut self, cx: &mut Context<Self>) {
        if Settings::get(cx).warn_empty_trash {
            self.overlay = Some(super::sheets::Sheet::EmptyTrash);
            cx.notify();
        } else {
            self.empty_trash(cx);
        }
    }

    pub(super) fn empty_trash(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { erase_trash() }).await;
            this.update(cx, |this, cx| {
                match result {
                    // (Without Full Disk Access Finder empties it, with its own sound.)
                    Ok(()) if macos::full_disk_access() => macos::play(macos::Sound::EmptyTrash),
                    Ok(()) => {}
                    Err(e) => this.show_notice(e, cx),
                }
                if this.virtual_folder == Some(Virtual::Trash) {
                    this.show_virtual(Virtual::Trash, cx);
                } else if this.in_trash() {
                    this.load(None, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Delete for good, after asking.
    pub(super) fn delete_now(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.iter().any(|p| crate::remote::is_remote(p)) {
            return self.delete_remote(paths, cx);
        }
        let mut failed = None;
        for path in &paths {
            let gone = match path.symlink_metadata() {
                Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
                Ok(_) => std::fs::remove_file(path),
                Err(e) => Err(e),
            };
            if let Err(e) = gone {
                failed = Some(e.to_string());
            }
        }
        forget_trashed(paths.iter().cloned());
        if let Some(e) = failed {
            self.show_notice(e, cx);
        }
        let dirs: Vec<PathBuf> = paths.iter().filter_map(|p| p.parent().map(Into::into)).collect();
        self.reload_folders(&dirs, cx);
    }
}

/// Why a folder couldn't be listed, in words that help.
pub(super) fn explain(dir: &Path, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        let what = if dir == home().join(".Trash") { format!("the {}", macos::bin()) } else { "this folder".into() };
        return format!(
            "To see what's in {what}, give {} Full Disk Access in System Settings > Privacy & Security.",
            crate::APP_NAME
        );
    }
    e.to_string()
}

/// Erase everything in your Trashes: the home folder's and each volume's.
pub(crate) fn erase_trash() -> Result<(), String> {
    let uid = unsafe { libc::getuid() };
    let mut trashes = vec![home().join(".Trash")];
    for volume in std::fs::read_dir("/Volumes").into_iter().flatten().flatten() {
        trashes.push(volume.path().join(".Trashes").join(uid.to_string()));
    }
    let mut failed = None;
    for trash in trashes {
        let Ok(items) = std::fs::read_dir(&trash) else {
            // The home folder's needs Full Disk Access; without it, Finder
            // empties it (all of them).
            if trash == home().join(".Trash")
                && let Err(e) = macos::finder_empty_trash()
            {
                failed = Some(format!(
                    "The {} couldn't be emptied ({e}). Give {} Full Disk Access in System Settings > Privacy & Security, or let it control Finder.",
                    macos::bin(),
                    crate::APP_NAME
                ));
            }
            continue;
        };
        for item in items.flatten() {
            let path = item.path();
            let gone = if item.file_type().is_ok_and(|t| t.is_dir()) { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            if let Err(e) = gone {
                failed = Some(e.to_string());
            }
        }
    }
    forget_trashed_all();
    failed.map_or(Ok(()), Err)
}

/// Where items trashed here came from, to put them back.
#[derive(Default, Serialize, Deserialize)]
struct TrashRecords(HashMap<PathBuf, PathBuf>);

fn records_file() -> PathBuf {
    // (With the settings, when they're kept elsewhere for testing.)
    if let Some(dir) = std::env::var_os("NEW_FINDER_SETTINGS") {
        return PathBuf::from(dir).join("trash.json");
    }
    home().join("Library/Application Support/NewFinder/trash.json")
}

fn trash_records() -> HashMap<PathBuf, PathBuf> {
    std::fs::read(records_file()).ok().and_then(|b| serde_json::from_slice::<TrashRecords>(&b).ok()).unwrap_or_default().0
}

fn save_records(records: HashMap<PathBuf, PathBuf>) {
    let file = records_file();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_vec(&TrashRecords(records)) {
        let _ = std::fs::write(file, json);
    }
}

/// Whether a path is in a Trash (the home folder's, or a volume's).
pub(crate) fn is_trashed(path: &Path) -> bool {
    path.starts_with(home().join(".Trash")) || path.ancestors().any(|p| p.file_name().is_some_and(|n| n == ".Trashes"))
}

/// Remember where trashed items came from: (from, where in the Trash).
pub(crate) fn remember_trashed(moved: &[(PathBuf, PathBuf)]) {
    let mut records = trash_records();
    for (from, to) in moved {
        records.insert(to.clone(), from.clone());
    }
    save_records(records);
}

fn forget_trashed(paths: impl Iterator<Item = PathBuf>) {
    let mut records = trash_records();
    for path in paths {
        records.remove(&path);
    }
    save_records(records);
}

fn forget_trashed_all() {
    save_records(HashMap::new());
}

/// Items in groups, as Finder's Group By makes them: (heading, items), in
/// the groups' order.
pub(super) fn group(items: Vec<Entry>, by: GroupBy, sizes: &HashMap<PathBuf, u64>) -> Vec<(SharedString, Vec<Entry>)> {
    let now = Local::now();
    let today = now.date_naive();
    let by_date = |secs: i64| -> (i64, String) {
        let Some(t) = Local.timestamp_opt(secs, 0).single() else { return (i64::MAX, "Unknown".into()) };
        let days = (today - t.date_naive()).num_days();
        match days {
            ..=0 => (0, "Today".into()),
            1 => (1, "Yesterday".into()),
            2..=7 => (2, "Previous 7 Days".into()),
            8..=30 => (3, "Previous 30 Days".into()),
            _ if t.year() == now.year() => (100 - t.month() as i64, t.format("%B").to_string()),
            _ => (10_000 - t.year() as i64, t.year().to_string()),
        }
    };
    let by_size = |e: &Entry| -> (i64, String) {
        let bytes = if e.is_dir { sizes.get(&e.path).copied() } else { Some(e.size) };
        let Some(bytes) = bytes else { return (-1, "Folders".into()) };
        const BUCKETS: [(u64, &str); 7] = [
            (1, "Zero KB"),
            (100_000, "Less than 100 KB"),
            (1_000_000, "100 KB to 1 MB"),
            (100_000_000, "1 MB to 100 MB"),
            (1_000_000_000, "100 MB to 1 GB"),
            (5_000_000_000, "1 GB to 5 GB"),
            (u64::MAX, "5 GB and up"),
        ];
        let at = BUCKETS.iter().position(|(limit, _)| bytes < *limit).unwrap_or(BUCKETS.len() - 1);
        (at as i64, BUCKETS[at].1.into())
    };
    let mut groups: Vec<((i64, String), Vec<Entry>)> = Vec::new();
    for entry in items {
        let key = match by {
            GroupBy::None => (0, String::new()),
            GroupBy::Kind => (if entry.is_dir && !entry.is_bundle() { 0 } else { 1 }, entry.kind_label().to_string()),
            GroupBy::DateModified => by_date(entry.mtime),
            GroupBy::DateAdded => by_date(entry.added),
            GroupBy::Size => by_size(&entry),
            GroupBy::Tags => match entry.tags.first() {
                Some((name, _)) => (0, name.to_string()),
                None => (1, "No Tags".into()),
            },
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(entry),
            None => groups.push((key, vec![entry])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    groups.into_iter().map(|((_, title), members)| (title.into(), members)).collect()
}

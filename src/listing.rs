//! Directory listing and the row model shared by folder views and search
//! results.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use chrono::{Days, Local, TimeZone};
use gpui_kit::SharedString;

use crate::macos;

/// Kind names by (extension, is folder): asking the system is cheap, but
/// sorting by kind asks a lot.
static KINDS: LazyLock<Mutex<HashMap<(String, bool), SharedString>>> = LazyLock::new(Default::default);
/// Whether folders with an extension are packages, likewise.
static PACKAGES: LazyLock<Mutex<HashMap<String, bool>>> = LazyLock::new(Default::default);

/// A package (an app, an .rtfd...): a folder opened as one item, not
/// browsed into.
pub fn is_package(path: &Path) -> bool {
    let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_lowercase()) else { return false };
    *PACKAGES.lock().unwrap().entry(ext).or_insert_with_key(|ext| macos::is_package(ext)) && path.is_dir()
}

#[derive(Clone, Default)]
pub struct Entry {
    pub name: SharedString,
    pub path: PathBuf,
    pub is_dir: bool,
    pub hidden: bool,
    pub size: u64,
    /// Seconds since the epoch.
    pub mtime: i64,
    /// For content search results, the line that matched.
    pub detail: Option<SharedString>,
    /// How deep in the folders expanded in the list it sits (0: the folder
    /// shown).
    pub depth: usize,
    /// A Finder alias or a symlink.
    pub alias: bool,
    /// Its tags: name, and Finder's color number (see `macos::tags`).
    pub tags: Vec<(SharedString, u8)>,
    /// Seconds since the epoch: made, put in its folder, last opened (0:
    /// unknown; last opened is only looked up when asked for).
    pub created: i64,
    pub added: i64,
    pub opened: i64,
    /// Finder hides its extension ("Hide extension" in Get Info).
    pub hide_ext: bool,
    /// In iCloud Drive or a cloud storage folder: whether it's here.
    pub cloud: Option<macos::Cloud>,
    /// Not a file: a server, opened at this URL.
    pub link: Option<SharedString>,
}

impl Entry {
    pub fn from_found(f: &fsearch::Found) -> Entry {
        let name = f.path.file_name().map_or_else(|| f.path.to_string_lossy(), |n| n.to_string_lossy());
        Entry {
            name: name.into_owned().into(),
            path: f.path.clone(),
            is_dir: f.kind & 3 == fsearch::walk::KIND_DIR,
            hidden: f.kind & fsearch::walk::FLAG_HIDDEN != 0,
            size: f.size,
            mtime: f.mtime as i64,
            ..Default::default()
        }
    }

    /// Finder's "Kind": the system's name for the type ("Markdown text
    /// file", "Folder", "Application").
    /// The name as Finder shows it: without ".app", or an extension Finder
    /// hides, unless all extensions show.
    pub fn display_name(&self, show_all_extensions: bool) -> SharedString {
        if show_all_extensions || !(self.hide_ext || self.is_bundle() && self.path.extension().is_some_and(|e| e == "app")) {
            return self.name.clone();
        }
        match self.name.rfind('.') {
            Some(dot) if dot > 0 => self.name[..dot].to_string().into(),
            _ => self.name.clone(),
        }
    }

    pub fn kind_label(&self) -> SharedString {
        if self.link.is_some() {
            return "Server".into();
        }
        if self.alias {
            return "Alias".into();
        }
        let ext = self.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let key = (ext, self.is_dir);
        let mut kinds = KINDS.lock().unwrap();
        if let Some(kind) = kinds.get(&key) {
            return kind.clone();
        }
        // Types nothing on the Mac declares are a plain "Document" to Finder.
        let fallback = || if key.1 { "Folder" } else { "Document" }.to_string();
        let kind: SharedString = macos::kind(&key.0, key.1).unwrap_or_else(fallback).into();
        kinds.insert(key, kind.clone());
        kind
    }

    /// A package: an app, an .rtfd, a photo library...
    pub fn is_bundle(&self) -> bool {
        self.is_dir && is_package(&self.path)
    }

    pub fn size_label(&self) -> SharedString {
        if self.is_dir { "--".into() } else { human_size(self.size).into() }
    }

    pub fn mtime_label(&self) -> SharedString {
        date_label(self.mtime)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum SortKey {
    Name,
    Modified,
    Created,
    Added,
    LastOpened,
    Size,
    Kind,
}

/// Sort by `key`; by name, folders first if `folders_on_top` (Finder's
/// "Keep folders on top: in windows when sorting by name"). `sizes` are
/// folders' sizes where known.
pub fn sort(entries: &mut [Entry], key: SortKey, ascending: bool, folders_on_top: bool, sizes: &HashMap<PathBuf, u64>) {
    let size = |e: &Entry| if e.is_dir { sizes.get(&e.path).copied().unwrap_or(0) } else { e.size };
    entries.sort_by(|a, b| {
        let order = match key {
            SortKey::Name => Ordering::Equal,
            SortKey::Modified => a.mtime.cmp(&b.mtime),
            SortKey::Created => a.created.cmp(&b.created),
            SortKey::Added => a.added.cmp(&b.added),
            SortKey::LastOpened => a.opened.cmp(&b.opened),
            SortKey::Size => size(a).cmp(&size(b)),
            SortKey::Kind => a.kind_label().cmp(&b.kind_label()),
        }
        .then_with(|| natural_cmp(&a.name, &b.name));
        let order = if ascending { order } else { order.reverse() };
        if folders_on_top && key == SortKey::Name { b.is_dir.cmp(&a.is_dir).then(order) } else { order }
    });
}

impl Entry {
    /// `lstat`, then `stat` for a symlink, so links to folders count as
    /// folders.
    pub fn from_path(path: PathBuf) -> Option<Entry> {
        let mut meta = std::fs::symlink_metadata(&path).ok()?;
        let link = meta.file_type().is_symlink();
        if link && let Ok(target) = std::fs::metadata(&path) {
            meta = target;
        }
        let name = path.file_name()?.to_string_lossy().into_owned();
        let (finder_alias, hide_ext, tags) = finder_attributes(&path);
        let cloud = in_cloud(&path).then(|| macos::cloud_state(&path)).flatten();
        Some(Entry {
            hidden: name.starts_with('.') || meta.st_flags() & libc::UF_HIDDEN != 0,
            name: name.into(),
            is_dir: meta.is_dir(),
            size: meta.len(),
            mtime: meta.mtime(),
            created: meta.st_birthtime(),
            added: added_time(&path).unwrap_or(0),
            alias: link || finder_alias,
            hide_ext,
            tags,
            cloud,
            path,
            ..Default::default()
        })
    }

    /// Look up when it was last opened (from Spotlight; slowish).
    pub fn with_opened(mut self) -> Entry {
        self.opened = macos::last_opened(&self.path).unwrap_or(0);
        self
    }
}

/// Within iCloud Drive or a cloud storage provider's folder.
fn in_cloud(path: &Path) -> bool {
    let bytes = path.as_os_str().as_bytes();
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    has(b"/Library/Mobile Documents/") || has(b"/Library/CloudStorage/")
}

/// When it was put in its folder (Finder's Date Added).
pub(crate) fn added_time(path: &Path) -> Option<i64> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut list = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: libc::ATTR_CMN_ADDEDTIME,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    // A u32 length, then a timespec.
    #[repr(C, packed(4))]
    struct Buf {
        len: u32,
        time: libc::timespec,
    }
    let mut buf: Buf = unsafe { std::mem::zeroed() };
    let r = unsafe {
        libc::getattrlist(
            c.as_ptr(),
            (&raw mut list).cast(),
            (&raw mut buf).cast(),
            std::mem::size_of::<Buf>(),
            libc::FSOPT_NOFOLLOW,
        )
    };
    let len = buf.len;
    (r == 0 && len as usize >= std::mem::size_of::<Buf>()).then(|| buf.time.tv_sec)
}

/// What Finder keeps in extended attributes: whether it's an alias and
/// whether its extension is hidden (flags in its Finder info), and its
/// tags. Most files have none, which costs one `listxattr`.
fn finder_attributes(path: &Path) -> (bool, bool, Vec<(SharedString, u8)>) {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return (false, false, Vec::new()) };
    let size = unsafe { libc::listxattr(c.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW) };
    if size <= 0 {
        return (false, false, Vec::new());
    }
    let mut names = vec![0u8; size as usize];
    let size = unsafe { libc::listxattr(c.as_ptr(), names.as_mut_ptr().cast(), names.len(), libc::XATTR_NOFOLLOW) };
    names.truncate(size.max(0) as usize);
    let has = |name: &[u8]| names.split(|&b| b == 0).any(|n| n == name);
    let finder_info = has(b"com.apple.FinderInfo");
    let alias = finder_info && {
        let mut info = [0u8; 32];
        let read = unsafe {
            libc::getxattr(c.as_ptr(), c"com.apple.FinderInfo".as_ptr(), info.as_mut_ptr().cast(), 32, 0, libc::XATTR_NOFOLLOW)
        };
        // fdFlags, big-endian; kIsAlias.
        read >= 10 && u16::from_be_bytes([info[8], info[9]]) & 0x8000 != 0
    };
    let hide_ext = finder_info && !alias && macos::has_hidden_extension(path);
    let tags = if has(b"com.apple.metadata:_kMDItemUserTags") {
        macos::tags(path).into_iter().map(|(name, color)| (name.into(), color)).collect()
    } else {
        Vec::new()
    };
    (alias, hide_ext, tags)
}

pub fn list_dir(dir: &Path) -> std::io::Result<Vec<Entry>> {
    // (A server's folder, standing in here: listed from the server.)
    if crate::remote::is_remote(dir) {
        return crate::remote::list(dir).map_err(std::io::Error::other);
    }
    Ok(std::fs::read_dir(dir)?.filter_map(|ent| Entry::from_path(ent.ok()?.path())).collect())
}

/// `list_dir`, with when each was last opened looked up too (for that
/// column, or sorting by it).
pub fn list_dir_opened(dir: &Path) -> std::io::Result<Vec<Entry>> {
    Ok(list_dir(dir)?.into_iter().map(Entry::with_opened).collect())
}

/// "Today at 15:00", "Yesterday at 21:31", "Sep 29, 2026 at 01:54".
pub fn date_label(secs: i64) -> SharedString {
    let Some(t) = Local.timestamp_opt(secs, 0).single() else { return "--".into() };
    let today = Local::now().date_naive();
    if t.date_naive() == today {
        t.format("Today at %H:%M").to_string().into()
    } else if today.checked_sub_days(Days::new(1)) == Some(t.date_naive()) {
        t.format("Yesterday at %H:%M").to_string().into()
    } else {
        t.format("%b %-d, %Y at %H:%M").to_string().into()
    }
}

/// "4,139,861".
pub fn thousands(n: impl ToString) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "1 item", "21 items".
pub fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

/// As Finder writes sizes: powers of 1000, "Zero bytes", "291 bytes",
/// "45 KB", "1.2 MB", "3.47 GB".
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 999.5 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    match unit {
        _ if bytes == 0 => "Zero bytes".into(),
        _ if bytes == 1 => "1 byte".into(),
        0 => format!("{bytes} bytes"),
        1 => format!("{v:.0} KB"),
        2 => format!("{v:.1} MB"),
        _ => format!("{v:.2} {}", UNITS[unit]),
    }
}

/// Case-insensitive, with digit runs compared as numbers ("file2" < "file10").
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut n = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        n.push(c);
                        it.next();
                    }
                    n
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Entry, SortKey, human_size, sort};

    #[test]
    fn sizes_as_finder_says_them() {
        assert_eq!(human_size(0), "Zero bytes");
        assert_eq!(human_size(1), "1 byte");
        assert_eq!(human_size(11), "11 bytes");
        assert_eq!(human_size(1_500), "2 KB");
        assert_eq!(human_size(88_776_000_000), "88.78 GB");
    }

    #[test]
    fn folders_go_on_top_only_by_name() {
        let entry = |name: &str, is_dir, mtime| Entry { name: name.to_string().into(), is_dir, mtime, ..Default::default() };
        let mut entries = vec![entry("b.zip", false, 30), entry("Old", true, 10), entry("a.pdf", false, 20), entry("New", true, 40)];
        let names = |entries: &[Entry]| entries.iter().map(|e| e.name.to_string()).collect::<Vec<_>>();
        sort(&mut entries, SortKey::Modified, false, true, &HashMap::new());
        assert_eq!(names(&entries), ["New", "b.zip", "a.pdf", "Old"], "newest first, folders among files");
        sort(&mut entries, SortKey::Name, true, true, &HashMap::new());
        assert_eq!(names(&entries), ["New", "Old", "a.pdf", "b.zip"]);
        sort(&mut entries, SortKey::Name, true, false, &HashMap::new());
        assert_eq!(names(&entries), ["a.pdf", "b.zip", "New", "Old"]);
    }
}

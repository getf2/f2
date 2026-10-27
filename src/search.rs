//! fsearch, embedded. It shares its index with the `fsearch` CLI (in
//! `~/Library/Application Support/FSearch`): whichever process starts first
//! owns and updates it, the others follow.

use std::collections::{HashMap, HashSet};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use std::sync::Arc;

use fsearch::frecency::{self, Frecency};
use fsearch::git::{self, Git};
use fsearch::live::Live;
use fsearch::walk::KIND_DIR;
use fsearch::{Engine, Found, Grep, Options, Query};

use crate::listing::Entry;
use crate::usage::{DEPTH, Node};

pub struct Search {
    engine: Engine,
    home: String,
    /// Where the index is kept.
    dir: PathBuf,
    /// Repos' status (for badges), kept up as their files change.
    git: Arc<Git>,
    /// What you've opened lately, for Quick Open's order.
    frecency: Arc<Frecency>,
    /// Folders the index doesn't hold, listed for searches in them (see
    /// `listed`), and when each was last searched.
    listed: std::sync::Mutex<Vec<(Arc<fsearch::Listed>, Instant)>>,
}

/// A folder listed for searches in it (see `Search::listed`) is searched
/// again, not listed afresh, this long after the last search in it: while
/// someone's typing.
const LISTED_FRESH: Duration = Duration::from_secs(30);

/// See `Search::health`.
#[derive(Clone, Debug, Default)]
pub struct Health {
    pub ready: bool,
    pub rebuilding: bool,
    /// While it's built: entries listed so far.
    pub listed: usize,
    /// Files and folders it holds.
    pub entries: usize,
    /// The index's size.
    pub bytes: u64,
    /// Text files whose words it holds (for `grep:`), and those waiting.
    pub text_files: usize,
    pub text_pending: usize,
    /// This app keeps the index (else it follows another's: fsearch's).
    pub owner: bool,
    pub full_disk_access: bool,
    /// Folders not opened, for want of Full Disk Access (macOS would ask
    /// before each), and others macOS refused anyway.
    pub left_out: Vec<PathBuf>,
    pub refused: Vec<PathBuf>,
    /// When it was last saved.
    pub saved: Option<std::time::SystemTime>,
}

/// The index doesn't hold `dir`: it's on another disk (the index stops at
/// other volumes), or in a folder it skips without Full Disk Access.
pub fn outside_index(dir: &Path) -> bool {
    if fsearch::walk::blocked(dir.as_os_str().as_bytes()) {
        return true;
    }
    // (The startup disk is two volumes: the system's, at /, and the data
    // one, whose folders the index has through /'s firmlinks: /Users, not
    // /System/Volumes/Data/Users.)
    match mount_of(dir) {
        Some(m) if m == Path::new("/") => false,
        Some(m) if m == Path::new("/System/Volumes/Data") => dir.starts_with("/System/Volumes/Data"),
        Some(_) => true,
        None => false,
    }
}

/// Where the volume `path` is on is mounted.
fn mount_of(path: &Path) -> Option<PathBuf> {
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(s.f_mntonname.as_ptr()) };
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(name.to_bytes())))
}

/// `path` is `dir` or inside it.
fn within(path: &[u8], dir: &[u8]) -> bool {
    path.starts_with(dir) && (path.len() == dir.len() || path[dir.len()] == b'/' || dir.ends_with(b"/"))
}

fn path_of(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

pub struct Outcome {
    pub entries: Vec<Entry>,
    pub elapsed: Duration,
    /// Content search (`grep:`, `regex:`, `sym:`) rather than names.
    pub content: bool,
    /// The folder isn't in the index (another disk's, say): it was looked
    /// through there and then, this many items.
    pub listed: Option<usize>,
}

impl Search {
    pub fn start() -> Result<Search, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let dir = std::env::var_os("NEW_FINDER_INDEX_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| fsearch::default_dir(&home));
        let engine = Engine::start(Options { dir: dir.clone(), home: home.clone(), skip: None })?;
        let git = Git::new(&home);
        let watching = git.clone();
        engine.on_change(move |dirs| watching.changed(dirs));
        let frecency = Frecency::load(crate::settings::data_dir().join("frecency"));
        Ok(Search { engine, home, dir, git, frecency, listed: Default::default() })
    }

    /// Quick Open (⌘P): anything on the Mac by name, what you've worked on
    /// lately first.
    pub fn quick_open(&self, text: &str, limit: usize) -> Result<Vec<Entry>, String> {
        let now = fsearch::query::now_secs();
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        // In plain words ("photos from last week"): fsearch's filters, many
        // found, then the newest (or biggest) first.
        let plain = crate::natural::translate(text);
        let in_plain_words = plain.is_some();
        let mut q = Query::parse(plain.as_ref().map_or(text, |p| p.query.as_str()), &self.home)?;
        q.limit = if in_plain_words { PLAIN_LIMIT } else { RERANK.max(limit) };
        let mut found = self.engine.search(&q)?;
        match plain.map(|p| p.order) {
            Some(crate::natural::Order::Newest) => found.sort_by_key(|f| std::cmp::Reverse(f.mtime)),
            Some(crate::natural::Order::Biggest) => found.sort_by_key(|f| std::cmp::Reverse(f.size)),
            _ => {
                // As fsearch's own daemon does: the best of what matches,
                // reordered by your opens, your commits, and how recently
                // each changed.
                let n = found.len().min(RERANK);
                for f in &mut found[..n] {
                    let p = f.path.as_os_str().as_bytes();
                    let lately = self.frecency.score(p, now) + self.git.history(p, now) + frecency::decay(now.saturating_sub(f.mtime));
                    f.score += frecency::boost(lately);
                }
                found[..n].sort_by_key(|f: &Found| std::cmp::Reverse(f.score));
            }
        }
        // Yours: in the home folder, Applications, other disks; not the
        // system's (/usr, /opt/homebrew, /Library...), apps' data
        // (~/Library), nor what's inside dependencies, caches and hidden
        // folders (where tools keep theirs: ~/.cargo, ~/.bun...), whose copies
        // of a name drown yours out. Unless the query asks: a path, or a
        // name starting with "." (a dotfile).
        let home = Path::new(&self.home);
        let library = home.join("Library");
        let asks = text.contains('/') || text.trim_start().starts_with('.');
        let noise = |f: &&Found| {
            let yours = [home, Path::new("/Applications"), Path::new("/Volumes")].iter().any(|d| f.path.starts_with(d)) && !f.path.starts_with(&library);
            let folders = f.path.parent().into_iter().flat_map(Path::components);
            !asks
                && (!yours
                    || folders.into_iter().any(|c| {
                        let name = c.as_os_str().to_string_lossy();
                        // (In plain words, "photos from last week", nor
                        // what's made, not yours: apps' and bundles'
                        // insides, build output.)
                        let made = in_plain_words
                            && (name.rsplit_once('.').is_some_and(|(_, ext)| matches!(ext, "app" | "framework" | "bundle" | "plugin" | "appex" | "xpc"))
                                || matches!(&*name, "dist" | "build" | "target" | "out" | "Pods" | "vendor"));
                        made || name.starts_with('.') || matches!(&*name, "node_modules" | "Caches" | "DerivedData")
                    }))
        };
        Ok(found.iter().filter(|f| !noise(f)).take(limit).map(Entry::from_found).collect())
    }

    /// The git repos in the home folder (where there's a ".git"), not
    /// those in dependencies, caches and ~/Library. Looked for again at
    /// most every minute.
    pub fn repos(&self) -> Vec<PathBuf> {
        static FOUND: std::sync::Mutex<Option<(Instant, Vec<PathBuf>)>> = std::sync::Mutex::new(None);
        if let Some((at, repos)) = &*FOUND.lock().unwrap()
            && at.elapsed() < Duration::from_secs(60)
        {
            return repos.clone();
        }
        let Ok(mut q) = Query::parse("'.git", &self.home) else { return Vec::new() };
        q.limit = 20_000;
        let found = self.engine.search(&q).unwrap_or_default();
        let home = Path::new(&self.home);
        let noise = ["node_modules", "Library", ".cargo", ".rustup", ".npm", ".cache", "pkg", "DerivedData", ".Trash"];
        let mut repos: Vec<PathBuf> = found
            .iter()
            .filter(|f| f.path.file_name().is_some_and(|n| n == ".git"))
            .filter_map(|f| f.path.parent().map(Path::to_path_buf))
            .filter(|root| root.starts_with(home) && root != home && !root.components().any(|c| noise.iter().any(|n| c.as_os_str() == *n)))
            .collect();
        repos.sort();
        repos.dedup();
        *FOUND.lock().unwrap() = Some((Instant::now(), repos.clone()));
        repos
    }

    /// How much `path` was opened of late (0: not at all).
    pub fn lately(&self, path: &Path) -> f32 {
        self.frecency.score(path.as_os_str().as_bytes(), fsearch::query::now_secs())
    }

    /// Remember that `path` was opened (Quick Open ranks it higher).
    pub fn opened(&self, path: &Path) {
        self.frecency.open(path.as_os_str().as_bytes(), fsearch::query::now_secs());
    }

    /// Start reading the git status of the repo `dir` is in (or is), if any,
    /// off the main thread; that repo.
    pub fn git_repo(&self, dir: &Path) -> Option<Arc<git::Repo>> {
        self.git.load(&[dir.as_os_str().as_bytes(), b"/."].concat())
    }

    /// Git's status for each of `paths` that has one: a file's own, and for
    /// a folder, everything changed inside it; a repo's folder shows its
    /// changes once its status has been read (`git_repo`). Ignored files
    /// aren't marked.
    pub fn git_marks(&self, paths: &[PathBuf]) -> HashMap<PathBuf, u8> {
        // Per repo: its status, and each folder's changes inside it.
        let mut repos: HashMap<Vec<u8>, Option<(Arc<git::Snap>, HashMap<Vec<u8>, u8>)>> = HashMap::new();
        let mut marks = HashMap::new();
        for path in paths {
            let bytes = path.as_os_str().as_bytes();
            let Some(repo) = self.git.repo(&[bytes, b"/."].concat()) else { continue };
            let read = repos.entry(repo.root.clone()).or_insert_with(|| {
                let snap = repo.snap(Duration::ZERO)?;
                let mut inside: HashMap<Vec<u8>, u8> = HashMap::new();
                for (file, &s) in &snap.files {
                    if s & git::IGNORED != 0 {
                        continue;
                    }
                    let mut at = &file[..];
                    while let Some(i) = at.iter().rposition(|&b| b == b'/') {
                        at = &at[..i];
                        *inside.entry(at.to_vec()).or_default() |= s;
                    }
                    *inside.entry(Vec::new()).or_default() |= s;
                }
                Some((snap, inside))
            });
            let Some((snap, inside)) = read else { continue };
            let Some(rel) = bytes.strip_prefix(repo.root.as_slice()).map(|r| r.strip_prefix(b"/").unwrap_or(r)) else { continue };
            let own = if rel.is_empty() { 0 } else { snap.status(rel) };
            let s = if own != 0 { own } else { inside.get(rel).copied().unwrap_or(0) };
            if s != 0 && s & git::IGNORED == 0 {
                marks.insert(path.clone(), s);
            }
        }
        marks
    }

    /// Run `text` in fsearch's query language, inside `scope` unless the
    /// query names a folder itself (`in:`).
    pub fn run(&self, text: &str, scope: Option<&Path>, limit: usize) -> Result<Outcome, String> {
        let started = Instant::now();
        // (In plain words, as Quick Open takes them.)
        let plain = crate::natural::translate(text);
        let mut q = Query::parse(plain.as_ref().map_or(text, |p| p.query.as_str()), &self.home)?;
        if q.scope.is_none() {
            q.scope = scope.filter(|d| *d != Path::new("/")).map(|d| d.as_os_str().as_bytes().to_vec());
        }
        q.limit = limit;

        let Some(pattern) = q.grep.clone() else {
            // (A folder the index doesn't hold: listed for it.)
            if let Some(dir) = q.scope.clone().filter(|d| outside_index(Path::new(std::ffi::OsStr::from_bytes(d)))) {
                let listed = self.listed(&dir);
                let entries = self.engine.search_listed(&listed, &q).iter().map(Entry::from_found).collect();
                return Ok(Outcome { entries, elapsed: started.elapsed(), content: false, listed: Some(listed.entries) });
            }
            let found = self.engine.search(&q)?;
            let entries = found.iter().map(Entry::from_found).collect();
            return Ok(Outcome { entries, elapsed: started.elapsed(), content: false, listed: None });
        };

        let mut g = Grep::new(&pattern, q.grep_mode)?;
        g.max_per_file = 1;
        g.budget = Some(Duration::from_millis(250));
        let (result, _) = self.engine.grep(&q, &g)?;
        let entries = result
            .files
            .iter()
            .filter_map(|f| {
                let path = PathBuf::from(std::ffi::OsStr::from_bytes(&f.path));
                let mut e = Entry::from_path(path)?;
                e.detail = f.lines.first().map(|(n, line)| format!("{n}: {}", line.trim()).into());
                Some(e)
            })
            .collect();
        Ok(Outcome { entries, elapsed: started.elapsed(), content: true, listed: None })
    }

    /// Bytes and items in each folder, all the way down, counted as
    /// `fileops::folder_size` does but added up from the index, so in
    /// milliseconds rather than a walk of the disk. None for a folder the
    /// index may not hold all of (on another volume, or with a folder in it
    /// macOS kept fsearch out of), and for all while the first index builds.
    /// (Files over 2 GB are rounded down to 2 MB in the index.)
    pub fn folder_sizes(&self, dirs: &[PathBuf]) -> Vec<Option<(u64, usize)>> {
        let gated = fsearch::gated(&self.home);
        let denied = fsearch::walk::DENIED.lock().map(|d| d.clone()).unwrap_or_default();
        let found = self.engine.with_live(|live| {
            let wanted: Vec<&[u8]> = dirs.iter().map(|d| d.as_os_str().as_bytes()).collect();
            let meets = |a: &[u8], b: &[u8]| under(a, b) || under(b, a);
            // Folders macOS guards that the index has nothing in, though
            // there's something there: fsearch was kept out. (Only those
            // asked about are looked at: looking may ask for permission.)
            let holes: Vec<&[u8]> = gated
                .iter()
                .map(|g| g.as_slice())
                .filter(|g| wanted.iter().any(|w| meets(w, g)) && !holds_all(live, g))
                .chain(denied.iter().map(|d| d.as_slice()))
                .collect();
            wanted.iter().map(|dir| (!holes.iter().any(|h| meets(dir, h))).then(|| subtree_size(live, dir)).flatten()).collect()
        });
        found.unwrap_or_else(|| vec![None; dirs.len()])
    }

    /// What's in `root`, ring by ring, from the index (see `usage`); None
    /// if it doesn't hold the folder (another disk, or one made since it
    /// was saved) or is still being built.
    pub fn usage(&self, root: &Path) -> Option<crate::usage::Node> {
        self.engine.with_live(|live| usage_tree(live, root)).flatten()
    }

    /// Every file inside `root` (the index's, and what's changed since)
    /// of `min` bytes or more: its path, length and modification time.
    pub fn files_at_least(&self, root: &Path, min: u64) -> Vec<(PathBuf, u64, u32)> {
        self.engine.with_live(|live| big_files(live, root, min)).unwrap_or_default()
    }

    /// Every folder inside `root` named one of `names` (the index's, and
    /// what's been made since), for Free Up Space's Build Folders.
    pub fn folders_named(&self, root: &Path, names: &[&str]) -> Vec<PathBuf> {
        self.engine.with_live(|live| named_folders(live, root, names)).unwrap_or_default()
    }

    /// Keep the CPU clocked up while someone is typing a query (see
    /// `fsearch::Engine::keep_warm`).
    pub fn warm(&self) {
        self.engine.keep_warm(Duration::from_millis(800));
    }

    /// The name index is loaded (false while the very first one builds).
    pub fn ready(&self) -> bool {
        self.engine.status().ready
    }

    /// How the index is doing, for Settings' Search: what it holds, what
    /// it's left out (and why), and whether it's being built.
    pub fn health(&self) -> Health {
        let status = self.engine.status();
        let mut left_out: Vec<PathBuf> = fsearch::walk::SKIP.get().into_iter().flatten().cloned().map(path_of).collect();
        let mut refused: Vec<PathBuf> = fsearch::walk::DENIED.lock().map(|d| d.iter().cloned().map(path_of).collect()).unwrap_or_default();
        left_out.sort();
        refused.sort();
        refused.dedup();
        refused.retain(|p| !left_out.iter().any(|s| p.starts_with(s)));
        let saved = std::fs::metadata(self.dir.join("index.bin")).and_then(|m| m.modified()).ok();
        Health {
            ready: status.ready,
            rebuilding: status.rebuilding,
            listed: fsearch::walk::LISTED.load(std::sync::atomic::Ordering::Relaxed),
            entries: status.entries.saturating_sub(status.removed) + status.overlay,
            bytes: status.index_bytes as u64,
            text_files: status.content_docs,
            text_pending: status.content_pending,
            owner: status.owner,
            full_disk_access: status.full_disk_access,
            left_out,
            refused,
            saved,
        }
    }

    /// `dir`, listed for searches in it: as it was a moment ago, if a
    /// search in it (or a folder holding it) was that recent (see
    /// `LISTED_FRESH`), else afresh. One listing at a time: a search
    /// typed meanwhile waits for it, then searches it too.
    fn listed(&self, dir: &[u8]) -> Arc<fsearch::Listed> {
        let mut kept = self.listed.lock().unwrap_or_else(|e| e.into_inner());
        kept.retain(|(_, used)| used.elapsed() < LISTED_FRESH);
        if let Some((listed, used)) = kept.iter_mut().find(|(l, _)| within(dir, &l.dir)) {
            *used = Instant::now();
            return listed.clone();
        }
        let listed = Arc::new(fsearch::Listed::new(dir, &self.home));
        kept.retain(|(l, _)| !within(&l.dir, dir));
        kept.push((listed.clone(), Instant::now()));
        listed
    }

    /// Crawl the whole disk afresh, in the background (searches answer
    /// from the index as it was meanwhile). False if another app keeps the
    /// index (fsearch's own), or the first one is still being built.
    pub fn rebuild(&self) -> bool {
        self.engine.rebuild()
    }

    /// Calls `f` with each folder a batch of disk changes touched (and
    /// whether everything below it may have changed too). Runs on fsearch's
    /// thread, so keep it quick.
    pub fn on_change(&self, f: impl Fn(&Path, bool) + Send + Sync + 'static) {
        self.engine.on_change(move |changes| {
            for (dir, deep) in changes {
                f(Path::new(std::ffi::OsStr::from_bytes(dir)), *deep);
            }
        });
    }
}

/// How many of a name search's best hits Quick Open reorders by what you
/// worked on (as fsearch's daemon does).
const RERANK: usize = 200;

/// How many a plain-words search ("photos from last week") takes in, to
/// put the newest (or biggest) first: its filters match in no order.
const PLAIN_LIMIT: usize = 5000;

/// `path` is `dir` or inside it.
fn under(path: &[u8], dir: &[u8]) -> bool {
    path.starts_with(dir) && (path.len() == dir.len() || path[dir.len()] == b'/' || dir == b"/")
}

/// The range of overlay paths inside `dir`: from "dir/" up to "dir0" ('/' + 1).
fn inside(dir: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut lo = dir.to_vec();
    if dir != b"/" {
        lo.push(b'/');
    }
    let mut hi = lo.clone();
    *hi.last_mut().unwrap() += 1;
    (lo, hi)
}

/// Bytes and items inside `dir` in the index: its range of the base (each
/// folder's whole subtree is one), less what's gone since, plus what's new.
fn subtree_size(live: &Live, dir: &[u8]) -> Option<(u64, usize)> {
    let (mut bytes, mut items) = (0u64, 0usize);
    let base = &live.base;
    let held = base.lookup(dir).filter(|&e| !live.is_dead(e)).and_then(|e| base.dir_of(e));
    match held {
        Some(k) => {
            for i in base.dir_start(k as usize) as usize..base.dir_end(k as usize) as usize {
                if !live.is_dead(i as u32) {
                    items += 1;
                    if base.kind(i) & 3 != KIND_DIR {
                        bytes += base.size_of(i);
                    }
                }
            }
        }
        // Made since the index was saved: all of it is in the overlay.
        None if live.over.get(dir).is_some_and(|o| o.kind & 3 == KIND_DIR) => {}
        None => return None,
    }
    let (lo, hi) = inside(dir);
    for o in live.over.range(lo..hi).map(|(_, o)| o) {
        items += 1;
        if o.kind & 3 != KIND_DIR {
            bytes += o.size;
        }
    }
    Some((bytes, items))
}

/// Every folder's total inside `root`, in one pass: each file's size into
/// its folder (the index's blocks: runs of siblings with their parent),
/// then each folder's into its parent's, deepest first (dir ids follow
/// entry order, so parents come before their folders). Then the tree of
/// what's worth showing.
fn usage_tree(live: &Live, root: &Path) -> Option<Node> {
    let base = &live.base;
    let path = root.as_os_str().as_bytes();
    let k = base.lookup(path).filter(|&e| !live.is_dead(e)).and_then(|e| base.dir_of(e))?;
    let below = base.descendants(k);
    // Totals by slot: 0 for `root`, then its folders by dir id.
    let slot = |dir: u32| -> Option<usize> {
        if dir == k {
            Some(0)
        } else {
            below.contains(&dir).then(|| (dir - below.start) as usize + 1)
        }
    };
    let mut bytes = vec![0u64; below.len() + 1];
    let mut items = vec![0u64; below.len() + 1];
    // Big files' sizes on disk (see `usage::MEASURED`), by entry.
    let mut measured: HashMap<usize, u64> = HashMap::new();
    let mut buf = Vec::new();
    for (run, parent) in base.blocks(base.dir_start(k as usize) as usize, base.dir_end(k as usize) as usize) {
        let Some(s) = slot(parent) else { continue };
        for i in run {
            if !live.is_dead(i as u32) {
                items[s] += 1;
                if base.kind(i) & 3 != KIND_DIR {
                    let len = base.size_of(i);
                    let size = if len >= crate::usage::MEASURED {
                        base.path(i, &mut buf);
                        let size = crate::usage::on_disk(Path::new(std::ffi::OsStr::from_bytes(&buf)), len);
                        measured.insert(i, size);
                        size
                    } else {
                        len
                    };
                    bytes[s] += size;
                }
            }
        }
    }
    // What changed since the index was saved, into the nearest folder it
    // has.
    let mut nearest: HashMap<Vec<u8>, usize> = HashMap::new();
    let (lo, hi) = inside(path);
    for (p, o) in live.over.range(lo..hi) {
        let parent = p[..p.iter().rposition(|&b| b == b'/').unwrap_or(0).max(1)].to_vec();
        let s = *nearest.entry(parent.clone()).or_insert_with(|| {
            let mut dir: &[u8] = &parent;
            loop {
                let found = base.lookup(dir).filter(|&e| !live.is_dead(e)).and_then(|e| base.dir_of(e)).and_then(slot);
                match found {
                    Some(s) => break s,
                    None if dir.len() <= path.len() => break 0,
                    None => dir = &dir[..dir.iter().rposition(|&b| b == b'/').unwrap_or(0).max(1)],
                }
            }
        });
        items[s] += 1;
        if o.kind & 3 != KIND_DIR {
            bytes[s] += crate::usage::on_disk(Path::new(std::ffi::OsStr::from_bytes(p)), o.size);
        }
    }
    for dir in below.clone().rev() {
        let (Some(from), Some(to)) = (slot(dir), slot(base.dir_parent(dir as usize))) else { continue };
        bytes[to] += bytes[from];
        items[to] += items[from];
    }

    let total = bytes[0];
    let tree = Tree { live, bytes: &bytes, items: &items, slot: &slot, total, measured: &measured };
    let mut node = Node {
        name: crate::usage::display_name(root),
        path: root.to_path_buf(),
        bytes: total,
        items: items[0],
        dir: true,
        children: Vec::new(),
        rest: (0, 0),
    };
    tree.fill(&mut node, k, 1);
    Some(node)
}

fn big_files(live: &Live, root: &Path, min: u64) -> Vec<(PathBuf, u64, u32)> {
    let base = &live.base;
    let path = root.as_os_str().as_bytes();
    let mut found = Vec::new();
    let mut buf = Vec::new();
    if let Some(k) = base.lookup(path).filter(|&e| !live.is_dead(e)).and_then(|e| base.dir_of(e)) {
        for (run, _) in base.blocks(base.dir_start(k as usize) as usize, base.dir_end(k as usize) as usize) {
            for i in run {
                if !live.is_dead(i as u32) && base.kind(i) & 3 != KIND_DIR && base.size_of(i) >= min {
                    base.path(i, &mut buf);
                    found.push((PathBuf::from(std::ffi::OsStr::from_bytes(&buf)), base.size_of(i), base.mtime(i)));
                }
            }
        }
    }
    // (Made or changed since the index was saved.)
    let (lo, hi) = inside(path);
    let mut seen: HashSet<PathBuf> = found.iter().map(|f| f.0.clone()).collect();
    for (p, o) in live.over.range(lo..hi) {
        if o.kind & 3 != KIND_DIR && o.size >= min {
            let p = PathBuf::from(std::ffi::OsStr::from_bytes(p));
            if seen.insert(p.clone()) {
                found.push((p, o.size, o.mtime));
            }
        }
    }
    found
}

fn named_folders(live: &Live, root: &Path, names: &[&str]) -> Vec<PathBuf> {
    let base = &live.base;
    let path = root.as_os_str().as_bytes();
    let wanted = |name: &[u8]| names.iter().any(|n| n.as_bytes() == name);
    let mut found = Vec::new();
    let mut buf = Vec::new();
    if let Some(k) = base.lookup(path).filter(|&e| !live.is_dead(e)).and_then(|e| base.dir_of(e)) {
        for i in base.dir_start(k as usize) as usize..base.dir_end(k as usize) as usize {
            if base.kind(i) & 3 == KIND_DIR && !live.is_dead(i as u32) && wanted(&base.name(i)) {
                base.path(i, &mut buf);
                found.push(PathBuf::from(std::ffi::OsStr::from_bytes(&buf)));
            }
        }
    }
    // (Made since the index was saved.)
    let (lo, hi) = inside(path);
    let mut seen: HashSet<PathBuf> = found.iter().cloned().collect();
    for (p, o) in live.over.range(lo..hi) {
        let name = &p[p.iter().rposition(|&b| b == b'/').map_or(0, |at| at + 1)..];
        if o.kind & 3 == KIND_DIR && wanted(name) {
            let p = PathBuf::from(std::ffi::OsStr::from_bytes(p));
            if seen.insert(p.clone()) {
                found.push(p);
            }
        }
    }
    found
}

/// The index and the totals, to lay out the tree from.
struct Tree<'a, F: Fn(u32) -> Option<usize>> {
    live: &'a Live,
    bytes: &'a [u64],
    items: &'a [u64],
    slot: &'a F,
    total: u64,
    /// Big files' sizes on disk, by entry.
    measured: &'a HashMap<usize, u64>,
}

impl<F: Fn(u32) -> Option<usize>> Tree<'_, F> {
    /// `node`'s children (folder `k` in the index), `level` rings below the
    /// folder shown, and theirs on down.
    fn fill(&self, node: &mut Node, k: u32, level: usize) {
        let base = &self.live.base;
        // Each with its folder in the index, if it's one.
        let mut found: Vec<(Node, Option<u32>)> = Vec::new();
        for i in base.children(k) {
            if self.live.is_dead(i as u32) {
                continue;
            }
            let name = String::from_utf8_lossy(&base.name(i)).into_owned();
            let path = node.path.join(&name);
            found.push(match base.dir_of(i as u32).filter(|_| base.kind(i) & 3 == KIND_DIR) {
                Some(d) => {
                    let s = (self.slot)(d).unwrap_or(0);
                    let dir = Node { name, path, bytes: self.bytes[s], items: self.items[s], dir: true, children: Vec::new(), rest: (0, 0) };
                    (dir, Some(d))
                }
                None => {
                    let size = self.measured.get(&i).copied().unwrap_or_else(|| base.size_of(i));
                    (Node::file(name, path, size), None)
                }
            });
        }
        // Made since the index was saved, right inside it. (What's new
        // deeper in a folder it has is in that folder's total already.)
        let has: HashSet<Vec<u8>> = found.iter().map(|(c, _)| c.name.as_bytes().to_vec()).collect();
        let (lo, hi) = inside(node.path.as_os_str().as_bytes());
        let mut made: HashMap<Vec<u8>, Node> = HashMap::new();
        for (p, o) in self.live.over.range(lo.clone()..hi) {
            let rest = &p[lo.len()..];
            let (first, deeper) = match rest.iter().position(|&b| b == b'/') {
                Some(at) => (&rest[..at], true),
                None => (rest, false),
            };
            if has.contains(first) {
                continue;
            }
            let name = String::from_utf8_lossy(first).into_owned();
            let made = made.entry(first.to_vec()).or_insert_with(|| Node::file(name.clone(), node.path.join(&name), 0));
            made.dir |= deeper || o.kind & 3 == KIND_DIR;
            made.items += u64::from(deeper);
            if o.kind & 3 != KIND_DIR {
                made.bytes += crate::usage::on_disk(Path::new(std::ffi::OsStr::from_bytes(p)), o.size);
            }
        }
        found.extend(made.into_values().map(|n| (n, None)));

        found.sort_unstable_by(|a, b| b.0.bytes.cmp(&a.0.bytes));
        let cut = crate::usage::kept(found.iter().map(|(c, _)| c.bytes), level, self.total);
        for (gone, _) in found.drain(cut..) {
            node.rest.0 += gone.bytes;
            node.rest.1 += 1 + gone.items;
        }
        for (mut child, dir) in found {
            match dir {
                Some(d) if level < DEPTH => self.fill(&mut child, d, level + 1),
                _ if child.dir => child.rest = (child.bytes, child.items),
                _ => {}
            }
            node.children.push(child);
        }
    }
}

/// The index has `dir` in full: anything at all inside it, or it's empty
/// or missing on disk too.
fn holds_all(live: &Live, dir: &[u8]) -> bool {
    if subtree_size(live, dir).is_some_and(|(_, items)| items > 0) {
        return true;
    }
    match std::fs::read_dir(Path::new(std::ffi::OsStr::from_bytes(dir))) {
        Ok(mut list) => list.next().is_none(),
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    #[test]
    fn what_the_index_holds() {
        use std::path::Path;
        assert!(!super::outside_index(Path::new("/")));
        assert!(!super::outside_index(Path::new("/Applications")));
        assert!(!super::outside_index(&std::env::temp_dir()), "the data volume, through /'s firmlinks");
        assert!(super::outside_index(Path::new("/System/Volumes/Data/Users")), "the data volume by its own path");
        assert!(super::outside_index(Path::new("/System/Volumes/Preboot")), "another volume");
        assert!(super::within(b"/Volumes/A/b", b"/Volumes/A") && super::within(b"/Volumes/A", b"/Volumes/A"));
        assert!(!super::within(b"/Volumes/AB", b"/Volumes/A"));
    }

    /// Search on another disk: a disk image made and mounted (hidden) for
    /// it, its files found, a folder in it too, then ejected.
    /// `cargo test --release -- --ignored --nocapture another_disk`
    #[test]
    #[ignore]
    fn searches_another_disk() {
        use std::process::Command;
        let id = std::process::id();
        let dmg = std::env::temp_dir().join(format!("nf-disk-{id}.dmg"));
        let name = format!("nf-disk-{id}");
        let made = Command::new("hdiutil").args(["create", "-size", "20m", "-fs", "APFS", "-volname", &name]).arg(&dmg).output().unwrap();
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        let attached = Command::new("hdiutil").args(["attach", "-nobrowse"]).arg(&dmg).output().unwrap();
        assert!(attached.status.success());
        let disk = PathBuf::from("/Volumes").join(&name);
        std::fs::create_dir_all(disk.join("Trip/Photos")).unwrap();
        for f in ["Trip/Photos/beach-photo.jpg", "Trip/Photos/sunset-photo.jpg", "Trip/notes.txt", "photo-list.txt"] {
            std::fs::write(disk.join(f), b"x").unwrap();
        }
        let index = std::env::temp_dir().join(format!("nf-index-{id}"));
        unsafe { std::env::set_var("NEW_FINDER_INDEX_DIR", &index) };
        let search = super::Search::start().unwrap();
        assert!(super::outside_index(&disk), "another disk");
        let found = |scope: &std::path::Path| {
            let o = search.run("photo", Some(scope), 50).unwrap();
            let mut names: Vec<String> = o.entries.iter().map(|e| e.path.strip_prefix(&disk).unwrap().to_string_lossy().into_owned()).collect();
            names.sort();
            (names, o.listed, o.elapsed)
        };
        let (all, listed, took) = found(&disk);
        println!("on the disk: {all:?}, {listed:?} items looked through in {took:.1?}");
        assert_eq!(all, ["Trip/Photos", "Trip/Photos/beach-photo.jpg", "Trip/Photos/sunset-photo.jpg", "photo-list.txt"]);
        assert!(listed.is_some_and(|n| n >= 6), "everything on it (and the disk's own hidden folders)");
        let (some, _, _) = found(&disk.join("Trip/Photos"));
        assert_eq!(some, ["Trip/Photos/beach-photo.jpg", "Trip/Photos/sunset-photo.jpg"], "in a folder of it, from the listing of the disk");
        let (again, _, took) = found(&disk);
        assert_eq!(again, all);
        println!("again, kept: {took:.1?}");
        drop(search);
        let detached = Command::new("hdiutil").args(["detach", "-force"]).arg(&disk).output().unwrap();
        assert!(detached.status.success());
        std::fs::remove_file(&dmg).ok();
        std::fs::remove_dir_all(&index).ok();
    }

    /// Rebuild: a whole-disk index of its own (in a temporary folder: read
    /// only, the user's own index untouched), crawled again, searches
    /// answering meanwhile. Takes a minute or two.
    /// `cargo test --release -- --ignored --nocapture rebuilds`
    #[test]
    #[ignore]
    fn rebuilds() {
        let dir = std::env::temp_dir().join(format!("nf-index-{}", std::process::id()));
        let probe = std::env::temp_dir().join(format!("nf-rebuild-probe-{}", std::process::id()));
        std::fs::create_dir_all(&probe).unwrap();
        unsafe { std::env::set_var("NEW_FINDER_INDEX_DIR", &dir) };
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(300) {
            std::thread::sleep(Duration::from_millis(200));
        }
        let first = search.health();
        println!("built in {:.1?}: {} entries, {} bytes, owner {}, fda {}, left out {:?}, refused {}", t.elapsed(), first.entries, first.bytes, first.owner, first.full_disk_access, first.left_out, first.refused.len());
        assert!(first.ready && first.owner && first.entries > 100_000);
        assert!(first.saved.is_some(), "saved once built");
        let name = format!("nf-rebuild-probe-{}", std::process::id());
        let finds = |s: &super::Search| s.run(&name, None, 10).map(|o| o.entries.len()).unwrap_or(0);
        let t = Instant::now();
        while finds(&search) == 0 && t.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(200));
        }
        assert!(finds(&search) > 0, "a new folder found before");
        assert!(search.rebuild(), "the owner may rebuild");
        assert!(search.health().rebuilding, "rebuilding as soon as it's asked");
        let t = Instant::now();
        let mut answered = 0;
        while search.health().rebuilding && t.elapsed() < Duration::from_secs(300) {
            answered += finds(&search).min(1);
            std::thread::sleep(Duration::from_millis(250));
        }
        let after = search.health();
        println!("rebuilt in {:.1?}: {} entries; found meanwhile {answered} times", t.elapsed(), after.entries);
        assert!(!after.rebuilding && after.ready);
        assert!(answered > 0, "searches answered during the rebuild");
        assert!(after.entries > first.entries / 2);
        assert!(finds(&search) > 0, "and after");
        std::fs::remove_dir_all(&probe).ok();
        drop(search);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An index saved before hard links' sizes were shared is crawled
    /// again once, a minute after starting. In a folder of its own (read
    /// only, the user's index untouched): run once to build it (it's
    /// marked), delete its `shares`, then run again.
    /// `SHARES_DIR=/tmp/nf-shares cargo test -- --ignored --nocapture shares_rebuild`
    #[test]
    #[ignore]
    fn shares_rebuild() {
        let Some(dir) = std::env::var_os("SHARES_DIR").map(PathBuf::from) else { return };
        let had = dir.join("index.bin").exists();
        unsafe { std::env::set_var("NEW_FINDER_INDEX_DIR", &dir) };
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(300) {
            std::thread::sleep(Duration::from_millis(200));
        }
        println!("ready in {:.1?} ({}), owner {}", t.elapsed(), if had { "loaded" } else { "built" }, search.health().owner);
        if !had {
            assert!(dir.join("shares").exists(), "a crawl is marked");
            return;
        }
        assert!(!dir.join("shares").exists(), "delete shares first");
        let (mut rebuilt_at, t) = (None, Instant::now());
        while !dir.join("shares").exists() && t.elapsed() < Duration::from_secs(240) {
            if rebuilt_at.is_none() && search.health().rebuilding {
                rebuilt_at = Some(t.elapsed());
                assert!(search.run("Applications", None, 3).is_ok_and(|o| !o.entries.is_empty()), "searches answer meanwhile");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        println!("rebuilding from {rebuilt_at:?}, marked after {:.1?}", t.elapsed());
        assert!(rebuilt_at.is_some_and(|at| at >= Duration::from_secs(55)), "a minute after starting");
        assert!(dir.join("shares").exists(), "marked once crawled again");
    }

    /// Index sizes against a walk of the disk, on real folders (read only).
    /// `cargo test --release -- --ignored --nocapture sizes_match`
    #[test]
    #[ignore]
    fn sizes_match_a_walk() {
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(200));
        }
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let dirs: Vec<PathBuf> = std::env::var("SIZE_DIRS")
            .map(|v| v.split(':').map(PathBuf::from).collect())
            .unwrap_or_else(|_| vec!["/Applications".into(), "/usr/share".into(), home.join(".cargo")]);
        let t = Instant::now();
        let fast = search.folder_sizes(&dirs);
        println!("index: {:?} for {} folders", t.elapsed(), dirs.len());
        for (dir, fast) in dirs.iter().zip(fast) {
            let t = Instant::now();
            let walked = crate::fileops::folder_size(dir, &AtomicBool::new(false));
            println!("{}: index {fast:?}, walk {walked:?} in {:?}", dir.display(), t.elapsed());
        }
    }

    /// The Disk Usage tree from the index, against folder sizes (read only).
    /// `QUERY='type:image mtime:<7d' cargo test --release -- --ignored --nocapture query_here`
    #[test]
    #[ignore]
    fn query_here() {
        let Some(query) = std::env::var("QUERY").ok() else { return };
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(200));
        }
        let t = Instant::now();
        let found = search.quick_open(&query, 8).unwrap();
        println!("{query}: {} in {:?}", found.len(), t.elapsed());
        for e in &found {
            println!("  {} ({})", e.path.display(), crate::listing::date_label(e.mtime));
        }
    }

    /// `USAGE_ROOT=/some/folder cargo test --release -- --ignored --nocapture usage_of`
    #[test]
    #[ignore]
    fn usage_of_a_folder() {
        let Some(root) = std::env::var_os("USAGE_ROOT").map(PathBuf::from) else { return };
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(200));
        }
        let t = Instant::now();
        let tree = search.usage(&root).unwrap();
        println!("{}: {:.2} GB on disk, {} items, in {:?}", root.display(), tree.bytes as f64 / 1e9, tree.items, t.elapsed());
        for c in tree.children.iter().take(5) {
            println!("  {} {:.2} GB", c.name, c.bytes as f64 / 1e9);
        }
    }

    /// `cargo test --release -- --ignored --nocapture usage_tree`
    #[test]
    #[ignore]
    fn usage_tree_matches() {
        let search = super::Search::start().unwrap();
        let t = Instant::now();
        while !search.ready() && t.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(200));
        }
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        for root in [PathBuf::from("/"), home.join(".cargo")] {
            let t = Instant::now();
            let tree = search.usage(&root).unwrap();
            let took = t.elapsed();
            let nodes = count(&tree);
            println!("{}: {} bytes, {} items, {nodes} nodes kept, in {took:?}", root.display(), tree.bytes, tree.items);
            let sized = search.folder_sizes(&[root.clone()]).pop().flatten();
            println!("  folder_sizes says {sized:?}");
            for c in tree.children.iter().take(5) {
                let check = search.folder_sizes(&[c.path.clone()]).pop().flatten().map(|s| s.0);
                println!("  {} {} (folder_sizes {check:?})", c.name, c.bytes);
            }
            // Children and rest add up to the folder.
            let sum: u64 = tree.children.iter().map(|c| c.bytes).sum::<u64>() + tree.rest.0;
            assert_eq!(sum, tree.bytes, "children + rest = total");
        }
        fn count(n: &crate::usage::Node) -> usize {
            1 + n.children.iter().map(count).sum::<usize>()
        }
    }
}

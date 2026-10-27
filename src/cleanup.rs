//! Disk Usage's Clean Up: the big files on the startup disk, and the files
//! in your home folder that are copies of one another. What's big, and
//! which lengths match, from fsearch's index; then the disk: what a big
//! file takes on it, and whether copies are copies (the same first and last
//! 16 KB, then the same SHA-256 of all of it). Left out: the system's, the
//! insides of apps and libraries (Photos'), the Trash, and (for copies)
//! ~/Library and code's folders, where copies are meant to be. Files only
//! in iCloud aren't read (nor downloaded).

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::os::darwin::fs::MetadataExt as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::search::Search;

/// Large Files: on the disk, at least this.
pub const LARGE: u64 = 100 << 20;
/// Duplicates: files at least this long (smaller ones free little).
pub const COPY_MIN: u64 = 1 << 20;

/// Folders whose insides are the system's (or a package manager's).
const SYSTEM: &[&str] = &["/System", "/usr", "/bin", "/sbin", "/private", "/Library/Apple", "/dev", "/cores", "/opt", "/nix"];

/// Packages: one item in Finder, their insides not yours to tidy.
const PACKAGES: &[&str] = &[
    "app", "framework", "bundle", "plugin", "appex", "kext", "xpc", "dsym", "photoslibrary", "photolibrary", "musiclibrary", "tvlibrary", "fcpbundle",
    "imovielibrary", "logicx", "band", "xcarchive", "lrlibrary", "aplibrary", "sparsebundle", "pkg", "mpkg",
];

/// Folders copies are meant to be in (and were put there by a tool).
const CODE: &[&str] = &["node_modules", ".git", ".svn", ".hg", "DerivedData", ".venv", "venv", "site-packages", ".cargo", ".rustup", ".gradle", ".m2", "Pods"];

#[derive(Clone, Debug)]
pub struct Found {
    pub path: PathBuf,
    /// What it takes on the disk.
    pub bytes: u64,
    /// What deleting it frees: its own blocks, not those an APFS clone of
    /// it shares (`bytes` for a large file).
    pub own: u64,
    /// When it was last changed, in seconds since 1970.
    pub modified: i64,
    /// An app's data (see `app_data`).
    pub app_data: bool,
    /// What to call it, if not its name ("Dagur's iPhone", for a backup).
    pub label: Option<String>,
    /// When it was last opened, as Spotlight knows (None: never, or not
    /// known); looked up for the lists where it matters.
    pub opened: Option<i64>,
}

/// Files that are the same, the oldest first (the one to keep).
#[derive(Clone, Debug)]
pub struct Copies {
    pub files: Vec<Found>,
    /// Each one's length.
    pub len: u64,
    /// What keeping only the oldest frees: the others' own blocks. Next to
    /// nothing for APFS clones of one another, which share theirs.
    pub freed: u64,
}

/// How far a duplicates search has got, and a way to stop it.
#[derive(Default)]
pub struct Progress {
    /// Bytes read, of the bytes to read.
    pub read: AtomicU64,
    pub to_read: AtomicU64,
    pub stop: AtomicBool,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// Whether Clean Up leaves `path` out; `copies`: also ~/Library, hidden
/// folders and code's.
fn left_out(path: &Path, home: &Path, copies: bool) -> bool {
    if SYSTEM.iter().any(|s| path.starts_with(s)) || path.starts_with(home.join(".Trash")) {
        return true;
    }
    if copies && path.starts_with(home.join("Library")) {
        return true;
    }
    let Some(parent) = path.parent() else { return false };
    parent.ancestors().any(|dir| {
        let name = dir.file_name().map_or(std::borrow::Cow::Borrowed(""), |n| n.to_string_lossy());
        let package = name.rsplit_once('.').is_some_and(|(_, ext)| PACKAGES.contains(&ext.to_lowercase().as_str()));
        // (A Rust project's build output: "target" beside its Cargo.toml.)
        let built = name == "target" && dir.parent().is_some_and(|p| p.join("Cargo.toml").is_file());
        package || (copies && (CODE.contains(&name.as_ref()) || built || (name.starts_with('.') && name.len() > 1)))
    })
}

/// An app's data rather than yours: in a Library, or a hidden folder in
/// your home (what deleting can break the app it's for).
fn app_data(path: &Path, home: &Path) -> bool {
    path.starts_with("/Library")
        || path.starts_with(home.join("Library"))
        || path.strip_prefix(home).is_ok_and(|rest| rest.components().next().is_some_and(|c| c.as_os_str().to_string_lossy().starts_with('.')))
}

/// Only in iCloud (or another provider's): its contents aren't here.
fn dataless(meta: &std::fs::Metadata) -> bool {
    const SF_DATALESS: u32 = 0x4000_0000;
    meta.st_flags() & SF_DATALESS != 0
}

/// The files of `LARGE` and more on the disk in `root` (the startup disk),
/// the biggest first.
pub fn large(search: &Search, root: &Path) -> Vec<Found> {
    let home = home();
    let mut found: Vec<Found> = search
        .files_at_least(root, LARGE)
        .into_iter()
        .filter(|(path, ..)| !left_out(path, &home, false))
        .filter_map(|(path, _, modified)| {
            let meta = path.symlink_metadata().ok()?;
            (!dataless(&meta) && meta.is_file()).then(|| {
                let bytes = crate::usage::on_disk_of(&meta);
                Found { bytes, own: bytes, modified: modified as i64, app_data: app_data(&path, &home), label: None, opened: crate::macos::last_opened(&path), path }
            })
        })
        // (A sparse disk image can be long and hold little.)
        .filter(|f| f.bytes >= LARGE)
        .collect();
    found.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes));
    found
}

/// Free Up Space's kinds of things that are safe to let go of.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    /// Disk images and installer packages in Downloads: the app's
    /// installed, the installer's left.
    Installers,
    /// What's sat in Downloads unopened for half a year.
    OldDownloads,
    /// iPhone and iPad backups on this Mac.
    Backups,
    /// Apps' caches: rebuilt as they're needed (not the system's).
    Caches,
    /// What Xcode builds and copies, made again as it's needed: DerivedData,
    /// devices' support files, simulators' caches.
    Xcode,
    /// Package managers' downloads (npm, Bun, Cargo, Gradle…) outside
    /// ~/Library/Caches: fetched again as they're needed.
    DevCaches,
    /// Apps' logs and crash reports.
    Logs,
    /// iPhone and iPad software updates kept, and macOS installers.
    Updates,
    /// What your projects' tools build and download (Rust's target,
    /// node_modules…): made again by a build or an install.
    Builds,
}

impl Kind {
    /// Free Up Space's.
    pub const ALL: [Kind; 5] = [Kind::Installers, Kind::OldDownloads, Kind::Backups, Kind::Caches, Kind::Builds];
    /// System Data's, that can go to the Trash.
    pub const SYSTEM: [Kind; 5] = [Kind::Xcode, Kind::DevCaches, Kind::Caches, Kind::Logs, Kind::Updates];

    /// Its name, what it is, and its SF Symbol.
    pub fn about(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Kind::Installers => ("Installers", "Disk images and packages in Downloads, once their apps are installed", "shippingbox"),
            Kind::OldDownloads => ("Old Downloads", "In Downloads for over six months, and not opened in that time", "arrow.down.circle"),
            Kind::Backups => ("iPhone & iPad Backups", "Device backups on this Mac (keep the one you'd restore from)", "iphone"),
            Kind::Caches => ("App Caches", "Apps rebuild these when they need them", "archivebox"),
            Kind::Xcode => ("Xcode Build Data", "Builds, device support and simulator caches: Xcode makes them again", "hammer"),
            Kind::DevCaches => ("Developer Caches", "npm, Bun, Cargo, Gradle and other downloads: fetched again when needed", "shippingbox.and.arrow.backward"),
            Kind::Logs => ("Logs", "Apps' logs and crash reports", "doc.text.magnifyingglass"),
            Kind::Updates => ("Software Updates", "iPhone and iPad updates kept on this Mac, and macOS installers", "arrow.down.app"),
            Kind::Builds => ("Build Folders", "Your projects' builds and packages (Rust's target, node_modules…): made again by a build or an install", "wrench.and.screwdriver"),
        }
    }

    /// Ticked to begin with: what's safe without a look.
    pub fn ticked(self) -> bool {
        matches!(self, Kind::Installers | Kind::Caches | Kind::Xcode)
    }
}

/// Installers' endings.
const INSTALLERS: &[&str] = &["dmg", "pkg", "mpkg", "iso", "xip"];

/// The project a DerivedData folder's for: "macsearch" of
/// "macsearch-djqqsapcghynvrhiclpmbqywuqgs".
fn project_name(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy();
    let (project, hash) = name.rsplit_once('-')?;
    (hash.len() == 28 && hash.chars().all(|c| c.is_ascii_lowercase()) && !project.is_empty()).then(|| project.to_string())
}

/// Package managers' downloads, under your home, and what to call each.
/// (Those in ~/Library/Caches are App Caches'.)
const DEV_CACHES: &[(&str, &str)] = &[
    (".npm/_cacache", "npm"),
    ("Library/pnpm/store", "pnpm"),
    (".bun/install/cache", "Bun"),
    (".yarn/berry/cache", "Yarn"),
    (".cargo/registry", "Cargo"),
    (".gradle/caches", "Gradle"),
    (".m2/repository", "Maven"),
    ("go/pkg/mod", "Go Modules"),
    (".nuget/packages", "NuGet"),
    (".pub-cache", "Dart & Flutter"),
    (".cocoapods/repos", "CocoaPods"),
];
/// Old Downloads: added longer ago than this.
const OLD: i64 = 182 * 86_400;

/// A kind of build folder: its name, a file beside it one of which says
/// what made it (any, if none), a file it holds that does (if any), and
/// what to call it.
struct Build {
    folder: &'static str,
    beside: &'static [&'static str],
    holds: Option<&'static str>,
    what: &'static str,
}

const PYTHON: &[&str] = &["pyproject.toml", "requirements.txt", "setup.py", "uv.lock", "Pipfile"];
const GRADLE: &[&str] = &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"];

/// Build folders, each found only beside what made it, so a folder of
/// yours that happens to share a name is never one.
const BUILDS: &[Build] = &[
    Build { folder: "target", beside: &["Cargo.toml"], holds: None, what: "Rust build" },
    Build { folder: "node_modules", beside: &["package.json"], holds: None, what: "Node packages" },
    Build { folder: ".next", beside: &["package.json"], holds: None, what: "Next.js build" },
    Build { folder: ".nuxt", beside: &["package.json"], holds: None, what: "Nuxt build" },
    Build { folder: ".svelte-kit", beside: &["package.json"], holds: None, what: "SvelteKit build" },
    Build { folder: ".turbo", beside: &["package.json"], holds: None, what: "Turborepo cache" },
    Build { folder: ".parcel-cache", beside: &["package.json"], holds: None, what: "Parcel cache" },
    Build { folder: ".angular", beside: &["angular.json"], holds: None, what: "Angular cache" },
    Build { folder: ".build", beside: &["Package.swift"], holds: None, what: "Swift build" },
    Build { folder: "Pods", beside: &["Podfile"], holds: None, what: "CocoaPods" },
    Build { folder: "build", beside: GRADLE, holds: None, what: "Gradle build" },
    Build { folder: ".gradle", beside: GRADLE, holds: None, what: "Gradle cache" },
    Build { folder: "build", beside: &["pubspec.yaml"], holds: None, what: "Flutter build" },
    Build { folder: ".dart_tool", beside: &["pubspec.yaml"], holds: None, what: "Dart tools" },
    Build { folder: "build", beside: &["CMakeLists.txt"], holds: Some("CMakeCache.txt"), what: "CMake build" },
    Build { folder: ".venv", beside: PYTHON, holds: Some("pyvenv.cfg"), what: "Python environment" },
    Build { folder: "venv", beside: PYTHON, holds: Some("pyvenv.cfg"), what: "Python environment" },
    Build { folder: ".tox", beside: &["tox.ini", "pyproject.toml"], holds: None, what: "tox environments" },
    Build { folder: ".zig-cache", beside: &["build.zig"], holds: None, what: "Zig cache" },
    Build { folder: "zig-out", beside: &["build.zig"], holds: None, what: "Zig build" },
    Build { folder: ".stack-work", beside: &["stack.yaml"], holds: None, what: "Haskell build" },
    Build { folder: "_build", beside: &["mix.exs"], holds: None, what: "Elixir build" },
    Build { folder: "deps", beside: &["mix.exs"], holds: None, what: "Elixir packages" },
];

/// Tools' own folders in your home: what's in them is theirs (an editor's
/// plugins, a version manager's installs), its packages what they need.
const TOOLS: &[&str] = &[
    ".local", ".config", ".cache", ".vim", ".emacs.d", ".oh-my-zsh", ".nvm", ".pyenv", ".rbenv", ".asdf", ".cargo", ".rustup", ".bun", ".npm", ".deno", ".vscode",
    ".vscode-insiders", ".cursor", ".windsurf",
];

/// What made `dir`, if it's a build folder of a project of yours: one of
/// `BUILDS`, beside what made it, in a git repository (yours, then, not an
/// app's or an editor extension's, whose packages they need), not inside
/// packages (one of a package's own is the package's) nor your Library or
/// a tool's folder. `repos`: which folders were found to be repositories.
fn build_of(dir: &Path, home: &Path, repos: &mut HashMap<PathBuf, bool>) -> Option<&'static str> {
    let name = dir.file_name()?.to_str()?;
    let project = dir.parent()?;
    let rest = project.strip_prefix(home).ok()?;
    let first = rest.components().next().map(|c| c.as_os_str());
    if !BUILDS.iter().any(|b| b.folder == name)
        || first.is_some_and(|c| c == "Library" || TOOLS.iter().any(|t| c == *t))
        || rest.components().any(|c| c.as_os_str() == "node_modules")
        || left_out(dir, home, false)
    {
        return None;
    }
    let build = BUILDS.iter().find(|b| b.folder == name && (b.beside.is_empty() || b.beside.iter().any(|f| project.join(f).is_file())) && b.holds.is_none_or(|f| dir.join(f).is_file()))?;
    if !dir.symlink_metadata().is_ok_and(|m| m.is_dir()) {
        return None;
    }
    let mut repo = |d: &Path| *repos.entry(d.to_path_buf()).or_insert_with(|| d.join(".git").exists());
    project.ancestors().take_while(|d| d.starts_with(home) && *d != home).any(|d| repo(d)).then_some(build.what)
}

/// When a build folder was last built or installed into: it, or what's
/// right inside it, last changed (Cargo and npm write a file there each
/// time).
fn built(dir: &Path) -> i64 {
    let mtime = |p: &Path| p.symlink_metadata().map_or(0, |m| m.mtime());
    std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| mtime(&e.path())).fold(mtime(dir), i64::max)
}

/// Your projects' build folders (`build_of`), the biggest first.
fn builds(search: &Search, home: &Path) -> Vec<Found> {
    let mut names: Vec<&str> = BUILDS.iter().map(|b| b.folder).collect();
    names.sort_unstable();
    names.dedup();
    let mut dirs = search.folders_named(home, &names);
    // (Outermost first, so one inside a build folder found is passed by.)
    dirs.sort_unstable();
    let mut repos = HashMap::new();
    let mut found: Vec<Found> = Vec::new();
    for dir in dirs {
        if found.last().is_some_and(|f| dir.starts_with(&f.path)) {
            continue;
        }
        let Some(what) = build_of(&dir, home, &mut repos) else { continue };
        let project = dir.parent().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = folder_bytes(search, &dir);
        found.push(Found { bytes, own: bytes, modified: built(&dir), app_data: false, label: Some(format!("{project} ({what})")), opened: None, path: dir });
    }
    found.into_iter().filter(|f| f.bytes >= 1 << 20).collect()
}

/// What a folder holds: from the index, else added up on the disk.
fn folder_bytes(search: &Search, dir: &Path) -> u64 {
    match search.folder_sizes(&[dir.to_path_buf()]).pop().flatten() {
        Some((bytes, _)) => bytes,
        None => crate::usage::walk(dir, &AtomicU64::new(0), &AtomicBool::new(false)).map_or(0, |n| n.bytes),
    }
}

/// An item of a folder Free Up Space looks in: a file, or a folder (all of
/// it, as one).
fn item(search: &Search, path: PathBuf, home: &Path) -> Option<Found> {
    let meta = path.symlink_metadata().ok()?;
    let bytes = if meta.is_dir() { folder_bytes(search, &path) } else { crate::usage::on_disk_of(&meta) };
    Some(Found { bytes, own: bytes, modified: meta.mtime(), app_data: app_data(&path, home), label: None, opened: None, path })
}

/// What a backup's Info.plist says it's of: the device's name.
fn backup_label(dir: &Path) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/plutil").args(["-convert", "json", "-o", "-"]).arg(dir.join("Info.plist")).output().ok()?;
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    info["Device Name"].as_str().or_else(|| info["Display Name"].as_str()).map(str::to_string)
}

/// The things of `kind` there are, the biggest first. (`home`: yours, or a
/// test's own folder.)
pub fn category(search: &Search, kind: Kind, home: &Path) -> Vec<Found> {
    let downloads = home.join("Downloads");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let ending = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let top = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))).collect()
    };
    let mut found: Vec<Found> = match kind {
        Kind::Installers => search
            .files_at_least(&downloads, 1)
            .into_iter()
            .filter(|(p, ..)| INSTALLERS.contains(&ending(p).as_str()))
            .filter_map(|(p, ..)| item(search, p, home))
            .map(|f| Found { opened: crate::macos::last_opened(&f.path), ..f })
            .collect(),
        Kind::OldDownloads => top(&downloads)
            .into_iter()
            .filter(|p| !INSTALLERS.contains(&ending(p).as_str()))
            .filter_map(|p| {
                // (When it came into Downloads: a file can be made long
                // before it's downloaded. And not opened since then, if
                // Spotlight knows: what you still use stays.)
                let added = crate::listing::added_time(&p).or_else(|| p.symlink_metadata().ok().map(|m| m.mtime())).unwrap_or(now);
                let opened = crate::macos::last_opened(&p);
                (now - added > OLD && opened.is_none_or(|opened| now - opened > OLD)).then(|| item(search, p, home).map(|f| Found { opened, ..f }))?
            })
            .collect(),
        Kind::Backups => top(&home.join("Library/Application Support/MobileSync/Backup"))
            .into_iter()
            .filter(|p| p.is_dir())
            .filter_map(|p| {
                let label = backup_label(&p);
                item(search, p, home).map(|f| Found { label, ..f })
            })
            .collect(),
        Kind::Caches => top(&home.join("Library/Caches"))
            .into_iter()
            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("com.apple.")))
            .filter_map(|p| item(search, p, home))
            .filter(|f| f.bytes >= 1 << 20)
            .collect(),
        Kind::Xcode => {
            let dev = home.join("Library/Developer");
            // (A project's builds by its name, not "macsearch-djqq…".)
            let mut found: Vec<Found> = top(&dev.join("Xcode/DerivedData"))
                .into_iter()
                .filter_map(|p| item(search, p, home))
                .map(|f| Found { label: project_name(&f.path).map(|n| format!("{n} (builds)")), ..f })
                .collect();
            for os in ["iOS", "watchOS", "tvOS", "visionOS"] {
                found.extend(top(&dev.join(format!("Xcode/{os} DeviceSupport"))).into_iter().filter_map(|p| item(search, p, home)));
            }
            found.extend(item(search, dev.join("CoreSimulator/Caches"), home).map(|f| Found { label: Some("Simulator Caches".into()), ..f }));
            found.into_iter().filter(|f| f.bytes >= 1 << 20).collect()
        }
        Kind::DevCaches => {
            let mut found: Vec<Found> = DEV_CACHES.iter().filter_map(|(at, name)| item(search, home.join(at), home).map(|f| Found { label: Some((*name).into()), ..f })).collect();
            // (And what's in ~/.cache: uv's, pip's, Hugging Face's…)
            found.extend(top(&home.join(".cache")).into_iter().filter_map(|p| item(search, p, home)));
            found.into_iter().filter(|f| f.bytes >= 1 << 20).collect()
        }
        Kind::Logs => top(&home.join("Library/Logs")).into_iter().filter_map(|p| item(search, p, home)).filter(|f| f.bytes >= 1 << 20).collect(),
        Kind::Updates => {
            let mut paths: Vec<PathBuf> = ["iPhone", "iPad", "iPod"]
                .iter()
                .flat_map(|device| top(&home.join(format!("Library/iTunes/{device} Software Updates"))))
                .filter(|p| ending(p) == "ipsw")
                .collect();
            paths.extend(top(Path::new("/Applications")).into_iter().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Install macOS")) && ending(p) == "app"));
            paths.into_iter().filter_map(|p| item(search, p, home)).collect()
        }
        Kind::Builds => builds(search, home),
    };
    found.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes));
    found
}

/// What's in your Trash, if it can be seen.
pub fn trash_bytes(search: &Search, home: &Path) -> Option<u64> {
    let trash = home.join(".Trash");
    std::fs::read_dir(&trash).ok()?;
    Some(folder_bytes(search, &trash))
}

/// The first and last 16 KB, hashed (to tell most same-length files apart
/// without reading them through).
fn ends(path: &Path, len: u64) -> Option<u64> {
    const END: u64 = 16 << 10;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; (2 * END).min(len) as usize];
    if len > 2 * END {
        file.read_exact(&mut buf[..END as usize]).ok()?;
        file.seek(SeekFrom::End(-(END as i64))).ok()?;
        file.read_exact(&mut buf[END as usize..]).ok()?;
    } else {
        file.read_exact(&mut buf).ok()?;
    }
    Some(buf.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3)))
}

/// Split `files` into those with the same `key` (dropping the unmatched).
fn regroup<K: std::hash::Hash + Eq>(files: Vec<Found>, key: impl Fn(&Found) -> Option<K>) -> Vec<Vec<Found>> {
    let mut by: HashMap<K, Vec<Found>> = HashMap::new();
    for f in files {
        if let Some(k) = key(&f) {
            by.entry(k).or_default().push(f);
        }
    }
    by.into_values().filter(|g| g.len() > 1).collect()
}

/// The files in `root` (your home folder) that are copies of one another,
/// those freeing the most first. None if it was stopped.
pub fn duplicates(search: &Search, root: &Path, progress: &Progress) -> Option<Vec<Copies>> {
    // (Never download what's only in iCloud to compare it.)
    fsearch::no_materialize();
    let home = home();
    // Same length, the first sieve; one file once (not its hard links).
    let mut by_len: HashMap<u64, Vec<Found>> = HashMap::new();
    let mut inodes: HashSet<(u64, u64)> = HashSet::new();
    for (path, len, modified) in search.files_at_least(root, COPY_MIN) {
        if left_out(&path, &home, true) {
            continue;
        }
        by_len.entry(len).or_default().push(Found { path, bytes: len, own: len, modified: modified as i64, app_data: false, label: None, opened: None });
    }
    let mut groups: Vec<(u64, Vec<Found>)> = Vec::new();
    for (len, files) in by_len.into_iter().filter(|(_, f)| f.len() > 1) {
        let files: Vec<Found> = files
            .into_iter()
            .filter(|f| {
                f.path.symlink_metadata().is_ok_and(|m| m.is_file() && m.len() == len && !dataless(&m) && inodes.insert((m.dev(), m.ino())))
            })
            .collect();
        if files.len() > 1 {
            groups.push((len, files));
        }
    }
    if progress.stop.load(Ordering::Relaxed) {
        return None;
    }
    // Same ends, the second; then the whole files.
    let mut sieved: Vec<(u64, Vec<Found>)> = Vec::new();
    for (len, files) in groups {
        if progress.stop.load(Ordering::Relaxed) {
            return None;
        }
        sieved.extend(regroup(files, |f| ends(&f.path, len)).into_iter().map(|g| (len, g)));
    }
    progress.to_read.store(sieved.iter().map(|(len, g)| len * g.len() as u64).sum(), Ordering::Relaxed);
    let mut copies = Vec::new();
    for (len, files) in sieved {
        let same = regroup(files, |f| {
            let hash = crate::macos::sha256_file(&f.path, &progress.stop);
            progress.read.fetch_add(len, Ordering::Relaxed);
            hash
        });
        if progress.stop.load(Ordering::Relaxed) {
            return None;
        }
        for mut files in same {
            files.sort_by_key(|f| (f.modified, f.path.as_os_str().len()));
            for f in &mut files {
                f.bytes = f.path.symlink_metadata().map_or(len, |m| crate::usage::on_disk_of(&m));
                f.own = crate::macos::private_size(&f.path).unwrap_or(f.bytes).min(f.bytes);
            }
            let freed = files[1..].iter().map(|f| f.own).sum();
            copies.push(Copies { files, len, freed });
        }
    }
    copies.sort_unstable_by(|a, b| b.freed.cmp(&a.freed).then(b.len.cmp(&a.len)));
    Some(copies)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ends, left_out, regroup, Found};

    /// Build folders: only beside what made them, in a project of yours.
    #[test]
    fn build_folders() {
        let home = std::env::temp_dir().join(format!("nf-builds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let dir = |p: &str| {
            std::fs::create_dir_all(home.join(p)).unwrap();
            home.join(p)
        };
        let file = |p: &str| {
            std::fs::create_dir_all(home.join(p).parent().unwrap()).unwrap();
            std::fs::write(home.join(p), "").unwrap();
        };
        dir("code/app/.git");
        file("code/app/Cargo.toml");
        file("code/app/site/package.json");
        file("code/app/site/node_modules/left-pad/package.json");
        file("code/app/py/pyproject.toml");
        file("code/app/py/.venv/pyvenv.cfg");
        file("code/app/other/pyproject.toml");
        dir("code/app/other/.venv");
        dir("code/app/docs/build");
        file("code/app/ios/Podfile");
        dir("code/app/ios/Pods");
        file("Downloads/thing/package.json");
        dir(".config/nvim/plugin/.git");
        file(".config/nvim/plugin/package.json");
        file("Library/Application Support/Some App/repo/package.json");
        dir("Library/Application Support/Some App/repo/.git");
        let builds = |p: &str| super::build_of(&dir(p), &home, &mut Default::default());
        assert_eq!(builds("code/app/target"), Some("Rust build"));
        assert_eq!(builds("code/app/site/node_modules"), Some("Node packages"), "in the repo, a folder down");
        assert_eq!(builds("code/app/site/node_modules/left-pad/node_modules"), None, "a package's own");
        assert_eq!(builds("code/app/py/.venv"), Some("Python environment"));
        assert_eq!(builds("code/app/other/.venv"), None, "no pyvenv.cfg: not an environment");
        assert_eq!(builds("code/app/docs/build"), None, "a folder called build, nothing that builds it");
        assert_eq!(builds("code/app/ios/Pods"), Some("CocoaPods"));
        assert_eq!(builds("code/app/site/target"), None, "no Cargo.toml beside it");
        assert_eq!(builds("Downloads/thing/node_modules"), None, "not in a repository");
        assert_eq!(builds(".config/nvim/plugin/node_modules"), None, "an editor plugin's");
        assert_eq!(builds("Library/Application Support/Some App/repo/node_modules"), None, "an app's");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn derived_data_names() {
        assert_eq!(super::project_name(Path::new("/x/DerivedData/macsearch-djqqsapcghynvrhiclpmbqywuqgs")).as_deref(), Some("macsearch"));
        assert_eq!(super::project_name(Path::new("/x/DerivedData/my-app-djqqsapcghynvrhiclpmbqywuqgs")).as_deref(), Some("my-app"));
        assert_eq!(super::project_name(Path::new("/x/DerivedData/ModuleCache.noindex")), None);
        assert_eq!(super::project_name(Path::new("/x/DerivedData/short-abc")), None);
    }

    /// Build Folders on this Mac, read only:
    /// `cargo test -- --ignored --nocapture build_folders_here`
    #[test]
    #[ignore]
    fn build_folders_here() {
        let search = crate::search::Search::start().unwrap();
        let t = std::time::Instant::now();
        while !search.ready() && t.elapsed().as_secs() < 120 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let t = std::time::Instant::now();
        let found = super::category(&search, super::Kind::Builds, &super::home());
        println!("{} build folders, {:.2} GB, in {:?}", found.len(), found.iter().map(|f| f.own).sum::<u64>() as f64 / 1e9, t.elapsed());
        for f in &found {
            println!("  {:>7.2} GB  {}  {}  {}", f.bytes as f64 / 1e9, crate::listing::date_label(f.modified), f.label.clone().unwrap_or_default(), f.path.display());
        }
    }

    /// System Data on this Mac, read only (simctl and tmutil asked, nothing
    /// deleted): `cargo test --release -- --ignored --nocapture system_data_here`
    #[test]
    #[ignore]
    fn system_data_here() {
        let search = crate::search::Search::start().unwrap();
        let t = std::time::Instant::now();
        while !search.ready() && t.elapsed().as_secs() < 120 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let home = super::home();
        for kind in super::Kind::SYSTEM {
            let t = std::time::Instant::now();
            let found = super::category(&search, kind, &home);
            println!("{:?}: {} items, {:.2} GB, in {:?}", kind, found.len(), found.iter().map(|f| f.own).sum::<u64>() as f64 / 1e9, t.elapsed());
            for f in found.iter().take(4) {
                println!("  {:.2} GB {}", f.bytes as f64 / 1e9, f.label.clone().unwrap_or_else(|| f.path.file_name().unwrap_or_default().to_string_lossy().into_owned()));
            }
        }
        let t = std::time::Instant::now();
        let parts = super::system_parts(&search, &home);
        println!("parts in {:?}:", t.elapsed());
        for p in &parts {
            println!("  {:>8} {} ({:?}): {}", p.bytes.map_or("?".into(), |b| format!("{:.2} GB", b as f64 / 1e9)), p.title, p.action.as_ref().map(|a| match a {
                super::PartAction::Open(label, _) => label.to_string(),
                super::PartAction::DeleteRuntime(_) => "delete runtime".into(),
                super::PartAction::DeleteUnavailable => "delete unavailable".into(),
            }), p.about);
        }
    }

    /// On this Mac's index, read only: `cargo test --release -- --ignored --nocapture clean_up_here`
    #[test]
    #[ignore]
    fn clean_up_here() {
        let search = crate::search::Search::start().unwrap();
        let t = std::time::Instant::now();
        while !search.ready() && t.elapsed().as_secs() < 120 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        // (And what's changed since it was saved, taken in.)
        std::thread::sleep(std::time::Duration::from_secs(8));
        let t = std::time::Instant::now();
        let large_root = std::env::var_os("COPIES_ROOT").map_or_else(|| std::path::PathBuf::from("/"), std::path::PathBuf::from);
        let large = super::large(&search, &large_root);
        println!("large: {} files, {:.1} GB, in {:?}", large.len(), large.iter().map(|f| f.bytes).sum::<u64>() as f64 / 1e9, t.elapsed());
        for f in large.iter().take(5) {
            println!("  {:.2} GB {}", f.bytes as f64 / 1e9, f.path.display());
        }
        let t = std::time::Instant::now();
        let progress = super::Progress::default();
        // (Not ~/Documents and the like: a test binary has no grant, and
        // macOS would ask.)
        let root = std::env::var_os("COPIES_ROOT").map_or_else(|| super::home().join("repos"), std::path::PathBuf::from);
        let copies = super::duplicates(&search, &root, &progress).unwrap();
        let freed: u64 = copies.iter().map(|c| c.freed).sum();
        println!("copies: {} groups, {:.2} GB freed, {:.1} GB read, in {:?}", copies.len(), freed as f64 / 1e9, progress.read.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9, t.elapsed());
        for c in copies.iter().take(5) {
            println!("  {} × {:.1} MB, frees {:.1} MB, keeps {}", c.files.len(), c.len as f64 / 1e6, c.freed as f64 / 1e6, c.files[0].path.display());
        }
    }

    #[test]
    fn leaves_out_what_isnt_yours_to_tidy() {
        let home = Path::new("/Users/me");
        assert!(left_out(Path::new("/System/Library/x.dylib"), home, false));
        assert!(left_out(Path::new("/Applications/Xcode.app/Contents/big.dmg"), home, false));
        assert!(left_out(Path::new("/Users/me/Pictures/Photos Library.photoslibrary/originals/a.heic"), home, false));
        assert!(left_out(Path::new("/Users/me/.Trash/old.zip"), home, false));
        assert!(!left_out(Path::new("/Users/me/Movies/trip.mov"), home, false));
        assert!(!left_out(Path::new("/Users/me/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"), home, false));
        // Copies: not ~/Library, hidden folders or code's.
        assert!(left_out(Path::new("/Users/me/Library/Caches/a.bin"), home, true));
        assert!(left_out(Path::new("/Users/me/repos/x/node_modules/y/z.js"), home, true));
        assert!(left_out(Path::new("/Users/me/.config/a.bin"), home, true));
        assert!(!left_out(Path::new("/Users/me/Downloads/report (1).pdf"), home, true));
        assert!(super::app_data(Path::new("/Users/me/.t3/userdata/state.sqlite"), home));
        assert!(super::app_data(Path::new("/Users/me/Library/Containers/x/Data/a.raw"), home));
        assert!(!super::app_data(Path::new("/Users/me/Movies/trip.mov"), home));
    }

    #[test]
    fn copies_are_found_by_their_contents() {
        let dir = std::env::temp_dir().join(format!("f2-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = big.clone();
        other[100_000] ^= 1; // the same ends, a different middle
        std::fs::write(dir.join("a"), &big).unwrap();
        std::fs::write(dir.join("b"), &big).unwrap();
        std::fs::write(dir.join("c"), &other).unwrap();
        let found = |name: &str| Found { path: dir.join(name), bytes: 200_000, own: 200_000, modified: 0, app_data: false, label: None, opened: None };
        let files = vec![found("a"), found("b"), found("c")];
        // The ends don't tell c apart; the whole file does.
        assert_eq!(regroup(files.clone(), |f| ends(&f.path, 200_000)).first().map(Vec::len), Some(3));
        let stop = std::sync::atomic::AtomicBool::new(false);
        let same = regroup(files, |f| crate::macos::sha256_file(&f.path, &stop));
        assert_eq!(same.len(), 1);
        let mut names: Vec<_> = same[0].iter().map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["a", "b"]);
        // A clone shares its blocks: it frees (next to) nothing.
        assert!(std::process::Command::new("/bin/cp").arg("-c").arg(dir.join("a")).arg(dir.join("clone")).status().unwrap().success());
        let private = crate::macos::private_size(&dir.join("clone"));
        assert!(private.is_some_and(|p| p < 100_000), "{private:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A part of what macOS's Storage settings call System Data that isn't
/// files to move to the Trash: what it is, what it takes (None: it can't
/// be seen), and what can be done about it.
#[derive(Clone, Debug)]
pub struct Part {
    pub title: String,
    pub about: String,
    pub icon: &'static str,
    pub bytes: Option<u64>,
    pub action: Option<PartAction>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PartAction {
    /// A System Settings pane, or another app, by its address.
    Open(&'static str, String),
    /// A simulator runtime deleted (`xcrun simctl runtime delete`), by its
    /// identifier.
    DeleteRuntime(String),
    /// The simulators for runtimes no longer here deleted (`xcrun simctl
    /// delete unavailable`).
    DeleteUnavailable,
}

/// Whether Xcode is the developer folder (simctl comes with it; asked
/// without it, macOS offers to install the command line tools).
fn xcode() -> bool {
    std::process::Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .is_some_and(|p| p.trim().ends_with(".app/Contents/Developer") && Path::new(p.trim()).is_dir())
}

fn simctl(args: &[&str]) -> Option<serde_json::Value> {
    let out = std::process::Command::new("/usr/bin/xcrun").arg("simctl").args(args).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

/// What a folder takes, all of it (unreadable parts left out); None if
/// it's not there.
fn walked(dir: &Path) -> Option<u64> {
    crate::usage::walk(dir, &AtomicU64::new(0), &AtomicBool::new(false)).map(|n| n.bytes)
}

/// System Data's parts that aren't files to move to the Trash, the
/// biggest first: simulators and their runtimes (with Xcode), Time
/// Machine's local snapshots, the sleep image and swap, temporary files,
/// Docker's disk. (Each only if it's there.)
pub fn system_parts(search: &Search, home: &Path) -> Vec<Part> {
    let mut parts = Vec::new();
    if xcode() {
        if let Some(runtimes) = simctl(&["runtime", "list", "-j"]).and_then(|v| v.as_object().cloned()) {
            for (id, r) in runtimes {
                let (Some(bytes), Some(version)) = (r["sizeBytes"].as_u64(), r["version"].as_str()) else { continue };
                let platform = r["runtimeIdentifier"].as_str().and_then(|i| i.rsplit('.').next()).and_then(|i| i.split('-').next()).unwrap_or("iOS");
                let used = match r["lastUsedAt"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
                    Some(t) => format!("last used {}", t.with_timezone(&chrono::Local).format("%b %-d, %Y")),
                    None => "never used".into(),
                };
                parts.push(Part {
                    title: format!("{platform} {version} Simulator"),
                    about: format!("A simulator runtime, {used}. Xcode downloads it again if you need it"),
                    icon: "iphone.gen3",
                    bytes: Some(bytes),
                    action: r["deletable"].as_bool().unwrap_or(false).then_some(PartAction::DeleteRuntime(id)),
                });
            }
        }
        // (Simulators whose runtime's gone: what they take.)
        let devices = home.join("Library/Developer/CoreSimulator/Devices");
        let gone: Vec<PathBuf> = simctl(&["list", "devices", "unavailable", "-j"])
            .and_then(|v| v["devices"].as_object().cloned())
            .into_iter()
            .flat_map(|by_runtime| by_runtime.into_values())
            .filter_map(|list| list.as_array().cloned())
            .flatten()
            .filter_map(|d| d["udid"].as_str().map(|udid| devices.join(udid)))
            .filter(|p| p.is_dir())
            .collect();
        // (What Xcode makes for each runtime, shared: it goes with them.)
        if let Some(bytes) = walked(Path::new("/Library/Developer/CoreSimulator/Caches")).filter(|b| *b >= 1 << 20) {
            parts.push(Part {
                title: "Simulator Runtimes' Caches".into(),
                about: "What Xcode works out for each simulator runtime, shared by all your simulators. They go with their runtimes".into(),
                icon: "cpu",
                bytes: Some(bytes),
                action: None,
            });
        }
        if !gone.is_empty() {
            let bytes = gone.iter().map(|p| folder_bytes(search, p)).sum();
            parts.push(Part {
                title: "Simulators of Removed Runtimes".into(),
                about: format!("{}: their runtimes aren't here any more", crate::listing::count(gone.len(), "simulator")),
                icon: "iphone.slash",
                bytes: Some(bytes),
                action: Some(PartAction::DeleteUnavailable),
            });
        }
    }
    // Time Machine's local snapshots (what they take can't be seen).
    let snapshots = std::process::Command::new("/usr/bin/tmutil")
        .args(["listlocalsnapshots", "/"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.starts_with("com.apple.TimeMachine.")).count())
        .unwrap_or(0);
    if snapshots > 0 {
        parts.push(Part {
            title: "Time Machine Snapshots".into(),
            about: format!("{} on this Mac, for backups while the backup disk's away. macOS deletes them by itself when it needs the room", crate::listing::count(snapshots, "snapshot")),
            icon: "clock.arrow.circlepath",
            bytes: None,
            action: Some(PartAction::Open("Time Machine Settings…", "x-apple.systempreferences:com.apple.Time-Machine-Settings.extension".into())),
        });
    }
    let vm: u64 = std::fs::read_dir("/private/var/vm").into_iter().flatten().flatten().filter_map(|e| e.metadata().ok()).map(|m| crate::usage::on_disk_of(&m)).sum();
    if vm > 0 {
        parts.push(Part {
            title: "Sleep Image & Swap".into(),
            about: "What's in memory, kept while your Mac sleeps (and spilled over when it's busy). macOS's own: swap shrinks after a restart".into(),
            icon: "memorychip",
            bytes: Some(vm),
            action: None,
        });
    }
    // (Yours in /private/var/folders: T, apps' temporary files; C, the
    // system's caches for you.)
    if let Some(mine) = std::env::var_os("TMPDIR").map(PathBuf::from).and_then(|t| t.parent().map(Path::to_path_buf)) {
        let bytes: u64 = ["T", "C"].iter().filter_map(|d| walked(&mine.join(d))).sum();
        if bytes > 0 {
            parts.push(Part {
                title: "Temporary Files".into(),
                about: "Apps' temporary files, and the system's caches for you. Restarting clears them".into(),
                icon: "clock.badge.xmark",
                bytes: Some(bytes),
                action: None,
            });
        }
    }
    // Docker's disk: as long as it can grow, from the index only (its
    // folder's another app's, which macOS asks before letting F2 look in;
    // and it's sparse, holding less than its length).
    let docker = home.join("Library/Containers/com.docker.docker/Data/vms");
    if let Some((bytes, _)) = search.folder_sizes(std::slice::from_ref(&docker)).pop().flatten().filter(|(b, _)| *b >= 1 << 30) {
        parts.push(Part {
            title: "Docker".into(),
            about: format!("Docker's disk of images and containers, which can grow to {}. Docker Desktop frees it: Troubleshoot › Clean / Purge data", crate::listing::human_size(bytes)),
            icon: "shippingbox",
            bytes: None,
            action: None,
        });
    }
    parts.sort_by_key(|p| std::cmp::Reverse(p.bytes.unwrap_or(0)));
    parts
}

/// Do what a part's action does (not Open: that's the app's to open). Waits.
pub fn run_part(action: &PartAction) -> Result<(), String> {
    let args: Vec<&str> = match action {
        PartAction::DeleteRuntime(id) => vec!["simctl", "runtime", "delete", id],
        PartAction::DeleteUnavailable => vec!["simctl", "delete", "unavailable"],
        PartAction::Open(..) => return Ok(()),
    };
    let out = std::process::Command::new("/usr/bin/xcrun").args(&args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let said = String::from_utf8_lossy(&out.stderr).lines().find(|l| !l.trim().is_empty()).unwrap_or("it didn't work").trim().to_string();
        Err(said)
    }
}

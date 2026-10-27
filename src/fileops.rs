//! File operations done here rather than through AppKit, named the way
//! Finder names things ("name copy.ext", "untitled folder 2"). Copies go
//! through `copyfile`: APFS clones where they can be (instant, sharing
//! blocks until either side changes), with progress and a way to stop.
//! What can't be copied (a file that can't be read, a name the disk won't
//! take) is left out and said, the rest copied on; what would fail every
//! file (the disk full, the drive gone) stops it. Stopped, what's copied
//! stays, to continue later (`Resolve::Continue`).

use std::ffi::{CStr, CString, OsStr, c_char, c_int, c_void};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::macos;

#[cfg(test)]
pub(crate) mod safety;

/// `name` split for numbering: files and apps keep their extension apart,
/// folders don't have one.
fn split(path: &Path) -> (String, String) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let is_folder = path.is_dir() && !crate::listing::is_package(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 && !is_folder => (name[..dot].to_string(), name[dot + 1..].to_string()),
        _ => (name, String::new()),
    }
}

/// `dir/base.ext`, or `base 2.ext`, `base 3.ext`... whichever is free.
fn free(dir: &Path, base: &str, ext: &str) -> PathBuf {
    let named = |n: Option<u32>| {
        let mut name = base.to_string();
        if let Some(n) = n {
            name += &format!(" {n}");
        }
        if !ext.is_empty() {
            name += &format!(".{ext}");
        }
        dir.join(name)
    };
    std::iter::once(None)
        .chain((2..).map(Some))
        .map(named)
        .find(|p| p.symlink_metadata().is_err())
        .unwrap()
}

/// `dir/base.ext`, or `base 2.ext`... whichever is free.
pub fn free_name(dir: &Path, base: &str, ext: &str) -> PathBuf {
    free(dir, base, ext)
}

/// How far a copy or move has got, and a way to stop it.
#[derive(Default)]
pub struct Progress {
    pub total: AtomicU64,
    pub done: AtomicU64,
    pub cancel: AtomicBool,
    /// What couldn't be copied (or moved), and why: left out, the rest done.
    pub failed: Mutex<Vec<(PathBuf, String)>>,
}

impl Progress {
    /// `path` left out, for `why` (once each).
    fn fail(&self, path: &Path, why: &io::Error) {
        if let Ok(mut failed) = self.failed.lock()
            && !failed.iter().any(|(p, _)| p == path)
        {
            failed.push((path.to_path_buf(), reason_for(path, why)));
        }
    }

    fn failures(&self) -> usize {
        self.failed.lock().map_or(0, |f| f.len())
    }
}

/// Why a file couldn't be copied (or a copy stopped), as said.
pub fn reason(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(libc::ENOSPC | libc::EDQUOT) => "There's no room left on that disk".into(),
        Some(libc::EROFS) => "That disk can't be written to".into(),
        Some(libc::ENOTCONN | libc::ESTALE | libc::ENETDOWN | libc::ENETUNREACH | libc::EHOSTDOWN | libc::EHOSTUNREACH | libc::ETIMEDOUT) => {
            "The connection to the server was lost".into()
        }
        Some(libc::ENXIO | libc::ENODEV) => "The disk went away".into(),
        Some(libc::EACCES | libc::EPERM) => "You don't have permission to read it".into(),
        Some(libc::ENOENT) => "It was gone before it could be copied".into(),
        Some(libc::ENAMETOOLONG) => "Its name is too long for that disk".into(),
        Some(libc::EILSEQ | libc::EINVAL) => "Its name has characters that disk can't take".into(),
        Some(libc::EFBIG) => "It's too big for that disk (FAT32 takes 4 GB at most)".into(),
        // (In a fresh copy: another of its names but for case, on a disk
        // that doesn't tell "README" from "readme".)
        Some(libc::EEXIST) => "Something with the same name but for capitals is there already, and that disk doesn't tell them apart".into(),
        _ => e.to_string(),
    }
}

/// Why `path` couldn't be copied or moved, as said: one that's locked
/// (Get Info › Locked) says so, not that you may not read it.
fn reason_for(path: &Path, e: &io::Error) -> String {
    use std::os::macos::fs::MetadataExt as _;
    let locked = path.symlink_metadata().is_ok_and(|m| m.st_flags() & (libc::UF_IMMUTABLE | libc::SF_IMMUTABLE) != 0);
    if locked && matches!(e.raw_os_error(), Some(libc::EPERM | libc::EACCES)) {
        return "It's locked (File › Get Info › Locked)".into();
    }
    reason(e)
}

/// An error every file after would get too (the disk full or read-only,
/// the drive or the server gone, the folder copied into gone): the copy
/// stops, rather than leaving out one file after another.
fn stops_all(e: &io::Error, dst: &Path) -> bool {
    matches!(
        e.raw_os_error(),
        Some(
            libc::ENOSPC
                | libc::EDQUOT
                | libc::EROFS
                | libc::ENOTCONN
                | libc::ESTALE
                | libc::ENETDOWN
                | libc::ENETUNREACH
                | libc::EHOSTDOWN
                | libc::EHOSTUNREACH
                | libc::ETIMEDOUT
                | libc::ENXIO
                | libc::ENODEV
        )
    ) || dst.parent().is_some_and(|dir| !dir.exists())
}

/// More left out than this: something's wrong with them all; stop.
const MOST_FAILED: usize = 10_000;

/// Destinations a copy left part-way (stopped, or the drive gone): copied
/// to again, they're offered Continue.
static PARTWAY: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Whether a copy into `dst` stopped part-way, to continue.
pub fn left_partway(dst: &Path) -> bool {
    PARTWAY.lock().is_ok_and(|p| p.iter().any(|d| d == dst))
}

fn partway(dst: &Path, left: bool) {
    if let Ok(mut p) = PARTWAY.lock() {
        p.retain(|d| d != dst);
        if left {
            p.push(dst.to_path_buf());
        }
    }
}

struct Tracking<'a> {
    progress: &'a Progress,
    /// Bytes done before the file being copied now.
    base: u64,
    /// What's being copied into (its folder gone: the copy stops).
    root: PathBuf,
    /// The file being written now (Stopped: it's half-written, removed).
    writing: Option<PathBuf>,
    /// Files whose copying failed part-way: removed after.
    broken: Vec<PathBuf>,
    /// What stopped it all.
    fatal: Option<io::Error>,
}

fn path_of(p: *const c_char) -> Option<PathBuf> {
    (!p.is_null()).then(|| PathBuf::from(OsStr::from_bytes(unsafe { CStr::from_ptr(p) }.to_bytes())))
}

fn size_at(p: *const c_char) -> u64 {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if !p.is_null() && unsafe { libc::lstat(p, &mut st) } == 0 { st.st_size as u64 } else { 0 }
}

extern "C" fn on_copy(
    what: c_int,
    stage: c_int,
    state: libc::copyfile_state_t,
    src: *const c_char,
    dst: *const c_char,
    ctx: *mut c_void,
) -> c_int {
    let tracking = unsafe { &mut *(ctx as *mut Tracking) };
    if tracking.progress.cancel.load(Ordering::Relaxed) {
        return libc::COPYFILE_QUIT;
    }
    match (what, stage) {
        (libc::COPYFILE_COPY_DATA, libc::COPYFILE_PROGRESS) => {
            let mut copied: libc::off_t = 0;
            unsafe { libc::copyfile_state_get(state, libc::COPYFILE_STATE_COPIED as u32, (&raw mut copied).cast()) };
            tracking.progress.done.store(tracking.base + copied as u64, Ordering::Relaxed);
        }
        (libc::COPYFILE_RECURSE_FILE, libc::COPYFILE_START) => tracking.writing = path_of(dst),
        (libc::COPYFILE_RECURSE_FILE, libc::COPYFILE_FINISH) => {
            tracking.writing = None;
            tracking.base += size_at(src);
            tracking.progress.done.store(tracking.base, Ordering::Relaxed);
        }
        // Something that can't be copied: left out (a half-written file
        // removed after), the rest copied on. Unless it'd be so for every
        // file: then it stops.
        (libc::COPYFILE_COPY_DATA | libc::COPYFILE_RECURSE_FILE | libc::COPYFILE_RECURSE_DIR | libc::COPYFILE_RECURSE_ERROR, libc::COPYFILE_ERR) => {
            let e = io::Error::last_os_error();
            if stops_all(&e, &tracking.root) || tracking.progress.failures() >= MOST_FAILED {
                tracking.fatal = Some(e);
                return libc::COPYFILE_QUIT;
            }
            if let Some(src) = path_of(src) {
                tracking.progress.fail(&src, &e);
            }
            // (Not there because it was taken, by another of its names but
            // for case, say: nothing was half-written, and what's there is
            // what was copied a moment ago. It stays.)
            let taken = e.raw_os_error() == Some(libc::EEXIST);
            if (what == libc::COPYFILE_COPY_DATA || what == libc::COPYFILE_RECURSE_FILE) && !taken {
                tracking.broken.extend(path_of(dst));
            }
            if taken {
                tracking.writing = None;
            }
            if what == libc::COPYFILE_RECURSE_FILE {
                tracking.writing = None;
                tracking.base += size_at(src);
                tracking.progress.done.store(tracking.base, Ordering::Relaxed);
            }
            return libc::COPYFILE_SKIP;
        }
        _ => {}
    }
    libc::COPYFILE_CONTINUE
}

/// Copy a file or a whole folder (into an existing folder: merged), as an
/// APFS clone where it can be, reporting to `progress`. What in a folder
/// can't be copied is left out (`Progress::failed`), the rest copied on.
/// Stopped (or what stopped it would stop every file), what's copied stays
/// and only the half-written file goes: copied again, it can continue.
fn copy_tracked(src: &Path, dst: &Path, progress: &Progress) -> io::Result<()> {
    let (s, d) = (cstr(src)?, cstr(dst)?);
    let existed = dst.symlink_metadata().is_ok();
    let start = progress.done.load(Ordering::Relaxed);
    let failed_before = progress.failures();
    let mut tracking = Tracking { progress, base: start, root: dst.to_path_buf(), writing: None, broken: Vec::new(), fatal: None };
    let flags = libc::COPYFILE_ACL
        | libc::COPYFILE_STAT
        | libc::COPYFILE_XATTR
        | libc::COPYFILE_DATA
        | libc::COPYFILE_RECURSIVE
        | libc::COPYFILE_CLONE
        | libc::COPYFILE_NOFOLLOW_SRC;
    let result = unsafe {
        let state = libc::copyfile_state_alloc();
        let callback: extern "C" fn(c_int, c_int, libc::copyfile_state_t, *const c_char, *const c_char, *mut c_void) -> c_int =
            on_copy;
        libc::copyfile_state_set(state, libc::COPYFILE_STATE_STATUS_CB as u32, callback as *const c_void);
        libc::copyfile_state_set(state, libc::COPYFILE_STATE_STATUS_CTX as u32, (&raw mut tracking).cast());
        let r = libc::copyfile(s.as_ptr(), d.as_ptr(), state, flags);
        let err = io::Error::last_os_error();
        libc::copyfile_state_free(state);
        if r == 0 { Ok(()) } else { Err(err) }
    };
    // (Files that failed part-way: not left half-written.)
    for broken in tracking.broken.iter().chain(&tracking.writing).filter(|b| b.as_path() != dst || !existed) {
        if broken.symlink_metadata().is_ok_and(|m| !m.is_dir()) {
            let _ = std::fs::remove_file(broken);
        }
    }
    let stopped = progress.cancel.load(Ordering::Relaxed);
    let src_is_dir = src.symlink_metadata().is_ok_and(|m| m.is_dir());
    if stopped || tracking.fatal.is_some() {
        // What's copied stays, to continue (a folder's); a file's half-copy goes.
        if !src_is_dir && !existed {
            let _ = remove(dst);
        }
        if src_is_dir && dst.exists() {
            partway(dst, true);
        }
        return Err(tracking.fatal.take().unwrap_or_else(|| io::Error::other("Stopped.")));
    }
    // (The item itself couldn't be: said by whoever asked, not twice.)
    if tracking.broken.iter().any(|b| b == dst) || (!src_is_dir && !dst.exists()) {
        if let Ok(mut failed) = progress.failed.lock() {
            failed.retain(|(p, _)| p != src);
        }
        return Err(result.err().unwrap_or_else(|| io::Error::other("it couldn't be copied")));
    }
    // (Some of what's in it left out: done, those said.)
    if let Err(e) = result
        && progress.failures() == failed_before
    {
        if !existed {
            let _ = remove(dst);
        }
        return Err(e);
    }
    partway(dst, false);
    // Clones report nothing as they go: count the whole item now.
    progress.done.fetch_max(start + size_of(src), Ordering::Relaxed);
    Ok(())
}

/// Put what's in folder `src` into folder `dst`: folders in both are merged
/// in turn, a file in both is replaced by the incoming one (that one to the
/// Trash); or, `continuing` (a copy that stopped part-way, copied again),
/// left as it is if it's the same length and date (copied before). With
/// `keep_source` false the items are moved, not copied. What can't be is
/// left out and said (`Progress::failed`), the rest done.
fn merge_into(src: &Path, dst: &Path, keep_source: bool, continuing: bool, progress: &Progress) -> io::Result<()> {
    for child in std::fs::read_dir(src)? {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Stopped."));
        }
        let child = child?.path();
        let target = dst.join(child.file_name().unwrap_or_default());
        let Ok(meta) = child.symlink_metadata() else { continue };
        let there = target.symlink_metadata().ok();
        let both_folders = meta.is_dir() && there.as_ref().is_some_and(|m| m.is_dir()) && !crate::listing::is_package(&target);
        let done = if both_folders {
            merge_into(&child, &target, keep_source, continuing, progress)
        } else if continuing && there.as_ref().is_some_and(|t| !meta.is_dir() && t.len() == meta.len() && t.mtime() == meta.mtime()) {
            // (Copied before the stop.)
            progress.done.fetch_add(meta.len(), Ordering::Relaxed);
            if !keep_source {
                let _ = std::fs::remove_file(&child);
            }
            Ok(())
        } else {
            (|| {
                if there.is_some() {
                    macos::trash(&target).map_err(io::Error::other)?;
                }
                if keep_source { copy_tracked(&child, &target, progress) } else { move_tracked(&child, &target, progress) }
            })()
        };
        match done {
            Ok(()) => {}
            Err(e) if progress.cancel.load(Ordering::Relaxed) || stops_all(&e, &target) => return Err(e),
            Err(e) => progress.fail(&child, &e),
        }
    }
    if !keep_source {
        // Moved out, leaving an empty folder (or what couldn't go).
        let _ = std::fs::remove_dir(src);
    }
    partway(dst, false);
    Ok(())
}

/// A copy of a file or a whole folder (see `copy_tracked`).
fn copy(src: &Path, dst: &Path) -> io::Result<()> {
    copy_tracked(src, dst, &Progress::default())
}

fn remove(path: &Path) -> io::Result<()> {
    if path.symlink_metadata()?.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) }
}

/// Bytes in a file, or in a folder all the way down.
fn size_of(path: &Path) -> u64 {
    match path.symlink_metadata() {
        Ok(meta) if meta.is_dir() => folder_size(path, &AtomicBool::new(false)).0,
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

fn cstr(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)
}

fn run(command: &mut Command) -> io::Result<()> {
    let out = command.output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

fn parent(path: &Path) -> io::Result<&Path> {
    path.parent().ok_or_else(|| io::Error::other("no enclosing folder"))
}

/// "name copy.ext" next to it.
pub fn duplicate(path: &Path) -> io::Result<PathBuf> {
    let (stem, ext) = split(path);
    let dst = free(parent(path)?, &format!("{stem} copy"), &ext);
    copy(path, &dst)?;
    Ok(dst)
}

/// "untitled folder" in `dir`.
pub fn new_folder(dir: &Path) -> io::Result<PathBuf> {
    let dst = free(dir, "untitled folder", "");
    std::fs::create_dir(&dst)?;
    Ok(dst)
}

/// An empty text file in `dir`: "untitled.txt" (or "untitled 2.txt"…).
pub fn new_file(dir: &Path) -> io::Result<PathBuf> {
    let dst = free(dir, "untitled", "txt");
    std::fs::OpenOptions::new().write(true).create_new(true).open(&dst)?;
    Ok(dst)
}

/// Move or rename, across volumes too (as a copy, then the original
/// removed).
pub fn move_item(src: &Path, dst: &Path) -> io::Result<()> {
    move_tracked(src, dst, &Progress::default())
}

fn move_tracked(src: &Path, dst: &Path, progress: &Progress) -> io::Result<()> {
    match std::fs::rename(src, dst) {
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            let failed_before = progress.failures();
            copy_tracked(src, dst, progress)?;
            // (Some of it couldn't be copied: the original stays, all of
            // it, rather than those being lost.)
            if progress.failures() > failed_before {
                return Err(io::Error::other("copied, but not moved: some of what's in it couldn't be copied, so the original's still there"));
            }
            remove(src)
        }
        Ok(()) => {
            progress.done.fetch_add(size_of(dst), Ordering::Relaxed);
            Ok(())
        }
        other => other,
    }
}

/// Whether two paths are the same item (as "a" and "A" are on a disk that
/// ignores case).
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (a.symlink_metadata(), b.symlink_metadata()) {
        (Ok(a), Ok(b)) => (a.dev(), a.ino()) == (b.dev(), b.ino()),
        _ => false,
    }
}

/// Whether two paths are on the same volume (a drag between them moves).
pub fn same_volume(a: &Path, b: &Path) -> bool {
    match (a.symlink_metadata(), b.metadata()) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

/// How files are put into a folder.
#[derive(Clone, Copy, PartialEq)]
pub enum How {
    /// As a drop: moved within a volume, copied to another.
    Drop,
    Copy,
    /// Moved even to another volume (copied, then the original removed):
    /// Move Item Here.
    Move,
}

/// One item of a copy or move: where it goes, and whether something there
/// has its name already.
#[derive(Clone)]
pub struct Item {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub copy: bool,
    pub taken: bool,
}

impl Item {
    /// Both folders: they can be merged.
    pub fn folders(&self) -> bool {
        self.src.is_dir() && self.dst.is_dir() && !crate::listing::is_package(&self.dst)
    }
}

/// What to do about a name that's taken (Finder's choices).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Resolve {
    /// Put it next to it, numbered ("name 2.ext").
    KeepBoth,
    /// The one there goes to the Trash.
    Replace,
    /// Folders: what's in this one goes into that one.
    Merge,
    /// A copy that stopped part-way, copied again: what's missing copied,
    /// what was copied (the same length and date) left as it is.
    Continue,
    Skip,
}

/// Work out where `sources` go in `dir`: moved within a volume or copied
/// (see `How`), already-there moves left out, and which names are taken.
/// An item copied into its own folder becomes "name copy", as in Finder.
pub fn plan(sources: &[PathBuf], dir: &Path, how: How) -> io::Result<Vec<Item>> {
    let mut items = Vec::new();
    for src in sources {
        let copy = how == How::Copy || (how == How::Drop && !same_volume(src, dir));
        if !copy && src.parent() == Some(dir) {
            continue;
        }
        if dir.starts_with(src) {
            let name = src.file_name().unwrap_or_default().to_string_lossy();
            return Err(io::Error::other(format!("“{name}” can't go inside itself.")));
        }
        let mut dst = dir.join(src.file_name().ok_or_else(|| io::Error::other("nothing to copy"))?);
        let mut taken = dst.symlink_metadata().is_ok();
        if taken && same_file(src, &dst) {
            let (stem, ext) = split(src);
            dst = free(dir, &format!("{stem} copy"), &ext);
            taken = false;
        }
        items.push(Item { src: src.clone(), dst, copy, taken });
    }
    Ok(items)
}

/// Bytes a plan will copy or move, for its progress.
pub fn plan_size(items: &[(Item, Resolve)]) -> u64 {
    items.iter().filter(|(_, r)| *r != Resolve::Skip).map(|(item, _)| size_of(&item.src)).sum()
}

/// What a plan did: moved (into the Trash too: what it replaced), and made.
#[derive(Default)]
pub struct Done {
    pub moved: Vec<(PathBuf, PathBuf)>,
    pub created: Vec<PathBuf>,
    /// Merged folders can't be undone.
    pub merged: bool,
}

/// Carry out a plan. On an error or when stopped it stops there, returning
/// what it did before (to undo) with the error.
pub fn execute(items: &[(Item, Resolve)], progress: &Progress) -> (Done, Option<io::Error>) {
    let mut done = Done::default();
    for (item, resolve) in items {
        if progress.cancel.load(Ordering::Relaxed) {
            return (done, Some(io::Error::other("Stopped.")));
        }
        match execute_one(item, *resolve, progress, &mut done) {
            Ok(()) => {}
            // (Stopped, or what'd stop every item after: the disk full.)
            Err(e) if progress.cancel.load(Ordering::Relaxed) || stops_all(&e, &item.dst) => return (done, Some(e)),
            // (This one couldn't be: said, and on to the next.)
            Err(e) => progress.fail(&item.src, &e),
        }
    }
    (done, None)
}

fn execute_one(item: &Item, resolve: Resolve, progress: &Progress, done: &mut Done) -> io::Result<()> {
    let mut dst = item.dst.clone();
    if item.taken {
        match resolve {
            Resolve::Skip => return Ok(()),
            Resolve::KeepBoth => {
                let (stem, ext) = split(&item.src);
                dst = free(parent(&item.dst)?, &stem, &ext);
            }
            Resolve::Replace => {
                let trashed = macos::trash(&dst).map_err(io::Error::other)?;
                done.moved.push((dst.clone(), trashed));
            }
            Resolve::Merge | Resolve::Continue => {
                done.merged = true;
                return merge_into(&item.src, &dst, item.copy, resolve == Resolve::Continue, progress);
            }
        }
    }
    if item.copy {
        copy_tracked(&item.src, &dst, progress)?;
        done.created.push(dst);
    } else {
        move_tracked(&item.src, &dst, progress)?;
        done.moved.push((item.src.clone(), dst));
    }
    Ok(())
}

/// Compress one item as "name.zip", or several as "Archive.zip", next to
/// them (they must share a folder).
pub fn compress_all(paths: &[PathBuf]) -> io::Result<PathBuf> {
    let [first, rest @ ..] = paths else { return Err(io::Error::other("nothing to compress")) };
    if rest.is_empty() {
        return compress(first);
    }
    let dir = parent(first)?;
    if rest.iter().any(|p| p.parent() != Some(dir)) {
        return Err(io::Error::other("Items from different folders can't go into one archive."));
    }
    let dst = free(dir, "Archive", "zip");
    let names = paths.iter().filter_map(|p| p.file_name());
    run(Command::new("/usr/bin/zip").current_dir(dir).args(["-r", "-y", "-q"]).arg(&dst).args(names))?;
    Ok(dst)
}

/// "name.zip" next to it, as Finder's Compress makes it (Archive Utility
/// runs the same `ditto`).
pub fn compress(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dst = free(parent(path)?, &name, "zip");
    run(Command::new("/usr/bin/ditto").args(["-c", "-k", "--sequesterRsrc", "--keepParent"]).arg(path).arg(&dst))?;
    Ok(dst)
}

/// Where Make Alias puts the alias: "name alias" next to it.
pub fn alias_path(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(free(parent(path)?, &format!("{name} alias"), ""))
}

/// Bytes and items in a folder, all the way down (links not followed),
/// for Get Info: a file with more than one name (hard links) once among
/// them, as the index counts it. Stops early once `cancel` is set.
pub fn folder_size(dir: &Path, cancel: &AtomicBool) -> (u64, usize) {
    let (mut bytes, mut items) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            items += 1;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += meta.len() / meta.nlink().max(1);
            }
        }
    }
    (bytes, items)
}

/// Bytes and items in several items together, folders all the way down
/// (by `folder_size`), for Get Info on a selection.
pub fn total_size(paths: &[PathBuf], folder_size: impl Fn(&Path) -> (u64, usize)) -> (u64, usize) {
    paths.iter().fold((0, 0), |(bytes, items), path| match path.symlink_metadata() {
        Ok(meta) if meta.is_dir() => {
            let (b, n) = folder_size(path);
            (bytes + b, items + n + 1)
        }
        Ok(meta) => (bytes + meta.len(), items + 1),
        Err(_) => (bytes, items),
    })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::{Progress, copy_tracked, merge_into};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("f2-fileops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A file in a folder that can't be read: left out and said, the rest
    /// of the folder copied (not all of it thrown away).
    #[test]
    fn copies_past_what_cant_be_read() {
        let dir = scratch("past");
        let src = dir.join("Folder");
        std::fs::create_dir_all(src.join("inner")).unwrap();
        for (name, text) in [("a.txt", "a"), ("secret.txt", "s"), ("inner/c.txt", "c")] {
            std::fs::write(src.join(name), text).unwrap();
        }
        std::fs::set_permissions(src.join("secret.txt"), std::fs::Permissions::from_mode(0o000)).unwrap();
        let dst = dir.join("Copy");
        let progress = Progress::default();
        let result = copy_tracked(&src, &dst, &progress);
        let failed = progress.failed.lock().unwrap().clone();
        std::fs::set_permissions(src.join("secret.txt"), std::fs::Permissions::from_mode(0o644)).unwrap();
        let (a, c, secret) = (dst.join("a.txt").exists(), dst.join("inner/c.txt").exists(), dst.join("secret.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "{result:?}");
        assert!(a && c, "the rest copied");
        assert!(!secret, "no empty or half copy of it");
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert!(failed[0].0.ends_with("secret.txt") && failed[0].1.contains("permission"), "{failed:?}");
    }

    /// A copy that stopped part-way, continued: what's missing copied, what
    /// was copied (the same length and date) left as it is.
    #[test]
    fn continues_a_stopped_copy() {
        let dir = scratch("continue");
        let (src, dst) = (dir.join("Folder"), dir.join("Copy"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("done.txt"), "copied").unwrap();
        std::fs::write(src.join("missing.txt"), "to copy").unwrap();
        // (Copied before the stop: the same length and date, other bytes,
        // to tell it's left as it is.)
        std::fs::write(dst.join("done.txt"), "COPIED").unwrap();
        let when = std::fs::metadata(src.join("done.txt")).unwrap().modified().unwrap();
        std::fs::File::options().write(true).open(dst.join("done.txt")).unwrap().set_modified(when).unwrap();
        let progress = Progress::default();
        let result = merge_into(&src, &dst, true, true, &progress);
        let (done, missing) = (std::fs::read_to_string(dst.join("done.txt")).unwrap(), std::fs::read_to_string(dst.join("missing.txt")).ok());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(done, "COPIED", "left as it was");
        assert_eq!(missing.as_deref(), Some("to copy"));
    }
}

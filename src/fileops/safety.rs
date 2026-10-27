//! What copying and moving must never do, tried the awkward ways: lose a
//! file, leave one half-written under its own name, remove an original
//! that wasn't all copied, or put something somewhere it wasn't asked to.
//!
//! The tests on a disk image (another disk, a full one, its own Trash, a
//! disk that can't take some names) are slower, and ignored by default:
//! `cargo test safety -- --ignored`. Nothing here touches your Trash: what's
//! replaced or trashed is on a disk image, which has its own.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;

use super::{How, Progress, Resolve, copy_tracked, execute, plan};

/// A disk image of `megabytes` formatted as `fs` ("APFS", "Case-sensitive
/// APFS", "MS-DOS FAT32"), mounted out of sight (-nobrowse) and detached
/// when dropped. Its own Trash takes what's trashed on it.
pub(crate) struct Disk {
    pub(crate) root: PathBuf,
    dir: PathBuf,
}

impl Disk {
    pub(crate) fn new(name: &str, megabytes: u32, fs: &str) -> Disk {
        let dir = std::env::temp_dir().join(format!("f2-disk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // (Sparse: only what's written takes room.)
        let image = dir.join("disk.sparseimage");
        let volname = if fs.contains("FAT") { "F2TEST" } else { "F2 Test" };
        let made = Command::new("/usr/bin/hdiutil")
            .args(["create", "-quiet", "-type", "SPARSE", "-size", &format!("{megabytes}m"), "-fs", fs, "-volname", volname])
            .arg(&image)
            .status()
            .unwrap();
        assert!(made.success(), "hdiutil create {fs}");
        let root = dir.join("mnt");
        std::fs::create_dir_all(&root).unwrap();
        let mounted = Command::new("/usr/bin/hdiutil")
            .args(["attach", "-quiet", "-nobrowse", "-noverify", "-noautoopen", "-mountpoint"])
            .arg(&root)
            .arg(&image)
            .status()
            .unwrap();
        assert!(mounted.success(), "hdiutil attach");
        // (As macOS gives paths back: /var is /private/var.)
        let root = std::fs::canonicalize(&root).unwrap();
        Disk { root, dir }
    }

    /// Where what's trashed on it goes (yours: .Trashes can't be read).
    pub(crate) fn trash(&self) -> PathBuf {
        self.root.join(".Trashes").join(unsafe { libc::getuid() }.to_string())
    }
}

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet", "-force"]).arg(&self.root).status();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A folder to work in on the startup disk, gone when dropped (what's
/// locked in it unlocked first).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("f2-safety-{name}-{}", std::process::id()));
        unlock_all(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        unlock_all(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

/// `n` bytes that don't repeat (nothing to compress or share).
fn noise(n: usize) -> Vec<u8> {
    let mut x: u64 = 0x9e3779b97f4a7c15;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// What's in a folder, all the way down: each file's bytes, each folder,
/// each link's target, by its path inside (as bytes: names compared
/// exactly).
#[derive(Debug, PartialEq)]
enum Node {
    File(Vec<u8>),
    Folder,
    Link(PathBuf),
}

fn tree(dir: &Path) -> BTreeMap<Vec<u8>, Node> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(dir).unwrap().as_os_str().as_bytes().to_vec();
            let meta = path.symlink_metadata().unwrap();
            let node = if meta.file_type().is_symlink() {
                Node::Link(std::fs::read_link(&path).unwrap())
            } else if meta.is_dir() {
                stack.push(path.clone());
                Node::Folder
            } else {
                Node::File(std::fs::read(&path).unwrap())
            };
            out.insert(rel, node);
        }
    }
    out
}

const LOCKED: u32 = libc::UF_IMMUTABLE as u32;

fn set_locked(path: &Path, locked: bool) {
    let c = CString::new(path.as_os_str().as_bytes()).unwrap();
    let flags = path.symlink_metadata().map(|m| m.st_flags()).unwrap_or(0);
    let flags = if locked { flags | LOCKED } else { flags & !LOCKED };
    unsafe { libc::chflags(c.as_ptr(), flags) };
}

fn unlock_all(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.symlink_metadata().is_ok_and(|m| m.st_flags() & LOCKED != 0) {
            set_locked(&path, false);
        }
        if path.symlink_metadata().is_ok_and(|m| m.is_dir()) {
            unlock_all(&path);
        }
    }
}

fn xattr(path: &Path, name: &str) -> Option<Vec<u8>> {
    let (p, n) = (CString::new(path.as_os_str().as_bytes()).unwrap(), CString::new(name).unwrap());
    let size = unsafe { libc::getxattr(p.as_ptr(), n.as_ptr(), std::ptr::null_mut(), 0, 0, libc::XATTR_NOFOLLOW) };
    if size < 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    unsafe { libc::getxattr(p.as_ptr(), n.as_ptr(), buf.as_mut_ptr().cast(), buf.len(), 0, libc::XATTR_NOFOLLOW) };
    Some(buf)
}

fn set_xattr(path: &Path, name: &str, value: &[u8]) {
    let (p, n) = (CString::new(path.as_os_str().as_bytes()).unwrap(), CString::new(name).unwrap());
    let r = unsafe { libc::setxattr(p.as_ptr(), n.as_ptr(), value.as_ptr().cast(), value.len(), 0, libc::XATTR_NOFOLLOW) };
    assert_eq!(r, 0, "setxattr");
}

/// Put `sources` into `dir` as `how` says, every taken name resolved as
/// `resolve`; what it did, what was left out, and its error.
fn put(sources: &[PathBuf], dir: &Path, how: How, resolve: Resolve) -> (super::Done, Vec<(PathBuf, String)>, Option<std::io::Error>) {
    let items = plan(sources, dir, how).unwrap();
    let plan: Vec<_> = items.into_iter().map(|i| (i, resolve)).collect();
    let progress = Progress::default();
    let (done, error) = execute(&plan, &progress);
    let failed = progress.failed.lock().unwrap().clone();
    (done, failed, error)
}

// --- On the startup disk. ---

/// A copy is the same file: its bytes, date, permissions and extended
/// attributes (tags, where it came from), and the original's untouched.
#[test]
fn a_copy_keeps_contents_dates_permissions_and_attributes() {
    let dir = Scratch::new("details");
    let src = dir.join("Report.pdf");
    let bytes = noise(300_000);
    write(&src, &bytes);
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o640)).unwrap();
    set_xattr(&src, "com.apple.metadata:kMDItemWhereFroms", b"https://example.com");
    let when = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    std::fs::File::options().write(true).open(&src).unwrap().set_modified(when).unwrap();
    std::fs::create_dir(dir.join("Out")).unwrap();
    let (done, failed, error) = put(&[src.clone()], &dir.join("Out"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    let copy = dir.join("Out/Report.pdf");
    assert_eq!(done.created, vec![copy.clone()]);
    assert_eq!(std::fs::read(&copy).unwrap(), bytes);
    let (a, b) = (src.metadata().unwrap(), copy.metadata().unwrap());
    assert_eq!(a.mtime(), b.mtime(), "the date");
    assert_eq!(a.mode() & 0o777, b.mode() & 0o777, "the permissions");
    assert_eq!(xattr(&copy, "com.apple.metadata:kMDItemWhereFroms").as_deref(), Some(&b"https://example.com"[..]));
    assert_eq!(std::fs::read(&src).unwrap(), bytes, "the original untouched");
}

/// A folder's copied whole: what's hidden, what's empty, what's deep; and
/// links as links (not what they point to, which may be outside it).
#[test]
fn a_folder_is_copied_whole_and_links_as_links() {
    let dir = Scratch::new("whole");
    let src = dir.join("Project");
    write(&src.join(".env"), "SECRET=1");
    write(&src.join("a/b/c/deep.txt"), "deep");
    std::fs::create_dir_all(src.join("empty")).unwrap();
    write(&dir.join("outside/big.bin"), noise(10_000));
    symlink("a/b/c/deep.txt", src.join("near link")).unwrap();
    symlink(dir.join("outside"), src.join("far link")).unwrap();
    symlink("nowhere", src.join("broken link")).unwrap();
    std::fs::create_dir(dir.join("Out")).unwrap();
    let (_, failed, error) = put(&[src.clone()], &dir.join("Out"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(tree(&dir.join("Out/Project")), tree(&src));
    assert!(dir.join("Out/Project/far link").symlink_metadata().unwrap().file_type().is_symlink(), "not followed");
}

/// Names of every kind come through exactly as they were: accents either
/// way Unicode writes them, emoji, Icelandic, Korean, a colon (a slash in
/// Finder), a leading dot, spaces at the ends, and the longest a name can
/// be. Copied and then moved.
#[test]
fn names_of_every_kind_come_through() {
    let dir = Scratch::new("names");
    let src = dir.join("Names");
    let names = [
        "caf\u{e9} (one character).txt".to_string(),
        "cafe\u{301} (two).txt".to_string(),
        "📁 Notes 🎉.txt".to_string(),
        "Þórður ætlar í bæinn.txt".to_string(),
        "안녕하세요.txt".to_string(),
        "a:b (a slash in Finder).txt".to_string(),
        ".hidden".to_string(),
        " spaced ".to_string(),
        // 255 bytes, in characters of two.
        format!("{}.t", "é".repeat(126)),
    ];
    for name in &names {
        assert!(name.len() <= 255, "{name}");
        write(&src.join(name), name.as_bytes());
    }
    std::fs::create_dir(dir.join("Copied")).unwrap();
    let (_, failed, error) = put(&[src.clone()], &dir.join("Copied"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(tree(&dir.join("Copied/Names")), tree(&src), "copied");
    std::fs::create_dir(dir.join("Moved")).unwrap();
    let before = tree(&src);
    let (_, failed, error) = put(&[src.clone()], &dir.join("Moved"), How::Move, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(tree(&dir.join("Moved/Names")), before, "moved");
    assert!(!src.exists());
}

/// Copied into its own folder, it's "name copy", then "name copy 2": never
/// onto itself.
#[test]
fn a_copy_into_its_own_folder_is_named_a_copy() {
    let dir = Scratch::new("own");
    let src = dir.join("Plan.key");
    write(&src, "v1");
    for expected in ["Plan copy.key", "Plan copy 2.key"] {
        let (done, failed, error) = put(&[src.clone()], &dir, How::Copy, Resolve::Replace);
        assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
        assert_eq!(done.created, vec![dir.join(expected)]);
        assert!(done.moved.is_empty(), "nothing replaced");
    }
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "v1");
}

/// A folder can't go into itself, or into what's in it.
#[test]
fn nothing_goes_inside_itself() {
    let dir = Scratch::new("itself");
    let src = dir.join("Folder");
    std::fs::create_dir_all(src.join("Inner")).unwrap();
    for into in [src.clone(), src.join("Inner")] {
        for how in [How::Copy, How::Move, How::Drop] {
            assert!(plan(&[src.clone()], &into, how).is_err(), "{into:?}");
        }
    }
    assert!(src.join("Inner").is_dir());
}

/// Moved within a disk, it's renamed, not copied: the same file, at once,
/// whatever its size.
#[test]
fn a_move_within_a_disk_is_a_rename() {
    let dir = Scratch::new("rename");
    let src = dir.join("Big.mov");
    write(&src, noise(1_000_000));
    let ino = src.metadata().unwrap().ino();
    std::fs::create_dir(dir.join("Out")).unwrap();
    let (done, failed, error) = put(&[src.clone()], &dir.join("Out"), How::Drop, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    let to = dir.join("Out/Big.mov");
    assert_eq!(done.moved, vec![(src.clone(), to.clone())]);
    assert_eq!(to.metadata().unwrap().ino(), ino, "the same file");
    assert!(!src.exists());
}

/// Keep Both puts it next to the one there, numbered; Skip leaves both as
/// they were. Neither touches the one there.
#[test]
fn keep_both_and_skip_leave_what_was_there() {
    let dir = Scratch::new("keep");
    let src = dir.join("In/notes.txt");
    write(&src, "new");
    write(&dir.join("Out/notes.txt"), "old");
    let (done, failed, error) = put(&[src.clone()], &dir.join("Out"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(done.created, vec![dir.join("Out/notes 2.txt")]);
    assert_eq!(std::fs::read_to_string(dir.join("Out/notes.txt")).unwrap(), "old");
    assert_eq!(std::fs::read_to_string(dir.join("Out/notes 2.txt")).unwrap(), "new");
    let (done, _, error) = put(&[src.clone()], &dir.join("Out"), How::Move, Resolve::Skip);
    assert!(error.is_none() && done.moved.is_empty() && done.created.is_empty());
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "new", "skipped: still where it was");
    assert_eq!(std::fs::read_to_string(dir.join("Out/notes.txt")).unwrap(), "old");
}

/// On a disk that ignores case (the Mac's own), "readme.md" and
/// "README.md" are one name: it's taken, and asked about, not overwritten.
#[test]
fn a_name_differing_only_in_case_is_taken() {
    let dir = Scratch::new("case");
    write(&dir.join("In/readme.md"), "new");
    write(&dir.join("Out/README.md"), "old");
    let items = plan(&[dir.join("In/readme.md")], &dir.join("Out"), How::Copy).unwrap();
    assert!(items[0].taken, "asked about");
    let (done, _, error) = put(&[dir.join("In/readme.md")], &dir.join("Out"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none());
    assert_eq!(done.created, vec![dir.join("Out/readme 2.md")]);
    assert_eq!(std::fs::read_to_string(dir.join("Out/README.md")).unwrap(), "old");
}

/// Stopped before it's begun, nothing's left half-written, and the
/// original's whole.
#[test]
fn stopped_at_once_leaves_nothing_half_written() {
    let dir = Scratch::new("stopped");
    let src = dir.join("Video.mp4");
    let bytes = noise(2_000_000);
    write(&src, &bytes);
    let progress = Progress::default();
    progress.cancel.store(true, Ordering::Relaxed);
    let result = copy_tracked(&src, &dir.join("Copy.mp4"), &progress);
    assert!(result.is_err());
    assert!(!dir.join("Copy.mp4").exists(), "no half copy");
    assert_eq!(std::fs::read(&src).unwrap(), bytes);
}

/// A locked file (Get Info › Locked) is copied, locked too, and the
/// original stays; it can't be moved, and that says it's locked, the
/// original left where it was.
#[test]
fn a_locked_file_is_copied_but_not_moved() {
    let dir = Scratch::new("locked");
    let src = dir.join("In/Contract.pdf");
    write(&src, "signed");
    set_locked(&src, true);
    std::fs::create_dir(dir.join("Out")).unwrap();
    let (done, failed, error) = put(&[src.clone()], &dir.join("Out"), How::Copy, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(done.created, vec![dir.join("Out/Contract.pdf")]);
    assert!(dir.join("Out/Contract.pdf").symlink_metadata().unwrap().st_flags() & LOCKED != 0, "locked too");
    std::fs::create_dir(dir.join("Moved")).unwrap();
    let (done, failed, error) = put(&[src.clone()], &dir.join("Moved"), How::Move, Resolve::KeepBoth);
    assert!(error.is_none() && done.moved.is_empty(), "{error:?}");
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert!(failed[0].1.contains("locked"), "says why: {failed:?}");
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "signed", "still where it was");
}

// --- On disk images. ---

/// Moved to another disk: copied whole, then the original removed.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_move_to_another_disk_copies_then_removes_the_original() {
    let (dir, disk) = (Scratch::new("across"), Disk::new("across", 64, "APFS"));
    let src = dir.join("Photos");
    write(&src.join("2024/a.heic"), noise(400_000));
    write(&src.join("2025/b.heic"), noise(300_000));
    let before = tree(&src);
    let (done, failed, error) = put(&[src.clone()], &disk.root, How::Move, Resolve::KeepBoth);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(done.moved, vec![(src.clone(), disk.root.join("Photos"))]);
    assert_eq!(tree(&disk.root.join("Photos")), before);
    assert!(!src.exists(), "the original removed once it's all there");
}

/// Dropped on another disk, it's copied: the original stays.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_drop_on_another_disk_copies() {
    let (dir, disk) = (Scratch::new("drop"), Disk::new("drop", 64, "APFS"));
    let src = dir.join("Notes.txt");
    write(&src, "keep me");
    let (done, _, error) = put(&[src.clone()], &disk.root, How::Drop, Resolve::KeepBoth);
    assert!(error.is_none());
    assert_eq!(done.created, vec![disk.root.join("Notes.txt")]);
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "keep me");
}

/// A move to another disk where part of it can't be copied keeps the
/// original, all of it: what couldn't come isn't lost.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_move_to_another_disk_keeps_the_original_if_any_of_it_failed() {
    let (dir, disk) = (Scratch::new("partial"), Disk::new("partial", 64, "APFS"));
    let src = dir.join("Work");
    write(&src.join("a.txt"), "a");
    write(&src.join("secret.txt"), "s");
    std::fs::set_permissions(src.join("secret.txt"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let (_, failed, error) = put(&[src.clone()], &disk.root, How::Move, Resolve::KeepBoth);
    std::fs::set_permissions(src.join("secret.txt"), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(error.is_some() || !failed.is_empty(), "said");
    assert_eq!(std::fs::read_to_string(src.join("a.txt")).unwrap(), "a", "the original, all of it");
    assert_eq!(std::fs::read_to_string(src.join("secret.txt")).unwrap(), "s");
}

/// A disk that fills up stops the copy: what's copied is whole, nothing's
/// left half-written, and a move keeps its original.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_full_disk_stops_it_and_keeps_the_original() {
    let (dir, disk) = (Scratch::new("full"), Disk::new("full", 24, "APFS"));
    let src = dir.join("Footage");
    for i in 0..6 {
        write(&src.join(format!("clip {i}.mov")), noise(6_000_000));
    }
    let before = tree(&src);
    for how in [How::Copy, How::Move] {
        let _ = std::fs::remove_dir_all(disk.root.join("Footage"));
        let (_, _, error) = put(&[src.clone()], &disk.root, how, Resolve::KeepBoth);
        let error = error.expect("stopped");
        assert!(super::reason(&error).contains("no room"), "{error:?}");
        assert_eq!(tree(&src), before, "the original whole");
        if let Ok(entries) = std::fs::read_dir(disk.root.join("Footage")) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                assert_eq!(std::fs::read(entry.path()).unwrap(), std::fs::read(src.join(&name)).unwrap(), "{name:?} whole or not there");
            }
        }
    }
}

/// Stopped part-way to another disk (no clone: the bytes are copied), the
/// file being written is gone, not left half-written under its name.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn stopped_part_way_leaves_no_half_written_file() {
    let (dir, disk) = (Scratch::new("partway"), Disk::new("partway", 600, "APFS"));
    let src = dir.join("Big.mov");
    let bytes = noise(300_000_000);
    write(&src, &bytes);
    let progress = std::sync::Arc::new(Progress::default());
    let watcher = {
        let progress = progress.clone();
        std::thread::spawn(move || {
            while progress.done.load(Ordering::Relaxed) == 0 {
                std::thread::yield_now();
            }
            progress.cancel.store(true, Ordering::Relaxed);
        })
    };
    let dst = disk.root.join("Big.mov");
    let result = copy_tracked(&src, &dst, &progress);
    progress.done.store(1, Ordering::Relaxed);
    watcher.join().unwrap();
    match result {
        // (Done before the stop got to it: then it's whole.)
        Ok(()) => assert_eq!(std::fs::read(&dst).unwrap().len(), bytes.len()),
        Err(_) => assert!(!dst.exists(), "no half-written file"),
    }
    assert_eq!(std::fs::metadata(&src).unwrap().len(), bytes.len() as u64, "the original whole");
}

/// Replace puts the one there in that disk's Trash (not lost, and not in
/// yours), and the new one in its place.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn replace_puts_the_old_one_in_that_disks_trash() {
    let disk = Disk::new("replace", 64, "APFS");
    write(&disk.root.join("In/budget.numbers"), "new");
    write(&disk.root.join("Out/budget.numbers"), "old");
    let (done, failed, error) = put(&[disk.root.join("In/budget.numbers")], &disk.root.join("Out"), How::Copy, Resolve::Replace);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert_eq!(std::fs::read_to_string(disk.root.join("Out/budget.numbers")).unwrap(), "new");
    let (from, trashed) = done.moved.first().cloned().expect("the old one moved");
    assert_eq!(from, disk.root.join("Out/budget.numbers"));
    assert!(trashed.starts_with(disk.trash()), "this disk's Trash: {trashed:?}");
    assert_eq!(std::fs::read_to_string(&trashed).unwrap(), "old", "not lost");
}

/// Merged: what's in both folders together; a file in both, the incoming
/// one, the other to the Trash.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn merge_puts_both_folders_together() {
    let disk = Disk::new("merge", 64, "APFS");
    let (src, dst) = (disk.root.join("New/Site"), disk.root.join("Site"));
    write(&src.join("index.html"), "new index");
    write(&src.join("css/new.css"), "n");
    write(&dst.join("index.html"), "old index");
    write(&dst.join("about.html"), "about");
    write(&dst.join("css/old.css"), "o");
    let (done, failed, error) = put(&[src.clone()], &disk.root, How::Copy, Resolve::Merge);
    assert!(error.is_none() && failed.is_empty(), "{error:?} {failed:?}");
    assert!(done.merged);
    assert_eq!(std::fs::read_to_string(dst.join("index.html")).unwrap(), "new index");
    for kept in ["about.html", "css/old.css", "css/new.css"] {
        assert!(dst.join(kept).exists(), "{kept}");
    }
    assert!(src.join("index.html").exists(), "copied: the source stays");
    let trash = disk.trash();
    assert!(tree(&trash).values().any(|n| *n == Node::File(b"old index".to_vec())), "the old index in the Trash");
}

/// A disk that can't take some names (FAT32, as many USB sticks are): each
/// file is either copied whole or left out and said, the rest copied.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_disk_that_cant_take_a_name_leaves_only_that_out() {
    let (dir, disk) = (Scratch::new("fat"), Disk::new("fat", 64, "MS-DOS FAT32"));
    let src = dir.join("Stick");
    let names = ["ok.txt", "what?.txt", "a\"quote\".txt", "pipe|bar.txt", "star*.txt", "less<more>.txt", "back\\slash.txt", "ends in dot.", "Þórður.txt"];
    for name in names {
        write(&src.join(name), name.as_bytes());
    }
    let (_, failed, error) = put(&[src.clone()], &disk.root, How::Copy, Resolve::KeepBoth);
    assert!(error.is_none(), "{error:?}");
    let copied = tree(&disk.root.join("Stick"));
    let copied_bodies: Vec<&Node> = copied.values().collect();
    for name in names {
        let body = Node::File(name.as_bytes().to_vec());
        let there = copied_bodies.contains(&&body);
        let left_out = failed.iter().any(|(p, why)| p.ends_with(name) && !why.is_empty());
        assert!(there || left_out, "{name}: neither copied nor said ({failed:?}, {:?})", copied.keys().map(|k| String::from_utf8_lossy(k).into_owned()).collect::<Vec<_>>());
    }
    assert!(copied.contains_key(&b"ok.txt"[..].to_vec()), "the rest copied");
}

/// From a disk that tells "README" from "readme" to one that doesn't (the
/// Mac's own): one mustn't silently overwrite the other.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn names_that_differ_only_in_case_dont_overwrite_each_other() {
    let (dir, disk) = (Scratch::new("cases"), Disk::new("cases", 64, "Case-sensitive APFS"));
    let src = disk.root.join("Repo");
    write(&src.join("README"), "upper");
    write(&src.join("readme"), "lower");
    assert_eq!(tree(&src).len(), 2, "two files there");
    let (_, failed, error) = put(&[src.clone()], &dir, How::Copy, Resolve::KeepBoth);
    assert!(error.is_none(), "{error:?}");
    let copied: Vec<Node> = tree(&dir.join("Repo")).into_values().collect();
    // Each is copied, or left out and said; and what was copied stays (it
    // was removed as the other's "half-written" copy, the two names one).
    for (name, body) in [("README", "upper"), ("readme", "lower")] {
        let there = copied.contains(&Node::File(body.as_bytes().to_vec()));
        let said = failed.iter().find(|(p, _)| p.ends_with(name));
        assert!(there || said.is_some(), "{name} lost without a word: {copied:?} {failed:?}");
        if let Some((_, why)) = said {
            assert!(why.contains("capitals"), "says why: {why}");
        }
    }
    assert!(!copied.is_empty(), "the one copied kept");
    assert_eq!(tree(&src).len(), 2, "the original as it was");
}

/// The other disk is gone (ejected, unplugged) mid-way: it stops, and the
/// original's whole.
#[test]
#[ignore = "mounts disk images: cargo test safety -- --ignored"]
fn a_disk_gone_while_moving_keeps_the_original() {
    let dir = Scratch::new("gone");
    let src = dir.join("Album");
    for i in 0..4 {
        write(&src.join(format!("track {i}.flac")), noise(2_000_000));
    }
    let before = tree(&src);
    let target = {
        let disk = Disk::new("gone", 64, "APFS");
        disk.root.clone()
        // (Detached here.)
    };
    let items = plan(&[src.clone()], &dir, How::Copy).unwrap();
    let mut item = items[0].clone();
    item.dst = target.join("Album");
    item.copy = false;
    let progress = Progress::default();
    let (_, error) = execute(&[(item, Resolve::KeepBoth)], &progress);
    assert!(error.is_some() || !progress.failed.lock().unwrap().is_empty(), "said");
    assert_eq!(tree(&src), before, "the original whole");
}

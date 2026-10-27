//! What takes up a disk, folder by folder: the data behind the Disk Usage
//! window. From fsearch's index where it holds the folder (the whole
//! startup disk in a fraction of a second: `Search::usage`), else by
//! walking it (other disks), as DaisyDisk scans.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// How many rings below the folder shown are worked out.
pub const DEPTH: usize = 6;
/// Its own items listed apart (the rest are "smaller items").
pub const LISTED: usize = 200;
/// Deeper, what's at least this share of the folder shown is kept (less
/// would be a sliver of the chart).
pub const SLIVER: u64 = 1000;

/// Files this big are measured by what they take on the disk (what's
/// allocated), not their length: a sparse disk image (Docker's Docker.raw,
/// a VM's) says 60 GB and holds a few. The index only has lengths, so it's
/// asked for big files alone (a few thousand on a disk): they're the ones
/// that can be off by much.
pub const MEASURED: u64 = 64 << 20;

/// What a file of `len` bytes takes on the disk: allocated, if that's less
/// (sparse, compressed); `len` for small files, unasked.
pub fn on_disk(path: &Path, len: u64) -> u64 {
    if len < MEASURED {
        return len;
    }
    path.symlink_metadata().map_or(len, |m| on_disk_of(&m))
}

/// The same, from what's been read of it already. A file with more than
/// one name (hard links) is one file on the disk: each name has its share,
/// as in the index (fsearch's walk), so a folder holding them all adds up
/// to it once.
pub fn on_disk_of(meta: &std::fs::Metadata) -> u64 {
    let bytes = if meta.len() < MEASURED { meta.len() } else { (meta.blocks() * 512).min(meta.len()) };
    if meta.is_file() && meta.nlink() > 1 { bytes / meta.nlink() } else { bytes }
}

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    /// Items inside, all the way down (0 for a file).
    pub items: u64,
    pub dir: bool,
    /// The bigger things inside, biggest first, to `DEPTH`.
    pub children: Vec<Node>,
    /// The rest inside, too small to keep apart: bytes and how many.
    pub rest: (u64, u64),
}

impl Node {
    pub fn file(name: String, path: PathBuf, bytes: u64) -> Node {
        Node { name, path, bytes, items: 0, dir: false, children: Vec::new(), rest: (0, 0) }
    }

    /// Keep the children worth showing, `level` rings below a folder of
    /// `total` bytes: at the first, the biggest `LISTED`; deeper, those not
    /// slivers. The rest go into `rest`.
    pub fn keep(&mut self, level: usize, total: u64) {
        self.children.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes));
        let cut = kept(self.children.iter().map(|c| c.bytes), level, total);
        for gone in self.children.drain(cut..) {
            self.rest.0 += gone.bytes;
            self.rest.1 += 1 + gone.items;
        }
    }

    /// Prune to what's shown from here on, this being the folder shown.
    pub fn prune(&mut self, level: usize, total: u64) {
        self.keep(level, total);
        if level >= DEPTH {
            for c in &mut self.children {
                c.rest.0 += c.children.iter().map(|g| g.bytes).sum::<u64>();
                c.rest.1 += c.children.iter().map(|g| 1 + g.items).sum::<u64>();
                c.children.clear();
            }
            return;
        }
        for c in &mut self.children {
            c.prune(level + 1, total);
        }
    }
}

/// Of children sorted biggest first (their sizes), how many are kept
/// `level` rings below a folder of `total` bytes (see `Node::keep`).
pub fn kept(sizes: impl Iterator<Item = u64>, level: usize, total: u64) -> usize {
    let min = if level <= 1 { 1 } else { (total / SLIVER).max(1) };
    sizes.take(LISTED).take_while(|&b| b >= min).count()
}

/// A folder's name as shown ("Macintosh HD" for /).
pub fn display_name(path: &Path) -> String {
    path.file_name().map_or("Macintosh HD".into(), |n| n.to_string_lossy().into_owned())
}

/// What's in `dir`, by walking it (not into other disks). `seen` counts
/// items as they're found; `stop` stops it (None then).
pub fn walk(dir: &Path, seen: &AtomicU64, stop: &AtomicBool) -> Option<Node> {
    let device = dir.symlink_metadata().ok()?.dev();
    let mut root = walk_dir(dir, display_name(dir), device, 0, seen, stop)?;
    let total = root.bytes;
    root.prune(1, total);
    Some(root)
}

fn walk_dir(dir: &Path, name: String, device: u64, level: usize, seen: &AtomicU64, stop: &AtomicBool) -> Option<Node> {
    if stop.load(Ordering::Relaxed) {
        return None;
    }
    let mut node = Node { name, path: dir.to_path_buf(), bytes: 0, items: 0, dir: true, children: Vec::new(), rest: (0, 0) };
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        seen.fetch_add(1, Ordering::Relaxed);
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if meta.is_dir() && meta.dev() == device {
            walk_dir(&entry.path(), name, device, level + 1, seen, stop)?
        } else if meta.is_dir() {
            // Another disk mounted here: not this one's.
            continue;
        } else {
            Node::file(name, entry.path(), on_disk_of(&meta))
        };
        node.bytes += child.bytes;
        node.items += 1 + child.items;
        node.children.push(child);
    }
    // (Kept generously while walking: what's a sliver is known at the end.)
    node.keep(1, 0);
    if level >= DEPTH {
        node.rest.0 += node.children.iter().map(|c| c.bytes).sum::<u64>();
        node.rest.1 += node.children.iter().map(|c| 1 + c.items).sum::<u64>();
        node.children.clear();
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::walk;
    use std::sync::atomic::{AtomicBool, AtomicU64};

    /// A sparse file (a disk image's: Docker's, a VM's) counts what's on
    /// the disk, not its length.
    #[test]
    fn sparse_files_count_what_they_hold() {
        let dir = std::env::temp_dir().join(format!("usage-sparse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = std::fs::File::create(dir.join("disk.raw")).unwrap();
        file.set_len(200 << 20).unwrap();
        drop(file);
        assert_eq!(std::fs::metadata(dir.join("disk.raw")).unwrap().len(), 200 << 20);
        let root = walk(&dir, &AtomicU64::new(0), &AtomicBool::new(false)).unwrap();
        assert!(root.bytes < 1 << 20, "{} bytes", root.bytes);
        assert_eq!(super::on_disk(&dir.join("disk.raw"), 200 << 20), root.bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file with two names (a hard link, as Rust's builds are full of)
    /// counts once.
    #[test]
    fn hard_links_count_once() {
        let dir = std::env::temp_dir().join(format!("usage-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("deps")).unwrap();
        std::fs::write(dir.join("deps/lib.rlib"), vec![0u8; 500_000]).unwrap();
        std::fs::hard_link(dir.join("deps/lib.rlib"), dir.join("lib.rlib")).unwrap();
        let root = walk(&dir, &AtomicU64::new(0), &AtomicBool::new(false)).unwrap();
        assert_eq!(root.bytes, 500_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn walks_and_prunes() {
        let dir = std::env::temp_dir().join(format!("usage-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("big/inner")).unwrap();
        std::fs::create_dir_all(dir.join("small")).unwrap();
        std::fs::write(dir.join("big/inner/a"), vec![0u8; 900_000]).unwrap();
        std::fs::write(dir.join("big/b"), vec![0u8; 100_000]).unwrap();
        std::fs::write(dir.join("small/c"), vec![0u8; 10]).unwrap();
        let root = walk(&dir, &AtomicU64::new(0), &AtomicBool::new(false)).unwrap();
        assert_eq!(root.bytes, 1_000_010);
        assert_eq!(root.items, 6);
        assert_eq!(root.children[0].name, "big");
        assert_eq!(root.children[0].children[0].name, "inner");
        // At the first ring everything's listed; deeper, slivers go to rest.
        assert_eq!(root.children.len(), 2);
        let small = &root.children[1];
        assert!(small.children.is_empty() && small.rest == (10, 1));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

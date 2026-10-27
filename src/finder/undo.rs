//! Undo and redo for file operations, as Finder has them. Each operation is
//! kept as what it did on disk (items moved, the Trash included, and items
//! made); undoing it does the opposite, and what that did is in turn what
//! redo undoes.

use std::io;
use std::path::{Path, PathBuf};

use gpui::*;

use super::Finder;
use crate::{fileops, macos};

/// How many operations back undo reaches.
const DEPTH: usize = 100;

#[derive(Default)]
pub(super) struct Op {
    /// Moved or renamed (into the Trash included): (from, to).
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// Made: new folders, copies, archives, aliases.
    pub created: Vec<PathBuf>,
}

impl Op {
    pub(super) fn moved(moved: Vec<(PathBuf, PathBuf)>) -> Op {
        Op { moved, created: Vec::new() }
    }

    pub(super) fn created(created: Vec<PathBuf>) -> Op {
        Op { moved: Vec::new(), created }
    }

    /// Where it left things, to show and select them.
    pub(super) fn results(&self) -> Vec<PathBuf> {
        self.moved.iter().map(|(_, to)| to.clone()).chain(self.created.iter().cloned()).collect()
    }

    fn is_empty(&self) -> bool {
        self.moved.is_empty() && self.created.is_empty()
    }

    /// Do the opposite: put what was made in the Trash, then move things
    /// back, last first (so what a copy replaced comes back once the copy
    /// is out of its way). Returns what that did.
    pub(super) fn revert(self) -> io::Result<Op> {
        let mut done = Op::default();
        // (A folder made to hold what was moved, New Folder with Selection's
        // or Tidy Up's, goes once they're out of it, and only if it's empty
        // then: not before, with them inside.)
        let (holders, made): (Vec<PathBuf>, Vec<PathBuf>) =
            self.created.into_iter().partition(|c| self.moved.iter().any(|(_, to)| to.starts_with(c)));
        for path in made {
            let trashed = macos::trash(&path).map_err(io::Error::other)?;
            done.moved.push((path, trashed));
        }
        for (from, to) in self.moved.into_iter().rev() {
            if from.symlink_metadata().is_ok() {
                let name = from.file_name().unwrap_or_default().to_string_lossy();
                return Err(io::Error::other(format!("Can't undo: “{name}” is back in its place already.")));
            }
            fileops::move_item(&to, &from)?;
            done.moved.push((to, from));
        }
        for path in holders {
            if std::fs::read_dir(&path).is_ok_and(|mut d| d.all(|e| e.is_ok_and(|e| e.file_name() == ".DS_Store"))) {
                let trashed = macos::trash(&path).map_err(io::Error::other)?;
                done.moved.push((path, trashed));
            }
        }
        Ok(done)
    }
}

/// Undo what was done outside a window (a Folder Rule's, src/rules.rs) as
/// ⌘Z does: what was made put in the Trash, what was moved put back (from
/// the Trash too).
pub(crate) fn revert_outside(moved: Vec<(PathBuf, PathBuf)>, created: Vec<PathBuf>) -> io::Result<()> {
    let done = Op { moved, created }.revert()?;
    let trashed: Vec<_> = done.moved.iter().filter(|(_, to)| super::places::is_trashed(to)).cloned().collect();
    if !trashed.is_empty() {
        super::places::remember_trashed(&trashed);
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Op>,
    redo: Vec<Op>,
}

impl Finder {
    /// Remember an operation for undo (a new one forgets what was undone).
    pub(super) fn record(&mut self, op: Op) {
        if op.is_empty() {
            return;
        }
        self.history.undo.push(op);
        self.history.redo.clear();
        if self.history.undo.len() > DEPTH {
            self.history.undo.remove(0);
        }
    }

    /// ⌘Z, or with `redo` ⇧⌘Z: then show where things are now.
    pub(super) fn undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        let op = if redo { self.history.redo.pop() } else { self.history.undo.pop() };
        let Some(op) = op else { return };
        // Folders whose listing changes: where things were and are.
        let touched: Vec<PathBuf> = op
            .moved
            .iter()
            .flat_map(|(a, b)| [a, b])
            .chain(&op.created)
            .filter_map(|p| p.parent().map(Path::to_path_buf))
            .collect();
        match op.revert() {
            Ok(done) => {
                // What went (back) to the Trash can be put back again.
                let trashed: Vec<_> = done.moved.iter().filter(|(_, to)| super::places::is_trashed(to)).cloned().collect();
                super::places::remember_trashed(&trashed);
                // (What's back fades in.)
                let back = done.results();
                self.arrive(&back, cx);
                self.select_when_listed(back);
                if redo { self.history.undo.push(done) } else { self.history.redo.push(done) }
            }
            Err(e) => self.show_notice(e.to_string(), cx),
        }
        self.reload_folders(&touched, cx);
    }
}

/// Undo and redo on a disk image, whose Trash is its own: what a copy made
/// goes to the Trash and comes back, what was replaced comes back, a move
/// goes back; and nothing's put back over something new in its place.
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::Op;
    use crate::fileops::safety::Disk;
    use crate::fileops::{self, How, Progress, Resolve};

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    /// What putting `src` into `dir` did, as Undo keeps it.
    fn put(src: &Path, dir: &Path, how: How, resolve: Resolve) -> Op {
        let plan: Vec<_> = fileops::plan(&[src.to_path_buf()], dir, how).unwrap().into_iter().map(|i| (i, resolve)).collect();
        let (done, error) = fileops::execute(&plan, &Progress::default());
        assert!(error.is_none(), "{error:?}");
        Op { moved: done.moved, created: done.created }
    }

    #[test]
    #[ignore = "mounts disk images: cargo test safety -- --ignored"]
    fn safety_undo_and_redo_a_copy() {
        let disk = Disk::new("undo-copy", 64, "APFS");
        let src = disk.root.join("In/letter.txt");
        write(&src, "dear");
        let copy = disk.root.join("Out/letter.txt");
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        let undone = put(&src, &disk.root.join("Out"), How::Copy, Resolve::KeepBoth).revert().unwrap();
        assert!(!copy.exists(), "the copy gone");
        let (_, trashed) = &undone.moved[0];
        assert!(trashed.starts_with(disk.trash()) && read(trashed).as_deref() == Some("dear"), "to this disk's Trash: {trashed:?}");
        assert_eq!(read(&src).as_deref(), Some("dear"), "the original untouched");
        undone.revert().unwrap();
        assert_eq!(read(&copy).as_deref(), Some("dear"), "redone: back from the Trash");
    }

    #[test]
    #[ignore = "mounts disk images: cargo test safety -- --ignored"]
    fn safety_undo_a_replace_brings_back_what_was_replaced() {
        let disk = Disk::new("undo-replace", 64, "APFS");
        let (src, there) = (disk.root.join("In/plan.txt"), disk.root.join("Out/plan.txt"));
        write(&src, "new");
        write(&there, "old");
        let op = put(&src, &disk.root.join("Out"), How::Copy, Resolve::Replace);
        assert_eq!(read(&there).as_deref(), Some("new"));
        let undone = op.revert().unwrap();
        assert_eq!(read(&there).as_deref(), Some("old"), "the replaced one back");
        assert!(undone.moved.iter().any(|(_, to)| to.starts_with(disk.trash()) && read(to).as_deref() == Some("new")), "the copy in the Trash, not lost");
        assert_eq!(read(&src).as_deref(), Some("new"));
    }

    #[test]
    #[ignore = "mounts disk images: cargo test safety -- --ignored"]
    fn safety_undo_a_move_and_not_over_something_new() {
        let disk = Disk::new("undo-move", 64, "APFS");
        let src = disk.root.join("In/photo.jpg");
        write(&src, "pixels");
        std::fs::create_dir_all(disk.root.join("Out")).unwrap();
        let op = put(&src, &disk.root.join("Out"), How::Move, Resolve::KeepBoth);
        let moved_to = disk.root.join("Out/photo.jpg");
        assert!(!src.exists() && moved_to.exists());
        op.revert().unwrap();
        assert_eq!(read(&src).as_deref(), Some("pixels"), "moved back");
        // Moved again, and something new put where it was: undo stops
        // rather than put it back over that.
        let op = put(&src, &disk.root.join("Out"), How::Move, Resolve::KeepBoth);
        write(&src, "something new");
        let refused = op.revert();
        assert!(refused.is_err(), "refused");
        assert_eq!(read(&src).as_deref(), Some("something new"), "not overwritten");
        assert_eq!(read(&moved_to).as_deref(), Some("pixels"), "and the moved one where it is");
    }
}

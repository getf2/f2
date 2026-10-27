//! Archives: .zip, .tar (.gz, .bz2, .xz), .7z, .rar. Opened, one is
//! unpacked into a cache folder and shown like a folder (its name in the
//! title and path bar, a bar saying it's a copy); Extract unpacks it
//! beside it, as Archive Utility does: what's inside, if that's one item,
//! or a folder named after the archive. Zips go through `ditto` (as
//! Archive Utility's do, keeping what Finder keeps in them), the rest
//! through `bsdtar` (libarchive), which leaves out absolute paths and "..".

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Endings, longest first ("x.tar.gz": not ".gz").
const KINDS: &[&str] = &[".tar.gz", ".tar.bz2", ".tar.xz", ".zip", ".tar", ".tgz", ".tbz", ".tbz2", ".txz", ".7z", ".rar", ".cpio", ".xar"];

/// Bigger archives aren't unpacked just to look inside.
const BROWSE_LIMIT: u64 = 2 << 30;

/// Unpacked archives not opened for this long are cleared away.
const KEEP: Duration = Duration::from_secs(24 * 3600);

/// The archive's name without its ending ("photos" for photos.tar.gz), if
/// it's an archive.
pub fn stem(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let lower = name.to_lowercase();
    let kind = KINDS.iter().find(|k| lower.ends_with(*k) && lower.len() > k.len())?;
    Some(name[..name.len() - kind.len()].to_string())
}

pub fn is_archive(path: &Path) -> bool {
    stem(path).is_some() && path.is_file()
}

/// Unpack `archive` into `dir` (made if needed).
fn unpack(archive: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let zip = archive.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().ends_with(".zip"));
    let out = if zip {
        Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(archive).arg(dir).output()?
    } else {
        Command::new("/usr/bin/bsdtar").arg("-xf").arg(archive).arg("-C").arg(dir).output()?
    };
    if out.status.success() {
        Ok(())
    } else {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let name = archive.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        Err(io::Error::other(if why.is_empty() { format!("“{name}” couldn't be unpacked.") } else { format!("“{name}” couldn't be unpacked: {why}") }))
    }
}

/// What's at the top of an unpacked archive (not the resource forks' and
/// Finder's leftovers).
fn top(dir: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n != "__MACOSX" && n != ".DS_Store"))
        .collect())
}

/// Extract `archive` into `into`, as Archive Utility does: its one item, or
/// a folder of its name holding them all; named afresh ("name 2") if taken.
/// What was made.
pub fn extract(archive: &Path, into: &Path) -> io::Result<PathBuf> {
    let stem = stem(archive).ok_or_else(|| io::Error::other("That isn't an archive F2 can open."))?;
    // (Unpacked beside where it ends up, so it's moved there, not copied.)
    let work = into.join(format!(".{stem}.f2-unpacking-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let done = unpack(archive, &work).and_then(|()| {
        let items = top(&work)?;
        if let [one] = items.as_slice() {
            let name = one.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let (base, ext) = match name.rfind('.') {
                Some(dot) if dot > 0 && !one.is_dir() => (name[..dot].to_string(), name[dot + 1..].to_string()),
                _ => (name.clone(), String::new()),
            };
            let dst = crate::fileops::free_name(into, &base, &ext);
            std::fs::rename(one, &dst)?;
            Ok(dst)
        } else {
            let dst = crate::fileops::free_name(into, &stem, "");
            for gone in ["__MACOSX", ".DS_Store"] {
                let _ = std::fs::remove_dir_all(work.join(gone));
                let _ = std::fs::remove_file(work.join(gone));
            }
            std::fs::rename(&work, &dst)?;
            Ok(dst)
        }
    });
    let _ = std::fs::remove_dir_all(&work);
    done
}

/// Where archives are unpacked to look inside, as the folders shown name
/// it (links resolved: /tmp is /private/tmp).
fn cache() -> PathBuf {
    static CACHE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            // (NEW_FINDER_SETTINGS: test copies keep theirs beside their settings.)
            let dir = if std::env::var_os("NEW_FINDER_SETTINGS").is_some() {
                crate::settings::data_dir().join("Archives")
            } else {
                let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
                home.join("Library/Caches").join(crate::APP_NAME).join("Archives")
            };
            let _ = std::fs::create_dir_all(&dir);
            std::fs::canonicalize(&dir).unwrap_or(dir)
        })
        .clone()
}

/// Unpacked folders, and the archive each is of.
static SOURCES: Mutex<Option<HashMap<PathBuf, PathBuf>>> = Mutex::new(None);

/// `archive` unpacked, to look inside: the folder (in the cache; again the
/// same one while the archive's unchanged).
pub fn unpacked(archive: &Path) -> io::Result<PathBuf> {
    let meta = std::fs::metadata(archive)?;
    if meta.len() > BROWSE_LIMIT {
        return Err(io::Error::other("That archive is too big to look inside: Extract it instead."));
    }
    let modified = meta.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    let key = format!("{}|{}|{}", archive.display(), meta.len(), modified);
    let id = format!("{:016x}", key.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)));
    let root = cache().join(&id);
    let source = cache().join(format!("{id}.source"));
    if !(root.is_dir() && source.is_file()) {
        let work = cache().join(format!("{id}.partial"));
        let _ = std::fs::remove_dir_all(&work);
        unpack(archive, &work)?;
        let _ = std::fs::remove_dir_all(&root);
        std::fs::rename(&work, &root)?;
    }
    // (Written each time: its date is when the archive was last opened.)
    std::fs::write(&source, archive.as_os_str().as_encoded_bytes())?;
    SOURCES.lock().unwrap().get_or_insert_with(HashMap::new).insert(root.clone(), archive.to_path_buf());
    Ok(root)
}

/// If `path` is inside an unpacked archive: the archive, and the folder it
/// was unpacked into.
pub fn of(path: &Path) -> Option<(PathBuf, PathBuf)> {
    let cache = cache();
    let first = path.strip_prefix(&cache).ok()?.components().next()?;
    let root = cache.join(first);
    let mut sources = SOURCES.lock().unwrap();
    let sources = sources.get_or_insert_with(HashMap::new);
    if let Some(archive) = sources.get(&root) {
        return Some((archive.clone(), root));
    }
    let bytes = std::fs::read(cache.join(format!("{}.source", first.as_os_str().to_string_lossy()))).ok()?;
    // (Safety: written from a path's own bytes, in `unpacked`.)
    let archive = PathBuf::from(unsafe { std::ffi::OsString::from_encoded_bytes_unchecked(bytes) });
    sources.insert(root.clone(), archive.clone());
    Some((archive, root))
}

/// Clear away what was unpacked to look inside and not opened for a day.
pub fn tidy() {
    let cache = cache();
    for entry in std::fs::read_dir(&cache).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.ends_with(".partial") {
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }
        if !name.ends_with(".source") {
            continue;
        }
        let old = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age > KEEP);
        if old {
            let _ = std::fs::remove_dir_all(cache.join(name.trim_end_matches(".source")));
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{extract, stem};

    #[test]
    fn stems() {
        assert_eq!(stem(Path::new("/a/photos.tar.gz")).as_deref(), Some("photos"));
        assert_eq!(stem(Path::new("/a/Report.ZIP")).as_deref(), Some("Report"));
        assert_eq!(stem(Path::new("/a/v1.2.7z")).as_deref(), Some("v1.2"));
        assert_eq!(stem(Path::new("/a/notes.txt")), None);
        assert_eq!(stem(Path::new("/a/.zip")), None);
    }

    /// Extract as Archive Utility does: one item as itself, several in a
    /// folder named after the archive, names taken numbered.
    #[test]
    fn extracts_like_archive_utility() {
        let dir = std::env::temp_dir().join(format!("f2-archives-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("folder")).unwrap();
        std::fs::write(src.join("folder/a.txt"), "a").unwrap();
        std::fs::write(src.join("b.txt"), "b").unwrap();
        let zip = |name: &str, items: &[&str]| {
            let out = std::process::Command::new("/usr/bin/zip").current_dir(&src).args(["-r", "-q"]).arg(dir.join(name)).args(items).status().unwrap();
            assert!(out.success());
            dir.join(name)
        };
        let one = zip("one.zip", &["folder"]);
        let two = zip("two.zip", &["folder", "b.txt"]);
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        assert_eq!(extract(&one, &out).unwrap(), out.join("folder"));
        assert_eq!(std::fs::read_to_string(out.join("folder/a.txt")).unwrap(), "a");
        assert_eq!(extract(&one, &out).unwrap(), out.join("folder 2"), "taken: numbered");
        assert_eq!(extract(&two, &out).unwrap(), out.join("two"));
        assert!(out.join("two/b.txt").is_file() && out.join("two/folder/a.txt").is_file());
        let tgz = dir.join("three.tar.gz");
        assert!(std::process::Command::new("/usr/bin/bsdtar").current_dir(&src).arg("-czf").arg(&tgz).arg("b.txt").status().unwrap().success());
        assert_eq!(extract(&tgz, &out).unwrap(), out.join("b.txt"));
        // Nothing left behind while unpacking.
        assert!(std::fs::read_dir(&out).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("f2-unpacking")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

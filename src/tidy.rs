//! Tidy Up: a folder's loose files (the Desktop's, Downloads') sorted into
//! folders of what they are: Screenshots, Images, Documents, Videos, Audio,
//! Installers, Archives. Folders stay where they are, and so do files of
//! kinds not known and downloads still coming in. Undone with ⌘Z (see
//! `finder::undo`). And, as Settings say, the Desktop's screenshots moved
//! into its Screenshots folder as they're taken.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::App;

use crate::settings::Settings;

/// The folders, and what goes in each (by ending; screenshots by name).
const KINDS: &[(&str, &[&str])] = &[
    ("Images", &["png", "jpg", "jpeg", "heic", "heif", "gif", "tif", "tiff", "webp", "bmp", "svg", "raw", "cr2", "cr3", "nef", "arw", "dng"]),
    ("Videos", &["mov", "mp4", "m4v", "avi", "mkv", "webm", "3gp"]),
    ("Audio", &["mp3", "m4a", "wav", "aiff", "aif", "flac", "aac", "ogg", "opus"]),
    (
        "Documents",
        &["pdf", "doc", "docx", "pages", "txt", "rtf", "md", "key", "ppt", "pptx", "numbers", "xls", "xlsx", "csv", "odt", "ods", "odp", "epub", "html", "htm"],
    ),
    ("Installers", &["dmg", "pkg", "mpkg", "iso", "xip"]),
    ("Archives", &["zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar"]),
];
pub const SCREENSHOTS: &str = "Screenshots";

/// Every kind, in the order they're offered (Folder Rules picks from them).
pub fn kinds() -> impl Iterator<Item = &'static str> {
    std::iter::once(SCREENSHOTS).chain(KINDS.iter().map(|(kind, _)| *kind))
}

/// What a downloading file is called until it's done (not to be moved).
pub const PARTIAL: &[&str] = &["download", "crdownload", "part", "partial"];

/// The folder a file goes in, if it's of a kind Tidy Up knows.
pub fn kind_of(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy();
    if ["Screenshot", "Screen Shot", "Screen Recording", "CleanShot"].iter().any(|s| name.starts_with(s)) {
        return Some(SCREENSHOTS);
    }
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    KINDS.iter().find(|(_, exts)| exts.contains(&ext.as_str())).map(|(kind, _)| *kind)
}

/// What Tidy Up would do in `dir`: each loose file, and where it'd go.
pub fn plan(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut moves = Vec::new();
    let mut taken = std::collections::HashSet::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    files.sort();
    for path in files {
        let Ok(meta) = path.symlink_metadata() else { continue };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let partial = path.extension().is_some_and(|e| PARTIAL.contains(&e.to_string_lossy().to_lowercase().as_str()));
        if !meta.is_file() || name.starts_with('.') || partial {
            continue;
        }
        let Some(kind) = kind_of(&path) else { continue };
        let to = free_name(&dir.join(kind), &name, &taken);
        taken.insert(to.clone());
        moves.push((path, to));
    }
    moves
}

/// Where `name` can go in `folder` without replacing anything: as it is,
/// or numbered ("cat 2.jpg") when that's there already, or `taken` by
/// another on its way.
pub fn free_name(folder: &Path, name: &str, taken: &std::collections::HashSet<PathBuf>) -> PathBuf {
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot + 1..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| {
            let base = if n == 1 { stem.to_string() } else { format!("{stem} {n}") };
            folder.join(if ext.is_empty() { base } else { format!("{base}.{ext}") })
        })
        .find(|p| p.symlink_metadata().is_err() && !taken.contains(p))
        .unwrap()
}

/// Tidy `dir` up: what moved where, and the folders made for it.
pub fn run(dir: &Path) -> io::Result<(Vec<(PathBuf, PathBuf)>, Vec<PathBuf>)> {
    let mut moved = Vec::new();
    let mut made = Vec::new();
    for (from, to) in plan(dir) {
        let folder = to.parent().unwrap_or(dir).to_path_buf();
        if !folder.exists() {
            std::fs::create_dir(&folder)?;
            made.push(folder);
        }
        crate::fileops::move_item(&from, &to)?;
        moved.push((from, to));
    }
    Ok((moved, made))
}

/// The Desktop's screenshots, into its Screenshots folder, a little after
/// they're taken (while Settings say so).
pub fn watch_desktop(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            if !cx.update(|cx| Settings::get(cx).shots.tidy_desktop) {
                continue;
            }
            cx.background_executor()
                .spawn(async {
                    let desktop = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Desktop");
                    for (from, to) in plan(&desktop) {
                        // (Done being written: a few seconds old.)
                        let settled = from.symlink_metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() >= 3);
                        if to.parent().is_some_and(|f| f.ends_with(SCREENSHOTS)) && settled {
                            let _ = std::fs::create_dir_all(desktop.join(SCREENSHOTS));
                            let _ = crate::fileops::move_item(&from, &to);
                        }
                    }
                })
                .await;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::{kind_of, plan, run};

    #[test]
    fn kinds() {
        let k = |name: &str| kind_of(std::path::Path::new(name));
        assert_eq!(k("Screenshot 2026-10-20 at 17.02.45.png"), Some("Screenshots"));
        assert_eq!(k("IMG_0001.HEIC"), Some("Images"));
        assert_eq!(k("Report.pdf"), Some("Documents"));
        assert_eq!(k("Zoom.pkg"), Some("Installers"));
        assert_eq!(k("notes"), None);
        assert_eq!(k("main.rs"), None, "kinds not known stay");
    }

    #[test]
    fn tidies_loose_files_only() {
        let dir = std::env::temp_dir().join(format!("f2-tidy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Project")).unwrap();
        std::fs::create_dir_all(dir.join("Images")).unwrap();
        for name in ["Screenshot 2026-10-20 at 10.00.00.png", "cat.jpg", "Report.pdf", "song.mp3", "Zoom.pkg", "notes", "big.zip.download", ".hidden.png"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        // (An Images folder with a cat.jpg already: the new one's numbered.)
        std::fs::write(dir.join("Images/cat.jpg"), "old").unwrap();
        let planned = plan(&dir);
        assert_eq!(planned.len(), 5, "{planned:?}");
        let (moved, made) = run(&dir).unwrap();
        assert_eq!(moved.len(), 5);
        let mut made: Vec<_> = made.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        made.sort();
        assert_eq!(made, ["Audio", "Documents", "Installers", "Screenshots"], "Images was there already");
        assert!(dir.join("Images/cat 2.jpg").is_file() && dir.join("Images/cat.jpg").is_file());
        assert!(dir.join("Screenshots/Screenshot 2026-10-20 at 10.00.00.png").is_file());
        for stays in ["notes", "big.zip.download", ".hidden.png", "Project"] {
            assert!(dir.join(stays).exists(), "{stays} stays");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

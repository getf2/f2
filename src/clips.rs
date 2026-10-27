//! Clipboard history, as Raycast's and Maccy's: what's copied (text,
//! links, images, files), newest first, kept in the data folder. Not
//! what apps mark as secret or passing through (password managers'
//! copies). The clipboard's change count is checked twice a second; what's
//! copied again moves to the top. Quick Open lists them (see
//! `quick_panel`), and puts one back on the clipboard, or pastes it.
//!
//! macOS (15.4 on) asks once before an app reads the clipboard by itself:
//! the first read is at start-up, so it asks then, not at some later copy.
//! Set to ask every time, or not to allow it, it isn't read at all.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::time::Duration;

use gpui::App;
use serde::{Deserialize, Serialize};

use crate::macos::{self, PasteAccess};
use crate::settings::Settings;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum What {
    Text(String),
    Link(String),
    /// Its file (in the history's folder), size in pixels, and a hash of it
    /// (to know it if it's copied again).
    Image { file: String, w: u32, h: u32, hash: u64 },
    Files(Vec<PathBuf>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub what: What,
    /// The app in front when it was copied.
    pub app: Option<String>,
    pub app_path: Option<PathBuf>,
    /// When, in seconds since 1970.
    pub at: u64,
    /// Kept for good: listed first, never let go of for room, nor by
    /// Clear Clipboard History.
    #[serde(default)]
    pub pinned: bool,
    /// An image's words, as Vision reads them, on this Mac (Some(""):
    /// read, none in it): found by them, its first line its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl Clip {
    /// One line of it, for the list.
    pub fn title(&self) -> String {
        match &self.what {
            What::Text(text) => text.trim().lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().chars().take(200).collect(),
            What::Link(url) => url.clone(),
            // (A screenshot's, kept elsewhere: when it was taken.)
            What::Image { file, .. } if file.starts_with('/') => shot_title(Path::new(file)),
            What::Image { w, h, .. } => match self.text.as_deref().and_then(|t| t.lines().map(str::trim).find(|l| !l.is_empty())) {
                Some(line) => format!("Image: {}", line.chars().take(120).collect::<String>()),
                None => format!("Image ({w}×{h})"),
            },
            What::Files(paths) => match paths.as_slice() {
                [one] => one.file_name().map_or_else(|| one.display().to_string(), |n| n.to_string_lossy().into_owned()),
                many => format!("{} Files", many.len()),
            },
        }
    }

    pub fn kind(&self) -> &'static str {
        match &self.what {
            What::Text(_) => "Text",
            What::Link(_) => "Link",
            What::Image { .. } => "Image",
            What::Files(paths) if paths.len() == 1 => "File",
            What::Files(_) => "Files",
        }
    }

    /// Where an image's file is.
    pub fn image(&self) -> Option<PathBuf> {
        match &self.what {
            What::Image { file, .. } => Some(dir().join(file)),
            _ => None,
        }
    }

    /// Whether `query` (lowercase) is in it.
    pub fn matches(&self, query: &str) -> bool {
        let has = |s: &str| s.to_lowercase().contains(query);
        query.is_empty()
            || match &self.what {
                What::Text(text) | What::Link(text) => has(text),
                What::Image { .. } => has("image") || self.text.as_deref().is_some_and(has),
                What::Files(paths) => paths.iter().any(|p| has(&p.to_string_lossy())),
            }
            || self.app.as_deref().is_some_and(has)
    }
}

/// A screenshot's name as macOS names them ("Screenshot 2026-10-20 at
/// 17.02.45 (edited).png") as its time: "20 Oct, 17:02:45 (edited)".
fn shot_title(path: &Path) -> String {
    let name = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (when, rest) = name.strip_prefix("Screenshot ").map_or((name.as_str(), ""), |n| n.split_at(n.len().min(22)));
    match chrono::NaiveDateTime::parse_from_str(when, "%Y-%m-%d at %H.%M.%S") {
        Ok(t) => format!("{}{rest}", t.format("%-d %b, %H:%M:%S")),
        Err(_) => name.clone(),
    }
}

/// How long ago `at` was: "Just now", "5 min ago", "Yesterday".
pub fn ago(at: u64) -> String {
    let now = now();
    let secs = now.saturating_sub(at);
    match secs {
        0..60 => "Just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "Yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn dir() -> PathBuf {
    crate::settings::data_dir().join("Clipboard")
}

fn index() -> PathBuf {
    dir().join("history.json")
}

static CLIPS: Mutex<Option<Vec<Clip>>> = Mutex::new(None);
/// The change count as last seen (read, or written here).
static SEEN: AtomicI64 = AtomicI64::new(-1);
static ON: AtomicBool = AtomicBool::new(false);
/// A snippet being pasted: its moment on the clipboard isn't a copy.
static PAUSED: AtomicBool = AtomicBool::new(false);
static KEEP: AtomicUsize = AtomicUsize::new(200);

fn load() -> Vec<Clip> {
    std::fs::read(index()).ok().and_then(|t| serde_json::from_slice(&t).ok()).unwrap_or_default()
}

fn with<R>(f: impl FnOnce(&mut Vec<Clip>) -> R) -> R {
    let mut clips = CLIPS.lock().unwrap();
    f(clips.get_or_insert_with(load))
}

fn save(clips: &[Clip]) {
    let _ = std::fs::create_dir_all(dir());
    let tmp = index().with_extension("new");
    if let Ok(json) = serde_json::to_vec(clips)
        && std::fs::write(&tmp, json).is_ok()
    {
        let _ = std::fs::rename(&tmp, index());
    }
}

/// The history, newest first.
pub fn list() -> Vec<Clip> {
    with(|clips| clips.clone())
}

pub fn delete(id: u64) {
    with(|clips| {
        if let Some(i) = clips.iter().position(|c| c.id == id) {
            let gone = clips.remove(i);
            if let Some(image) = gone.image() {
                let _ = std::fs::remove_file(image);
            }
            save(clips);
        }
    });
}

/// Everything but what's pinned.
pub fn clear() {
    let kept: Vec<PathBuf> = with(|clips| {
        clips.retain(|c| c.pinned);
        save(clips);
        clips.iter().filter_map(Clip::image).collect()
    });
    for entry in std::fs::read_dir(dir()).into_iter().flatten().flatten() {
        if entry.path().extension().is_some_and(|e| e == "png") && !kept.contains(&entry.path()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Pin one (or unpin it).
pub fn set_pinned(id: u64, pinned: bool) {
    with(|clips| {
        if let Some(clip) = clips.iter_mut().find(|c| c.id == id) {
            clip.pinned = pinned;
            save(clips);
        }
    });
}

/// Put `clip` back on the clipboard (and at the top of the history).
pub fn copy(clip: &Clip) -> bool {
    let done = match &clip.what {
        What::Text(text) | What::Link(text) => {
            macos::copy_text(text);
            true
        }
        What::Image { .. } => clip.image().is_some_and(|p| macos::copy_image(&p)),
        What::Files(paths) => {
            let paths: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
            macos::copy_files(&paths);
            true
        }
    };
    // (Not taken as a new copy.)
    SEEN.store(macos::pasteboard_count(), Ordering::SeqCst);
    with(|clips| {
        if let Some(i) = clips.iter().position(|c| c.id == clip.id) {
            let mut moved = clips.remove(i);
            moved.at = now();
            clips.insert(0, moved);
            save(clips);
        }
    });
    done
}

/// Don't take what's copied for now (`true`), or do again.
pub fn pause(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    if !paused {
        ignore_this();
    }
}

/// What's on the clipboard now was put there here, not copied: not for the
/// history (an emoji, say).
pub fn ignore_this() {
    SEEN.store(macos::pasteboard_count(), Ordering::SeqCst);
}

/// Read the clipboard once now (off the main thread): macOS asks then,
/// if it hasn't yet, whether Finder2 may.
pub fn ask() {
    std::thread::spawn(take);
}

/// Why the history can't be kept, if it can't (macOS doesn't let it read
/// the clipboard).
pub fn blocked() -> Option<&'static str> {
    match macos::pasteboard_access() {
        PasteAccess::Denied => Some("macOS doesn't let F2 read the clipboard."),
        PasteAccess::Ask => Some("macOS asks before each time F2 reads the clipboard."),
        _ => None,
    }
}

/// Keep the history (as Settings say), from now on.
pub fn init(cx: &mut App) {
    let apply = |cx: &mut App| {
        let s = Settings::get(cx);
        ON.store(s.clipboard_history, Ordering::SeqCst);
        KEEP.store(s.clipboard_keep.max(10), Ordering::SeqCst);
    };
    apply(cx);
    cx.observe_global::<Settings>(apply).detach();
    // (Images copied before their words were read: once F2's settled in.)
    std::thread::Builder::new()
        .name("clipboard-words".into())
        .spawn(|| {
            std::thread::sleep(Duration::from_secs(20));
            if ON.load(Ordering::SeqCst) {
                read_unread();
            }
        })
        .ok();
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(|| {
            loop {
                if ON.load(Ordering::SeqCst) && !PAUSED.load(Ordering::SeqCst) && blocked().is_none() {
                    let count = macos::pasteboard_count();
                    if SEEN.swap(count, Ordering::SeqCst) != count {
                        take();
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })
        .ok();
}

/// What was just copied, into the history.
fn take() {
    let _ = std::fs::create_dir_all(dir());
    let incoming = dir().join("incoming.png");
    let _ = std::fs::remove_file(&incoming);
    let Some(found) = macos::pasteboard_take(&incoming) else { return };
    if found["skip"].as_bool() == Some(true) {
        return;
    }
    let what = if let Some(files) = found["files"].as_array() {
        What::Files(files.iter().filter_map(|f| f.as_str()).map(PathBuf::from).collect())
    } else if found["image"].as_bool() == Some(true) {
        let Ok(bytes) = std::fs::read(&incoming) else { return };
        let hash = fnv(&bytes);
        let file = format!("{hash:016x}.png");
        let _ = std::fs::rename(&incoming, dir().join(&file));
        What::Image { file, w: found["w"].as_u64().unwrap_or(0) as u32, h: found["h"].as_u64().unwrap_or(0) as u32, hash }
    } else if let Some(text) = found["text"].as_str() {
        if text.trim().is_empty() || text.len() > 2 << 20 {
            return;
        }
        let t = text.trim();
        let link = !t.contains(char::is_whitespace) && (t.starts_with("https://") || t.starts_with("http://"));
        if link { What::Link(t.to_string()) } else { What::Text(text.to_string()) }
    } else {
        return;
    };
    let mut clip = Clip {
        id: (now() << 16) | (fnv(format!("{what:?}").as_bytes()) & 0xffff),
        what,
        app: found["app"].as_str().map(str::to_string),
        app_path: found["app_path"].as_str().map(PathBuf::from),
        at: now(),
        pinned: false,
        text: None,
    };
    let (id, image) = (clip.id, clip.image());
    let unread = with(|clips| {
        // (Copied again: up to the top, the same entry.)
        let same = |c: &Clip| match (&c.what, &clip.what) {
            (What::Image { hash: a, .. }, What::Image { hash: b, .. }) => a == b,
            (a, b) => a == b,
        };
        // (A pinned one copied again stays pinned; an image's words, read.)
        clip.pinned = clips.iter().any(|c| c.pinned && same(c));
        clip.text = clips.iter().find(|c| same(c)).and_then(|c| c.text.clone());
        let unread = clip.text.is_none();
        clips.retain(|c| !same(c));
        clips.insert(0, clip);
        let keep = KEEP.load(Ordering::SeqCst);
        // (An image is in one entry only: copied again, its entry moved.
        // The pinned aren't counted, nor let go of.)
        let mut unpinned = 0;
        let mut gone = Vec::new();
        clips.retain(|c| {
            if c.pinned {
                return true;
            }
            unpinned += 1;
            let kept = unpinned <= keep;
            if !kept {
                gone.push(c.clone());
            }
            kept
        });
        for gone in gone {
            if let Some(image) = gone.image() {
                let _ = std::fs::remove_file(image);
            }
        }
        save(clips);
        unread
    });
    // (An image's words read meanwhile, off this thread.)
    if unread && let Some(image) = image {
        std::thread::spawn(move || read_text(id, &image));
    }
}

/// Read the words in the image clip `id` (its file `image`), and keep
/// them with it.
fn read_text(id: u64, image: &Path) {
    let text = macos::recognize_text(image).unwrap_or_default();
    with(|clips| {
        if let Some(clip) = clips.iter_mut().find(|c| c.id == id) {
            clip.text = Some(text);
            save(clips);
        }
    });
}

/// The images copied before their words were read (by an F2 before this
/// one): read now, one at a time.
fn read_unread() {
    let unread: Vec<(u64, PathBuf)> = with(|clips| clips.iter().filter(|c| c.text.is_none()).filter_map(|c| c.image().map(|i| (c.id, i))).collect());
    for (id, image) in unread {
        read_text(id, &image);
    }
}

/// FNV-1a: a quick hash, to know an image again.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

#[cfg(test)]
mod tests {
    use super::{Clip, What, ago, now, shot_title};

    #[test]
    fn titles_and_matching() {
        let clip = |what| Clip { id: 1, what, app: Some("Safari".into()), app_path: None, at: now(), pinned: false, text: None };
        assert_eq!(clip(What::Text("\n  first line \nsecond".into())).title(), "first line");
        assert_eq!(clip(What::Files(vec!["/a/b.txt".into()])).title(), "b.txt");
        assert_eq!(clip(What::Files(vec!["/a".into(), "/b".into()])).title(), "2 Files");
        assert!(clip(What::Link("https://github.com".into())).matches("github"));
        assert!(clip(What::Text("x".into())).matches("safari"), "by the app it's from");
        assert!(!clip(What::Text("x".into())).matches("zz"));
        let image = What::Image { file: "a.png".into(), w: 800, h: 600, hash: 1 };
        assert_eq!(clip(image.clone()).title(), "Image (800×600)");
        let read = Clip { text: Some("\n Invoice 4471\nTotal 129 EUR".into()), ..clip(image.clone()) };
        assert_eq!(read.title(), "Image: Invoice 4471");
        assert!(read.matches("129 eur"), "by the words in it");
        assert!(!clip(image).matches("invoice"));
        assert_eq!(ago(now()), "Just now");
        assert_eq!(ago(now() - 125), "2 min ago");
        let shot = |name: &str| shot_title(std::path::Path::new(&format!("/k/{name}.png")));
        assert_eq!(shot("Screenshot 2026-10-20 at 17.02.45"), "20 Oct, 17:02:45");
        assert_eq!(shot("Screenshot 2026-10-20 at 09.15.02 (edited)"), "20 Oct, 09:15:02 (edited)");
        assert_eq!(shot("Screenshot 2026-10-20 at 09.15.02 (2)"), "20 Oct, 09:15:02 (2)");
        assert_eq!(shot("Something else"), "Something else");
    }
}

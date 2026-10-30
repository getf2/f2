//! Disk images (.dmg, .iso, .sparseimage…), opened here. macOS's own
//! DiskImageMounter mounts one and leaves its window to Finder, so with
//! Finder quit nothing shows. Here it's attached with hdiutil (a locked
//! one's password asked first, in F2's dialog), and its volume shown as
//! its maker laid it out for Finder (see `ds_store`): in a window its
//! size, with no toolbar or sidebar, their picture behind the icons, each
//! where they put it. A downloaded one (quarantined) is checked through
//! first, in case it came damaged: seconds for a big one, so one made here
//! isn't. One with a license to agree to goes to
//! DiskImageMounter, which shows the license. With an app alone in it, the
//! window offers to install it in Applications (see `finder::install`).
//! Settings can make F2 what opens them from other apps too.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gpui::*;

use crate::finder::Finder;

/// The endings DiskImageMounter opens.
const KINDS: [&str; 7] = ["dmg", "img", "iso", "cdr", "sparseimage", "sparsebundle", "asif"];

/// Passwords tried before giving up.
const TRIES: usize = 3;

pub fn is_disk_image(path: &Path) -> bool {
    path.extension().is_some_and(|e| KINDS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// An image attached, and where its volumes are.
pub struct Attached {
    pub image: PathBuf,
    pub mounts: Vec<PathBuf>,
}

/// hdiutil's answer (its plist, as JSON), or what it said went wrong.
fn hdiutil(args: &[&str], path: Option<&Path>, password: Option<&str>) -> Result<serde_json::Value, String> {
    let mut command = Command::new("/usr/bin/hdiutil");
    command.args(args).arg("-plist");
    if password.is_some() {
        command.arg("-stdinpass");
    }
    command.args(path);
    command.stdin(if password.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    // (Never on its command line: read from stdin, ending in a NUL.)
    if let (Some(password), Some(mut stdin)) = (password, child.stdin.take()) {
        let _ = stdin.write_all(password.as_bytes()).and_then(|()| stdin.write_all(&[0]));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        // ("hdiutil: attach failed - Authentication error": the end.)
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("it didn't say why");
        return Err(said.rsplit(" - ").next().unwrap_or(said).trim().to_string());
    }
    crate::macos::plist_json(&out.stdout).ok_or_else(|| "hdiutil's answer couldn't be read.".into())
}

/// Where an image's volumes are mounted, from hdiutil's description of it.
fn mounts_of(image: &serde_json::Value) -> Vec<PathBuf> {
    let entities = image.get("system-entities").and_then(|e| e.as_array());
    entities.into_iter().flatten().filter_map(|e| e.get("mount-point")?.as_str()).map(PathBuf::from).collect()
}

/// The images attached now (hdiutil's list).
pub fn attached() -> Vec<Attached> {
    let Ok(info) = hdiutil(&["info"], None, None) else { return Vec::new() };
    let images = info.get("images").and_then(|i| i.as_array());
    images
        .into_iter()
        .flatten()
        .filter_map(|i| Some(Attached { image: PathBuf::from(i.get("image-path")?.as_str()?), mounts: mounts_of(i) }))
        .collect()
}

/// The image a volume mounted at `mount` is of, if it's one.
pub fn image_of(mount: &Path) -> Option<PathBuf> {
    attached().into_iter().find(|a| a.mounts.iter().any(|m| m == mount)).map(|a| a.image)
}

/// The app alone at the top of a volume (an app's DMG), if there's one.
pub fn lone_app(volume: &Path) -> Option<PathBuf> {
    let mut apps = std::fs::read_dir(volume)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "app") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) && p.is_dir());
    let app = apps.next()?;
    apps.next().is_none().then_some(app)
}

/// Whether macOS marked it as downloaded (quarantined).
fn downloaded(image: &Path) -> bool {
    let Ok(path) = std::ffi::CString::new(image.as_os_str().as_encoded_bytes()) else { return false };
    unsafe { libc::getxattr(path.as_ptr(), c"com.apple.quarantine".as_ptr(), std::ptr::null_mut(), 0, 0, 0) >= 0 }
}

/// How opening an image went.
pub enum Outcome {
    Mounted(Vec<PathBuf>),
    /// Its password wasn't given.
    Cancelled,
    /// It has a license to agree to first: DiskImageMounter's to show.
    License,
    Failed(String),
}

/// A locked image's password, asked in F2's own dialog (F2 run again to
/// show it: see `macos::askpass`); None if it's cancelled.
fn ask_password(image: &Path, again: bool) -> Option<String> {
    let name = image.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let then = if again { "That password didn't open it. Try again." } else { "Enter its password to open it." };
    let child = Command::new(std::env::current_exe().ok()?)
        .arg(format!("“{name}” is locked.\n{then}"))
        .env("F2_ASKPASS", "1")
        .env("SSH_ASKPASS_PROMPT", "unlock")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    crate::macos::yield_to(child.id());
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let answer = String::from_utf8(out.stdout).ok()?;
    Some(answer.strip_suffix('\n').unwrap_or(&answer).to_string())
}

/// Attach `image` (or find it attached already): its volumes. Waits for
/// hdiutil (which checks a compressed image through first) and for a
/// password: not on the main thread.
pub fn attach(image: &Path) -> Outcome {
    let image = std::fs::canonicalize(image).unwrap_or_else(|_| image.to_path_buf());
    if let Some(already) = attached().into_iter().find(|a| a.image == image && !a.mounts.is_empty()) {
        return Outcome::Mounted(already.mounts);
    }
    let locked = hdiutil(&["isencrypted"], Some(&image), None).ok().and_then(|v| v.get("encrypted")?.as_bool()).unwrap_or(false);
    for attempt in 0..TRIES {
        let password = if locked {
            match ask_password(&image, attempt > 0) {
                Some(p) => Some(p),
                None => return Outcome::Cancelled,
            }
        } else {
            None
        };
        let check = if downloaded(&image) { "-verify" } else { "-noverify" };
        match hdiutil(&["attach", "-noautoopen", check], Some(&image), password.as_deref()) {
            Ok(attached) => return Outcome::Mounted(mounts_of(&attached)),
            Err(e) if locked && e.to_lowercase().contains("authentication") => continue,
            Err(e) => {
                let license = hdiutil(&["imageinfo"], Some(&image), None)
                    .ok()
                    .and_then(|i| i.get("Properties")?.get("Software License Agreement")?.as_bool())
                    .unwrap_or(false);
                return if license { Outcome::License } else { Outcome::Failed(e) };
            }
        }
    }
    Outcome::Failed("That password didn't open it.".into())
}

/// Open a disk image: attached (off the main thread), then its volume
/// shown. What goes wrong is said in `from`'s window, if it was opened
/// from one, else in a note.
pub fn open(image: PathBuf, from: Option<WeakEntity<Finder>>, cx: &mut App) {
    let name = image.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let checked = downloaded(&image);
    cx.spawn(async move |cx| {
        let attaching = image.clone();
        let attaching = cx.background_executor().spawn(async move { attach(&attaching) });
        // (A download being checked, with no window it was opened from to
        // say so: said in a note, if it's taking a while.)
        let slow = cx.background_executor().timer(std::time::Duration::from_millis(1500));
        let outcome = match futures::future::select(attaching, slow).await {
            futures::future::Either::Left((outcome, _)) => outcome,
            futures::future::Either::Right((_, attaching)) => {
                if checked && from.is_none() {
                    let title = format!("Opening “{name}”");
                    cx.update(|cx| crate::shots::note(title, "It was downloaded, so it's checked through first.".into(), None, cx));
                }
                attaching.await
            }
        };
        cx.update(|cx| {
            let tell = |why: String, cx: &mut App| match from.and_then(|f| f.upgrade()) {
                Some(finder) => finder.update(cx, |f, cx| f.say(format!("“{name}” couldn't be opened: {why}"), cx)),
                None => crate::shots::note(format!("“{name}” Couldn't Be Opened"), why, None, cx),
            };
            match outcome {
                Outcome::Mounted(mounts) => match mounts.into_iter().next() {
                    Some(mount) => show(mount, cx),
                    None => tell("it has no disk macOS can read.".into(), cx),
                },
                Outcome::License => cx.open_with_system(&image),
                Outcome::Cancelled => {}
                Outcome::Failed(why) => tell(why, cx),
            }
        });
    })
    .detach();
}

/// Room under the picture for the bar offering to install its app.
pub const INSTALL_BAR: f32 = 44.;

/// A volume shown: in its window as laid out for Finder (no toolbar, its
/// size), else in an ordinary one. Already shown: that window, in front.
pub fn show(mount: PathBuf, cx: &mut App) {
    if crate::browser::bring_forward(&mount, cx) {
        return;
    }
    let store = crate::ds_store::read(&mount).unwrap_or_default();
    match store.window.filter(|w| !w.toolbar) {
        Some(laid_out) => {
            let (w, h) = laid_out.size;
            let bar = if lone_app(&mount).is_some() { INSTALL_BAR } else { 0. };
            let size = (w.max(240.), (h - crate::macos::titlebar_height()).max(120.) + bar);
            crate::browser::open_laid_out(mount, size, cx);
        }
        None => crate::browser::open(mount, None, cx),
    }
    cx.activate(true);
}

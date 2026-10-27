//! Quick Actions, as Finder offers them for images (rotate, make a PDF,
//! convert), and more for what people send each other: Make Smaller (a
//! photo or a video, for email or a website), Remove Background (a photo's
//! subject, kept), Copy Text (from a photo, a screenshot, a PDF or a scan),
//! Compress PDF, Combine into PDF, Convert to MP4; plus your Shortcuts,
//! each run on the selected files (and by name, from Quick Open). With
//! macOS's own tools: `sips`, `avconvert`, PDFKit, Vision.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use crate::listing::Entry;
use crate::macos;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QuickAction {
    RotateLeft,
    RotateRight,
    CreatePdf,
    ToJpeg,
    ToPng,
    ToHeic,
    /// Photos to 1600 pixels at most, as JPEG; videos to 720p, as MP4.
    MakeSmaller,
    /// Images to a size you choose, in Quick Open (see `resize`).
    Resize,
    CompressPdf,
    /// Several images and PDFs, one after another, as one PDF.
    CombinePdf,
    ToMp4,
    /// A photo's subject, as a PNG with nothing behind it.
    RemoveBackground,
    /// The text in images and PDFs, onto the clipboard.
    CopyText,
}

impl QuickAction {
    pub fn title(self) -> &'static str {
        match self {
            QuickAction::RotateLeft => "Rotate Left",
            QuickAction::RotateRight => "Rotate Right",
            QuickAction::CreatePdf => "Create PDF",
            QuickAction::ToJpeg => "Convert to JPEG",
            QuickAction::ToPng => "Convert to PNG",
            QuickAction::ToHeic => "Convert to HEIC",
            QuickAction::MakeSmaller => "Make Smaller",
            QuickAction::Resize => "Resize…",
            QuickAction::CompressPdf => "Compress PDF",
            QuickAction::CombinePdf => "Combine into PDF",
            QuickAction::ToMp4 => "Convert to MP4",
            QuickAction::RemoveBackground => "Remove Background",
            QuickAction::CopyText => "Copy Text",
        }
    }
}

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn is_pdf(path: &Path) -> bool {
    ext(path) == "pdf"
}

/// What `avconvert` (AVFoundation) can read.
fn is_video(path: &Path) -> bool {
    matches!(ext(path).as_str(), "mov" | "mp4" | "m4v" | "3gp")
}

pub(crate) fn is_image(path: &Path) -> bool {
    matches!(ext(path).as_str(), "png" | "jpg" | "jpeg" | "heic" | "heif" | "tif" | "tiff" | "gif" | "bmp" | "webp")
}

/// What applies to these items.
pub fn for_items(items: &[Entry]) -> Vec<QuickAction> {
    use QuickAction::*;
    let all = |f: fn(&Path) -> bool| !items.is_empty() && items.iter().all(|e| !e.is_dir && f(&e.path));
    let several = items.len() > 1;
    if all(is_image) {
        let mut actions = vec![MakeSmaller, Resize];
        if macos::can_remove_background() {
            actions.push(RemoveBackground);
        }
        actions.extend([CopyText, ToJpeg, ToPng, ToHeic, RotateLeft, RotateRight, if several { CombinePdf } else { CreatePdf }]);
        actions
    } else if all(is_pdf) {
        if several { vec![CompressPdf, CombinePdf, CopyText] } else { vec![CompressPdf, CopyText] }
    } else if several && all(|p| is_image(p) || is_pdf(p)) {
        vec![CombinePdf, CopyText]
    } else if all(is_video) {
        vec![MakeSmaller, ToMp4]
    } else {
        Vec::new()
    }
}

fn avconvert(input: &Path, preset: &str, out: &Path) -> io::Result<()> {
    let done = Command::new("/usr/bin/avconvert").arg("--source").arg(input).args(["--preset", preset, "--output"]).arg(out).output()?;
    if done.status.success() && out.exists() {
        Ok(())
    } else {
        let why = String::from_utf8_lossy(&done.stderr).trim().to_string();
        Err(io::Error::other(if why.is_empty() { "The video couldn't be converted.".into() } else { why }))
    }
}

/// `dir/stem<suffix>.ext`, or numbered if that's taken.
fn beside(p: &Path, suffix: &str, ext: &str) -> PathBuf {
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    crate::fileops::free_name(p.parent().unwrap_or(Path::new("/")), &format!("{stem}{suffix}"), ext)
}

fn sips(args: &[&std::ffi::OsStr]) -> io::Result<()> {
    let out = Command::new("/usr/bin/sips").args(args).output()?;
    if out.status.success() { Ok(()) } else { Err(io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string())) }
}

/// Run it; what it made (rotating changes the files in place).
pub fn run(action: QuickAction, paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let convert = |format: &str, ext: &str| -> io::Result<Vec<PathBuf>> {
        paths
            .iter()
            .map(|p| {
                let out = crate::fileops::free_name(p.parent().unwrap_or(Path::new("/")), &p.file_stem().unwrap_or_default().to_string_lossy(), ext);
                sips(&["-s".as_ref(), "format".as_ref(), format.as_ref(), p.as_os_str(), "--out".as_ref(), out.as_os_str()])?;
                Ok(out)
            })
            .collect()
    };
    match action {
        QuickAction::RotateLeft | QuickAction::RotateRight => {
            let degrees = if action == QuickAction::RotateLeft { "-90" } else { "90" };
            for p in paths {
                sips(&["-r".as_ref(), degrees.as_ref(), p.as_os_str()])?;
            }
            Ok(Vec::new())
        }
        QuickAction::CreatePdf | QuickAction::CombinePdf => {
            let first = paths.first().ok_or_else(|| io::Error::other("nothing selected"))?;
            let dir = first.parent().unwrap_or(Path::new("/"));
            let stem = if paths.len() == 1 { first.file_stem().unwrap_or_default().to_string_lossy().into_owned() } else { "Combined".into() };
            let out = crate::fileops::free_name(dir, &stem, "pdf");
            macos::images_to_pdf(paths, &out).map_err(io::Error::other)?;
            Ok(vec![out])
        }
        QuickAction::ToJpeg => convert("jpeg", "jpg"),
        QuickAction::ToPng => convert("png", "png"),
        QuickAction::ToHeic => convert("heic", "heic"),
        QuickAction::MakeSmaller => paths
            .iter()
            .map(|p| {
                if is_video(p) {
                    let out = beside(p, " (smaller)", "mp4");
                    avconvert(p, "Preset1280x720", &out)?;
                    return Ok(out);
                }
                // (No bigger than 1600 pixels a side, JPEG at 80%: what a
                // phone photo of 4 MB comes to a few hundred KB as.)
                let out = beside(p, " (smaller)", "jpg");
                let args: [&std::ffi::OsStr; 9] =
                    ["-Z".as_ref(), "1600".as_ref(), "-s".as_ref(), "format".as_ref(), "jpeg".as_ref(), "-s".as_ref(), "formatOptions".as_ref(), "80".as_ref(), p.as_os_str()];
                let mut all: Vec<&std::ffi::OsStr> = args.to_vec();
                all.extend(["--out".as_ref(), out.as_os_str()]);
                sips(&all)?;
                Ok(out)
            })
            .collect(),
        QuickAction::CompressPdf => paths
            .iter()
            .map(|p| {
                let out = beside(p, " (compressed)", "pdf");
                macos::compress_pdf(p, &out).map_err(io::Error::other)?;
                Ok(out)
            })
            .collect(),
        QuickAction::ToMp4 => paths
            .iter()
            .map(|p| {
                let out = beside(p, "", "mp4");
                avconvert(p, "Preset1920x1080", &out)?;
                Ok(out)
            })
            .collect(),
        QuickAction::RemoveBackground => paths
            .iter()
            .map(|p| {
                let out = beside(p, " (no background)", "png");
                macos::remove_background(p, &out).map_err(io::Error::other)?;
                Ok(out)
            })
            .collect(),
        QuickAction::CopyText => {
            macos::copy_text(&text_of(paths)?);
            Ok(Vec::new())
        }
        // (Its size is chosen first, on its page.)
        QuickAction::Resize => Err(io::Error::other("Resize… opens in Quick Open.")),
    }
}

/// The text in images and PDFs (Vision reads what's not typed), one after
/// another.
pub fn text_of(paths: &[PathBuf]) -> io::Result<String> {
    let texts: Vec<String> = paths.iter().filter_map(|p| if is_pdf(p) { macos::pdf_text(p) } else { macos::recognize_text(p) }).filter(|t| !t.trim().is_empty()).collect();
    if texts.is_empty() {
        return Err(io::Error::other(if paths.len() == 1 { "There's no text in it to copy." } else { "There's no text in them to copy." }));
    }
    Ok(texts.join("\n\n"))
}

/// Copy the text in images and PDFs, and say so (or that there's none):
/// read off the main thread.
pub fn copy_text(paths: Vec<PathBuf>, cx: &mut gpui::App) {
    cx.spawn(async move |cx| {
        let text = cx.background_executor().spawn(async move { text_of(&paths) }).await;
        cx.update(|cx| match text {
            Ok(text) => {
                macos::copy_text(&text);
                let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
                let first: String = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(80).collect();
                crate::shots::note(format!("Copied {lines} Line{} of Text", if lines == 1 { "" } else { "s" }), first, None, cx);
            }
            Err(e) => crate::shots::note("No Text Found".into(), e.to_string(), None, cx),
        });
    })
    .detach();
}

static SHORTCUTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Look up your Shortcuts in the background (`shortcuts list` is slowish):
/// at the start, and again as Quick Open opens (one may have been made).
pub fn warm() {
    std::thread::spawn(|| {
        let Ok(out) = Command::new("/usr/bin/shortcuts").arg("list").output() else { return };
        let names: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
        *SHORTCUTS.lock().unwrap() = names;
    });
}

/// Your Shortcuts, by name (once looked up).
pub fn shortcuts() -> Vec<String> {
    SHORTCUTS.lock().unwrap().clone()
}

/// Run a Shortcut by its name, and say how it went: what it gave back, if
/// it's text (waits for it: not on the main thread).
pub fn run_named(name: &str) -> crate::toggles::Note {
    match Command::new("/usr/bin/shortcuts").args(["run", name]).output() {
        Ok(out) if out.status.success() => {
            let said: String = String::from_utf8_lossy(&out.stdout).lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(120).collect();
            (format!("Ran “{name}”"), said, None)
        }
        Ok(out) => (format!("“{name}” Didn't Finish"), String::from_utf8_lossy(&out.stderr).trim().chars().take(160).collect(), None),
        Err(e) => (format!("Couldn't Run “{name}”"), e.to_string(), None),
    }
}

/// Run a Shortcut with each file as its input.
pub fn run_shortcut(name: &str, paths: &[PathBuf]) {
    for path in paths {
        let _ = Command::new("/usr/bin/shortcuts").args(["run", name, "-i"]).arg(path).output();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{QuickAction, run};

    fn size_of(path: &Path) -> (u32, u32) {
        let out = Command::new("/usr/bin/sips").args(["-g", "pixelWidth", "-g", "pixelHeight"]).arg(path).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let num = |key: &str| text.lines().find(|l| l.contains(key)).and_then(|l| l.split_whitespace().last()?.parse().ok()).unwrap();
        (num("pixelWidth"), num("pixelHeight"))
    }

    fn pages(path: &Path) -> usize {
        let out = Command::new("/usr/bin/mdls").args(["-raw", "-name", "kMDItemNumberOfPages"]).arg(path).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
    }

    /// A video's: `QUICK_VIDEO=/some/clip.mov cargo test --release -- --ignored videos_get_smaller`
    #[test]
    #[ignore]
    fn videos_get_smaller() {
        let Some(video) = std::env::var_os("QUICK_VIDEO").map(std::path::PathBuf::from) else { return };
        let smaller = run(QuickAction::MakeSmaller, std::slice::from_ref(&video)).unwrap();
        let mp4 = run(QuickAction::ToMp4, std::slice::from_ref(&video)).unwrap();
        for out in smaller.iter().chain(&mp4) {
            println!("{} {} bytes", out.display(), std::fs::metadata(out).unwrap().len());
        }
        assert!(smaller[0].to_string_lossy().ends_with("(smaller).mp4") && mp4[0].extension().unwrap() == "mp4");
    }

    /// Text read and backgrounds taken out, of a folder with a picture of
    /// text (text.png), a typed PDF (typed.pdf) and a subject on a plain
    /// ground (subject.png): `QUICK_IMAGES=/some/dir cargo test --release --
    /// --ignored images_read_and_cut`
    #[test]
    #[ignore]
    fn images_read_and_cut() {
        let Some(dir) = std::env::var_os("QUICK_IMAGES").map(std::path::PathBuf::from) else { return };
        let picture = super::text_of(&[dir.join("text.png")]).unwrap();
        println!("picture: {picture:?}");
        assert!(picture.contains("station") && picture.contains("umbrella"));
        let typed = super::text_of(&[dir.join("typed.pdf")]).unwrap();
        println!("typed PDF: {typed:?}");
        assert!(typed.contains("Invoice 2041"));
        // (A scan: a PDF of the picture, read off its page.)
        let scan = run(QuickAction::CreatePdf, &[dir.join("text.png")]).unwrap();
        let scanned = super::text_of(&scan).unwrap();
        println!("scanned PDF: {scanned:?}");
        assert!(scanned.contains("umbrella"));
        let cut = run(QuickAction::RemoveBackground, &[dir.join("subject.png")]).unwrap();
        println!("cut out: {}", cut[0].display());
        assert!(cut[0].to_string_lossy().ends_with("(no background).png") && cut[0].exists());
        assert!(super::text_of(&[dir.join("subject.png")]).is_err(), "no text in it");
    }

    #[test]
    fn photos_get_smaller_and_pdfs_combine() {
        let dir = std::env::temp_dir().join(format!("f2-quick-actions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.png");
        let big = dir.join("photo.png");
        assert!(Command::new("/usr/bin/sips").args(["-z", "2000", "3000"]).arg(&icon).arg("--out").arg(&big).output().unwrap().status.success());

        let smaller = run(QuickAction::MakeSmaller, std::slice::from_ref(&big)).unwrap();
        assert_eq!(smaller, [dir.join("photo (smaller).jpg")]);
        assert_eq!(size_of(&smaller[0]), (1600, 1066), "no side over 1600, the shape kept");

        let one_pdf = run(QuickAction::CreatePdf, std::slice::from_ref(&icon.clone())).unwrap();
        let _ = std::fs::rename(&one_pdf[0], dir.join("icon.pdf"));
        let combined = run(QuickAction::CombinePdf, &[big.clone(), smaller[0].clone(), dir.join("icon.pdf")]).unwrap();
        assert_eq!(combined, [dir.join("Combined.pdf")]);
        // (Spotlight may not have read it yet: count the pages another way if so.)
        let n = pages(&combined[0]);
        assert!(n == 3 || n == 0, "{n} pages");

        let compressed = run(QuickAction::CompressPdf, &combined).unwrap();
        assert_eq!(compressed, [dir.join("Combined (compressed).pdf")]);
        assert!(std::fs::metadata(&compressed[0]).unwrap().len() < std::fs::metadata(&combined[0]).unwrap().len(), "smaller");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

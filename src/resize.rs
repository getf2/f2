//! Resize Images: images made smaller to a size you choose (a share of it,
//! a width, a height, a box to fit in), as copies beside them, each named
//! by its new size ("IMG_1234 (1920×1440).jpg"), as they are or as JPEG,
//! PNG or HEIC. Quick Open's page of F2's own (`extension`), opened by the
//! Quick Action Resize…, or on what's selected in F2. Never made bigger.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{Value, json};

use crate::extensions::{Extension, Tell};

/// How big to make them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Size {
    Percent(u32),
    Width(u32),
    Height(u32),
    /// At most so wide and so tall.
    Fit(u32, u32),
}

/// What's offered before anything's typed.
const PRESETS: [Size; 11] = [
    Size::Percent(75),
    Size::Percent(50),
    Size::Percent(25),
    Size::Width(3840),
    Size::Width(2560),
    Size::Width(1920),
    Size::Width(1280),
    Size::Width(1024),
    Size::Width(800),
    Size::Width(640),
    Size::Width(512),
];

impl Size {
    /// What's typed as a size: "50%", "1920" (wide), "w1920", "h600" or
    /// "600h" (tall), "800x600" (a box).
    pub fn parse(text: &str) -> Option<Size> {
        let t = text.trim().to_lowercase().replace(' ', "");
        let n = |s: &str| s.parse::<u32>().ok().filter(|n| (1..=100_000).contains(n));
        if let Some(p) = t.strip_suffix('%') {
            return n(p).filter(|p| *p < 100).map(Size::Percent);
        }
        if let Some((w, h)) = t.split_once(['x', '×', '*']) {
            return Some(Size::Fit(n(w)?, n(h)?));
        }
        if let Some(h) = t.strip_prefix('h').or_else(|| t.strip_suffix('h')) {
            return n(h).map(Size::Height);
        }
        n(t.strip_prefix('w').or_else(|| t.strip_suffix('w')).unwrap_or(&t)).map(Size::Width)
    }

    /// As it's kept in an action's arguments, and read back.
    fn key(&self) -> String {
        match self {
            Size::Percent(p) => format!("{p}%"),
            Size::Width(w) => format!("w{w}"),
            Size::Height(h) => format!("h{h}"),
            Size::Fit(w, h) => format!("{w}x{h}"),
        }
    }

    pub fn title(&self) -> String {
        match self {
            Size::Percent(p) => format!("{p}%"),
            Size::Width(w) => format!("{w} Pixels Wide"),
            Size::Height(h) => format!("{h} Pixels Tall"),
            Size::Fit(w, h) => format!("To Fit in {w}×{h}"),
        }
    }

    /// An image `w` by `h` at this size, if that's smaller.
    pub fn of(&self, w: u32, h: u32) -> Option<(u32, u32)> {
        let scale = match *self {
            Size::Percent(p) => p as f64 / 100.,
            Size::Width(to) => to as f64 / w as f64,
            Size::Height(to) => to as f64 / h as f64,
            Size::Fit(tw, th) => (tw as f64 / w as f64).min(th as f64 / h as f64),
        };
        let (nw, nh) = ((w as f64 * scale).round().max(1.) as u32, (h as f64 * scale).round().max(1.) as u32);
        (scale < 1. && (nw < w || nh < h)).then_some((nw, nh))
    }
}

/// Each image's size as shown, and whether it can be see-through (asked
/// once a file: as you type, it's asked again).
fn info(path: &Path) -> Option<(u32, u32, bool)> {
    static KNOWN: OnceLock<Mutex<HashMap<PathBuf, (u32, u32, bool)>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(i) = known.lock().unwrap().get(path) {
        return Some(*i);
    }
    let i = crate::macos::shown_size(path)?;
    known.lock().unwrap().insert(path.to_path_buf(), i);
    Some(i)
}

/// What an image of `path` is written as, for `format` ("same", "jpeg",
/// "png", "heic"): its UTI and extension. One that can't be written as it
/// is (WebP, a camera's raw…) is written as PNG if it can be see-through,
/// else JPEG.
fn written_as(path: &Path, format: &str, alpha: bool) -> (&'static str, String) {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match (format, ext.as_str()) {
        ("jpeg", _) => ("public.jpeg", "jpg".into()),
        ("png", _) => ("public.png", "png".into()),
        ("heic", _) => ("public.heic", "heic".into()),
        (_, "jpg" | "jpeg") => ("public.jpeg", ext),
        (_, "png") => ("public.png", ext),
        (_, "heic" | "heif") => ("public.heic", ext),
        (_, "tif" | "tiff") => ("public.tiff", ext),
        (_, "gif") => ("com.compuserve.gif", ext),
        (_, "bmp") => ("com.microsoft.bmp", ext),
        _ if alpha => ("public.png", "png".into()),
        _ => ("public.jpeg", "jpg".into()),
    }
}

/// Each of `paths` made `size`, beside it, as `format` says: what was made,
/// and what couldn't be (by why). Those already smaller are left alone.
pub fn resize(paths: &[PathBuf], size: Size, format: &str) -> (Vec<PathBuf>, Vec<String>) {
    let (mut made, mut failed) = (Vec::new(), Vec::new());
    for path in paths {
        let Some((w, h, alpha)) = info(path) else {
            failed.push(format!("{}: it isn't an image that can be read", name(path)));
            continue;
        };
        let Some((nw, nh)) = size.of(w, h) else { continue };
        let (kind, ext) = written_as(path, format, alpha);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let out = crate::fileops::free_name(path.parent().unwrap_or(Path::new("/")), &format!("{stem} ({nw}×{nh})"), &ext);
        match crate::macos::resize_image(path, nw.max(nh), kind, &out) {
            Ok(()) => made.push(out),
            Err(why) => failed.push(format!("{}: {why}", name(path))),
        }
    }
    (made, failed)
}

fn name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

// Quick Open's page.

/// The page of F2's own for it: `resize` (on `{"paths": […]}`), and its
/// rows' action, `run`.
pub fn extension() -> Arc<Extension> {
    static IT: OnceLock<Arc<Extension>> = OnceLock::new();
    IT.get_or_init(|| {
        let manifest = json!({
            "api": 1,
            "id": "f2.resize",
            "name": "Resize Images",
            "icon": "arrow.down.right.and.arrow.up.left",
            "color": "#0a84ff",
            "commands": [
                { "id": "resize", "title": "Resize Images", "search": "script" },
                { "id": "run", "title": "Resize", "hidden": true }
            ]
        });
        Arc::new(Extension { id: "f2.resize".into(), dir: PathBuf::new(), manifest: serde_json::from_value(manifest).expect("Resize's manifest"), native: Some(page) })
    })
    .clone()
}

/// Its page, on `paths`.
pub fn args(paths: &[PathBuf]) -> Value {
    json!({ "paths": paths })
}

/// The images selected in F2's window in front (none: it says to select
/// some).
pub fn selected(cx: &gpui::App) -> Vec<PathBuf> {
    crate::browser::front_selection(cx).map(|(_, paths)| paths.into_iter().filter(|p| crate::quick_actions::is_image(p)).collect()).unwrap_or_default()
}

fn page(command: &str, args: &Value, query: &str, tell: Tell, cx: &mut gpui::App) {
    let paths: Vec<PathBuf> = args.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from).collect();
    let said = match command {
        "resize" => rows(&paths, query),
        "run" => {
            let size = args.get("size").and_then(Value::as_str).and_then(Size::parse);
            let format = args.get("format").and_then(Value::as_str).unwrap_or("same").to_string();
            let Some(size) = size else { return };
            cx.spawn(async move |cx| {
                let (made, failed) = cx.background_executor().spawn(async move { resize(&paths, size, &format) }).await;
                cx.update(|cx| {
                    let (title, text) = match (made.as_slice(), failed.first()) {
                        ([], Some(why)) => ("Couldn't Resize".to_string(), why.clone()),
                        ([one], None) => ("Resized".to_string(), name(one)),
                        (made, None) => (format!("Resized {} Images", made.len()), "Beside them, each named by its size.".into()),
                        (made, Some(why)) => (format!("Resized {}; {} Couldn't Be", made.len(), failed.len()), why.clone()),
                    };
                    crate::shots::note(title, text, None, cx);
                    // (Selected where they are, in the window in front.)
                    if !made.is_empty() {
                        crate::browser::made(made, cx);
                    }
                });
            })
            .detach();
            json!({ "close": true })
        }
        _ => json!({ "error": format!("Resize Images has no \"{command}\".") }),
    };
    tell.say(said);
}

/// The page's rows: what's typed, as a size, first; then the sizes most
/// often wanted, those that make one of them smaller.
fn rows(paths: &[PathBuf], query: &str) -> Value {
    let images: Vec<(&PathBuf, (u32, u32))> = paths.iter().filter_map(|p| info(p).map(|(w, h, _)| (p, (w, h)))).collect();
    let title = match paths {
        [one] => format!("Resize {}", name(one)),
        many => format!("Resize {} Images", many.len()),
    };
    let placeholder = "A size: 1920, 50%, 800x600, h600…";
    if images.is_empty() {
        let why = if paths.is_empty() { "Select images in an F2 window first, then Resize Images." } else { "None of these is an image that can be read." };
        return json!({ "title": title, "placeholder": placeholder, "items": [], "error": why, "loading": false });
    }
    let typed = Size::parse(query);
    let mut rows: Vec<Value> = Vec::new();
    if let Some(size) = typed {
        rows.push(row(size, paths, &images));
    } else if !query.trim().is_empty() {
        rows.push(json!({ "title": format!("“{}” isn't a size", query.trim()), "subtitle": "A width (1920), a share (50%), a box (800x600) or a height (h600)", "icon": "questionmark.circle", "color": "gray" }));
    }
    rows.extend(PRESETS.iter().filter(|s| Some(**s) != typed && images.iter().any(|(_, (w, h))| s.of(*w, *h).is_some())).map(|s| row(*s, paths, &images)));
    json!({ "title": title, "placeholder": placeholder, "items": rows, "loading": false })
}

/// A size's row: what it makes them, and resizing to it (as they are, or
/// as JPEG, PNG or HEIC).
fn row(size: Size, paths: &[PathBuf], images: &[(&PathBuf, (u32, u32))]) -> Value {
    let smaller: Vec<(u32, u32)> = images.iter().filter_map(|(_, (w, h))| size.of(*w, *h)).collect();
    let subtitle = match (images, smaller.as_slice()) {
        ([(_, (w, h))], [(nw, nh)]) => format!("{w}×{h} → {nw}×{nh}"),
        ([(_, (w, h))], _) => format!("It's {w}×{h} already"),
        (all, made) if made.len() == all.len() => format!("All {}", all.len()),
        (all, []) => format!("All {} are that small already", all.len()),
        (all, made) => format!("{} of {}: the others are that small already", made.len(), all.len()),
    };
    let run = |format: &str| json!({ "command": "run", "args": { "paths": paths, "size": size.key(), "format": format } });
    let actions = if smaller.is_empty() {
        json!([])
    } else {
        json!([
            { "title": "Resize", "run": run("same"), "icon": "arrow.down.right.and.arrow.up.left" },
            { "title": "Resize as JPEG", "run": run("jpeg"), "icon": "photo" },
            { "title": "Resize as PNG", "run": run("png"), "icon": "photo" },
            { "title": "Resize as HEIC", "run": run("heic"), "icon": "photo" }
        ])
    };
    json!({
        "title": size.title(),
        "subtitle": subtitle,
        "icon": match size { Size::Percent(_) => "percent", Size::Height(_) => "arrow.up.and.down", Size::Fit(..) => "rectangle.dashed", Size::Width(_) => "arrow.left.and.right" },
        "color": if smaller.is_empty() { "gray" } else { "#0a84ff" },
        "actions": actions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_typed() {
        assert_eq!(Size::parse("50%"), Some(Size::Percent(50)));
        assert_eq!(Size::parse("1920"), Some(Size::Width(1920)));
        assert_eq!(Size::parse("w1920"), Some(Size::Width(1920)));
        assert_eq!(Size::parse("h600"), Some(Size::Height(600)));
        assert_eq!(Size::parse("600h"), Some(Size::Height(600)));
        assert_eq!(Size::parse("800x600"), Some(Size::Fit(800, 600)));
        assert_eq!(Size::parse("800 × 600"), Some(Size::Fit(800, 600)));
        for bad in ["", "big", "0", "150%", "100%", "x600", "800x", "-5"] {
            assert_eq!(Size::parse(bad), None, "{bad}");
        }
        for s in PRESETS.into_iter().chain([Size::Height(600), Size::Fit(800, 600)]) {
            assert_eq!(Size::parse(&s.key()), Some(s));
        }
    }

    #[test]
    fn sizes_only_smaller() {
        assert_eq!(Size::Percent(50).of(4032, 3024), Some((2016, 1512)));
        assert_eq!(Size::Width(1920).of(4032, 3024), Some((1920, 1440)));
        assert_eq!(Size::Height(600).of(4032, 3024), Some((800, 600)));
        // (A tall one, in a box: its height's what counts.)
        assert_eq!(Size::Fit(800, 800).of(3024, 4032), Some((600, 800)));
        assert_eq!(Size::Width(1920).of(1280, 720), None);
        assert_eq!(Size::Width(1280).of(1280, 720), None);
    }

    /// Photos resized, of all kinds: a folder of them, `RESIZE_IMAGES=/some/dir
    /// cargo test --release -- --ignored photos_resized`, each made 50% (and
    /// as JPEG), beside it.
    #[test]
    #[ignore]
    fn photos_resized() {
        let Some(dir) = std::env::var_os("RESIZE_IMAGES").map(PathBuf::from) else { return };
        let photos: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| !p.to_string_lossy().contains('×')).collect();
        for format in ["same", "jpeg"] {
            let (made, failed) = resize(&photos, Size::Percent(50), format);
            for out in &made {
                println!("{} {:?}", out.display(), crate::macos::shown_size(out));
            }
            assert!(failed.is_empty(), "{failed:?}");
        }
    }
}

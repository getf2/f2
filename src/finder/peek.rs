//! F2's own preview (Space): the file picked, shown at once over the files,
//! following the selection as it moves, for what F2 draws itself: images
//! (any ImageIO reads, HEIC and camera raw too), text and code, PDFs,
//! folders, and anything else as its icon, large. Video, audio and Office or
//! iWork documents go to macOS's Quick Look, as before (and the rest can,
//! from the preview). Each is read off the main thread, the ones on either
//! side too, and the last few are kept.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::Finder;
use super::chrome::TOOLBAR_HEIGHT;
use super::items::file_image;
use crate::listing::{Entry, human_size};
use crate::macos;
use crate::style::{SEMIBOLD, glass, symbol};

/// How a kind of file is shown.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Image,
    Pdf,
    Folder,
    /// Quick Look's to show (video, audio, documents only macOS draws).
    QuickLook,
    /// As text, if it reads as text; else its icon.
    Maybe,
}

const IMAGES: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "avif", "tif", "tiff", "bmp", "ico", "icns", "jp2", "psd", "tga", "exr", "hdr",
    "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "srw", "pef", "raw", "3fr", "erf", "kdc", "mos", "mrw", "x3f",
];

/// What macOS draws better: moving pictures and sound, documents with
/// layouts, models, fonts, web pages and vector art as they look.
const QUICK_LOOK: &[&str] = &[
    "mov", "mp4", "m4v", "avi", "mkv", "webm", "mpg", "mpeg", "3gp", "mts", "m2ts", "wmv", "flv", "mp3", "m4a", "m4b", "aac", "wav", "aiff",
    "aif", "flac", "ogg", "opus", "caf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pages", "numbers", "key", "rtf", "rtfd", "odt", "ods",
    "odp", "epub", "usdz", "obj", "stl", "dae", "scn", "reality", "ttf", "otf", "ttc", "woff", "woff2", "html", "htm", "webarchive", "svg",
];

pub(super) fn kind(entry: &Entry) -> Kind {
    if entry.link.is_some() {
        return Kind::QuickLook;
    }
    if entry.is_dir && !entry.is_bundle() {
        return Kind::Folder;
    }
    let ext = entry.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if IMAGES.contains(&ext.as_str()) {
        Kind::Image
    } else if ext == "pdf" {
        Kind::Pdf
    } else if QUICK_LOOK.contains(&ext.as_str()) {
        Kind::QuickLook
    } else {
        Kind::Maybe
    }
}

/// What a file looks like in the preview, as read.
pub(super) enum Look {
    Image(Arc<RenderImage>),
    /// Its first `TEXT_BYTES`, a line each (`more`: there's more after).
    Text { lines: Vec<String>, more: bool },
    /// Its first pages (`count` in all).
    Pdf { pages: Vec<Arc<RenderImage>>, count: usize },
    /// What's in it: how many, and the first of them (folders: true).
    Folder { count: usize, names: Vec<(String, bool)> },
    /// Nothing F2 draws: its icon, large, and Quick Look for more.
    Other,
}

/// How much of a text file is read.
const TEXT_BYTES: usize = 256 << 10;
const TEXT_LINES: usize = 5000;
/// A PDF's pages read at once, then (shown) up to this many.
const PDF_FIRST: usize = 2;
const PDF_PAGES: usize = 40;
/// Looks kept (the last ones seen, and those either side).
const KEPT: usize = 24;

/// Read what `path` looks like (`wide`: the room it has, in pixels). Waits:
/// not on the main thread.
pub(super) fn load(path: &Path, kind: Kind, wide: u32) -> Look {
    match kind {
        Kind::Image => macos::image_bitmap(path, wide).map_or(Look::Other, |image| Look::Image(Arc::new(image))),
        Kind::Pdf => match macos::pdf_pages(path, 0, PDF_FIRST, wide.min(1800)) {
            Some((count, pages)) if !pages.is_empty() => Look::Pdf { pages: pages.into_iter().map(Arc::new).collect(), count },
            _ => Look::Other,
        },
        Kind::Folder => folder(path),
        Kind::Maybe => text(path).unwrap_or(Look::Other),
        Kind::QuickLook => Look::Other,
    }
}

fn folder(path: &Path) -> Look {
    let mut names: Vec<(String, bool)> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then(|| (name, e.file_type().is_ok_and(|t| t.is_dir())))
        })
        .collect();
    names.sort_by_key(|(name, _)| name.to_lowercase());
    let count = names.len();
    names.truncate(400);
    Look::Folder { count, names }
}

/// A file as text, if that's what it is: no NULs, and next to nothing that
/// isn't UTF-8.
pub(super) fn text(path: &Path) -> Option<Look> {
    let mut file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.is_dir() {
        return None;
    }
    let mut bytes = Vec::with_capacity(TEXT_BYTES.min(1 << 16));
    file.by_ref().take(TEXT_BYTES as u64).read_to_end(&mut bytes).ok()?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return None;
    }
    let more = file.metadata().is_ok_and(|m| m.len() as usize > bytes.len());
    let text = String::from_utf8_lossy(&bytes);
    // (A character cut off at the end aside.)
    if text.matches('\u{fffd}').count() > 1 + text.len() / 200 {
        return None;
    }
    let lines: Vec<String> = text.lines().take(TEXT_LINES).map(|l| l.replace('\t', "    ")).collect();
    let more = more || text.lines().count() > TEXT_LINES;
    Some(Look::Text { lines, more })
}

/// Text's lines broken to fit `wide` characters (a line too long goes on
/// below, as an editor's soft wrap: after a space, where there's one past
/// halfway).
pub(super) fn wrapped(lines: &[String], wide: usize) -> Vec<SharedString> {
    let wide = wide.max(8);
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let mut start = 0;
        while chars.len() - start > wide {
            let edge = start + wide;
            let cut = (start + wide / 2..edge).rev().find(|&i| chars[i] == ' ').map_or(edge, |i| i + 1);
            out.push(chars[start..cut].iter().collect::<String>().trim_end().to_string().into());
            start = cut;
        }
        out.push(chars[start..].iter().collect::<String>().into());
    }
    out
}

/// The looks read, the newest last, and those being read.
#[derive(Default)]
pub(super) struct Cache {
    looks: HashMap<PathBuf, (i64, Arc<Look>)>,
    order: VecDeque<PathBuf>,
    reading: HashSet<PathBuf>,
}

impl Cache {
    fn get(&self, entry: &Entry) -> Option<Arc<Look>> {
        self.looks.get(&entry.path).filter(|(mtime, _)| *mtime == entry.mtime).map(|(_, look)| look.clone())
    }

    fn put(&mut self, path: PathBuf, mtime: i64, look: Look) {
        self.order.retain(|p| *p != path);
        self.order.push_back(path.clone());
        self.looks.insert(path, (mtime, Arc::new(look)));
        while self.order.len() > KEPT {
            if let Some(old) = self.order.pop_front() {
                self.looks.remove(&old);
            }
        }
    }
}

/// The preview, while it's up.
pub(super) struct Peek {
    /// What it shows.
    path: PathBuf,
    /// Text's lines as they fit, and at how many characters.
    lines: Option<(usize, Arc<Vec<SharedString>>)>,
    text_scroll: UniformListScrollHandle,
    scroll: ScrollHandle,
}

impl Peek {
    fn new(path: PathBuf) -> Peek {
        Peek { path, lines: None, text_scroll: UniformListScrollHandle::new(), scroll: ScrollHandle::new() }
    }
}

/// Text in the preview: its size, and a character's width (a monospaced
/// one's is 0.6 of it).
const TEXT_SIZE: f32 = 12.;
const LINE: f32 = 17.;
/// The preview's top (its name, what it is, the buttons), and the card's
/// edges in the window.
const HEADER: f32 = 54.;
const CARD_TOP: f32 = TOOLBAR_HEIGHT + 4.;
const CARD_BOTTOM: f32 = 12.;

impl Finder {
    /// Whether F2 shows what's picked itself (one item, not Quick Look's).
    pub(super) fn peeks_here(&self) -> bool {
        self.selection_paths().len() == 1 && self.selected_entry().is_some_and(|e| kind(e) != Kind::QuickLook)
    }

    pub(super) fn open_peek(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry() {
            self.peek = Some(Peek::new(entry.path.clone()));
            cx.notify();
        }
    }

    /// Put the preview away: whether it was up.
    pub(crate) fn close_peek(&mut self, cx: &mut Context<Self>) -> bool {
        let was = self.peek.take().is_some();
        if was {
            cx.notify();
        }
        was
    }

    /// Read what `entry` looks like, if it's not read (or being read),
    /// off the main thread.
    fn fetch_look(&mut self, entry: &Entry, wide: u32, cx: &mut Context<Self>) {
        let kind = kind(entry);
        if kind == Kind::QuickLook || self.peeks.get(entry).is_some() || !self.peeks.reading.insert(entry.path.clone()) {
            return;
        }
        let (path, mtime) = (entry.path.clone(), entry.mtime);
        cx.spawn(async move |this, cx| {
            let look = cx.background_executor().spawn({
                let path = path.clone();
                async move { load(&path, kind, wide) }
            });
            let look = look.await;
            // (A long PDF: its first pages shown, then the rest read.)
            let more_pages = matches!(&look, Look::Pdf { pages, count } if pages.len() < (*count).min(PDF_PAGES));
            this.update(cx, |this, cx| {
                this.peeks.reading.remove(&path);
                this.peeks.put(path.clone(), mtime, look);
                cx.notify();
            })
            .ok();
            if more_pages {
                let rest = cx.background_executor().spawn({
                    let path = path.clone();
                    async move { macos::pdf_pages(&path, PDF_FIRST, PDF_PAGES - PDF_FIRST, wide.min(1800)) }
                });
                if let Some((count, rest)) = rest.await {
                    this.update(cx, |this, cx| {
                        let first = this.peeks.looks.get(&path).and_then(|(_, look)| match &**look {
                            Look::Pdf { pages, .. } => Some(pages.clone()),
                            _ => None,
                        });
                        if let Some(mut pages) = first {
                            pages.extend(rest.into_iter().map(Arc::new));
                            this.peeks.put(path, mtime, Look::Pdf { pages, count });
                            cx.notify();
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// The preview over the files, if it's up: what's picked, read (and the
    /// items either side read too).
    pub(super) fn render_peek(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.peek.as_ref()?;
        let Some(entry) = self.selected_entry().cloned() else {
            self.peek = None;
            return None;
        };
        let palette = self.palette;
        let scale = window.scale_factor();
        let card_w = (self.toolbar_width - 32.).max(200.);
        let viewport = window.viewport_size();
        let card_h = (f32::from(viewport.height) - CARD_TOP - CARD_BOTTOM).max(160.);
        let wide = ((card_w.max(card_h) * scale) as u32).clamp(400, 4096);
        let peek = self.peek.as_mut()?;
        if peek.path != entry.path {
            *peek = Peek::new(entry.path.clone());
        }
        self.fetch_look(&entry, wide, cx);
        if let Some(ix) = self.selected {
            for near in [ix + 1, ix.wrapping_sub(1)] {
                if let Some(near) = self.rows.get(near).cloned() {
                    self.fetch_look(&near, wide, cx);
                }
            }
        }
        let look = self.peeks.get(&entry);
        let kind = kind(&entry);
        // (Text: its lines, as they fit the card now.)
        if let Some(Look::Text { lines, .. }) = look.as_deref() {
            let fits = ((card_w - 40.) / (TEXT_SIZE * 0.6)).floor() as usize;
            let peek = self.peek.as_mut()?;
            if peek.lines.as_ref().is_none_or(|(at, _)| *at != fits) {
                peek.lines = Some((fits, Arc::new(wrapped(lines, fits))));
            }
        }
        let header = self.peek_header(&entry, kind, window, cx);
        let body = match look.as_deref() {
            None => self.peek_icon(&entry, "", window, cx),
            Some(Look::Image(image)) => {
                // (Fitted here: GPUI sizes an image by its width alone.
                // Never bigger than it is.)
                let pixels = image.size(0);
                let (iw, ih) = (pixels.width.0 as f32 / scale, pixels.height.0 as f32 / scale);
                let (room_w, room_h) = (card_w - 24., card_h - HEADER - 24.);
                let fit = (room_w / iw.max(1.)).min(room_h / ih.max(1.)).min(1.);
                div().size_full().flex().items_center().justify_center().child(img(image.clone()).w(px(iw * fit)).h(px(ih * fit))).into_any_element()
            }
            Some(Look::Text { more, .. }) => {
                let lines = self.peek.as_ref().and_then(|p| p.lines.as_ref()).map(|(_, l)| l.clone()).unwrap_or_default();
                let scroll = self.peek.as_ref().map(|p| p.text_scroll.clone())?;
                let n = lines.len();
                let shown = lines.clone();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list("peek-text", n, move |range: Range<usize>, _, _| {
                            range.map(|i| div().h(px(LINE)).whitespace_nowrap().child(shown[i].clone()).into_any_element()).collect()
                        })
                        .track_scroll(&scroll)
                        .flex_1()
                        .px_5()
                        .py_3()
                        .font_family("Menlo")
                        .text_size(px(TEXT_SIZE))
                        .text_color(palette.label),
                    )
                    .when(*more, |d| {
                        d.child(div().px_5().py_1().border_t_1().border_color(palette.separator).text_size(px(11.)).text_color(palette.tertiary).child("The rest of it isn't shown here: Open shows all of it"))
                    })
                    .into_any_element()
            }
            Some(Look::Pdf { pages, count }) => {
                let scroll = self.peek.as_ref().map(|p| p.scroll.clone())?;
                let page_w = card_w - 48.;
                let pages: Vec<AnyElement> = pages
                    .iter()
                    .map(|page| {
                        let size = page.size(0);
                        let h = page_w * size.height.0 as f32 / size.width.0.max(1) as f32;
                        img(page.clone()).w(px(page_w)).h(px(h)).flex_shrink_0().rounded_sm().shadow_sm().into_any_element()
                    })
                    .collect();
                let shown = pages.len();
                div()
                    .id("peek-pdf")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .px_6()
                    .py_4()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .children(pages)
                    .when(*count > shown, |d| {
                        d.child(div().pb_2().text_size(px(11.)).text_color(palette.tertiary).child(format!("{shown} of {count} pages shown: Open shows all of it")))
                    })
                    .into_any_element()
            }
            Some(Look::Folder { count, names }) => {
                let rows: Vec<AnyElement> = names
                    .iter()
                    .map(|(name, is_dir)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .py(px(3.))
                            .text_size(px(12.))
                            .child(symbol(if *is_dir { "folder.fill" } else { "doc" }, 12., SEMIBOLD, if *is_dir { palette.accent } else { palette.secondary }, window))
                            .child(div().truncate().child(name.clone()))
                            .into_any_element()
                    })
                    .collect();
                let about = match count {
                    0 => "Empty".to_string(),
                    1 => "1 item".to_string(),
                    n => format!("{n} items"),
                };
                let more = count.saturating_sub(names.len());
                div()
                    .size_full()
                    .flex()
                    .child(div().w(px(220.)).flex_shrink_0().flex().flex_col().items_center().justify_center().gap_2().child(self.peek_big_icon(&entry, 128., window)).child(div().text_color(palette.secondary).child(about)))
                    .child(
                        div()
                            .id("peek-folder")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .py_4()
                            .pr_4()
                            .children(rows)
                            .when(more > 0, |d| d.child(div().pt_1().text_size(px(11.)).text_color(palette.tertiary).child(format!("and {more} more")))),
                    )
                    .into_any_element()
            }
            Some(Look::Other) => {
                let note = if kind == Kind::QuickLook { "Quick Look shows this one" } else { "No preview of this one here: Quick Look may have one" };
                self.peek_icon(&entry, note, window, cx)
            }
        };
        Some(
            div()
                .id("peek")
                .absolute()
                .top(px(CARD_TOP))
                .left(px(16.))
                .right(px(16.))
                .bottom(px(CARD_BOTTOM))
                .occlude()
                .flex()
                .flex_col()
                .rounded(px(14.))
                .bg(palette.background)
                .border_1()
                .border_color(palette.glass_border)
                .shadow_2xl()
                .overflow_hidden()
                .child(header)
                // (Pinned to the room left: what's in it sized to that, an
                // image fitted, not running off the bottom.)
                .child(div().flex_1().min_h_0().relative().child(div().absolute().inset_0().child(body)))
                .into_any_element(),
        )
    }

    /// The preview's top: the item, what it is, Open, Quick Look, close.
    fn peek_header(&mut self, entry: &Entry, kind: Kind, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let icon_px = (18. * window.scale_factor()).round() as u32;
        let shown = self.icons.get(entry, icon_px);
        let size = match (entry.is_dir, self.sizes.get(&entry.path)) {
            (true, Some(bytes)) => human_size(*bytes).into(),
            _ => entry.size_label(),
        };
        let about = [entry.kind_label(), size, entry.mtime_label()].into_iter().filter(|s| !s.is_empty() && s.as_ref() != "--").collect::<Vec<_>>().join(" · ");
        let button = |id: &'static str, label: &'static str| glass(26., &palette).id(id).px_3().text_size(px(12.)).child(label);
        let (open, look) = (entry.clone(), entry.path.clone());
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(palette.separator)
            .child(file_image(shown, entry, 18., palette.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::SEMIBOLD).truncate().child(entry.name.clone()))
                    .child(div().text_size(px(11.)).text_color(palette.secondary).truncate().child(about)),
            )
            .when(kind != Kind::Folder, |d| {
                d.child(button("peek-quick-look", "Quick Look").on_click(cx.listener(move |this, _, _, cx| {
                    this.peek = None;
                    this.ql_shown = vec![look.clone()];
                    let paths = vec![look.clone()];
                    cx.spawn(async move |_, _| macos::quicklook_show(&paths, 0)).detach();
                    cx.notify();
                })))
            })
            .child(button("peek-open", "Open").on_click(cx.listener(move |this, _, window, cx| {
                this.peek = None;
                this.open_entries(vec![open.clone()], window, cx);
            })))
            .child(
                div()
                    .id("peek-close")
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .hover(|d| d.bg(palette.glass_selected))
                    .child(symbol("xmark", 11., SEMIBOLD, palette.secondary, window))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_peek(cx);
                    })),
            )
            .into_any_element()
    }

    /// A big icon (a thumbnail where there is one).
    fn peek_big_icon(&mut self, entry: &Entry, size: f32, window: &mut Window) -> AnyElement {
        let shown = self.icons.get(entry, (size * window.scale_factor()).round() as u32);
        file_image(shown, entry, size, self.palette.accent)
    }

    /// What there's no drawing of: its icon, large, and a word on it.
    fn peek_icon(&mut self, entry: &Entry, note: &'static str, window: &mut Window, _: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(self.peek_big_icon(entry, 256., window))
            .when(!note.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(palette.tertiary).child(note)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Look, text, wrapped};

    #[test]
    fn long_lines_go_on_below() {
        let lines = vec!["short".to_string(), "abcdefghij".to_string(), "héllo wörld".to_string(), "".to_string()];
        let out: Vec<String> = wrapped(&lines, 8).into_iter().map(|s| s.to_string()).collect();
        assert_eq!(out, ["short", "abcdefgh", "ij", "héllo", "wörld", ""], "after a space where there's one; else where it fits");
    }

    #[test]
    fn text_or_not() {
        let dir = std::env::temp_dir().join(format!("f2-peek-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (code, binary) = (dir.join("main"), dir.join("blob"));
        std::fs::write(&code, "fn main() {\n\tprintln!(\"hi\");\n}\n").unwrap();
        std::fs::write(&binary, [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
        let read = text(&code);
        let not = text(&binary);
        std::fs::remove_dir_all(&dir).unwrap();
        match read {
            Some(Look::Text { lines, more }) => {
                assert_eq!(lines, ["fn main() {", "    println!(\"hi\");", "}"]);
                assert!(!more);
            }
            _ => panic!("read as text"),
        }
        assert!(not.is_none(), "a binary isn't text");
    }
}

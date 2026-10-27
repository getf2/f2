//! Customize Folder, as Finder 26 has it: a folder's color (a color tag: on
//! macOS 26 a tagged folder takes its tag's color) and a symbol or an emoji
//! on it (the `com.apple.icon.folder#S` attribute, `{"sym":…}` or
//! `{"emoji":…}`, with FinderInfo's custom-icon flag set, as Finder writes
//! them). The sheet shows the folder as it'll look while you choose.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::Finder;
use super::menu::TAGS;
use super::sheets::{Sheet, button, buttons, card};
use crate::macos;
use crate::style::{REGULAR, symbol, tag_color, tag_rgb};

const ATTRIBUTE: &str = "com.apple.icon.folder#S";
const FINDER_INFO: &str = "com.apple.FinderInfo";

/// What's on a folder.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Mark {
    Emoji(String),
    Symbol(String),
}

/// The symbols to choose from (SF Symbols, as Finder offers them: things
/// a folder might hold).
const SYMBOLS: &[&str] = &[
    "star.fill",
    "heart.fill",
    "bookmark.fill",
    "flag.fill",
    "bolt.fill",
    "flame.fill",
    "sparkles",
    "leaf.fill",
    "sun.max.fill",
    "moon.fill",
    "cloud.fill",
    "globe.americas.fill",
    "house.fill",
    "briefcase.fill",
    "graduationcap.fill",
    "book.fill",
    "doc.text.fill",
    "tray.full.fill",
    "archivebox.fill",
    "shippingbox.fill",
    "cart.fill",
    "creditcard.fill",
    "banknote.fill",
    "gift.fill",
    "photo.fill",
    "camera.fill",
    "film.fill",
    "music.note",
    "headphones",
    "gamecontroller.fill",
    "paintbrush.fill",
    "pencil",
    "hammer.fill",
    "wrench.and.screwdriver.fill",
    "terminal.fill",
    "chevron.left.forwardslash.chevron.right",
    "person.fill",
    "person.2.fill",
    "airplane",
    "car.fill",
    "lock.fill",
    "key.fill",
    "clock.fill",
    "calendar",
    "checkmark.circle.fill",
    "exclamationmark.triangle.fill",
    "trash.fill",
    "arrow.down.circle.fill",
];

/// Symbols a row in the sheet.
const PER_ROW: usize = 12;

/// The preview's side, in points.
const PREVIEW: f32 = 96.;

fn c(path: &Path) -> Option<CString> {
    CString::new(path.as_os_str().as_encoded_bytes()).ok()
}

fn get(path: &Path, name: &str) -> Option<Vec<u8>> {
    let (path, name) = (c(path)?, CString::new(name).ok()?);
    let size = unsafe { libc::getxattr(path.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0, 0, 0) };
    if size < 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    let read = unsafe { libc::getxattr(path.as_ptr(), name.as_ptr(), data.as_mut_ptr().cast(), data.len(), 0, 0) };
    (read >= 0).then(|| {
        data.truncate(read as usize);
        data
    })
}

fn set(path: &Path, name: &str, value: &[u8]) -> std::io::Result<()> {
    let (Some(path), Ok(name)) = (c(path), CString::new(name)) else { return Err(std::io::ErrorKind::InvalidInput.into()) };
    match unsafe { libc::setxattr(path.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0, 0) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

fn remove(path: &Path, name: &str) -> std::io::Result<()> {
    let (Some(path), Ok(name)) = (c(path), CString::new(name)) else { return Err(std::io::ErrorKind::InvalidInput.into()) };
    match unsafe { libc::removexattr(path.as_ptr(), name.as_ptr(), 0) } {
        0 => Ok(()),
        _ => match std::io::Error::last_os_error() {
            e if e.raw_os_error() == Some(libc::ENOATTR) => Ok(()),
            e => Err(e),
        },
    }
}

/// The symbol or emoji on a folder, if it has one.
pub(crate) fn mark(path: &Path) -> Option<Mark> {
    let data = get(path, ATTRIBUTE)?;
    let json: serde_json::Value = serde_json::from_slice(&data).ok()?;
    if let Some(emoji) = json["emoji"].as_str() {
        return Some(Mark::Emoji(emoji.to_string()));
    }
    json["sym"].as_str().map(|s| Mark::Symbol(s.to_string()))
}

/// Put a symbol or an emoji on a folder (or, for None, take it off), as
/// Finder does it: the attribute, and FinderInfo's "has a custom icon"
/// (bit 0x04 of its ninth byte), kept for an old-style custom icon (an
/// "Icon\r" file inside).
pub(crate) fn set_mark(path: &Path, to: Option<&Mark>) -> std::io::Result<()> {
    let mut info = get(path, FINDER_INFO).filter(|i| i.len() == 32).unwrap_or_else(|| vec![0; 32]);
    match to {
        Some(mark) => {
            let json = match mark {
                Mark::Emoji(emoji) => serde_json::json!({ "emoji": emoji }),
                Mark::Symbol(name) => serde_json::json!({ "sym": name }),
            };
            set(path, ATTRIBUTE, json.to_string().as_bytes())?;
            info[8] |= 0x04;
            set(path, FINDER_INFO, &info)
        }
        None => {
            remove(path, ATTRIBUTE)?;
            if info[8] & 0x04 != 0 && !path.join("Icon\r").exists() {
                info[8] &= !0x04;
                if info.iter().all(|b| *b == 0) { remove(path, FINDER_INFO) } else { set(path, FINDER_INFO, &info) }
            } else {
                Ok(())
            }
        }
    }
}

/// A folder's color: its first color tag's (Finder's own, by name, else
/// any with a color).
fn color_of(path: &Path) -> Option<u8> {
    let tags = macos::tags(path);
    tags.iter().find_map(|(name, _)| TAGS.iter().find(|(n, _)| n == name).map(|(_, c)| *c)).or_else(|| tags.iter().map(|(_, c)| *c).find(|c| *c != 0))
}

/// The folder as it'll look, drawn as macOS draws it: a scratch folder
/// with the symbol or emoji on it (a new one each time, as icons are kept by
/// path), in the color picked (when folders take their tags' colors).
fn preview(color: Option<u8>, mark: Option<&Mark>) -> Option<Arc<RenderImage>> {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("f2-folder-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let folder = dir.join(N.fetch_add(1, Ordering::Relaxed).to_string());
    std::fs::create_dir_all(&folder).ok()?;
    if mark.is_some() {
        set_mark(&folder, mark).ok()?;
    }
    let px = (PREVIEW * 2.) as u32;
    let image = match color.and_then(tag_rgb) {
        Some(rgb) if macos::tag_tinting() => macos::folder_icon_tinted(&folder, px, rgb),
        _ => macos::file_icon(&folder, px),
    };
    image.map(Arc::new)
}

/// Whether a folder can be customized: a plain folder, not a package, nor
/// macOS's own (Finder won't for Applications, Library, System, Users).
pub(super) fn customizable(path: &Path, is_dir: bool, bundle: bool) -> bool {
    is_dir && !bundle && !matches!(path.to_str(), Some("/" | "/Applications" | "/Library" | "/System" | "/Users")) && !path.starts_with("/System")
}

/// The sheet: which folders, and what's picked so far.
pub(super) struct Customize {
    folders: Vec<PathBuf>,
    /// A tag color's number (see `menu::TAGS`), or None.
    color: Option<u8>,
    mark: Option<Mark>,
    emoji: Entity<InputState>,
    /// As they were, to change only what's changed.
    was: (Option<u8>, Option<Mark>),
    /// How it'll look (see `preview`).
    shown: Option<Arc<RenderImage>>,
    _subscription: Subscription,
}

impl Finder {
    pub(super) fn open_customize(&mut self, folders: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(first) = folders.first() else { return };
        let (color, mark) = (color_of(first), mark(first));
        let typed = match &mark {
            Some(Mark::Emoji(e)) => e.clone(),
            _ => String::new(),
        };
        let emoji = cx.new(|cx| InputState::new(window, cx).placeholder("Type an emoji").default_value(typed));
        let subscription = cx.subscribe_in(&emoji, window, |this, input, event: &InputEvent, window, cx| match event {
            InputEvent::Change => {
                let text: String = input.read(cx).value().trim().chars().take(16).collect();
                if let Some(Sheet::Customize(c)) = &mut this.overlay {
                    match (&c.mark, text.is_empty()) {
                        (_, false) => c.mark = Some(Mark::Emoji(text)),
                        (Some(Mark::Emoji(_)), true) => c.mark = None,
                        _ => {}
                    }
                }
                this.redraw_preview(cx);
            }
            InputEvent::PressEnter { .. } => this.finish_customize(window, cx),
            _ => {}
        });
        let shown = preview(color, mark.as_ref());
        self.overlay = Some(Sheet::Customize(Customize { folders, color, mark: mark.clone(), emoji, was: (color, mark), shown, _subscription: subscription }));
        cx.notify();
    }

    /// The preview, drawn again for what's picked now.
    fn redraw_preview(&mut self, cx: &mut Context<Self>) {
        let Some(Sheet::Customize(c)) = &mut self.overlay else { return };
        let old = std::mem::replace(&mut c.shown, preview(c.color, c.mark.as_ref()));
        if let Some(old) = old {
            cx.drop_image(old, None);
        }
        cx.notify();
    }

    fn set_customize(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Customize)) {
        let Some(Sheet::Customize(c)) = &mut self.overlay else { return };
        f(c);
        // (A symbol picked: the emoji typed goes.)
        if matches!(c.mark, Some(Mark::Symbol(_)) | None) && !c.emoji.read(cx).value().is_empty() {
            c.emoji.update(cx, |i, cx| i.set_value("", window, cx));
        }
        self.redraw_preview(cx);
    }

    /// Done (or Return): the color and the symbol or emoji, on each folder.
    pub(super) fn finish_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Sheet::Customize(c)) = self.overlay.take() else { return };
        window.focus(&self.focus, cx);
        cx.notify();
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("f2-folder-preview-{}", std::process::id())));
        if let Some(shown) = c.shown.clone() {
            cx.drop_image(shown, None);
        }
        let (color_changed, mark_changed) = (c.color != c.was.0, c.mark != c.was.1);
        for folder in &c.folders {
            if color_changed {
                // Finder's color tags swapped; the folder's other tags kept.
                let mut names: Vec<String> = macos::tags(folder).into_iter().map(|(n, _)| n).filter(|n| !TAGS.iter().any(|(t, _)| t == n)).collect();
                if let Some((name, _)) = c.color.and_then(|color| TAGS.iter().find(|(_, n)| *n == color)) {
                    names.push(name.to_string());
                }
                if let Err(e) = macos::set_tags(folder, &names) {
                    return self.show_notice(e, cx);
                }
            }
            if mark_changed {
                if let Err(e) = set_mark(folder, c.mark.as_ref()) {
                    return self.show_notice(format!("Couldn't customize “{}”: {e}", folder.file_name().unwrap_or_default().to_string_lossy()), cx);
                }
                self.icons.forget(folder);
            }
        }
        if color_changed || mark_changed {
            self.pending_select = c.folders.clone();
            let parents: Vec<PathBuf> = c.folders.iter().filter_map(|f| f.parent().map(Path::to_path_buf)).collect();
            if self.flat() {
                for entry in self.rows.iter_mut().filter(|e| c.folders.contains(&e.path)) {
                    entry.tags = macos::tags(&entry.path).into_iter().map(|(n, c)| (n.into(), c)).collect();
                }
            } else {
                self.reload_folders(&parents, cx);
            }
        }
    }

    pub(super) fn render_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let palette = self.palette;
        let Some(Sheet::Customize(c)) = &self.overlay else { return div() };
        let name = |p: &Path| p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let title = match c.folders.as_slice() {
            [one] => format!("Customize “{}”", name(one)),
            all => format!("Customize {} Folders", all.len()),
        };
        // The folder as it'll look, as macOS draws it.
        let preview = div()
            .size(px(PREVIEW))
            .flex_shrink_0()
            .children(c.shown.clone().map(|image| img(image).size(px(PREVIEW)).object_fit(ObjectFit::Contain)));
        let label = |text: &'static str| div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.tertiary).child(text);
        // Colors: none, then Finder's.
        let swatch = |id: usize, color: Option<u8>, cx: &mut Context<Self>| {
            let on = c.color == color;
            let fill = color.and_then(tag_color);
            div()
                .id(("folder-color", id))
                .size(px(24.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .border_2()
                .border_color(if on { palette.label.opacity(0.85) } else { transparent_black() })
                .child(
                    div()
                        .size(px(18.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(fill, |d, fill| d.bg(fill))
                        .when(fill.is_none(), |d| d.border_1().border_color(palette.separator).child(symbol("slash.circle", 12., REGULAR, palette.secondary, window))),
                )
                .role(Role::Button)
                .aria_label(match color.and_then(|n| TAGS.iter().find(|(_, c)| *c == n)) {
                    Some((name, _)) => name.to_string(),
                    None => "No Color".into(),
                })
                .on_click(cx.listener(move |this, _, window, cx| this.set_customize(window, cx, |c| c.color = color)))
        };
        let mut colors = vec![swatch(0, None, cx).into_any_element()];
        for (i, (_, n)) in TAGS.iter().enumerate() {
            colors.push(swatch(i + 1, Some(*n), cx).into_any_element());
        }
        // Symbols: none, then the rest, a dozen a row.
        let cell = |id: usize, sym: Option<&'static str>, cx: &mut Context<Self>| {
            let on = match (&c.mark, sym) {
                (Some(Mark::Symbol(s)), Some(sym)) => s == sym,
                (None, None) => true,
                _ => false,
            };
            div()
                .id(("folder-symbol", id))
                .size(px(28.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(on, |d| d.bg(palette.accent.opacity(0.22)))
                .when(!on, |d| d.hover(|s| s.bg(palette.glass_selected)))
                .child(match sym {
                    Some(s) => symbol(s, 14., REGULAR, if on { palette.accent } else { palette.label }, window),
                    None => symbol("nosign", 13., REGULAR, palette.secondary, window),
                })
                .role(Role::Button)
                .aria_label(sym.unwrap_or("No Symbol").replace('.', " "))
                .on_click(cx.listener(move |this, _, window, cx| this.set_customize(window, cx, |c| c.mark = sym.map(|s| Mark::Symbol(s.to_string())))))
        };
        let mut cells = vec![cell(0, None, cx).into_any_element()];
        for (i, s) in SYMBOLS.iter().enumerate() {
            cells.push(cell(i + 1, Some(s), cx).into_any_element());
        }
        let mut grid = div().flex().flex_col().gap(px(2.));
        while !cells.is_empty() {
            let row: Vec<AnyElement> = cells.drain(..cells.len().min(PER_ROW + 1)).collect();
            grid = grid.child(div().flex().gap(px(2.)).children(row));
        }
        let emoji_field = c.emoji.clone();
        let picker = div()
            .id("emoji-picker")
            .h(px(24.))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded_md()
            .text_size(px(12.))
            .text_color(palette.secondary)
            .cursor_pointer()
            .hover(|s| s.bg(palette.glass_selected))
            .role(Role::Button)
            .aria_label("Emoji & Symbols")
            .child(symbol("face.smiling", 13., REGULAR, palette.secondary, window))
            .child("Emoji & Symbols")
            .on_click(cx.listener(move |_, _, window, cx| {
                // (Into the field: the Character Viewer types into what has the keyboard.)
                emoji_field.update(cx, |i, cx| i.focus(window, cx));
                macos::emoji_palette();
            }));
        card(&palette)
            .w(px(470.))
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).truncate().child(title))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(preview)
                    .child(div().flex().flex_col().gap_2().child(label("Color")).child(div().flex().gap(px(4.)).children(colors))),
            )
            .child(div().flex().flex_col().gap_2().child(label("Symbol")).child(grid))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label("Or an Emoji"))
                    .child(div().flex().items_center().gap_2().child(div().w(px(150.)).child(Input::new(&c.emoji).small())).child(picker)),
            )
            .child(
                buttons()
                    .child(button("customize-cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx)))
                    .child(button("customize-done", "Done", true, &palette, cx, |this, window, cx| this.finish_customize(window, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Mark, mark, set_mark};

    #[test]
    fn marks() {
        let dir = std::env::temp_dir().join(format!("f2-customize-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert_eq!(mark(&dir), None);
        set_mark(&dir, Some(&Mark::Emoji("🌴".into()))).unwrap();
        assert_eq!(mark(&dir), Some(Mark::Emoji("🌴".into())));
        set_mark(&dir, Some(&Mark::Symbol("camera.fill".into()))).unwrap();
        assert_eq!(mark(&dir), Some(Mark::Symbol("camera.fill".into())));
        set_mark(&dir, None).unwrap();
        assert_eq!(mark(&dir), None);
        set_mark(&dir, None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

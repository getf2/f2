//! Get Info, in windows of its own as Finder's (several at once): what an
//! item is, its tags, More Info, its name and extension, comments, the app
//! that opens it (and its kind), its preview, and who may do what with it.
//! ⌥⌘I's Inspector is one such window that follows the selection.

use std::ffi::CStr;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use futures::StreamExt;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use crate::finder::file_image;
use crate::icons::Icons;
use crate::listing::{Entry, count, date_label, human_size, thousands};
use crate::macos::{self, MenuItem};
use crate::style::{Palette, SEMIBOLD, glass, symbol};

/// The Inspector window, if open.
#[derive(Default)]
struct Inspector(Option<WeakEntity<InfoWindow>>);

impl Global for Inspector {}

pub fn inspector_open(cx: &App) -> bool {
    cx.try_global::<Inspector>().is_some_and(|i| i.0.as_ref().is_some_and(|w| w.upgrade().is_some()))
}

/// Get Info on items: a window each (a summary for many).
pub fn open(entries: Vec<Entry>, cx: &mut App) {
    if entries.len() > 5 {
        open_window(entries, false, cx);
    } else {
        for entry in entries {
            open_window(vec![entry], false, cx);
        }
    }
}

/// ⌥⌘I: the Inspector, or close it.
pub fn toggle_inspector(entries: Vec<Entry>, cx: &mut App) {
    if let Some(window) = cx.try_global::<Inspector>().and_then(|i| i.0.clone()).and_then(|w| w.upgrade()) {
        window.update(cx, |w, _| w.close = true);
        cx.set_global(Inspector(None));
        cx.refresh_windows();
        return;
    }
    if let Some(entity) = open_window(entries, true, cx) {
        cx.set_global(Inspector(Some(entity.downgrade())));
    }
}

/// The selection changed: the Inspector shows it.
pub fn follow_selection(entries: Vec<Entry>, cx: &mut App) {
    let Some(window) = cx.try_global::<Inspector>().and_then(|i| i.0.clone()).and_then(|w| w.upgrade()) else { return };
    let same = window.read(cx).entries.iter().map(|e| &e.path).eq(entries.iter().map(|e| &e.path));
    if !same && !entries.is_empty() {
        window.update(cx, |w, cx| w.show(entries, cx));
    }
}

fn open_window(entries: Vec<Entry>, follows: bool, cx: &mut App) -> Option<Entity<InfoWindow>> {
    let offset = px(30. * (cx.windows().len() % 6) as f32);
    let bounds = Bounds { origin: point(px(120.) + offset, px(120.) + offset), size: size(px(320.), px(640.)) };
    let title = match entries.as_slice() {
        [one] => format!("{} Info", one.name),
        many => format!("{} Info", count(many.len(), "item")),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some(if follows { "Inspector".into() } else { title.into() }), ..Default::default() }),
        is_resizable: true,
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| InfoWindow::new(entries, follows, window, cx))).ok().map(|(_, e)| e)
}

pub struct InfoWindow {
    focus: FocusHandle,
    entries: Vec<Entry>,
    follows: bool,
    /// Bytes and items in a folder (or several items), once added up.
    size: Option<(u64, usize)>,
    cancel: Arc<AtomicBool>,
    name: Entity<InputState>,
    comment: Entity<InputState>,
    locked: bool,
    hide_ext: bool,
    created: Option<i64>,
    app: Option<PathBuf>,
    /// The app chosen for "Change All".
    chosen_app: Option<PathBuf>,
    more: Vec<(String, String)>,
    permissions: Vec<(String, String)>,
    icons: Icons,
    palette: Palette,
    notice: Option<SharedString>,
    /// Asked to close (the Inspector toggled off).
    close: bool,
    handle: Option<AnyWindowHandle>,
    _subscriptions: Vec<Subscription>,
}

impl InfoWindow {
    fn new(entries: Vec<Entry>, follows: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        let comment = cx.new(|cx| InputState::new(window, cx).placeholder("Comments"));
        let subscriptions = vec![
            cx.subscribe_in(&name, window, |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let text = input.read(cx).value().trim().to_string();
                    this.rename(text, cx);
                }
            }),
            cx.subscribe_in(&comment, window, |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && let [entry] = this.entries.as_slice()
                {
                    let text = input.read(cx).value().to_string();
                    if let Err(e) = macos::set_comment(&entry.path, &text) {
                        this.notice = Some(e.into());
                        cx.notify();
                    }
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.palette = Palette::for_window(window);
                cx.notify();
            }),
        ];
        let (icons, mut finished) = Icons::new();
        cx.spawn(async move |this, cx| {
            while let Some(done) = finished.next().await {
                let alive = this.update(cx, |this, cx| {
                    for evicted in this.icons.finish(done) {
                        cx.drop_image(evicted, None);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut this = InfoWindow {
            focus,
            entries: Vec::new(),
            follows,
            size: None,
            cancel: Arc::new(AtomicBool::new(false)),
            name,
            comment,
            locked: false,
            hide_ext: false,
            created: None,
            app: None,
            chosen_app: None,
            more: Vec::new(),
            permissions: Vec::new(),
            icons,
            palette: Palette::for_window(window),
            notice: None,
            close: false,
            handle: Some(window.window_handle()),
            _subscriptions: subscriptions,
        };
        this.show_now(entries, Some(window), cx);
        this
    }

    fn show(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        self.show_now(entries, None, cx);
    }

    /// Look everything up for these items (slow parts off the main thread).
    fn show_now(&mut self, entries: Vec<Entry>, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.entries = entries;
        self.size = None;
        self.more.clear();
        self.notice = None;
        self.chosen_app = None;
        let Some(first) = self.entries.first().cloned() else { return };
        let one = self.entries.len() == 1;
        if one {
            self.locked = macos::is_locked(&first.path);
            self.hide_ext = macos::has_hidden_extension(&first.path);
            self.created = std::fs::metadata(&first.path)
                .and_then(|m| m.created())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            self.app = (!first.is_dir).then(|| macos::default_app(&first.path)).flatten();
            self.permissions = permissions(&first.path);
            let (name, comment) = (first.name.to_string(), macos::comment(&first.path));
            if let Some(window) = window {
                self.name.update(cx, |i, cx| i.set_value(name, window, cx));
                self.comment.update(cx, |i, cx| i.set_value(comment, window, cx));
            } else if let Some(handle) = self.handle {
                // (Following the selection: from another window's update.)
                let (n, c) = (self.name.clone(), self.comment.clone());
                cx.defer(move |cx| {
                    handle
                        .update(cx, |_, window, cx| {
                            n.update(cx, |i, cx| i.set_value(name, window, cx));
                            c.update(cx, |i, cx| i.set_value(comment, window, cx));
                        })
                        .ok();
                });
            }
        }
        // More Info, and sizes to add up.
        let paths: Vec<PathBuf> = self.entries.iter().map(|e| e.path.clone()).collect();
        let needs_size = !one || first.is_dir;
        let flag = self.cancel.clone();
        let search = cx.global::<crate::browser::Shared>().search.clone();
        cx.spawn(async move |this, cx| {
            let counting = flag.clone();
            let (more, size) = cx
                .background_executor()
                .spawn(async move {
                    let more = if paths.len() == 1 { macos::more_info(&paths[0]) } else { Vec::new() };
                    // From the index if it holds the folder, else a walk.
                    let folder_size = |dir: &std::path::Path| {
                        search
                            .as_ref()
                            .and_then(|s| s.folder_sizes(&[dir.to_path_buf()]).pop().flatten())
                            .unwrap_or_else(|| crate::fileops::folder_size(dir, &counting))
                    };
                    let size = needs_size.then(|| match paths.as_slice() {
                        [folder] => folder_size(folder),
                        all => crate::fileops::total_size(all, folder_size),
                    });
                    (more, size)
                })
                .await;
            this.update(cx, |this, cx| {
                if !flag.load(Ordering::Relaxed) {
                    this.more = more;
                    this.size = size;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn rename(&mut self, name: String, cx: &mut Context<Self>) {
        let [entry] = self.entries.as_slice() else { return };
        if name.is_empty() || name == entry.name.as_ref() {
            return;
        }
        let new = entry.path.with_file_name(&name);
        if name.contains(['/', ':']) || (new.exists() && !crate::fileops::same_file(&entry.path, &new)) {
            self.notice = Some("That name can't be used here.".into());
            return cx.notify();
        }
        match std::fs::rename(&entry.path, &new) {
            Ok(()) => {
                if let Some(updated) = Entry::from_path(new) {
                    self.entries = vec![updated];
                }
            }
            Err(e) => self.notice = Some(e.to_string().into()),
        }
        cx.notify();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let entries: Vec<Entry> = self.entries.iter().filter_map(|e| Entry::from_path(e.path.clone())).collect();
        self.show(entries, cx);
    }

    fn choose_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [entry] = self.entries.as_slice() else { return };
        let apps = macos::apps_for(&entry.path);
        let items: Vec<MenuItem> = apps
            .iter()
            .enumerate()
            .map(|(i, app)| MenuItem {
                title: Some(app.file_stem().unwrap_or_default().to_string_lossy().into_owned()),
                tag: i as i32,
                enabled: true,
                icon: Some(app.clone()),
                ..Default::default()
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let Some(tag) = macos::popup_menu(&items) else { return };
            this.update(cx, |this, cx| {
                this.chosen_app = apps.get(tag as usize).cloned();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn header(&mut self, window: &mut Window) -> AnyElement {
        let palette = self.palette;
        let entry = self.entries[0].clone();
        let many = self.entries.len() > 1;
        let shown = self.icons.get(&entry, (48. * window.scale_factor()).round() as u32);
        let size = self.size_text();
        let title: SharedString = if many { count(self.entries.len(), "item").into() } else { entry.name.clone() };
        div()
            .flex()
            .gap_3()
            .items_center()
            .child(file_image(shown, &entry, 48., palette.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::SEMIBOLD).truncate().child(title))
                    .child(div().text_size(px(11.)).text_color(palette.secondary).child(size))
                    .when(!many, |d| d.child(div().text_size(px(11.)).text_color(palette.secondary).child(format!("Modified: {}", entry.mtime_label())))),
            )
            .into_any_element()
    }

    fn size_text(&self) -> String {
        let bytes = |n: u64| if n < 1000 { human_size(n) } else { format!("{} bytes ({})", thousands(n), human_size(n)) };
        match (self.entries.as_slice(), self.size) {
            // (A sparse or compressed file: what it takes, too, as Finder says.)
            ([one], _) if !one.is_dir => match std::fs::symlink_metadata(&one.path).map(|m| m.blocks() * 512) {
                Ok(held) if one.size >= 1 << 20 && held < one.size / 10 * 9 => format!("{}, {} on disk", bytes(one.size), human_size(held)),
                _ => bytes(one.size),
            },
            (_, None) => "Calculating size…".into(),
            (_, Some((n, items))) => format!("{} for {}", bytes(n), count(items, "item")),
        }
    }
}

/// Who may do what: Finder's Sharing & Permissions, read from the mode.
fn permissions(path: &std::path::Path) -> Vec<(String, String)> {
    let Ok(meta) = std::fs::metadata(path) else { return Vec::new() };
    let mode = meta.mode();
    let access = |bits: u32| match (bits & 4 != 0, bits & 2 != 0) {
        (true, true) => "Read & Write",
        (true, false) => "Read only",
        (false, true) => "Write only",
        (false, false) => "No Access",
    };
    let user = unsafe {
        let pw = libc::getpwuid(meta.uid());
        if pw.is_null() { meta.uid().to_string() } else { CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned() }
    };
    let group = unsafe {
        let gr = libc::getgrgid(meta.gid());
        if gr.is_null() { meta.gid().to_string() } else { CStr::from_ptr((*gr).gr_name).to_string_lossy().into_owned() }
    };
    vec![
        (format!("{user} (Me)"), access(mode >> 6).into()),
        (group, access(mode >> 3).into()),
        ("everyone".into(), access(mode).into()),
    ]
}

impl Render for InfoWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.close {
            window.remove_window();
        }
        let palette = self.palette;
        if self.entries.is_empty() {
            return div().size_full().bg(palette.background).into_any_element();
        }
        if let [one] = self.entries.as_slice() {
            window.set_window_title(&if self.follows { format!("{} Inspector", one.name) } else { format!("{} Info", one.name) });
        }
        let entry = self.entries[0].clone();
        let one = self.entries.len() == 1;
        let section = |title: &'static str| {
            div().mt_3().pb_1().border_b_1().border_color(palette.separator).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(title)
        };
        let row = |label: &str, value: SharedString| {
            div()
                .flex()
                .gap_2()
                .text_size(px(11.))
                .child(div().w(px(84.)).flex_shrink_0().flex().justify_end().text_color(palette.secondary).child(format!("{label}:")))
                .child(div().flex_1().min_w_0().text_ellipsis_middle().child(value))
        };
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut InfoWindow, &mut Context<InfoWindow>), window: &Window, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(11.))
                .child(
                    div()
                        .size(px(13.))
                        .rounded_sm()
                        .border_1()
                        .border_color(palette.glass_border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 7., SEMIBOLD, palette.selected_text, window))),
                )
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| f(this, cx)))
        };
        let kinds: std::collections::BTreeSet<SharedString> = self.entries.iter().map(Entry::kind_label).collect();
        let kind = if kinds.len() == 1 { entry.kind_label() } else { "Multiple Kinds".into() };
        let parent = entry.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let mut general = vec![("Kind", kind), ("Size", self.size_text().into()), ("Where", parent.into())];
        if one {
            general.push(("Created", self.created.map_or("--".into(), date_label)));
            general.push(("Modified", entry.mtime_label()));
            if entry.alias
                && let Some(original) = macos::resolve_alias(&entry.path)
            {
                general.push(("Original", original.display().to_string().into()));
            }
            if entry.is_dir && macos::volume_flags(&entry.path).0 {
                general.push(("Available", macos::available_bytes(&entry.path).map_or("--".into(), |b| human_size(b).into())));
            }
        }
        let tags: Vec<AnyElement> = crate::finder::TAG_COLORS
            .iter()
            .map(|(name, color)| {
                let on = self.entries.iter().all(|e| e.tags.iter().any(|(t, _)| t == name));
                let name = *name;
                div()
                    .id(name)
                    .size(px(16.))
                    .rounded_full()
                    .bg(crate::style::tag_color(*color).unwrap_or(palette.tertiary))
                    .border_2()
                    .border_color(if on { palette.label } else { palette.background })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        for e in &this.entries {
                            let mut names: Vec<String> = macos::tags(&e.path).into_iter().map(|(n, _)| n).filter(|n| n != name).collect();
                            if !on {
                                names.push(name.to_string());
                            }
                            let _ = macos::set_tags(&e.path, &names);
                        }
                        this.reload(cx);
                    }))
                    .into_any_element()
            })
            .collect();
        let shown = self.icons.get(&entry, (200. * window.scale_factor()).round() as u32);
        let app_name = self.chosen_app.as_ref().or(self.app.as_ref()).map(|a| a.file_stem().unwrap_or_default().to_string_lossy().into_owned());
        div()
            .id("info")
            .key_context("Utility")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(12.))
            .p_4()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.header(window))
            .child(div().mt_2().flex().gap_1p5().children(tags))
            .child(section("General"))
            .children(general.into_iter().map(|(l, v)| row(l, v)))
            .when(one, |d| {
                d.child(div().mt_1().child(check("locked", "Locked", self.locked, |this, cx| {
                    if let [e] = this.entries.as_slice() {
                        match macos::set_locked(&e.path, !this.locked) {
                            Ok(()) => this.locked = !this.locked,
                            Err(err) => this.notice = Some(err.into()),
                        }
                    }
                    cx.notify();
                }, window, cx)))
            })
            .when(!self.more.is_empty(), |d| {
                d.child(section("More Info")).children(self.more.iter().map(|(l, v)| row(l, v.clone().into())))
            })
            .when(one, |d| {
                d.child(section("Name & Extension"))
                    .child(Input::new(&self.name).small())
                    .when(!entry.is_dir, |d| {
                        d.child(check("hide-ext", "Hide extension", self.hide_ext, |this, cx| {
                            if let [e] = this.entries.as_slice()
                                && macos::set_hidden_extension(&e.path, !this.hide_ext).is_ok()
                            {
                                this.hide_ext = !this.hide_ext;
                            }
                            cx.notify();
                        }, window, cx))
                    })
                    .child(section("Comments"))
                    .child(Input::new(&self.comment).small())
            })
            .when(one && !entry.is_dir, |d| {
                d.child(section("Open with")).child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            glass(24., &palette)
                                .id("choose-app")
                                .flex_1()
                                .px_3()
                                .gap_2()
                                .text_size(px(12.))
                                .children(self.chosen_app.clone().or(self.app.clone()).map(|a| img(macos_icon(&a)).size(px(16.))))
                                .child(div().flex_1().truncate().child(app_name.unwrap_or_else(|| "Choose…".into())))
                                .child(symbol("chevron.up.chevron.down", 9., SEMIBOLD, palette.secondary, window))
                                .on_click(cx.listener(|this, _, window, cx| this.choose_app(window, cx))),
                        ),
                )
                .child(div().text_size(px(11.)).text_color(palette.secondary).child(format!("Use this app to open all documents like this one ({}).", entry.kind_label())))
                .child(
                    glass(24., &palette)
                        .id("change-all")
                        .px_3()
                        .text_size(px(12.))
                        .when(self.chosen_app.is_none(), |d| d.opacity(0.5))
                        .child("Change All…")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let (Some(app), [e]) = (this.chosen_app.clone(), this.entries.as_slice()) {
                                match macos::set_default_app(&e.path, &app) {
                                    Ok(()) => {
                                        this.app = Some(app);
                                        this.chosen_app = None;
                                        this.notice = Some("Changed for all documents of this kind.".into());
                                    }
                                    Err(err) => this.notice = Some(err.into()),
                                }
                                cx.notify();
                            }
                        })),
                )
            })
            .when(one, |d| {
                d.child(section("Preview"))
                    .child(div().h(px(200.)).flex().items_center().justify_center().child(file_image(shown, &entry, 180., palette.accent)))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(glass(22., &palette).id("paste-icon").px_2().text_size(px(11.)).child("Paste Icon").on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let [e] = this.entries.as_slice() {
                                        if let Err(err) = macos::set_icon(&e.path, false) {
                                            this.notice = Some(err.into());
                                        }
                                        this.icons.forget(&e.path.clone());
                                        this.reload(cx);
                                    }
                                },
                            )))
                            .child(glass(22., &palette).id("clear-icon").px_2().text_size(px(11.)).child("Remove Custom Icon").on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let [e] = this.entries.as_slice() {
                                        let _ = macos::set_icon(&e.path, true);
                                        this.icons.forget(&e.path.clone());
                                        this.reload(cx);
                                    }
                                },
                            ))),
                    )
                    .child(section("Sharing & Permissions"))
                    .children(self.permissions.iter().map(|(who, what)| row(who, what.clone().into())))
            })
            .when_some(self.notice.clone(), |d, notice| d.child(div().mt_2().text_size(px(11.)).text_color(palette.secondary).child(notice)))
            .into_any_element()
    }
}

/// An app's icon, for the Open With button.
fn macos_icon(app: &std::path::Path) -> Arc<RenderImage> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<PathBuf, Arc<RenderImage>>> = Default::default();
    }
    CACHE.with_borrow_mut(|c| {
        c.entry(app.to_path_buf())
            .or_insert_with(|| Arc::new(macos::file_icon(app, 32).unwrap_or_else(|| RenderImage::new(Vec::new()))))
            .clone()
    })
}

impl Drop for InfoWindow {
    fn drop(&mut self) {
        // Stop adding up a folder nobody's looking at.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

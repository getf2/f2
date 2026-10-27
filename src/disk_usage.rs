//! Disk Usage: what takes up a disk, as DaisyDisk shows it. A ring chart
//! of the folder shown (each ring one level deeper, each slice as big as
//! what it holds) beside a list of what's in it, biggest first. Click a
//! slice to go into it, the middle to go back out; right-click to show an
//! item in Finder2 or move it to the Trash. The startup disk comes from
//! fsearch's index in a blink; other disks are scanned. Clean Up, under
//! the disks, lists large files and copies to move to the Trash (`clean`).

mod clean;

use std::cell::Cell;
use std::ffi::CString;
use std::f32::consts::TAU;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::listing::{human_size, thousands};
use crate::macos;
use crate::style::{MEDIUM, Palette, SEMIBOLD, glass, symbol};
use crate::usage::{DEPTH, Node};

/// The window, if open.
#[derive(Default)]
struct Open(Option<(AnyWindowHandle, WeakEntity<DiskUsage>)>);

impl Global for Open {}

pub fn open(cx: &mut App) {
    open_on(None, cx);
}

/// Disk Usage on Clean Up's Free Up Space.
pub fn free_up_space(cx: &mut App) {
    open_on(Some(clean::List::Space), cx);
}

fn open_on(list: Option<clean::List>, cx: &mut App) {
    let show = move |usage: &mut DiskUsage, cx: &mut Context<DiskUsage>| {
        if let Some(list) = list {
            usage.open_list(list, cx);
        }
    };
    if let Some((handle, usage)) = cx.try_global::<Open>().and_then(|o| o.0.clone())
        && let Some(usage) = usage.upgrade()
        && crate::raise(handle, cx)
    {
        usage.update(cx, show);
        cx.activate(true);
        return;
    }
    let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(760.), px(480.))),
        titlebar: Some(TitlebarOptions { title: Some("Disk Usage".into()), ..Default::default() }),
        ..Default::default()
    };
    if let Ok((handle, usage)) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| DiskUsage::new(window, cx))) {
        cx.set_global(Open(Some((handle, usage.downgrade()))));
        usage.update(cx, show);
    }
    cx.activate(true);
}

struct Volume {
    name: String,
    path: PathBuf,
    total: u64,
    /// Free as Finder counts it (and the rest of F2): what's purgeable too.
    free: u64,
    /// Free on the disk itself, without what's purgeable: what's in no
    /// folder is the rest, less what's in folders.
    free_on_disk: u64,
}

/// A disk's size and free space on it (purgeable space not counted).
fn space(path: &Path) -> Option<(u64, u64)> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statfs(c.as_ptr(), &mut st) } == 0).then(|| (st.f_blocks * st.f_bsize as u64, st.f_bavail * st.f_bsize as u64))
}

/// The startup disk, then what's mounted in /Volumes (not a plain folder
/// left there, nor the link to the startup disk).
fn volumes() -> Vec<Volume> {
    use std::os::unix::fs::MetadataExt;
    let mut paths = vec![PathBuf::from("/")];
    let startup = std::fs::metadata("/").map(|m| m.dev()).ok();
    let mut mounted: Vec<PathBuf> = std::fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.symlink_metadata().is_ok_and(|m| m.is_dir() && Some(m.dev()) != startup))
        .collect();
    mounted.sort();
    paths.extend(mounted);
    paths
        .into_iter()
        .filter_map(|path| {
            let (total, free_on_disk) = space(&path)?;
            let free = macos::available_bytes(&path).unwrap_or(free_on_disk);
            Some(Volume { name: crate::usage::display_name(&path), path, total, free, free_on_disk })
        })
        .collect()
}

/// One slice of the chart: a ring (1 the innermost), and where it starts
/// and ends, in turns from the top, clockwise.
#[derive(Clone)]
struct Slice {
    path: PathBuf,
    ring: usize,
    start: f32,
    end: f32,
    color: Hsla,
}

/// Slices thinner than this (in turns) aren't drawn.
const THINNEST: f32 = 0.002;

/// The first ring's colors, round the chart.
const HUES: [f32; 10] = [0.58, 0.76, 0.92, 0.07, 0.13, 0.33, 0.5, 0.67, 0.0, 0.42];

fn slice_color(hue: Option<f32>, ring: usize, dark: bool) -> Hsla {
    let depth = ring as f32 - 1.;
    match hue {
        // Lighter, and less colorful, the deeper.
        Some(h) => hsla(h, (0.72 - depth * 0.07).max(0.25), if dark { 0.52 + depth * 0.05 } else { 0.55 + depth * 0.05 }, 1.),
        // Files: gray.
        None => hsla(0., 0., if dark { 0.42 + depth * 0.04 } else { 0.62 + depth * 0.04 }, 1.),
    }
}

fn slices(root: &Node, dark: bool) -> Vec<Slice> {
    fn add(node: &Node, ring: usize, start: f32, end: f32, hue: Option<f32>, dark: bool, out: &mut Vec<Slice>) {
        if node.bytes == 0 || ring > DEPTH {
            return;
        }
        let mut at = start;
        for (i, child) in node.children.iter().enumerate() {
            let span = (end - start) * (child.bytes as f64 / node.bytes as f64) as f32;
            let hue = if ring == 1 { child.dir.then_some(HUES[i % HUES.len()]) } else { hue.filter(|_| child.dir) };
            if span >= THINNEST {
                out.push(Slice { path: child.path.clone(), ring, start: at, end: at + span, color: slice_color(hue, ring, dark) });
                add(child, ring + 1, at, at + span, hue, dark, out);
            }
            at += span;
        }
    }
    let mut out = Vec::new();
    add(root, 1, 0., 1., None, dark, &mut out);
    out
}

/// The chart's geometry in its bounds: center, the middle's radius, and
/// each ring's width.
fn geometry(bounds: Bounds<Pixels>) -> (Point<Pixels>, f32, f32) {
    let size = f32::from(bounds.size.width.min(bounds.size.height));
    let outer = size / 2. - 12.;
    let inner = outer * 0.3;
    (bounds.center(), inner, (outer - inner) / DEPTH as f32)
}

/// What's under the pointer in the chart.
#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Middle,
    Slice(usize),
    Nothing,
}

/// Where a scan is: items found so far.
struct Scan {
    seen: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    _poll: Task<()>,
}

struct DiskUsage {
    focus: FocusHandle,
    palette: Palette,
    dark: bool,
    volumes: Vec<Volume>,
    /// The disk picked, and the folder shown on it.
    volume: PathBuf,
    root: PathBuf,
    tree: Option<Arc<Node>>,
    slices: Vec<Slice>,
    /// How it was worked out.
    source: SharedString,
    scan: Option<Scan>,
    task: Task<()>,
    /// The item pointed at (in the chart or the list).
    hovered: Option<PathBuf>,
    hit: Hit,
    chart: Rc<Cell<Bounds<Pixels>>>,
    /// One of Clean Up's lists, shown instead of the chart; and what's in
    /// them.
    list: Option<clean::List>,
    clean: clean::Clean,
    _subscriptions: Vec<Subscription>,
}

impl DiskUsage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe_window_appearance(window, |this, window, cx| {
            this.palette = Palette::for_window(window);
            this.dark = window.appearance() == WindowAppearance::Dark || window.appearance() == WindowAppearance::VibrantDark;
            if let Some(tree) = &this.tree {
                this.slices = slices(tree, this.dark);
            }
            cx.notify();
        })];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let dark = window.appearance() == WindowAppearance::Dark || window.appearance() == WindowAppearance::VibrantDark;
        let mut this = DiskUsage {
            focus,
            palette: Palette::for_window(window),
            dark,
            volumes: volumes(),
            volume: PathBuf::from("/"),
            root: PathBuf::from("/"),
            tree: None,
            slices: Vec::new(),
            source: "".into(),
            scan: None,
            task: Task::ready(()),
            hovered: None,
            hit: Hit::Nothing,
            chart: Rc::new(Cell::new(Bounds::default())),
            list: None,
            clean: Default::default(),
            _subscriptions: subscriptions,
        };
        this.show(PathBuf::from("/"), cx);
        this
    }

    /// Show a folder: from the index if it holds it, else scanned.
    fn show(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if let Some(scan) = self.scan.take() {
            scan.stop.store(true, Ordering::Relaxed);
        }
        if let Some(v) = self.volumes.iter().filter(|v| root.starts_with(&v.path)).max_by_key(|v| v.path.as_os_str().len()) {
            self.volume = v.path.clone();
        }
        self.root = root.clone();
        self.hovered = None;
        let search = cx.global::<crate::browser::Shared>().search.clone();
        self.task = cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let target = root.clone();
            let indexed = match search {
                Some(search) => cx.background_executor().spawn(async move { search.usage(&target) }).await,
                None => None,
            };
            let (tree, source) = match indexed {
                Some(tree) => (Some(tree), format!("From the index in {} ms", started.elapsed().as_millis().max(1))),
                None => {
                    // Not in the index: scan it, showing how far it's got.
                    let (seen, stop) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
                    let poll = cx.spawn({
                        let this = this.clone();
                        async move |cx| loop {
                            cx.background_executor().timer(Duration::from_millis(200)).await;
                            if this.update(cx, |_, cx| cx.notify()).is_err() {
                                break;
                            }
                        }
                    });
                    this.update(cx, |this, cx| {
                        this.scan = Some(Scan { seen: seen.clone(), stop: stop.clone(), _poll: poll });
                        cx.notify();
                    })
                    .ok();
                    let target = root.clone();
                    let tree = cx.background_executor().spawn(async move { crate::usage::walk(&target, &seen, &stop) }).await;
                    let took = started.elapsed();
                    let took = if took.as_secs() >= 1 { format!("{:.1} s", took.as_secs_f32()) } else { format!("{} ms", took.as_millis().max(1)) };
                    (tree, format!("Scanned in {took}"))
                }
            };
            this.update(cx, |this, cx| {
                this.scan = None;
                if this.root == root {
                    this.slices = tree.as_ref().map(|t| slices(t, this.dark)).unwrap_or_default();
                    this.tree = tree.map(Arc::new);
                    this.source = source.into();
                }
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if self.root != self.volume
            && let Some(parent) = self.root.parent()
        {
            self.show(parent.to_path_buf(), cx);
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.volumes = volumes();
        self.show(self.root.clone(), cx);
    }

    fn hit_test(&self, position: Point<Pixels>) -> Hit {
        let (center, inner, ring) = geometry(self.chart.get());
        let (dx, dy) = (f32::from(position.x - center.x), f32::from(position.y - center.y));
        let r = dx.hypot(dy);
        if r < inner {
            return Hit::Middle;
        }
        let n = ((r - inner) / ring).floor() as usize + 1;
        // Turns from the top, clockwise.
        let turn = (dy.atan2(dx) / TAU + 0.25).rem_euclid(1.);
        self.slices.iter().position(|s| s.ring == n && s.start <= turn && turn < s.end).map_or(Hit::Nothing, Hit::Slice)
    }

    /// The node for a path in the tree shown.
    fn node(&self, path: &Path) -> Option<&Node> {
        let mut node: &Node = self.tree.as_ref()?;
        let rest = path.strip_prefix(&node.path).ok()?;
        for part in rest.components() {
            node = node.children.iter().find(|c| c.path.file_name() == Some(part.as_os_str()))?;
        }
        Some(node)
    }

    fn open_item(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.node(&path).is_some_and(|n| n.dir) {
            self.show(path, cx);
        }
    }

    /// Right-click on an item: show it in a Finder2 window, or trash it.
    fn item_menu(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let items = [
            macos::MenuItem { title: Some(format!("Show in {}", crate::APP_NAME)), tag: 0, enabled: true, ..Default::default() },
            macos::MenuItem { title: None, ..Default::default() },
            macos::MenuItem { title: Some(format!("Move to {}", macos::bin())), tag: 1, enabled: true, ..Default::default() },
        ];
        // (Not inside this event's handling: the menu runs its own loop.)
        cx.spawn(async move |this, cx| {
            let chosen = macos::popup_menu(&items);
            this.update(cx, |this, cx| match chosen {
                Some(0) => {
                    if let Some(parent) = path.parent() {
                        crate::browser::open(parent.to_path_buf(), Some(path.clone()), cx);
                    }
                }
                Some(1) => this.trash(path, cx),
                _ => {}
            })
            .ok();
        })
        .detach();
    }

    fn trash(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match macos::trash(&path) {
            Ok(to) => {
                crate::finder::remember_trashed(&[(path, to)]);
                macos::play(macos::Sound::MoveToTrash);
                // (Once the index has taken it in.)
                self.task = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(600)).await;
                    this.update(cx, |this, cx| this.refresh(cx)).ok();
                });
            }
            Err(e) => {
                self.source = format!("Couldn't move it to the {}: {e}", macos::bin()).into();
                cx.notify();
            }
        }
    }

    fn render_chart(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let cell = self.chart.clone();
        let slices = self.slices.clone();
        let hovered = self.hovered.clone();
        let middle = if self.dark { palette.background.blend(white().opacity(0.06)) } else { palette.background.blend(black().opacity(0.04)) };
        let pointed = match self.hit {
            Hit::Middle => None,
            _ => hovered.as_ref().and_then(|p| self.node(p)),
        };
        let shown = pointed.or_else(|| self.tree.as_deref());
        let (title, size, detail) = match shown {
            Some(n) => (
                n.name.clone(),
                human_size(n.bytes),
                if n.dir { format!("{} items", thousands(n.items)) } else { "File".to_string() },
            ),
            None => (crate::usage::display_name(&self.root), String::new(), String::new()),
        };
        let can_go_up = self.root != self.volume && pointed.is_none();
        let chart = canvas(
            move |bounds, _, _| cell.set(bounds),
            move |bounds, _, window, _| {
                let (center, inner, ring) = geometry(bounds);
                let point = |r: f32, turn: f32| {
                    let a = turn * TAU - TAU / 4.;
                    point(center.x + px(r * a.cos()), center.y + px(r * a.sin()))
                };
                // The middle.
                let mut disc = PathBuilder::fill();
                let round: Vec<Point<Pixels>> = (0..96).map(|i| point(inner - 3., i as f32 / 96.)).collect();
                disc.add_polygon(&round, true);
                if let Ok(path) = disc.build() {
                    window.paint_path(path, middle);
                }
                for s in &slices {
                    let (r0, r1) = (inner + ring * (s.ring - 1) as f32 + 1., inner + ring * s.ring as f32 - 1.);
                    // A hair's gap between slices.
                    let gap = (0.6 / r1 / TAU).min((s.end - s.start) / 4.);
                    let (a0, a1) = (s.start + gap, s.end - gap);
                    let steps = (((a1 - a0) * 240.).ceil() as usize).max(2);
                    let mut points = Vec::with_capacity(steps * 2 + 2);
                    for i in 0..=steps {
                        points.push(point(r1, a0 + (a1 - a0) * i as f32 / steps as f32));
                    }
                    for i in (0..=steps).rev() {
                        points.push(point(r0, a0 + (a1 - a0) * i as f32 / steps as f32));
                    }
                    let mut builder = PathBuilder::fill();
                    builder.add_polygon(&points, true);
                    let lit = hovered.as_ref().is_some_and(|h| s.path.starts_with(h));
                    let color = if lit { s.color.blend(white().opacity(0.35)) } else { s.color };
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full();
        div()
            .id("chart")
            .relative()
            .flex_1()
            .min_w(px(300.))
            .h_full()
            .child(chart)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(div().max_w(px(180.)).truncate().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(size))
                    .child(div().text_size(px(11.)).text_color(palette.secondary).child(detail))
                    .when(can_go_up, |d| d.child(div().mt_1().text_size(px(11.)).text_color(palette.tertiary).child("Click to go back out"))),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let hit = this.hit_test(e.position);
                let hovered = match hit {
                    Hit::Slice(i) => Some(this.slices[i].path.clone()),
                    _ => None,
                };
                if hit != this.hit || hovered != this.hovered {
                    this.hit = hit;
                    this.hovered = hovered;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| match this.hit_test(e.position) {
                    Hit::Middle => this.go_up(cx),
                    Hit::Slice(i) => {
                        let path = this.slices[i].path.clone();
                        this.open_item(path, cx);
                    }
                    Hit::Nothing => {}
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    if let Hit::Slice(i) = this.hit_test(e.position) {
                        let path = this.slices[i].path.clone();
                        this.item_menu(path, cx);
                    }
                }),
            )
    }

    fn render_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let Some(tree) = self.tree.clone() else { return div().into_any_element() };
        let color_of = |path: &Path| self.slices.iter().find(|s| s.path == path).map(|s| s.color);
        let rows: Vec<AnyElement> = tree
            .children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                let path = child.path.clone();
                let (hover_path, click_path, menu_path) = (path.clone(), path.clone(), path.clone());
                let lit = self.hovered.as_ref() == Some(&path);
                let share = if tree.bytes > 0 { child.bytes as f32 / tree.bytes as f32 } else { 0. };
                let color = color_of(&path).unwrap_or(palette.tertiary);
                div()
                    .id(("usage-row", i))
                    .px_2()
                    .py(px(5.))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .when(lit, |d| d.bg(palette.glass_selected))
                    .hover(|s| s.bg(palette.glass_selected))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(9.)).rounded_full().flex_shrink_0().bg(color))
                            .child(div().flex_1().min_w_0().truncate().child(child.name.clone()))
                            .when(child.dir, |d| d.child(symbol("chevron.right", 9., MEDIUM, palette.tertiary, window)))
                            .child(div().text_color(palette.secondary).child(human_size(child.bytes))),
                    )
                    .child(
                        div()
                            .ml(px(17.))
                            .h(px(3.))
                            .rounded_full()
                            .bg(palette.separator)
                            .child(div().h_full().rounded_full().bg(color).w(relative(share.max(0.004)))),
                    )
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if this.hovered.as_ref() != Some(&hover_path) {
                            this.hovered = Some(hover_path.clone());
                            this.hit = Hit::Nothing;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_item(click_path.clone(), cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, _, cx| this.item_menu(menu_path.clone(), cx)))
                    .into_any_element()
            })
            .collect();
        let (rest_bytes, rest_count) = tree.rest;
        div()
            .id("usage-list")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .px_2()
            .py_2()
            .text_size(px(12.))
            .children(rows)
            // (Not for what's empty: a folder it may not look in, say.)
            .when(rest_bytes > 0, |d| {
                d.child(
                    div()
                        .px_2()
                        .py(px(5.))
                        .flex()
                        .gap_2()
                        .text_color(palette.secondary)
                        .child(div().size(px(9.)))
                        .child(div().flex_1().child(if rest_count == 1 {
                            "1 smaller item".to_string()
                        } else {
                            format!("{} smaller items", thousands(rest_count))
                        }))
                        .child(human_size(rest_bytes)),
                )
            })
            .into_any_element()
    }

    fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let rows: Vec<AnyElement> = self
            .volumes
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let path = v.path.clone();
                let on = self.volume == v.path && self.list.is_none();
                let used = v.total.saturating_sub(v.free);
                let share = if v.total > 0 { used as f32 / v.total as f32 } else { 0. };
                div()
                    .id(("volume", i))
                    .px_2()
                    .py_2()
                    .rounded_lg()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(on, |d| d.bg(palette.glass_selected))
                    .hover(|s| s.bg(palette.glass_selected))
                    .child(div().font_weight(FontWeight::MEDIUM).truncate().child(v.name.clone()))
                    .child(
                        div()
                            .h(px(5.))
                            .rounded_full()
                            .bg(palette.separator)
                            .child(div().h_full().rounded_full().bg(palette.accent).w(relative(share))),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(palette.secondary)
                            .child(format!("{} free of {}", human_size(v.free), human_size(v.total))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.list = None;
                        this.show(path.clone(), cx)
                    }))
                    .into_any_element()
            })
            .collect();
        let clean = self.render_clean_sidebar(window, cx).into_any_element();
        div()
            .w(px(210.))
            .h_full()
            .flex_shrink_0()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .bg(palette.sidebar)
            .border_r_1()
            .border_color(palette.separator)
            .text_size(px(12.))
            .child(div().px_2().pt_1().pb_1().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child("Disks"))
            .children(rows)
            .child(clean)
    }
}

impl Render for DiskUsage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        // The way here from the disk: each folder a step back.
        let mut crumbs: Vec<(String, PathBuf)> = vec![(crate::usage::display_name(&self.volume), self.volume.clone())];
        if let Ok(rest) = self.root.strip_prefix(&self.volume) {
            let mut at = self.volume.clone();
            for part in rest.components() {
                at.push(part);
                crumbs.push((part.as_os_str().to_string_lossy().into_owned(), at.clone()));
            }
        }
        let last = crumbs.len() - 1;
        let path_bar = div().flex().items_center().gap_1().min_w_0().children(crumbs.into_iter().enumerate().map(|(i, (name, path))| {
            div()
                .id(("crumb", i))
                .flex()
                .items_center()
                .gap_1()
                .when(i > 0, |d| d.child(symbol("chevron.right", 9., MEDIUM, palette.tertiary, window)))
                .child(
                    div()
                        .px_1()
                        .rounded_md()
                        .when(i == last, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .when(i < last, |d| d.text_color(palette.secondary).hover(|s| s.bg(palette.glass_selected)))
                        .child(name),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.root != path {
                        this.show(path.clone(), cx);
                    }
                }))
        }));
        let at_top = self.root == self.volume;
        // What's on the disk but in no folder seen: the system's own,
        // snapshots, what's purgeable.
        let other = (at_top && self.tree.is_some())
            .then(|| self.volumes.iter().find(|v| v.path == self.volume))
            .flatten()
            .and_then(|v| v.total.saturating_sub(v.free_on_disk).checked_sub(self.tree.as_ref().map_or(0, |t| t.bytes)))
            .filter(|&b| b > 0);
        let status = match &self.scan {
            Some(scan) => format!("Scanning… {} items", thousands(scan.seen.load(Ordering::Relaxed))),
            None => {
                let mut s = self.source.to_string();
                if let Some(other) = other {
                    s += &format!(" · {} not in any folder (system, snapshots, purgeable)", human_size(other));
                }
                if !macos::full_disk_access() {
                    s += " · without Full Disk Access some folders are left out";
                }
                s
            }
        };
        let body = if self.tree.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(palette.secondary)
                .child(if self.scan.is_some() { "Scanning…" } else { "Working it out…" })
                .into_any_element()
        } else {
            div().flex_1().min_h_0().flex().child(self.render_chart(cx)).child(self.render_list(window, cx)).into_any_element()
        };
        let sidebar = self.render_sidebar(window, cx).into_any_element();
        let clean = self.list.map(|list| self.render_clean(list, window, cx));
        let chart = clean.is_none();
        div()
            .key_context("Utility")
            .track_focus(&self.focus)
            // Back out a folder: ⌘↑, ⌘[ or escape.
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let k = &e.keystroke;
                if (k.modifiers.platform && (k.key == "up" || k.key == "[")) || k.key == "escape" {
                    this.go_up(cx);
                }
            }))
            .size_full()
            .flex()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.))
            .child(sidebar)
            .children(clean)
            .when(chart, |d| d.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(palette.separator)
                            .child(
                                glass(26., &palette)
                                    .id("back")
                                    .px_2()
                                    .when(at_top, |d| d.opacity(0.4))
                                    .child(symbol("chevron.left", 12., SEMIBOLD, palette.label, window))
                                    .on_click(cx.listener(|this, _, _, cx| this.go_up(cx))),
                            )
                            .child(div().flex_1().min_w_0().child(path_bar))
                            .child(
                                glass(26., &palette)
                                    .id("refresh")
                                    .px_2()
                                    .child(symbol("arrow.clockwise", 12., SEMIBOLD, palette.label, window))
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                            ),
                    )
                    .child(body)
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .border_t_1()
                            .border_color(palette.separator)
                            .text_size(px(11.))
                            .text_color(palette.secondary)
                            .truncate()
                            .child(status),
                    ),
            ))
    }
}

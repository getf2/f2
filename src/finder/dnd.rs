//! Moving files by dragging, as in Finder: items dropped on a folder (a
//! row, an icon, a sidebar place, a folder in the path bar, or the empty
//! space of the folder shown) move there within a volume and are copied to
//! another; ⌥ copies anyway. Files dragged in from Finder or other apps
//! land the same way, and items dragged out of the window go to them as
//! files.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::{Finder, View};
use crate::fileops::How;
use crate::style::Palette;

/// Items being dragged within the window.
#[derive(Clone)]
pub(super) struct Dragged {
    pub paths: Arc<[PathBuf]>,
    pub icon: Option<Arc<RenderImage>>,
    pub name: SharedString,
    pub palette: Palette,
    /// The one taken hold of, and where (from its top left).
    pub lead: Option<PathBuf>,
    pub grab: Point<Pixels>,
}

impl Render for Dragged {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let count = self.paths.len();
        // As Finder's: how many, in a red badge, and a green + while ⌥
        // makes it a copy.
        let badge = |bg: Hsla| {
            div()
                .absolute()
                .top(px(-7.))
                .min_w(px(17.))
                .h(px(17.))
                .px(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(bg)
                .text_color(white())
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
        };
        let copying = window.modifiers().alt;
        div()
            .relative()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(palette.selection)
            .text_color(palette.selected_text)
            .text_size(px(12.))
            .shadow_md()
            .children(self.icon.clone().map(|icon| img(icon).size(px(16.))))
            .child(self.name.clone())
            .when(count > 1, |d| d.child(badge(rgb(0xff3b30).into()).right(px(-8.)).child(count.to_string())))
            .when(copying, |d| d.child(badge(rgb(0x34c759).into()).left(px(-8.)).child("+")))
            // (Lifted off as it's taken hold of.)
            .with_animation("lift", Animation::new(Duration::from_millis(140)).with_easing(ease_out_quint()), |el, t| el.opacity(0.6 + 0.35 * t))
    }
}

/// How long a drag waits over a folder before it springs open.
pub(super) const SPRING_DELAY: Duration = Duration::from_millis(900);

/// Make `element` a place to drop files into `dir`: highlighted while
/// something is over it, and springing open if it waits there.
pub(super) fn drop_target<E: InteractiveElement>(element: E, dir: &Path, highlight: Hsla, cx: &mut Context<Finder>) -> E {
    let (target, inner, outer, spring) = (dir.to_path_buf(), dir.to_path_buf(), dir.to_path_buf(), dir.to_path_buf());
    element
        .on_drag_move::<Dragged>(cx.listener(move |this, e: &DragMoveEvent<Dragged>, window, cx| {
            this.spring_over(&spring, e.bounds.contains(&e.event.position), window, cx)
        }))
        .drag_over::<Dragged>(move |style, _, _, _| style.bg(highlight))
        .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(highlight))
        // Not a folder into itself, or into what's in it; nor where they
        // are already (but to copy them there: ⌥).
        .can_drop(move |value, window, _| {
            value.downcast_ref::<Dragged>().is_none_or(|d| {
                let here = d.paths.iter().all(|p| p.parent() == Some(target.as_path()));
                !d.paths.iter().any(|p| target.starts_with(p)) && (!here || window.modifiers().alt)
            })
        })
        .on_drop(cx.listener(move |this, dragged: &Dragged, window, cx| {
            this.drop_paths(dragged.paths.to_vec(), inner.clone(), window, cx)
        }))
        .on_drop(cx.listener(move |this, external: &ExternalPaths, window, cx| {
            this.drop_paths(external.paths().to_vec(), outer.clone(), window, cx)
        }))
}

/// Items dragged out of the window: the files, for Finder, Mail, the Dock...
pub(super) fn files_for_other_apps(dragged: &Dragged, _: &mut Window, _: &mut App) -> Option<ExternalDragPayload> {
    let files = dragged.paths.iter().map(|p| (p.clone(), p.is_dir()));
    Some(ExternalDragPayload::Files(FileDragPaths::new(files)))
}

impl Finder {
    /// A drag moved: over `dir` (`over`) it starts waiting to spring it
    /// open, else stops waiting on it.
    fn spring_over(&mut self, dir: &Path, over: bool, window: &mut Window, cx: &mut Context<Self>) {
        let waiting = self.spring.as_ref().is_some_and(|(p, _)| p == dir);
        if !over {
            if waiting {
                self.spring = None;
                cx.notify();
            }
            return;
        }
        let columns = self.view == View::Columns && !self.flat();
        if waiting || self.shows(dir) || (columns && self.column_open(dir)) {
            return;
        }
        let started = Instant::now();
        self.spring = Some((dir.to_path_buf(), started));
        // (Blinking as it waits: see `motion`.)
        self.motions.spring_started();
        cx.notify();
        let dir = dir.to_path_buf();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SPRING_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                let still = this.spring.as_ref().is_some_and(|(p, t)| *p == dir && *t == started);
                // (Let go meanwhile: not waiting any more.)
                if still && !cx.has_active_drag() {
                    this.spring = None;
                    cx.notify();
                }
                if still && cx.has_active_drag() {
                    this.spring = None;
                    // In columns it opens as clicked: selected in its
                    // column, what's in it in the next.
                    match dir.parent() {
                        Some(parent) if columns && dir.starts_with(&this.col_base) && dir != this.col_base => {
                            this.column_into(parent.to_path_buf(), Some(dir.clone()), cx)
                        }
                        _ => this.navigate(dir.clone(), None, window, cx),
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// What all the selected items' drags carry (when several are
    /// selected): worked out once per frame, not per item.
    pub(super) fn dragged_selection(&self) -> Option<Arc<[PathBuf]>> {
        (self.chosen.len() > 1).then(|| self.selection_paths().into())
    }

    /// The payload of a drag starting at `ix`: the whole selection if it's
    /// in it, else just that item.
    pub(super) fn dragged_from(&mut self, ix: usize, selection: &Option<Arc<[PathBuf]>>, window: &Window) -> Dragged {
        let entry = self.rows[ix].clone();
        let paths = match selection {
            Some(all) if self.chosen.contains(&entry.path) => all.clone(),
            _ => Arc::new([entry.path.clone()]),
        };
        let icon = self.icons.get(&entry, (16. * window.scale_factor()).round() as u32).map(|s| s.image);
        Dragged { paths, icon, name: entry.name, palette: self.palette, lead: Some(entry.path), grab: Point::default() }
    }

    /// Whether `dir` is the folder shown (by another way there too: /tmp is
    /// /private/tmp).
    pub(super) fn shows(&self, dir: &Path) -> bool {
        dir == self.cwd || std::fs::canonicalize(dir).is_ok_and(|d| std::fs::canonicalize(&self.cwd).is_ok_and(|c| c == d))
    }

    pub(super) fn drop_paths(&mut self, paths: Vec<PathBuf>, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // ⌥ copies, ⌘ moves even to another volume, as in Finder.
        let m = window.modifiers();
        let how = if m.alt { How::Copy } else if m.platform { How::Move } else { How::Drop };
        self.spring = None;
        // (Dropped in the folder shown: what's arrived shows it.)
        if !self.shows(&dir) || self.flat() {
            self.land(super::motion::Spot::Folder(dir.clone()), cx);
        }
        self.transfer_from(paths, dir, how, true, cx);
    }
}

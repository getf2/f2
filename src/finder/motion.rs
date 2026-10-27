//! Small motions that say something happened, as quick as it was: a folder
//! something's dropped on pulses, items moved or trashed fade away before
//! they go, what's copied or moved in fades in, a folder a drag waits over
//! blinks before it springs open (as Finder's does), and sheets ease in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::*;

use super::Finder;

/// How long a place something's dropped on pulses.
const PULSE: Duration = Duration::from_millis(650);
/// How long items fade before they're moved or trashed.
pub(super) const LEAVE: Duration = Duration::from_millis(160);
/// How long what's arrived takes to fade in.
const ARRIVE: Duration = Duration::from_millis(300);
/// How long a sheet takes to come in.
const SHEET: Duration = Duration::from_millis(180);
/// (What's arrived fades in once it's listed, which can take a moment.)
const ARRIVALS_KEPT: Duration = Duration::from_secs(2);
/// (What's leaving is shown again after this, if it's still listed: a
/// move to another disk, say, that's still copying.)
const LEAVING_KEPT: Duration = Duration::from_secs(3);

/// Where something can be dropped.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Spot {
    Folder(PathBuf),
    Trash,
    AirDrop,
}

/// What's in motion (each by a number of its own, so a motion played again
/// starts over).
#[derive(Default)]
pub(super) struct Motions {
    next: usize,
    landed: Vec<(Spot, usize)>,
    leaving: HashMap<PathBuf, usize>,
    arrived: HashMap<PathBuf, usize>,
    /// The sheets up (by kind: a question about a taken name can come up
    /// over another), each since when (its number).
    sheets: HashMap<&'static str, usize>,
    /// The drag waiting over a folder to spring it open.
    pub(super) spring: usize,
}

impl Motions {
    fn number(&mut self) -> usize {
        self.next += 1;
        self.next
    }

    /// A new drag is waiting over a folder.
    pub(super) fn spring_started(&mut self) {
        self.spring = self.number();
    }

    pub(super) fn is_leaving(&self, path: &Path) -> bool {
        self.leaving.contains_key(path)
    }

    /// Those that didn't go after all (couldn't be moved or trashed): back.
    pub(super) fn stayed(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            self.leaving.remove(&path);
        }
    }

    /// Listed again: what left is gone from the rows, and what didn't is
    /// shown again.
    pub(super) fn listed(&mut self) {
        self.leaving.clear();
    }

    /// The `kind` of sheet isn't up (any more).
    pub(super) fn sheet_down(&mut self, kind: &'static str) {
        self.sheets.remove(kind);
    }

    /// The number of the `kind` of sheet up (a new one if it's just come up).
    fn sheet_up(&mut self, kind: &'static str) -> usize {
        if let Some(n) = self.sheets.get(kind) {
            return *n;
        }
        let n = self.number();
        self.sheets.insert(kind, n);
        n
    }
}

/// Up quickly, then slowly away.
fn pulse(t: f32) -> f32 {
    const UP: f32 = 0.18;
    if t < UP { 1. - (1. - t / UP).powi(2) } else { (1. - (t - UP) / (1. - UP)).powi(2) }
}

/// Nothing, then two quick blinks at the end (the spring's wait), off
/// once it's over.
fn blinks(t: f32) -> f32 {
    const FROM: f32 = 0.6;
    if !(FROM..1.).contains(&t) {
        return 0.;
    }
    if ((t - FROM) / ((1. - FROM) / 4.)) as u32 % 2 == 0 { 1. } else { 0. }
}

/// A highlight over a place, under what's in it (its first child, in a
/// `relative` element).
fn overlay(color: Hsla, radius: f32) -> Div {
    div().absolute().top_0().left_0().size_full().rounded(px(radius)).bg(color)
}

impl Finder {
    /// Something was dropped on `spot`: it pulses.
    pub(super) fn land(&mut self, spot: Spot, cx: &mut Context<Self>) {
        let n = self.motions.number();
        self.motions.landed.retain(|(s, _)| *s != spot);
        self.motions.landed.push((spot, n));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PULSE).await;
            this.update(cx, |this, cx| {
                this.motions.landed.retain(|(_, k)| *k != n);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// `paths` are on their way out: they fade (and nothing more is done to
    /// them meanwhile).
    pub(super) fn leave(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let numbers: Vec<usize> = paths
            .iter()
            .map(|path| {
                let n = self.motions.number();
                self.motions.leaving.insert(path.clone(), n);
                n
            })
            .collect();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LEAVING_KEPT).await;
            this.update(cx, |this, cx| {
                this.motions.leaving.retain(|_, n| !numbers.contains(n));
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// `paths` just arrived: they fade in once they're listed.
    pub(super) fn arrive(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let numbers: Vec<usize> = paths
            .iter()
            .map(|path| {
                let n = self.motions.number();
                // (Back where it left: not leaving any more.)
                self.motions.leaving.remove(path);
                self.motions.arrived.insert(path.clone(), n);
                // (Listed by another way there, too: /tmp is /private/tmp.)
                if let Ok(real) = std::fs::canonicalize(path)
                    && real != *path
                {
                    self.motions.leaving.remove(&real);
                    self.motions.arrived.insert(real, n);
                }
                n
            })
            .collect();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ARRIVALS_KEPT).await;
            this.update(cx, |this, _| this.motions.arrived.retain(|_, n| !numbers.contains(n))).ok();
        })
        .detach();
    }

    /// The highlight that goes over `spot` just now, if one does: a pulse
    /// (dropped on), or blinks (a drag waiting there to spring it open).
    pub(super) fn spot_motion(&self, spot: &Spot, radius: f32) -> Option<AnyElement> {
        let accent = self.palette.accent;
        if let Some((_, n)) = self.motions.landed.iter().find(|(s, _)| s == spot) {
            let pulsing = overlay(accent, radius).with_animation(("landed", *n), Animation::new(PULSE).with_easing(pulse), |el, t| el.opacity(0.45 * t));
            return Some(pulsing.into_any_element());
        }
        match (spot, &self.spring) {
            (Spot::Folder(path), Some((waiting, _))) if path == waiting => {
                let blinking = overlay(accent, radius)
                    .with_animation(("spring", self.motions.spring), Animation::new(super::dnd::SPRING_DELAY).with_easing(blinks), |el, t| {
                        el.opacity(0.4 * t)
                    });
                Some(blinking.into_any_element())
            }
            _ => None,
        }
    }

    /// `item` as it comes in or goes out, if it's doing either (`dim`: how
    /// it's shown at rest, as 0.5 for a hidden file).
    pub(super) fn item_motion<E: Styled + IntoElement + 'static>(&self, path: &Path, dim: f32, item: E) -> AnyElement {
        if let Some(n) = self.motions.leaving.get(path) {
            let fading = Animation::new(LEAVE).with_easing(ease_in_out);
            return item.with_animation(("leaving", *n), fading, move |el, t| el.opacity(dim * (1. - t))).into_any_element();
        }
        if let Some(n) = self.motions.arrived.get(path) {
            let coming = Animation::new(ARRIVE).with_easing(ease_out_quint());
            return item.with_animation(("arrived", *n), coming, move |el, t| el.opacity(dim * t)).into_any_element();
        }
        item.into_any_element()
    }

    /// A sheet over the window (`body` on a dimmed backdrop), easing in when
    /// it's put up.
    pub(super) fn sheet_motion(&mut self, kind: &'static str, body: AnyElement) -> AnyElement {
        let n = self.motions.sheet_up(kind);
        let coming = || Animation::new(SHEET).with_easing(ease_out_quint());
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(black().opacity(0.2))
            .occlude()
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .justify_center()
                    .items_start()
                    .child(body)
                    .with_animation(("sheet-in", n), coming(), |el, t| el.top(px(-10. * (1. - t)))),
            )
            .with_animation(("sheet", n), coming(), |el, t| el.opacity(t))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pulses_and_blinks() {
        assert_eq!(super::pulse(0.), 0.);
        assert!((super::pulse(0.18) - 1.).abs() < 1e-6, "up at once");
        assert!(super::pulse(0.6) < 0.5 && super::pulse(1.) == 0., "then away");
        assert_eq!(super::blinks(0.3), 0., "nothing while it waits");
        assert_eq!(super::blinks(0.65), 1.);
        assert_eq!(super::blinks(0.75), 0.);
        assert_eq!(super::blinks(0.85), 1.);
        assert_eq!(super::blinks(1.), 0., "off at the end, whatever's left over");
    }
}

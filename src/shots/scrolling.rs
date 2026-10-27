//! Scrolling Capture, as CleanShot's: an area dragged out, then what's in it
//! scrolled down a step at a time (as a trackpad would, at the area's
//! middle) and taken each time, the steps put together where they overlap
//! into one tall screenshot. What stays put (a header, a toolbar) is kept
//! once. It stops at the bottom (a step that changes nothing), at
//! `MOST_ROWS` pixels, or at Esc (or its shortcut again).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;

use super::{Action, captured, kept, new_file, note};
use crate::macos;

/// Esc's id while it's going (see `hotkeys`).
pub const STOP_KEY: u32 = 8;
/// Steps, and the tallest it gets (in pixels), at most.
const MOST_STEPS: usize = 60;
const MOST_ROWS: usize = 30_000;

static GOING: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);

/// Drag out the area, then capture it scrolling (again while it's going:
/// stop there).
pub fn start(cx: &mut App) {
    if GOING.load(Ordering::SeqCst) {
        STOP.store(true, Ordering::SeqCst);
        return;
    }
    if !macos::accessibility(false) {
        macos::accessibility(true);
        return note("Allow Accessibility".into(), "Scrolling Capture scrolls for you, which takes Accessibility.".into(), None, cx);
    }
    let (tx, rx) = futures::channel::oneshot::channel();
    macos::pick_area(tx);
    cx.spawn(async move |cx| {
        let Ok(Some(area)) = rx.await else { return };
        GOING.store(true, Ordering::SeqCst);
        STOP.store(false, Ordering::SeqCst);
        crate::hotkeys::set(STOP_KEY, Some("escape"));
        let dir = kept();
        let _ = std::fs::create_dir_all(&dir);
        let file = new_file(&dir, "");
        let made = cx
            .background_executor()
            .spawn({
                let file = file.clone();
                async move { take(area, &file) }
            })
            .await;
        crate::hotkeys::set(STOP_KEY, None);
        GOING.store(false, Ordering::SeqCst);
        cx.update(|cx| match made {
            Ok(()) => captured(Action::Area, file, cx),
            Err(e) => note("Scrolling Capture Didn't Work".into(), e, None, cx),
        });
    })
    .detach();
}

/// A shortcut was pressed: whether it was Esc, stopping it.
pub fn pressed(id: u32) -> bool {
    if id == STOP_KEY && GOING.load(Ordering::SeqCst) {
        STOP.store(true, Ordering::SeqCst);
        return true;
    }
    false
}

/// Capture the area (x, y, width, height, in points) scrolling, into `out`
/// (a PNG). The pointer goes back where it was. Waits: not on the main
/// thread.
pub fn take(area: (f64, f64, f64, f64), out: &Path) -> Result<(), String> {
    let (x, y, w, h) = area;
    let (mid_x, mid_y) = (x + w / 2., y + h / 2.);
    let back = macos::pointer();
    let shot = std::env::temp_dir().join(format!("f2-scrolling-{}.png", std::process::id()));
    let grab = || -> Option<Frame> {
        let taken = std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", &format!("-R{x},{y},{w},{h}")])
            .arg(&shot)
            .status()
            .is_ok_and(|s| s.success());
        let read = taken.then(|| macos::read_rgba(&shot)).flatten();
        let _ = std::fs::remove_file(&shot);
        read.map(|(rgba, w, h)| Frame::new(rgba, w, h))
    };
    let first = grab().ok_or("the area couldn't be captured (Screen Recording may be off)")?;
    let mut stitch = Stitch::new(first);
    // (Each step a little less than the area: an overlap to match up.)
    let step = (h * 0.7).round().max(20.) as i32;
    for _ in 0..MOST_STEPS {
        if STOP.load(Ordering::SeqCst) || stitch.rows() >= MOST_ROWS {
            break;
        }
        macos::move_pointer(mid_x, mid_y);
        macos::scroll_at(mid_x, mid_y, -step);
        // (Smooth scrolling settles.)
        std::thread::sleep(Duration::from_millis(450));
        let Some(next) = grab() else { break };
        if !stitch.add(next) {
            break;
        }
    }
    macos::move_pointer(back.0, back.1);
    let (rgba, w, h) = stitch.finish();
    macos::write_png(out, &rgba, w, h)
}

/// A step's capture: its pixels, and each row's hash, and whether the row
/// is all one color (a plain row says nothing about where it is).
pub(crate) struct Frame {
    rgba: Vec<u8>,
    w: usize,
    h: usize,
    hashes: Vec<u64>,
    plain: Vec<bool>,
}

impl Frame {
    pub(crate) fn new(rgba: Vec<u8>, w: usize, h: usize) -> Frame {
        let rows: Vec<&[u8]> = rgba.chunks_exact(w * 4).collect();
        let hashes = rows.iter().map(|row| row.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))).collect();
        let plain = rows.iter().map(|row| row.chunks_exact(4).all(|px| px == &row[..4])).collect();
        Frame { rgba, w, h, hashes, plain }
    }

    fn rows(&self, from: usize, to: usize) -> &[u8] {
        &self.rgba[from * self.w * 4..to * self.w * 4]
    }
}

/// The steps put together: what's above the bottom band (a footer, kept
/// once, from the last step), and the last step.
pub(crate) struct Stitch {
    out: Vec<u8>,
    /// What stays put at the top and the bottom, in rows (from the first
    /// two steps).
    bands: Option<(usize, usize)>,
    last: Frame,
}

impl Stitch {
    pub(crate) fn new(first: Frame) -> Stitch {
        Stitch { out: first.rgba.clone(), bands: None, last: first }
    }

    /// Rows so far.
    pub(crate) fn rows(&self) -> usize {
        self.out.len() / (self.last.w * 4).max(1)
    }

    /// The next step's capture, put on: false if it's the end (nothing
    /// moved, or it doesn't follow on from the last).
    pub(crate) fn add(&mut self, next: Frame) -> bool {
        let (w, h) = (self.last.w, self.last.h);
        if (next.w, next.h) != (w, h) {
            return false;
        }
        let (top, bottom) = match self.bands {
            Some(bands) => bands,
            None => {
                let top = (0..h).take_while(|&i| self.last.hashes[i] == next.hashes[i]).count();
                if top == h {
                    return false;
                }
                let bottom = (0..h - top).take_while(|&i| self.last.hashes[h - 1 - i] == next.hashes[h - 1 - i]).count();
                // (The first step's footer comes back at the end, from the last.)
                self.out.truncate((h - bottom) * w * 4);
                self.bands = Some((top, bottom));
                (top, bottom)
            }
        };
        let Some(s) = shift(&self.last.hashes, &next.hashes, &next.plain, top, h - bottom) else { return false };
        self.out.extend_from_slice(next.rows(h - bottom - s, h - bottom));
        self.last = next;
        true
    }

    /// The tall picture: its pixels, width and height.
    pub(crate) fn finish(mut self) -> (Vec<u8>, usize, usize) {
        let (w, h) = (self.last.w, self.last.h);
        let bottom = self.bands.map_or(0, |(_, b)| b);
        if self.bands.is_some() {
            self.out.extend_from_slice(self.last.rows(h - bottom, h));
        }
        let rows = self.out.len() / (w * 4);
        (self.out, w, rows)
    }
}

/// How far `next` scrolled on from `prev` in rows `from..to`: the shift
/// with `next[from + j] == prev[from + s + j]` for the most rows that
/// aren't plain (nine in ten at least, eight at least). None if nothing
/// moved, or none fits.
pub(crate) fn shift(prev: &[u64], next: &[u64], plain: &[bool], from: usize, to: usize) -> Option<usize> {
    let n = to.saturating_sub(from);
    let mut best: Option<(usize, usize)> = None;
    for s in 1..n {
        let (mut seen, mut matched) = (0, 0);
        for j in 0..n - s {
            if plain[from + j] {
                continue;
            }
            seen += 1;
            if next[from + j] == prev[from + s + j] {
                matched += 1;
            }
        }
        if seen >= 8 && matched * 10 >= seen * 9 && best.is_none_or(|(m, _)| matched > m) {
            best = Some((matched, s));
        }
    }
    best.map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::{Frame, Stitch};

    /// The area (points: "x,y,w,h") captured scrolling, on this Mac:
    /// `F2_SCROLL_AREA=… F2_SCROLL_OUT=/tmp/….png cargo test --release -- --ignored --nocapture scrolling_here`
    #[test]
    #[ignore]
    fn scrolling_here() {
        let Some(area) = std::env::var("F2_SCROLL_AREA").ok() else { return };
        let n: Vec<f64> = area.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let out = std::path::PathBuf::from(std::env::var("F2_SCROLL_OUT").unwrap());
        let t = std::time::Instant::now();
        super::take((n[0], n[1], n[2], n[3]), &out).unwrap();
        println!("{} in {:?}", out.display(), t.elapsed());
    }

    /// A page of `rows` rows, each its own color (row i: shade i).
    fn page(rows: usize) -> Vec<[u8; 4]> {
        (0..rows).map(|i| [(i % 251) as u8, (i / 251) as u8, 7, 255]).collect()
    }

    /// What an area `h` rows tall shows scrolled to `at`: a header of
    /// `header` rows that stays put, the page under it.
    fn view(page: &[[u8; 4]], at: usize, h: usize, header: usize) -> Frame {
        let mut rgba = Vec::new();
        for r in 0..h {
            let px = if r < header { [250, 250, 250, 255].map(|v| v - r as u8) } else { page[at + r - header] };
            // (Two pixels unalike: a row of one color says nothing.)
            let other = [255 - px[0], px[1], 200, 255];
            rgba.extend_from_slice(&[px, other].concat());
        }
        Frame::new(rgba, 2, h)
    }

    #[test]
    fn steps_put_together() {
        let page = page(400);
        let (h, header) = (100, 10);
        let mut stitch = Stitch::new(view(&page, 0, h, header));
        let mut at = 0;
        for step in [70, 70, 70, 70, 70] {
            at = (at + step).min(page.len() - (h - header));
            assert!(stitch.add(view(&page, at, h, header)));
        }
        // (At the bottom: nothing moves.)
        assert!(!stitch.add(view(&page, at, h, header)));
        let (rgba, w, rows) = stitch.finish();
        assert_eq!(w, 2);
        assert_eq!(rows, header + page.len(), "the header once, then the whole page");
        let px = |r: usize| [rgba[r * 8], rgba[r * 8 + 1], rgba[r * 8 + 2], rgba[r * 8 + 3]];
        for (i, want) in page.iter().enumerate() {
            assert_eq!(&px(header + i), want, "row {i}");
        }
    }

    #[test]
    fn a_short_last_step() {
        let page = page(150);
        let (h, header) = (100, 0);
        let mut stitch = Stitch::new(view(&page, 0, h, header));
        assert!(stitch.add(view(&page, 50, h, header)), "only 50 of 70 left to scroll");
        let (_, _, rows) = stitch.finish();
        assert_eq!(rows, 150);
    }
}

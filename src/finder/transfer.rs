//! Copying and moving with Finder's manners: when a name is taken, ask
//! (Replace, Keep Both, Merge for two folders, Stop; Apply to All), and
//! show a long one's progress with a way to stop it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::Finder;
use super::undo::Op;
use crate::fileops::{self, How, Item, Progress, Resolve};
use crate::listing::{Entry, count, human_size};
use crate::style::{SEMIBOLD, glass, symbol};

/// Long operations show after this.
const SHOW_AFTER: Duration = Duration::from_millis(400);

/// How long's left, as said: "about 40 s left", "about 3 min left".
fn time_left(secs: f64) -> String {
    let secs = secs.max(0.).round() as u64;
    match secs {
        0..=4 => "almost done".into(),
        5..=59 => format!("about {} s left", if secs > 10 { secs.div_ceil(5) * 5 } else { secs }),
        60..=3599 => format!("about {} min left", secs.div_ceil(60)),
        _ => format!("about {} h {} min left", secs / 3600, (secs % 3600) / 60),
    }
}

/// What's cut (⌘X), and the clipboard's change count once it was put on
/// it: pasted while the clipboard still has it, it's moved, not copied.
static CUT: Mutex<Option<(Vec<PathBuf>, i64)>> = Mutex::new(None);

/// Cut `paths`: on the clipboard as if copied (another app pasting them
/// gets copies), and moved when they're pasted here.
pub(super) fn cut(paths: Vec<PathBuf>) {
    crate::macos::copy_files(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>());
    *CUT.lock().unwrap() = Some((paths, crate::macos::pasteboard_changes()));
}

/// What's cut, while the clipboard still has it (None: nothing is, or
/// something's been copied since).
pub(super) fn cut_items() -> Option<Vec<PathBuf>> {
    let mut cut = CUT.lock().unwrap();
    if cut.as_ref().is_some_and(|(_, changes)| *changes != crate::macos::pasteboard_changes()) {
        *cut = None;
    }
    cut.as_ref().map(|(paths, _)| paths.clone())
}

/// Whether `path` is cut (shown dimmed until it's pasted).
pub(super) fn is_cut(path: &Path) -> bool {
    let listed = CUT.lock().unwrap().as_ref().is_some_and(|(paths, _)| paths.iter().any(|p| p == path));
    listed && cut_items().is_some()
}

/// Nothing cut any more (pasted, or Escape): whether something was.
pub(super) fn uncut() -> bool {
    CUT.lock().unwrap().take().is_some()
}

/// Whether `pasted` is what's cut: the same files, whatever form their
/// names come back from the clipboard in ("é" as one character or two).
pub(super) fn pasting_cut(pasted: &[PathBuf]) -> bool {
    !pasted.is_empty() && cut_items().is_some_and(|cut| same_files(&cut, pasted))
}

/// Whether `a` and `b` are the same files (by disk and file number), all
/// there.
fn same_files(a: &[PathBuf], b: &[PathBuf]) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let ids = |paths: &[PathBuf]| -> Option<Vec<(u64, u64)>> {
        let mut ids = paths.iter().map(|p| p.symlink_metadata().ok().map(|m| (m.dev(), m.ino()))).collect::<Option<Vec<_>>>()?;
        ids.sort();
        Some(ids)
    };
    ids(a).is_some_and(|a| Some(a) == ids(b))
}

/// What's on the clipboard to paste, as it is now (cut and pasted once,
/// it's gone from where it was); and whether it's what was cut (then not
/// cut any more: it's being moved).
pub(super) fn to_paste() -> (Vec<PathBuf>, bool) {
    let pasted: Vec<PathBuf> = crate::macos::pasted_files().into_iter().filter(|p| p.symlink_metadata().is_ok()).collect();
    let was_cut = pasting_cut(&pasted);
    if was_cut {
        uncut();
    }
    (pasted, was_cut)
}

#[cfg(test)]
mod tests {
    #[test]
    fn time_left_as_said() {
        assert_eq!(super::time_left(2.), "almost done");
        assert_eq!(super::time_left(8.), "about 8 s left");
        assert_eq!(super::time_left(37.), "about 40 s left");
        assert_eq!(super::time_left(150.), "about 3 min left");
        assert_eq!(super::time_left(3900.), "about 1 h 5 min left");
    }

    use std::path::PathBuf;

    /// Cut, then pasted: the same files whatever form their names are in.
    #[test]
    fn same_files_by_identity() {
        let dir = std::env::temp_dir().join(format!("f2-cut-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (one, two) = (dir.join("caf\u{e9}"), dir.join("other"));
        std::fs::write(&one, "").unwrap();
        std::fs::write(&two, "").unwrap();
        let decomposed = dir.join("cafe\u{301}");
        assert!(super::same_files(&[one.clone(), two.clone()], &[two.clone(), decomposed]), "the same, in any order and form");
        assert!(!super::same_files(&[one.clone()], &[two.clone()]));
        assert!(!super::same_files(&[one.clone(), two.clone()], &[one.clone()]));
        assert!(!super::same_files(&[one.clone(), PathBuf::from("/nowhere/at/all")], &[one.clone(), PathBuf::from("/nowhere/at/all")]), "gone: not cut");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// A copy or move waiting on what to do about taken names.
pub(super) struct Conflicts {
    items: Vec<Item>,
    choices: Vec<Option<Resolve>>,
    /// The item asked about now.
    at: usize,
    apply_all: bool,
    dir: PathBuf,
}

/// A copy or move under way.
pub(super) struct Job {
    pub(super) id: usize,
    pub(super) label: SharedString,
    pub(super) progress: Arc<Progress>,
    pub(super) started: Instant,
}

impl Finder {
    /// Put files into `dir` (a drop, a paste, Move Item Here): asking first
    /// about names taken there.
    pub(super) fn transfer(&mut self, paths: Vec<PathBuf>, dir: PathBuf, how: How, cx: &mut Context<Self>) {
        self.transfer_from(paths, dir, how, false, cx)
    }

    /// `transfer`; `dropped` there, with a sound once it's going ahead (as
    /// Settings say).
    pub(super) fn transfer_from(&mut self, paths: Vec<PathBuf>, dir: PathBuf, how: How, dropped: bool, cx: &mut Context<Self>) {
        // (Not those already on their way out.)
        let paths: Vec<PathBuf> = paths.into_iter().filter(|p| !self.motions.is_leaving(p)).collect();
        if paths.is_empty() || self.connecting.is_some() {
            return;
        }
        let sound = |cx: &App| {
            if dropped && crate::settings::Settings::get(cx).drop_sound {
                crate::macos::play(crate::macos::Sound::Drop);
            }
        };
        // (To or from a server: see `servers`.)
        if crate::remote::is_remote(&dir) || paths.iter().any(|p| crate::remote::is_remote(p)) {
            sound(cx);
            return self.transfer_remote(paths, dir, how, cx);
        }
        let items = match fileops::plan(&paths, &dir, how) {
            Ok(items) if items.is_empty() => return,
            Ok(items) => items,
            Err(e) => return self.show_notice(e.to_string(), cx),
        };
        sound(cx);
        match items.iter().position(|i| i.taken) {
            Some(at) => {
                let choices = vec![None; items.len()];
                self.conflicts = Some(Conflicts { items, choices, at, apply_all: false, dir });
                cx.notify();
            }
            None => self.run_transfer(items.into_iter().map(|i| (i, Resolve::KeepBoth)).collect(), dir, cx),
        }
    }

    /// The answer about the name asked about (`None`: Stop, nothing done).
    pub(super) fn resolve(&mut self, choice: Option<Resolve>, cx: &mut Context<Self>) {
        cx.notify();
        let Some(c) = self.conflicts.as_mut() else { return };
        let Some(choice) = choice else {
            self.conflicts = None;
            return;
        };
        c.choices[c.at] = Some(choice);
        if c.apply_all {
            for ix in c.at + 1..c.items.len() {
                if c.items[ix].taken && c.choices[ix].is_none() {
                    // Merge and Continue only go for folders; files are replaced.
                    let fits = !matches!(choice, Resolve::Merge | Resolve::Continue) || c.items[ix].folders();
                    c.choices[ix] = Some(if fits { choice } else { Resolve::Replace });
                }
            }
        }
        if let Some(next) = (c.at + 1..c.items.len()).find(|&ix| c.items[ix].taken && c.choices[ix].is_none()) {
            c.at = next;
            return;
        }
        let Some(c) = self.conflicts.take() else { return };
        let plan = c.items.into_iter().zip(c.choices).map(|(item, choice)| (item, choice.unwrap_or(Resolve::KeepBoth))).collect();
        self.run_transfer(plan, c.dir, cx);
    }

    fn run_transfer(&mut self, plan: Vec<(Item, Resolve)>, dir: PathBuf, cx: &mut Context<Self>) {
        let n = plan.iter().filter(|(_, r)| *r != Resolve::Skip).count();
        if n == 0 {
            return;
        }
        let copying = plan.iter().all(|(item, _)| item.copy);
        let what = if n == 1 { format!("“{}”", plan[0].0.src.file_name().unwrap_or_default().to_string_lossy()) } else { count(n, "item") };
        let label = format!("{} {what} to “{}”", if copying { "Copying" } else { "Moving" }, super::display_name(&dir));
        // What's moved out of what's shown fades first.
        let shown: std::collections::HashSet<&PathBuf> = self.rows.iter().map(|e| &e.path).collect();
        let leaving: Vec<PathBuf> =
            plan.iter().filter(|(item, r)| !item.copy && *r != Resolve::Skip && shown.contains(&item.src)).map(|(item, _)| item.src.clone()).collect();
        self.leave(&leaving, cx);
        let progress = Arc::new(Progress::default());
        let watched = progress.clone();
        self.next_job += 1;
        let id = self.next_job;
        self.jobs.push(Job { id, label: label.clone().into(), progress: progress.clone(), started: Instant::now() });
        self.tick_jobs(cx);
        cx.spawn(async move |this, cx| {
            if !leaving.is_empty() {
                cx.background_executor().timer(super::motion::LEAVE).await;
            }
            let (done, error) = cx
                .background_executor()
                .spawn(async move {
                    progress.total.store(fileops::plan_size(&plan), Ordering::Relaxed);
                    fileops::execute(&plan, &progress)
                })
                .await;
            let failed = watched.failed.lock().map(|f| f.clone()).unwrap_or_default();
            let (copied, total) = (watched.done.load(Ordering::Relaxed), watched.total.load(Ordering::Relaxed));
            this.update(cx, |this, cx| {
                this.jobs.retain(|j| j.id != id);
                let op = Op { moved: done.moved, created: done.created };
                let touched: Vec<PathBuf> =
                    op.moved.iter().flat_map(|(a, b)| [a, b]).chain(&op.created).filter_map(|p| p.parent().map(Into::into)).collect();
                // What didn't go after all shows again (found items aren't
                // listed again: all of them).
                let flat = this.flat();
                this.motions.stayed(leaving.into_iter().filter(|p| flat || p.symlink_metadata().is_ok()));
                let arrived = op.results();
                let any = !arrived.is_empty();
                this.arrive(&arrived, cx);
                this.select_when_listed(arrived);
                // Put somewhere not shown: said where.
                let elsewhere = flat || !this.shows(&dir);
                if elsewhere && any && error.is_none() && failed.is_empty() {
                    let said = label.replacen("Copying", "Copied", 1).replacen("Moving", "Moved", 1);
                    this.show_notice(said, cx);
                }
                // (A merge can't be undone: what it overwrote is gone.)
                if !done.merged {
                    this.record(op);
                }
                // Stopped (or the disk full, the drive gone): how far it got,
                // and that copying again continues. What was left out: listed.
                let verb = if copying { "copied" } else { "moved" };
                if let Some(e) = error {
                    let how_far = format!("{} of {} {verb}", human_size(copied.min(total)), human_size(total));
                    let why = if e.to_string() == "Stopped." { "Stopped".to_string() } else { fileops::reason(&e) };
                    this.show_notice(format!("{why}: {how_far}. Copy it again to continue where it stopped."), cx);
                }
                if !failed.is_empty() {
                    this.overlay = Some(super::sheets::Sheet::LeftOut { verb, items: failed });
                }
                this.reload_folders(&touched, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Redraw while operations run, for their progress.
    pub(super) fn tick_jobs(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let running = this.update(cx, |this, cx| {
                    cx.notify();
                    this.ticking = !this.jobs.is_empty();
                    this.ticking
                });
                if !running.unwrap_or(false) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Operations running for a while: what, how far, and a stop button.
    pub(super) fn render_jobs(&self, window: &mut Window) -> Option<AnyElement> {
        let palette = self.palette;
        let shown: Vec<&Job> = self.jobs.iter().filter(|j| j.started.elapsed() >= SHOW_AFTER).collect();
        if shown.is_empty() {
            return None;
        }
        Some(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_2()
                .border_t_1()
                .border_color(palette.separator)
                .children(shown.into_iter().map(|job| {
                    let (total, done) = (job.progress.total.load(Ordering::Relaxed), job.progress.done.load(Ordering::Relaxed));
                    let fraction = if total == 0 { 0. } else { (done as f32 / total as f32).min(1.) };
                    // How fast, and how long it'll take (once there's enough to tell).
                    let secs = job.started.elapsed().as_secs_f64();
                    let pace = (secs > 1.5 && done > 0).then(|| done as f64 / secs);
                    let said = match pace {
                        Some(pace) => format!("{} of {} · {}/s · {}", human_size(done.min(total)), human_size(total), human_size(pace as u64), time_left(total.saturating_sub(done) as f64 / pace)),
                        None => format!("{} of {}", human_size(done.min(total)), human_size(total)),
                    };
                    let progress = job.progress.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .gap_2()
                                        .text_size(px(11.))
                                        .child(div().truncate().text_color(palette.label).child(job.label.clone()))
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_color(palette.secondary)
                                                .child(said),
                                        ),
                                )
                                .child(
                                    div().h(px(4.)).rounded_full().bg(palette.glass_selected).child(
                                        div().h_full().rounded_full().bg(palette.accent).w(relative(fraction)),
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .id(("stop", job.id))
                                .child(symbol("xmark.circle.fill", 14., SEMIBOLD, palette.tertiary, window))
                                .on_click(move |_, _, _| progress.cancel.store(true, Ordering::Relaxed)),
                        )
                }))
                .into_any_element(),
        )
    }

    /// Finder's question when a name is taken: over the window, as a sheet.
    pub(super) fn render_conflict(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette;
        let Some(c) = self.conflicts.as_ref() else {
            self.motions.sheet_down("conflict");
            return None;
        };
        let item = c.items[c.at].clone();
        let left = c.items[c.at..].iter().filter(|i| i.taken).count();
        let apply_all = c.apply_all;
        let folders = item.folders();
        // (Left part-way by a copy that stopped: Continue copies the rest.)
        let partway = folders && fileops::left_partway(&item.dst);
        let name = item.dst.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let verb = if item.copy { "copying" } else { "moving" };
        let entry = Entry::from_path(item.src.clone()).unwrap_or_default();
        let icon = self.icons.get(&entry, (48. * window.scale_factor()).round() as u32);
        let button = |id: &'static str, label: &'static str, primary: bool, choice: Option<Resolve>, cx: &mut Context<Self>| {
            glass(26., &palette)
                .id(id)
                .px_4()
                .text_size(px(12.))
                .when(primary, |d| d.bg(palette.accent).text_color(palette.selected_text).border_color(palette.accent))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.resolve(choice, cx)))
        };
        let card = div()
            .mt(px(60.))
            .w(px(460.))
            // (A half-width pane is narrower.)
            .max_w(relative(0.92))
            .h_auto()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(18.))
            .bg(palette.sidebar)
            .border_1()
            .border_color(palette.glass_border)
            .shadow_2xl()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(super::items::file_image(icon, &entry, 48., palette.accent))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("An item named “{name}” already exists in this location.")),
                            )
                            .child(div().text_size(px(12.)).text_color(palette.secondary).child(if partway {
                                format!("It's the one a copy stopped part-way through: Continue {verb} what's missing, and leaves what's there.")
                            } else {
                                format!(
                                    "Do you want to replace it with the one you're {verb}?{}",
                                    if folders { " Merge puts what's in both folders together." } else { "" }
                                )
                            })),
                    ),
            )
            .when(left > 1, |d| {
                d.child(
                    div()
                        .id("apply-all")
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .child(
                            div()
                                .size(px(14.))
                                .rounded_sm()
                                .border_1()
                                .border_color(palette.glass_border)
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(apply_all, |d| d.bg(palette.accent))
                                .when(apply_all, |d| {
                                    d.child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))
                                }),
                        )
                        .child(format!("Apply to All ({left})"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(c) = this.conflicts.as_mut() {
                                c.apply_all = !c.apply_all;
                            }
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("stop", "Stop", false, None, cx))
                    .child(button("keep-both", "Keep Both", false, Some(Resolve::KeepBoth), cx))
                    .when(folders, |d| d.child(button("merge", "Merge", false, Some(Resolve::Merge), cx)))
                    .child(button("replace", "Replace", !partway, Some(Resolve::Replace), cx))
                    .when(partway, |d| d.child(button("continue", "Continue", true, Some(Resolve::Continue), cx))),
            );
        Some(self.sheet_motion("conflict", card.into_any_element()))
    }
}

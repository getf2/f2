//! Icons placed freely in icon view (Sort By: None), as Finder places them:
//! where Finder put them (its .DS_Store's, read when a folder's shown), or
//! where you've moved them in F2 (kept in F2's own file: Finder's files are
//! never written), the rest in the grid's first free places. Dragging moves
//! them (the selection together); Clean Up puts each in the grid's nearest
//! free place, Clean Up By lines them up by name, kind, date or size. The
//! arrow keys go to the nearest icon that way.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gpui::*;

use super::dnd::Dragged;
use super::items::{GRID_PADDING, cell_height, cell_width};
use super::{Finder, View};
use crate::listing::{Entry, SortKey};

/// Past this many, a folder's icons go in the grid (all of them laid out
/// at once would be slow).
const MOST: usize = 3000;

/// Icons' centers moved in F2, by folder, then name.
type Saved = HashMap<PathBuf, HashMap<String, (f32, f32)>>;

static SAVED: Mutex<Option<Saved>> = Mutex::new(None);

fn file() -> PathBuf {
    crate::settings::data_dir().join("icon-positions.json")
}

fn with_saved<R>(f: impl FnOnce(&mut Saved) -> R) -> R {
    let mut saved = SAVED.lock().unwrap();
    let saved = saved.get_or_insert_with(|| std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    f(saved)
}

/// Remember where these were put in `dir`.
fn save(dir: &Path, moved: &[(String, (f32, f32))]) {
    let json = with_saved(|saved| {
        // (Folders gone, forgotten, so the file stays small.)
        if saved.len() > 1000 {
            saved.retain(|d, _| d.is_dir());
        }
        let folder = saved.entry(dir.to_path_buf()).or_default();
        for (name, at) in moved {
            folder.insert(name.clone(), *at);
        }
        serde_json::to_vec(saved).ok()
    });
    if let Some(json) = json {
        let _ = std::fs::create_dir_all(crate::settings::data_dir());
        let _ = std::fs::write(file(), json);
    }
}

/// Where a folder's icons are: Finder's, then F2's over them; and whether
/// Finder arranges it freely (with icons it's placed).
pub(super) fn load(dir: &Path) -> (HashMap<String, (f32, f32)>, bool) {
    let finder = crate::ds_store::read(dir).unwrap_or_default();
    let free = finder.free && !finder.positions.is_empty();
    let mut positions = finder.positions;
    positions.extend(with_saved(|saved| saved.get(dir).cloned().unwrap_or_default()));
    (positions, free)
}

fn name(entry: &Entry) -> String {
    entry.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A grid place's cell (column, row), from a cell's top left.
fn slot_of(x: f32, y: f32, w: f32, h: f32) -> (i32, i32) {
    (((x - GRID_PADDING) / w).round() as i32, ((y - 8.) / h).round() as i32)
}

impl Finder {
    /// Whether icon view shows this folder's icons where they've been put.
    pub(super) fn free_layout_on(&self) -> bool {
        self.free && self.view == View::Icons && self.lines.is_empty() && !self.flat() && self.rows.len() <= MOST
    }

    /// The icon's center in its cell, from the cell's top left.
    fn center_in_cell(&self) -> (f32, f32) {
        (cell_width(self.icon_size) / 2., 8. + (self.icon_size + 12.) / 2.)
    }

    /// Each icon's cell's top left, and the space they take (the grid's
    /// first free places for those not put anywhere).
    pub(super) fn free_layout(&self) -> (Vec<(f32, f32)>, (f32, f32)) {
        let (w, h) = (cell_width(self.icon_size), cell_height(self.icon_size));
        let (cx, cy) = self.center_in_cell();
        let mut spots: Vec<Option<(f32, f32)>> = self.rows.iter().map(|e| self.placed.get(&name(e)).map(|(x, y)| ((x - cx).max(0.), (y - cy).max(0.)))).collect();
        let placed: Vec<(f32, f32)> = spots.iter().flatten().copied().collect();
        // (A place is taken by any icon much over it, on the grid or not.)
        let taken = |at: (f32, f32)| placed.iter().any(|p| (p.0 - at.0).abs() < w * 0.8 && (p.1 - at.1).abs() < h * 0.8);
        let columns = self.columns.max(1) as i32;
        let mut next = 0;
        for spot in spots.iter_mut().filter(|s| s.is_none()) {
            loop {
                let (c, r) = (next % columns, next / columns);
                next += 1;
                let at = (GRID_PADDING + c as f32 * w, 8. + r as f32 * h);
                if !taken(at) {
                    *spot = Some(at);
                    break;
                }
            }
        }
        let spots: Vec<(f32, f32)> = spots.into_iter().map(|s| s.unwrap_or_default()).collect();
        let size = spots.iter().fold((0f32, 0f32), |(mw, mh), (x, y)| (mw.max(x + w), mh.max(y + h)));
        (spots, (size.0 + GRID_PADDING, size.1 + 16.))
    }

    /// Icon view, freely arranged: each icon where it was put, scrolling
    /// both ways; a drop on the empty space puts the icons dragged there.
    pub(super) fn render_free(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (spots, (width, height)) = self.free_layout();
        let cells: Vec<AnyElement> = spots
            .iter()
            .enumerate()
            .map(|(ix, (x, y))| div().absolute().left(px(*x)).top(px(*y)).child(self.render_cell(ix, window, cx)).into_any_element())
            .collect();
        let canvas = div()
            .relative()
            .w(px(width))
            .h(px(height))
            .min_w_full()
            .min_h_full()
            .children(cells)
            .on_drag_move::<Dragged>(cx.listener(|this, e: &DragMoveEvent<Dragged>, _, _| {
                this.drag_at = Some((e.event.position, e.bounds.origin));
            }))
            .on_drop(cx.listener(|this, dragged: &Dragged, window, cx| this.drop_here(dragged, window, cx)));
        div().id("free-icons").flex_1().min_h_0().overflow_scroll().track_scroll(&self.free_scroll).child(canvas).into_any_element()
    }

    /// Icons dropped on the empty space: this folder's moved there (the
    /// selection kept as it was, around the one dragged); others' moved or
    /// copied into the folder, as anywhere else in it.
    fn drop_here(&mut self, dragged: &Dragged, window: &mut Window, cx: &mut Context<Self>) {
        let here = dragged.paths.iter().all(|p| p.parent() == Some(self.cwd.as_path()));
        let Some((at, origin)) = self.drag_at.take().filter(|_| here) else {
            return self.drop_paths(dragged.paths.to_vec(), self.cwd.clone(), window, cx);
        };
        let (spots, _) = self.free_layout();
        let (cx_, cy_) = self.center_in_cell();
        let spot_of = |path: &Path| self.rows.iter().position(|e| e.path == path).map(|ix| spots[ix]);
        let lead = dragged.lead.clone().or_else(|| dragged.paths.first().cloned());
        let Some(from) = lead.as_deref().and_then(spot_of) else { return };
        let to = (f32::from(at.x - origin.x - dragged.grab.x), f32::from(at.y - origin.y - dragged.grab.y));
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let moved: Vec<(String, (f32, f32))> = dragged
            .paths
            .iter()
            .filter_map(|p| {
                let (x, y) = spot_of(p)?;
                let name = p.file_name()?.to_string_lossy().into_owned();
                Some((name, ((x + dx).max(0.) + cx_, (y + dy).max(0.) + cy_)))
            })
            .collect();
        for (name, at) in &moved {
            self.placed.insert(name.clone(), *at);
        }
        save(&self.cwd, &moved);
        cx.notify();
    }

    /// Clean Up: each icon in the grid's nearest free place (in the order
    /// they're read, left to right, top down). With `by`, all of them lined
    /// up in that order instead.
    pub(super) fn clean_up(&mut self, by: Option<SortKey>, cx: &mut Context<Self>) {
        let (w, h) = (cell_width(self.icon_size), cell_height(self.icon_size));
        let (cx_, cy_) = self.center_in_cell();
        let columns = self.columns.max(1) as i32;
        let (spots, _) = self.free_layout();
        let mut order: Vec<usize> = (0..self.rows.len()).collect();
        let mut places: Vec<(i32, i32)> = Vec::new();
        match by {
            Some(key) => {
                let mut entries: Vec<Entry> = self.rows.clone();
                crate::listing::sort(&mut entries, key, true, crate::settings::Settings::get(cx).folders_on_top, &self.sizes);
                order = entries.iter().filter_map(|e| self.rows.iter().position(|r| r.path == e.path)).collect();
                places = (0..order.len() as i32).map(|n| (n % columns, n / columns)).collect();
            }
            None => {
                order.sort_by(|a, b| spots[*a].1.total_cmp(&spots[*b].1).then(spots[*a].0.total_cmp(&spots[*b].0)));
                let mut taken: HashSet<(i32, i32)> = HashSet::new();
                for ix in &order {
                    let (c, r) = slot_of(spots[*ix].0, spots[*ix].1, w, h);
                    let (c, r) = (c.clamp(0, columns - 1), r.max(0));
                    // (The nearest free one, looking further out each time.)
                    let mut found = (c, r);
                    'out: for reach in 0..1000i32 {
                        for dr in -reach..=reach {
                            for dc in -reach..=reach {
                                if dr.abs().max(dc.abs()) != reach {
                                    continue;
                                }
                                let at = (c + dc, r + dr);
                                if at.0 >= 0 && at.0 < columns && at.1 >= 0 && !taken.contains(&at) {
                                    found = at;
                                    break 'out;
                                }
                            }
                        }
                    }
                    taken.insert(found);
                    places.push(found);
                }
            }
        }
        let moved: Vec<(String, (f32, f32))> = order
            .iter()
            .zip(places)
            .map(|(ix, (c, r))| (name(&self.rows[*ix]), (GRID_PADDING + c as f32 * w + cx_, 8. + r as f32 * h + cy_)))
            .collect();
        for (name, at) in &moved {
            self.placed.insert(name.clone(), *at);
        }
        save(&self.cwd, &moved);
        cx.notify();
    }

    /// The arrow keys, freely arranged: the nearest icon that way (`dx`,
    /// `dy` one of -1, 0, 1), else the one that's selected stays.
    pub(super) fn select_toward(&mut self, dx: i32, dy: i32, cx: &mut Context<Self>) {
        let (spots, _) = self.free_layout();
        let Some(from) = self.selected.and_then(|ix| spots.get(ix).copied()) else {
            // (None selected: the top left's.)
            let first = (0..spots.len()).min_by(|a, b| (spots[*a].1 + spots[*a].0).total_cmp(&(spots[*b].1 + spots[*b].0)));
            self.select_only(first);
            return cx.notify();
        };
        let best = spots
            .iter()
            .enumerate()
            .filter(|(ix, _)| Some(*ix) != self.selected)
            .filter_map(|(ix, (x, y))| {
                let (ox, oy) = (x - from.0, y - from.1);
                // (That way: more that way than across, as Finder goes.)
                let along = ox * dx as f32 + oy * dy as f32;
                let across = (ox * dy as f32).abs() + (oy * dx as f32).abs();
                (along > 1. && across <= along * 2.).then_some((ix, along + across * 2.))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(ix, _)| ix);
        if let Some(ix) = best {
            self.select_only(Some(ix));
            self.scroll_free_to(ix);
            cx.notify();
        }
    }

    /// An icon renamed keeps its place here.
    pub(super) fn keep_place(&mut self, old: &Path, new: &Path) {
        let (Some(from), Some(to)) = (old.file_name(), new.file_name()) else { return };
        if old.parent() != Some(self.cwd.as_path()) || new.parent() != Some(self.cwd.as_path()) {
            return;
        }
        let (from, to) = (from.to_string_lossy().into_owned(), to.to_string_lossy().into_owned());
        if let Some(at) = self.placed.remove(&from) {
            self.placed.insert(to.clone(), at);
            save(&self.cwd, &[(to, at)]);
        }
    }

    /// Scroll so the icon at `ix` shows.
    pub(super) fn scroll_free_to(&self, ix: usize) {
        let (spots, _) = self.free_layout();
        let Some((x, y)) = spots.get(ix).copied() else { return };
        let (w, h) = (cell_width(self.icon_size), cell_height(self.icon_size));
        let view = self.free_scroll.bounds();
        let (vw, vh) = (f32::from(view.size.width), f32::from(view.size.height));
        if vw <= 0. || vh <= 0. {
            return;
        }
        let offset = self.free_scroll.offset();
        // (Offsets are negative, scrolled down or right.)
        let (sx, sy) = (-f32::from(offset.x), -f32::from(offset.y));
        let nx = if x < sx { x } else if x + w > sx + vw { x + w - vw } else { sx };
        let ny = if y < sy { y } else if y + h > sy + vh { y + h - vh } else { sy };
        if (nx, ny) != (sx, sy) {
            self.free_scroll.set_offset(point(px(-nx), px(-ny)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::slot_of;

    #[test]
    fn slots() {
        assert_eq!(slot_of(8., 8., 100., 90.), (0, 0));
        assert_eq!(slot_of(108., 98., 100., 90.), (1, 1));
        assert_eq!(slot_of(150., 140., 100., 90.), (1, 1), "nearest");
    }
}

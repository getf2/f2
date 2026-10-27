//! Gallery view, as Finder's: a big preview of the item selected (its Quick
//! Look thumbnail, so images, PDFs, videos and the rest), its name and what
//! it is, and the folder's thumbnails in a strip along the bottom (only
//! those in view drawn). ← → (and ↑ ↓) go along it; space is Quick Look.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::Finder;
use super::items::file_image;
use crate::listing::{date_label, human_size};
use crate::settings::Settings;

/// The strip's thumbnails, each one's room, and the strip's height.
const THUMB: f32 = 56.;
const CELL: f32 = THUMB + 14.;
const STRIP: f32 = CELL + 18.;
const STRIP_INSET: f32 = 8.;
/// The preview's size in pixels (one size: thumbnailed once).
const PREVIEW_PX: u32 = 1024;

impl Finder {
    pub(super) fn render_gallery(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let n = self.rows.len();
        // (Finder's gallery always shows one.)
        if self.selected.is_none() && n > 0 {
            self.select_only(Some(0));
        }
        let ix = self.selected.unwrap_or(0).min(n.saturating_sub(1));
        self.bring_into_strip(ix);
        let entry = self.rows[ix].clone();
        let show_all = Settings::get(cx).show_all_extensions;
        let preview = match self.icons.get(&entry, PREVIEW_PX) {
            Some(shown) => img(shown.image).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            None => div().into_any_element(),
        };
        let mut about = vec![entry.kind_label().to_string()];
        if !entry.is_dir || entry.is_bundle() {
            about.push(human_size(entry.size));
        }
        if entry.mtime > 0 {
            about.push(format!("Modified {}", date_label(entry.mtime)));
        }

        // The strip: just what's in view of it (and a little either side).
        let focused = self.focus.is_focused(window);
        let offset = -f32::from(self.gallery_scroll.offset().x);
        let first = ((offset / CELL).floor() as isize - 2).max(0) as usize;
        let last = (first + (self.list_width / CELL).ceil() as usize + 4).min(n);
        let thumb_px = (THUMB * window.scale_factor()).round() as u32;
        let cells: Vec<AnyElement> = (first..last).map(|i| self.render_strip_cell(i, thumb_px, focused, window, cx)).collect();
        let strip = div()
            .id("gallery-strip")
            .h(px(STRIP))
            .flex_shrink_0()
            .border_t_1()
            .border_color(palette.separator)
            .overflow_x_scroll()
            .track_scroll(&self.gallery_scroll)
            // (Scrolled: other cells come into view.)
            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
            .child(div().h_full().w(px(n as f32 * CELL + 2. * STRIP_INSET)).relative().children(cells));

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().p_4().flex().items_center().justify_center().child(preview))
            .child(
                div()
                    .flex_shrink_0()
                    .px_4()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .child(div().max_w_full().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(entry.display_name(show_all)))
                    .child(div().max_w_full().truncate().text_size(px(12.)).text_color(palette.secondary).child(about.join(" · "))),
            )
            .child(strip)
            .into_any_element()
    }

    fn render_strip_cell(&mut self, i: usize, thumb_px: u32, focused: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let selection = self.dragged_selection();
        let dragged = self.dragged_from(i, &selection, window);
        let selected = self.is_chosen(i);
        let entry = &self.rows[i];
        let shown = self.icons.get(entry, thumb_px);
        div()
            .id(("strip", i))
            .absolute()
            .left(px(STRIP_INSET + i as f32 * CELL))
            .top(px(STRIP_INSET))
            .size(px(CELL - 4.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_lg()
            .border_2()
            .border_color(transparent_black())
            .role(Role::ListItem)
            .aria_label(entry.name.clone())
            .when(entry.hidden || super::transfer::is_cut(&entry.path), |d| d.opacity(0.5))
            .when(selected, |d| d.bg(palette.glass_selected))
            .when(selected && focused, |d| d.border_color(palette.selection))
            .child(file_image(shown, entry, THUMB, palette.accent))
            .on_click(self.on_item_click(i, cx))
            .on_mouse_down(MouseButton::Left, self.on_item_press(cx))
            .on_mouse_down(MouseButton::Right, self.on_item_menu(i, cx))
            .on_mouse_down(MouseButton::Left, self.on_item_menu(i, cx))
            .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .external_drag_payload(super::dnd::files_for_other_apps)
            .into_any_element()
    }

    /// Scroll the strip to show item `ix` (once each time it changes:
    /// scrolling it by hand otherwise stays).
    fn bring_into_strip(&mut self, ix: usize) {
        if self.gallery_shown == Some(ix) {
            return;
        }
        self.gallery_shown = Some(ix);
        let offset = -f32::from(self.gallery_scroll.offset().x);
        let x = STRIP_INSET + ix as f32 * CELL;
        if x < offset || x + CELL > offset + self.list_width {
            let to = (x + CELL / 2. - self.list_width / 2.).max(0.);
            self.gallery_scroll.set_offset(point(px(-to), px(0.)));
        }
    }
}

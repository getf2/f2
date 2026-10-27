//! Quick Look following the selection, and the preview pane (⇧⌘P): the
//! selected item large, with what Get Info's More Info knows.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::items::file_image;
use super::{Finder, PREVIEW_PANE_WIDTH};
use crate::listing::{count, date_label};
use crate::macos;
use crate::settings::Settings;

impl Finder {
    /// With Quick Look up, show it what's selected now (from a task: see
    /// `toggle_preview`).
    pub(super) fn sync_quicklook(&mut self, cx: &mut Context<Self>) {
        if self.ql_shown.is_empty() {
            return;
        }
        if !macos::quicklook_visible() {
            // (Closed, unless it's still on its way up.)
            if !self.ql_opening {
                self.ql_shown.clear();
            }
            return;
        }
        let items = self.quicklook_items();
        if !items.is_empty() && items != self.ql_shown {
            self.ql_shown = items.clone();
            cx.spawn(async move |_, _| macos::quicklook_show(&items, 0)).detach();
        }
    }

    pub(super) fn render_preview_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let show_all = Settings::get(cx).show_all_extensions;
        let selection = self.selection();
        let pane = div()
            .w(px(PREVIEW_PANE_WIDTH))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(palette.separator)
            .p_4()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .overflow_hidden();
        let [entry] = selection.as_slice() else {
            let text = if selection.is_empty() { "No Selection".to_string() } else { format!("{} selected", count(selection.len(), "item")) };
            return pane.justify_center().text_color(palette.tertiary).child(text).into_any_element();
        };
        // More Info, looked up once per item (Spotlight).
        if self.preview_info.as_ref().is_none_or(|(p, _)| *p != entry.path) {
            self.preview_info = Some((entry.path.clone(), Vec::new()));
            let path = entry.path.clone();
            cx.spawn(async move |this, cx| {
                let target = path.clone();
                let info = cx.background_executor().spawn(async move { macos::more_info(&target) }).await;
                this.update(cx, |this, cx| {
                    if this.preview_info.as_ref().is_some_and(|(p, _)| *p == path) {
                        this.preview_info = Some((path, info));
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        let shown = self.icons.get(entry, (220. * window.scale_factor()).round() as u32);
        // Icons look best at icon sizes; thumbnails get the room.
        let image = if shown.as_ref().is_some_and(|s| s.thumbnail) { 200. } else { 128. };
        let mut rows: Vec<(String, String)> = vec![
            ("Created".into(), date_label(entry.created).to_string()),
            ("Modified".into(), entry.mtime_label().to_string()),
        ];
        if let Some((_, info)) = &self.preview_info {
            rows.extend(info.iter().cloned());
        }
        if !entry.tags.is_empty() {
            rows.push(("Tags".into(), entry.tags.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>().join(", ")));
        }
        let more = entry.clone();
        pane.child(div().size(px(220.)).flex().items_center().justify_center().child(file_image(shown, entry, image, palette.accent)))
            .child(div().max_w_full().text_center().font_weight(FontWeight::SEMIBOLD).child(entry.display_name(show_all)))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(palette.secondary)
                    .child(if entry.is_dir { entry.kind_label().to_string() } else { format!("{} · {}", entry.kind_label(), entry.size_label()) }),
            )
            .child(div().w_full().h(px(1.)).my_1().bg(palette.separator))
            .child(div().w_full().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("Information"))
            .children(rows.into_iter().map(|(label, value)| {
                div()
                    .w_full()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .text_size(px(11.))
                    .child(div().flex_shrink_0().text_color(palette.secondary).child(label))
                    .child(div().min_w_0().truncate().child(value))
            }))
            .child(
                div()
                    .id("more-info")
                    .mt_1()
                    .text_size(px(11.))
                    .text_color(palette.accent)
                    .cursor_pointer()
                    .child("More…")
                    .on_click(cx.listener(move |_, _, _, cx| crate::info::open(vec![more.clone()], cx))),
            )
            .when(entry.cloud == Some(macos::Cloud::NotDownloaded), |d| {
                d.child(div().text_size(px(11.)).text_color(palette.secondary).child("In iCloud (not downloaded)"))
            })
            .into_any_element()
    }
}

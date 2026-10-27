//! AirDrop, in the window: a place to drop (or choose) files to send to
//! people nearby. Sending goes through the system's own AirDrop window
//! (Finder's AirDrop page, with the people in it, isn't open to other
//! apps).

use std::path::PathBuf;
use std::time::Duration;

use gpui::*;

use super::Finder;
use super::dnd::Dragged;
use crate::macos;
use crate::style::{REGULAR, glass, symbol};

/// Where macOS keeps who can AirDrop to you.
const SETTINGS: &str = "x-apple.systempreferences:com.apple.AirDrop-Handoff-Settings.extension";

impl Finder {
    /// Send files with AirDrop. (From a task: the system's AirDrop window
    /// mustn't open in the middle of a GPUI update.)
    pub(super) fn send_airdrop(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(30)).await;
            if !macos::airdrop(&paths) {
                this.update(cx, |this, cx| {
                    this.show_notice("AirDrop isn't available. Check that Wi‑Fi and Bluetooth are on.", cx)
                })
                .ok();
            }
        })
        .detach();
    }

    /// Pick files to send.
    fn choose_for_airdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Send".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                this.update_in(cx, |this, window, cx| this.send_airdrop(paths, window, cx)).ok();
            }
        })
        .detach();
    }

    pub(super) fn render_airdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let highlight = palette.selection.opacity(0.12);
        div()
            .id("airdrop")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .m_4()
            .rounded(px(18.))
            .border_2()
            .border_dashed()
            .border_color(palette.separator)
            .drag_over::<Dragged>(move |s, _, _, _| s.bg(highlight).border_color(palette.accent))
            .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(highlight).border_color(palette.accent))
            .on_drop(cx.listener(|this, d: &Dragged, window, cx| this.send_airdrop(d.paths.to_vec(), window, cx)))
            .on_drop(cx.listener(|this, d: &ExternalPaths, window, cx| this.send_airdrop(d.paths().to_vec(), window, cx)))
            .child(
                div()
                    .size(px(120.))
                    .rounded_full()
                    .bg(palette.accent.opacity(0.15))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(symbol("dot.radiowaves.left.and.right", 52., REGULAR, palette.accent, window)),
            )
            .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child("AirDrop"))
            .child(
                div()
                    .max_w(px(360.))
                    .text_center()
                    .text_color(palette.secondary)
                    .child("Drop files here, or on AirDrop in the sidebar, to send them to people nearby."),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_2()
                    .child(
                        glass(28., &palette)
                            .id("airdrop-choose")
                            .px_4()
                            .bg(palette.accent)
                            .border_color(palette.accent)
                            .text_color(palette.selected_text)
                            .child("Choose Files…")
                            .on_click(cx.listener(|this, _, window, cx| this.choose_for_airdrop(window, cx))),
                    )
                    .child(
                        glass(28., &palette)
                            .id("airdrop-settings")
                            .px_4()
                            .child("Who Can Send to Me…")
                            .on_click(|_, _, cx| cx.open_url(SETTINGS)),
                    ),
            )
            .into_any_element()
    }
}

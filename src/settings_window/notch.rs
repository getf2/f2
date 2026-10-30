//! Settings › Notch (src/notch.rs): on or off, notes from it, screens
//! without a notch, what's playing; and a note to try it with.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::SettingsWindow;
use crate::settings::Settings;
use crate::style::{SEMIBOLD, glass, symbol};

impl SettingsWindow {
    pub(super) fn render_notch(&mut self, settings: &Settings, window: &mut Window) -> AnyElement {
        let palette = self.palette;
        let n = settings.notch.clone();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: &'static str| div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(text);
        let check = |id: &'static str, label: &'static str, on: bool, enabled: bool, f: fn(&mut Settings)| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .when(!enabled, |d| d.opacity(0.45))
                .child(
                    div()
                        .size(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(palette.glass_border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                )
                .child(label)
                .when(enabled, |d| d.on_click(move |_, _, cx| Settings::update(cx, f)))
        };
        div()
            .child(heading("Notch:"))
            .child(check("notch-on", "Show what's going on in the notch", n.on, true, |s| s.notch.on = !s.notch.on))
            .child(note(
                "A timer counting down, the stopwatch, a screen recording, dictation listening and what's playing show beside it. \
                 Point at the notch to see them all, stop them, and the last notes.",
            ))
            .child(div().mt_2().child(check("notch-notes", "Notes come down from the notch", n.notes, n.on, |s| s.notch.notes = !s.notch.notes)))
            .child(note("Else they show in the corner of the screen, as before."))
            .child(div().mt_2().child(check(
                "notch-screens",
                "Screens without a notch get one while something's going on",
                n.other_screens,
                n.on,
                |s| s.notch.other_screens = !s.notch.other_screens,
            )))
            .child(note("It hangs from the middle of the menu bar, and goes when there's nothing more to show."))
            .child(div().mt_2().child(check("notch-music", "What's playing in Music or Spotify", n.music, n.on, |s| s.notch.music = !s.notch.music)))
            .child(note("Beside the notch while it plays. Its artwork shows once F2 may ask the app, as “now playing” in Quick Open asks."))
            .child(
                div().mt_4().flex().child(
                    glass(26., &palette)
                        .id("notch-try")
                        .px_3()
                        .child("Show a Note")
                        .on_click(|_, _, cx| crate::shots::note("A Note".into(), "Notes come down from here. Point at the notch to see the last few again.".into(), None, cx)),
                ),
            )
            .into_any_element()
    }
}

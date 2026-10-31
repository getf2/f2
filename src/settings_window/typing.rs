//! Settings › Typing (src/typing.rs): on or off, the languages it types
//! besides English (Icelandic, which keys type what; Korean), the shortcut
//! that switches by hand, and what it still needs allowed.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::{Recording, SettingsWindow};
use crate::settings::Settings;
use crate::style::{SEMIBOLD, glass, symbol};

/// ABC's key, and what it types in Icelandic.
const KEYS: [(&str, &str); 5] = [("[", "ð"), (";", "æ"), ("/", "þ"), ("-", "ö"), ("' a", "á")];

impl SettingsWindow {
    pub(super) fn render_typing(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let t = settings.typing.clone();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: &'static str| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let key = |text: &'static str| {
            div().min_w(px(22.)).h(px(20.)).px_1().rounded(px(4.)).border_1().border_color(palette.glass_border).bg(palette.glass).flex().items_center().justify_center().text_size(px(12.)).child(text)
        };
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
            .child(heading("Typing:"))
            .child(check("typing-on", "Type in more than English on ABC, without switching", t.on, true, |s| s.typing.on = !s.typing.on))
            .child(div().pl(px(22.)).child(note(
                "Stay on ABC. As each word ends, it comes out in the language it's in, of English and those below. \
                 Every key not theirs types as on ABC, always.",
            )))
            .child(heading("Languages:"))
            .child(check("typing-icelandic", "Icelandic", t.icelandic, t.on, |s| s.typing.icelandic = !s.typing.icelandic))
            .child(div().pl(px(22.)).child(note(
                "The keys Icelandic has letters on type them when that makes a word: /a[ becomes það, while don't and and/or stay as typed. \
                 While you're writing Icelandic, the letters show as you type.",
            )))
            .child(div().pl(px(22.)).mt_1().mb_2().flex().flex_wrap().gap_3().children(KEYS.into_iter().map(|(abc, icelandic)| {
                div().flex().items_center().gap_1().text_size(px(12.)).child(key(abc)).child(div().text_color(palette.secondary).child("→")).child(icelandic)
            })))
            .child(check("typing-korean", "Korean", t.korean, t.on, |s| s.typing.korean = !s.typing.korean))
            .child(div().pl(px(22.)).child(note(
                "Keys as on Korean's 2-Set: dkssud becomes 안녕, rkatkgkqslek 감사합니다, zz ㅋㅋ. \
                 While you're writing Korean, Hangul builds up as you type, and a word that's English goes back as you finish it.",
            )))
            .when(t.on && !crate::macos::accessibility(false), |d| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!(
                            "Hearing what's typed, and typing in its place, need Accessibility: allow {} in System Settings > Privacy & Security > Accessibility.",
                            crate::APP_NAME
                        )))
                        .child(glass(24., &palette).id("typing-accessibility").px_3().text_size(px(12.)).child("Allow…").on_click(|_, _, cx| {
                            crate::macos::accessibility(true);
                            cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        })),
                )
            })
            .child(heading("Switch by hand:"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .when(!t.on, |d| d.opacity(0.45))
                    .child(div().flex_1().flex().flex_col().child("The next language").child(note(
                        "English, Icelandic, Korean (those on), and round again. The word you're typing, or the one you just typed, \
                         switches too. Words don't switch it back for 5 seconds.",
                    )))
                    .child(self.recorder(30_000, Recording::Typing, settings, window, cx)),
            )
            .child(heading("Good to know:"))
            .child(div().flex().flex_col().gap_1().children([
                note("Delete right after a word switched by itself puts it back as you typed it."),
                note("Each app keeps its language. In Terminal and code editors it only switches by hand: a[ there is code more often than að."),
                note("With the Icelandic layout itself (or Korean's input source) chosen in the menu bar, keys are left as they are."),
                note(
                    "Words from BÍN, Stofnun Árna Magnússonar í íslenskum fræðum (CC BY-SA 4.0), SCOWL, and Korean from OpenSubtitles \
                     by Hermit Dave's FrequencyWords (CC BY-SA 4.0). Nothing typed leaves this Mac.",
                ),
            ]))
            .into_any_element()
    }
}

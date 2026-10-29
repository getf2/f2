//! Settings › Dictation (src/dictation.rs): on or off, its shortcut (one
//! key or two, recorded as they're held), the language, how getting ready
//! goes, the microphone, and what it still needs allowed.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::SettingsWindow;
use crate::dictation::{self, Shortcut};
use crate::macos;
use crate::settings::{DictationMode, Settings};
use crate::style::{SEMIBOLD, glass, symbol};

/// Shortcuts to pick from, as well as recording one.
const PRESETS: [&str; 5] = ["right-option", "right-command", "fn", "control+option", "option+space"];

impl SettingsWindow {
    pub(super) fn render_dictation(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let d = settings.dictation.clone();
        let status = dictation::status();
        let recording = dictation::recording();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: String| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let warning = |id: &'static str, text: String, button: &'static str, on_click: fn(&mut App)| {
            div()
                .mt_2()
                .p_2()
                .rounded_md()
                .bg(rgb(0xff9f0a).opacity(0.12))
                .flex()
                .items_center()
                .gap_3()
                .child(div().flex_1().min_w_0().text_size(px(11.)).child(text))
                .child(glass(24., &palette).id(id).px_3().text_size(px(12.)).child(button).on_click(move |_, _, cx| on_click(cx)))
        };
        let shortcut = Shortcut::parse(&d.shortcut);
        let field = div()
            .id("dictation-shortcut")
            .w(px(150.))
            .h(px(24.))
            .rounded_md()
            .border_1()
            .border_color(if recording { palette.accent } else { palette.glass_border })
            .bg(palette.glass)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .text_color(palette.label)
            .child(if recording { "Hold a key, or two…".to_string() } else { shortcut.as_ref().map_or_else(|| "Record Shortcut".into(), Shortcut::label) })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.set_recording(None, window, cx);
                dictation::record(!recording, cx);
            }));
        let presets = div().flex().flex_wrap().items_center().gap_1().children(PRESETS.into_iter().enumerate().map(|(i, name)| {
            let on = shortcut.as_ref().is_some_and(|s| s.name() == name);
            let label = Shortcut::parse(name).map(|s| s.label()).unwrap_or_default();
            div()
                .id(("dictation-preset", i))
                .px_3()
                .py(px(3.))
                .rounded_md()
                .text_size(px(12.))
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(move |_, _, cx| {
                    dictation::record(false, cx);
                    Settings::update(cx, |s| {
                        s.dictation.shortcut = name.to_string();
                        s.dictation.on = true;
                    })
                })
        }));
        let languages = status.languages.clone();
        let named = |code: &str| languages.iter().find(|l| l.code == code).map_or_else(|| code.to_string(), |l| l.name.clone());
        let language = match (&d.language, &status.here) {
            (Some(code), _) => named(code),
            (None, Some(code)) => format!("This Mac's: {}", named(code)),
            (None, None) => "This Mac's".to_string(),
        };
        let chosen = d.language.clone();
        let pick = glass(26., &palette)
            .id("dictation-language")
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .child(language)
            .child(symbol("chevron.up.chevron.down", 9., SEMIBOLD, palette.secondary, window))
            .on_click(cx.listener(move |_, _, _, cx| {
                let mut items = vec![
                    macos::MenuItem { title: Some("This Mac's Language".into()), tag: 0, enabled: true, checked: chosen.is_none(), ..Default::default() },
                    macos::MenuItem { title: None, ..Default::default() },
                ];
                items.extend(languages.iter().enumerate().map(|(i, l)| macos::MenuItem {
                    title: Some(l.name.clone()),
                    tag: i as i32 + 1,
                    enabled: true,
                    checked: chosen.as_deref() == Some(&l.code),
                    ..Default::default()
                }));
                let codes: Vec<String> = languages.iter().map(|l| l.code.clone()).collect();
                // (Not inside this event's handling: the menu runs its own loop.)
                cx.spawn(async move |_, cx| {
                    let Some(tag) = macos::popup_menu(&items) else { return };
                    let language = (tag > 0).then(|| codes.get(tag as usize - 1).cloned()).flatten();
                    cx.update(|cx| Settings::update(cx, |s| s.dictation.language = language));
                })
                .detach();
            }));
        let inputs = macos::mic_inputs();
        let wanted = d.microphone.clone();
        let now = macos::mic_chosen(wanted.as_deref().unwrap_or("")).and_then(|uid| inputs.iter().find(|i| i.uid == uid).cloned());
        let picked = wanted.as_deref().filter(|w| *w != "system").and_then(|uid| inputs.iter().find(|i| i.uid == uid));
        let microphone = match wanted.as_deref() {
            None => "Automatic".to_string(),
            Some("system") => "Same as Sound Settings".to_string(),
            Some(_) => picked.map_or_else(|| "Automatic (it isn't connected)".to_string(), |i| i.name.clone()),
        };
        let about_mic = match (&now, wanted.as_deref()) {
            (Some(i), _) if i.kind == "bluetooth" => format!("Now {}: Bluetooth headphones switch to call quality while their microphone listens.", i.name),
            (Some(i), None) => format!("Now {}: Sound settings' input, but not Bluetooth headphones' (AirPods would switch to call quality).", i.name),
            (Some(i), _) => format!("Now {}.", i.name),
            (None, _) => "There's no microphone.".to_string(),
        };
        let listed: Vec<(String, String)> = inputs.iter().map(|i| (i.uid.clone(), i.name.clone())).collect();
        let pick_mic = glass(26., &palette)
            .id("dictation-microphone")
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .child(microphone)
            .child(symbol("chevron.up.chevron.down", 9., SEMIBOLD, palette.secondary, window))
            .on_click(cx.listener(move |_, _, _, cx| {
                let mut items = vec![
                    macos::MenuItem { title: Some("Automatic".into()), tag: 0, enabled: true, checked: wanted.is_none(), ..Default::default() },
                    macos::MenuItem { title: Some("Same as Sound Settings".into()), tag: 1, enabled: true, checked: wanted.as_deref() == Some("system"), ..Default::default() },
                    macos::MenuItem { title: None, ..Default::default() },
                ];
                items.extend(listed.iter().enumerate().map(|(i, (uid, name))| macos::MenuItem {
                    title: Some(name.clone()),
                    tag: i as i32 + 2,
                    enabled: true,
                    checked: wanted.as_deref() == Some(uid.as_str()),
                    ..Default::default()
                }));
                let uids: Vec<String> = listed.iter().map(|(uid, _)| uid.clone()).collect();
                // (Not inside this event's handling: the menu runs its own loop.)
                cx.spawn(async move |_, cx| {
                    let Some(tag) = macos::popup_menu(&items) else { return };
                    let microphone = match tag {
                        0 => None,
                        1 => Some("system".to_string()),
                        n => uids.get(n as usize - 2).cloned(),
                    };
                    cx.update(|cx| Settings::update(cx, |s| s.dictation.microphone = microphone));
                })
                .detach();
            }));
        let green: Hsla = rgb(0x30d158).into();
        let red: Hsla = rgb(0xff453a).into();
        let ready = match (&status.preparing, &status.error) {
            (Some(Some(part)), _) => Some((format!("Downloading its speech model… {}%", (part * 100.).round()), palette.secondary)),
            (Some(None), _) => Some(("Getting ready…".to_string(), palette.secondary)),
            (None, Some(why)) => Some((why.clone(), red)),
            (None, None) if d.on && status.language.is_some() => Some(("Ready".to_string(), green)),
            _ => None,
        };
        div()
            .child(heading("Dictation:"))
            .child(
                div()
                    .id("dictation-on")
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_sm()
                            .border_1()
                            .border_color(palette.glass_border)
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(d.on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                    )
                    .child("Dictate in any app, with a shortcut")
                    .on_click(|_, _, cx| Settings::update(cx, |s| s.dictation.on = !s.dictation.on)),
            )
            .child(div().pl(px(22.)).child(note(match d.mode {
                DictationMode::Hold => "Hold it while you speak, and let go: what you said is pasted where you're typing. Press it twice to keep listening hands-free, and once more to finish. Escape stops, pasting nothing.",
                DictationMode::Press => "Press it, speak, and press it again: what you said is pasted where you're typing. Escape stops, pasting nothing.",
            }
            .into())))
            .child(heading("Shortcut:"))
            .child(div().flex().items_center().gap_1().mb_1().children([(DictationMode::Hold, "Hold to talk"), (DictationMode::Press, "Press to start, again to stop")].into_iter().enumerate().map(
                |(i, (mode, label))| {
                    div()
                        .id(("dictation-mode", i))
                        .px_3()
                        .py(px(3.))
                        .rounded_md()
                        .text_size(px(12.))
                        .when(d.mode == mode, |d| d.bg(palette.glass_selected))
                        .child(label)
                        .on_click(move |_, _, cx| Settings::update(cx, |s| s.dictation.mode = mode))
                },
            )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .child(div().flex_1().flex().flex_col().child(if d.mode == DictationMode::Hold { "Hold to dictate" } else { "Press to dictate" }).child(note("One key held alone, or two together: click, then hold them and let go.".into())))
                    .child(field),
            )
            .when_some(status.hint.filter(|_| recording), |d, hint| d.child(div().text_size(px(11.)).text_color(red).child(hint)))
            .child(presets)
            .when(d.shortcut == "fn", |d| {
                d.child(note("If the Fn key also does something of its own (emoji, dictation), set System Settings > Keyboard > “Press 🌐 key to” to Do Nothing.".into()))
            })
            .child(heading("Language:"))
            .child(div().flex().items_center().gap_3().child(pick).when_some(ready, |d, (text, color)| d.child(div().text_size(px(12.)).text_color(color).child(text))))
            .child(heading("Microphone:"))
            .child(pick_mic)
            .child(div().mt_1().child(note(about_mic)))
            .when(!macos::accessibility(false), |d| {
                d.child(warning(
                    "dictation-accessibility",
                    format!("Hearing the shortcut and pasting need Accessibility: allow {} in System Settings > Privacy & Security > Accessibility.", crate::APP_NAME),
                    "Allow…",
                    |cx| {
                        macos::accessibility(true);
                        cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                    },
                ))
            })
            .when(d.on && macos::mic_access() != Some(true), |d| {
                d.child(warning("dictation-mic", "Dictation hears you through the microphone: allow it.".into(), "Allow…", |cx| {
                    crate::permissions::Permission::Microphone.ask(cx)
                }))
            })
            .child(div().mt_4().child(note(
                "What you say is turned into text on this Mac, by Apple's own speech recognition, and nothing is kept but the last text (Quick Open's Paste Last Dictation pastes it again).".into(),
            )))
            .child(
                div()
                    .id("dictation-hex")
                    .mt_1()
                    .text_size(px(11.))
                    .text_color(palette.tertiary)
                    .cursor_pointer()
                    .child("After Hex's dictation, by Kit Langton (github.com/anomalyco/hex).")
                    .on_click(|_, _, cx| cx.open_url("https://github.com/anomalyco/hex")),
            )
            .into_any_element()
    }
}

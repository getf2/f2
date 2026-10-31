//! What F2 asks macOS for, and why: Full Disk Access (every folder seen
//! and searched), Accessibility (windows moved, pasting, snippets'
//! keywords), Screen Recording (screenshots), reading the clipboard (its
//! history); for Quick Open, the calendar and Reminders; and for
//! dictation, the microphone. Each as it is now, with what grants it: the
//! welcome and Settings > Permissions show them.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::macos::{self, PasteAccess};
use crate::style::{Palette, SEMIBOLD, glass, symbol};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Permission {
    FullDisk,
    Accessibility,
    ScreenRecording,
    Clipboard,
    Calendar,
    Reminders,
    Contacts,
    Microphone,
}

impl Permission {
    pub const ALL: [Permission; 8] = [
        Permission::FullDisk,
        Permission::Accessibility,
        Permission::ScreenRecording,
        Permission::Clipboard,
        Permission::Calendar,
        Permission::Reminders,
        Permission::Contacts,
        Permission::Microphone,
    ];
    /// Those most of F2 needs (the calendar and Reminders are asked for on
    /// their own, as only Quick Open's events and reminders need them).
    pub const CORE: [Permission; 4] = [Permission::FullDisk, Permission::Accessibility, Permission::ScreenRecording, Permission::Clipboard];

    /// Its name, what it's for, its symbol and tile's color.
    pub fn about(self) -> (&'static str, &'static str, &'static str, u32) {
        match self {
            Permission::FullDisk => ("Full Disk Access", "So F2 can see and search every folder, as Finder does.", "externaldrive.fill", 0x8e8e93),
            Permission::Accessibility => ("Accessibility", "So F2 can move windows, paste from Quick Open and expand snippets.", "accessibility", 0x0a84ff),
            Permission::ScreenRecording => ("Screen Recording", "So F2 can take screenshots and recordings. They stay on your Mac.", "record.circle", crate::commands::SCREENSHOTS),
            Permission::Clipboard => ("Clipboard", "So F2 can keep what you copy. Password managers' secrets are left out.", "doc.on.clipboard.fill", crate::commands::CLIPBOARD),
            Permission::Calendar => ("Calendar", "Add events as you say them in Quick Open, and see what's next.", "calendar", 0xff3b30),
            Permission::Reminders => ("Reminders", "Add reminders from Quick Open, with an alert at the time.", "checklist", 0xff9500),
            Permission::Contacts => ("Contacts", "Call, message and email people from Quick Open: “call mom”.", "person.crop.circle.fill", 0x8e8e93),
            Permission::Microphone => ("Microphone", "Dictate in any app, and have your voice in screen recordings.", "mic.fill", crate::dictation::COLOR),
        }
    }

    /// Whether it's allowed now.
    pub fn allowed(self) -> bool {
        match self {
            Permission::FullDisk => macos::full_disk_access_now(),
            Permission::Accessibility => macos::accessibility(false),
            Permission::ScreenRecording => macos::capture_allowed(),
            Permission::Clipboard => macos::pasteboard_access() == PasteAccess::Allowed,
            Permission::Calendar => macos::calendar_access() == Some(true),
            Permission::Reminders => macos::reminders_access() == Some(true),
            Permission::Contacts => macos::contacts_access() == Some(true),
            Permission::Microphone => macos::mic_access() == Some(true),
        }
    }

    /// Ask for it: macOS's own prompt where there is one, and the pane in
    /// System Settings that turns it on.
    pub fn ask(self, cx: &mut App) {
        match self {
            Permission::FullDisk => cx.open_url(macos::FULL_DISK_ACCESS_SETTINGS),
            Permission::Accessibility => {
                if !macos::accessibility(true) {
                    cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
                }
            }
            Permission::ScreenRecording => {
                if !macos::capture_request() {
                    cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture");
                }
            }
            Permission::Clipboard => match macos::pasteboard_access() {
                // (Reading it once is what makes macOS ask.)
                PasteAccess::NotAsked => crate::clips::ask(),
                _ => cx.open_url("x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension"),
            },
            // (macOS asks once; after a no, only System Settings can.)
            Permission::Calendar => match macos::calendar_access() {
                None => ask_waiting(macos::calendar_request, cx),
                _ => cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"),
            },
            Permission::Reminders => match macos::reminders_access() {
                None => ask_waiting(macos::reminders_request, cx),
                _ => cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders"),
            },
            Permission::Contacts => match macos::contacts_access() {
                None => ask_waiting(macos::contacts_request, cx),
                _ => cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts"),
            },
            Permission::Microphone => match macos::mic_access() {
                None => macos::mic_request(),
                _ => cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"),
            },
        }
    }

    /// What to do in System Settings, once there. (None for what macOS
    /// asks about itself, until it's been said no to.)
    pub fn hint(self) -> Option<&'static str> {
        Some(match self {
            Permission::FullDisk => "In the list, turn F2 on (or add it with +). It works once F2 reopens.",
            Permission::Accessibility => "Turn F2 on in the list.",
            Permission::ScreenRecording => "Turn F2 on in the list. It works once F2 reopens.",
            Permission::Clipboard => "Under Paste from Other Apps, set F2 to Allow.",
            Permission::Calendar if macos::calendar_access().is_none() => return None,
            Permission::Reminders if macos::reminders_access().is_none() => return None,
            Permission::Contacts if macos::contacts_access().is_none() => return None,
            Permission::Microphone if macos::mic_access().is_none() => return None,
            Permission::Calendar => "Turn F2 on in the list (Full Access, to see what's next).",
            Permission::Reminders | Permission::Contacts | Permission::Microphone => "Turn F2 on in the list.",
        })
    }

    /// Whether macOS applies it only as F2 starts: allowed, F2 has to
    /// reopen.
    pub fn reopens(self) -> bool {
        matches!(self, Permission::FullDisk | Permission::ScreenRecording)
    }

    /// Its button: "Allow…", or where macOS won't ask again, "Open Settings…".
    pub fn button(self) -> &'static str {
        match self {
            Permission::Calendar | Permission::Reminders | Permission::Contacts | Permission::Microphone if self.hint().is_some() => "Open Settings…",
            _ => "Allow…",
        }
    }
}

/// F2 quit and opened again, for what macOS applies only as it starts:
/// once this one's gone, the app opens again (or, run as a program, the
/// program, as it was run). The welcome, if it was open, comes back at its
/// step.
pub fn reopen(cx: &mut App) {
    let Ok(exe) = std::env::current_exe() else { return };
    let app = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(std::path::Path::to_path_buf);
    let then = if app.is_some() { "open \"$0\"" } else { "exec \"$0\"" };
    let waiting = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("while kill -0 {} 2>/dev/null; do sleep 0.1; done; {then}", std::process::id()))
        .arg(app.as_deref().unwrap_or(&exe))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if waiting.is_ok() {
        cx.quit();
    }
}

/// Ask through EventKit, which waits for the answer: not on the main
/// thread. (The windows showing it look again each second.)
fn ask_waiting(request: fn() -> bool, cx: &mut App) {
    cx.background_executor()
        .spawn(async move {
            request();
        })
        .detach();
}

/// A line each: its tile, name and what it's for, then a green tick, or a
/// button that asks for it.
pub fn rows(palette: &Palette, window: &Window) -> Vec<AnyElement> {
    Permission::ALL
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let (name, about, sym, color) = p.about();
            let allowed = p.allowed();
            let green: Hsla = rgb(0x30d158).into();
            div()
                .id(("permission", i))
                .px_3()
                .py(px(10.))
                .rounded_lg()
                .border_1()
                .border_color(if allowed { green.opacity(0.35) } else { palette.glass_border })
                .bg(if allowed { green.opacity(0.06) } else { palette.glass })
                .flex()
                .items_center()
                .gap_3()
                .child(div().size(px(32.)).flex_shrink_0().rounded(px(8.)).bg(rgb(color)).flex().items_center().justify_center().child(symbol(sym, 16., SEMIBOLD, white(), window)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                        .child(div().text_size(px(12.)).text_color(palette.secondary).child(about))
                        .when_some((!allowed).then(|| p.hint()).flatten(), |d, hint| d.child(div().text_size(px(11.)).text_color(palette.tertiary).child(hint))),
                )
                .child(if allowed {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(green)
                        .child(symbol("checkmark.circle.fill", 14., SEMIBOLD, green, window))
                        .child("Allowed")
                        .into_any_element()
                } else {
                    glass(28., palette).id(("allow", i)).px_3().child(p.button()).on_click(move |_, _, cx| p.ask(cx)).into_any_element()
                })
                .into_any_element()
        })
        .collect()
}

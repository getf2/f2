//! The shortcuts that work in any app (Carbon hot keys: no permission
//! needed): Quick Open's and the screenshots'. Written as GPUI writes
//! keystrokes ("shift-cmd-4"), shown as macOS shows them ("⇧⌘4").

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{App, Keystroke};

use crate::macos;

/// Quick Open's, its clipboard history's and emoji's; the screenshots' are
/// `shots::Action`s' ids (10…), the windows' `windows::Place`s' (40…), the
/// rest `keyboard`'s (128…). Fewer than 512 (`HOT_KEYS`, in panel.m).
pub const QUICK_OPEN: u32 = 1;
pub const CLIPBOARD: u32 = 2;
pub const EMOJI: u32 = 3;
pub const SELECTION: u32 = 4;

static PRESSED: OnceLock<UnboundedSender<u32>> = OnceLock::new();
/// What each id is set to (kept while suspended, to set again).
static WANTED: Mutex<Option<HashMap<u32, (u32, u32)>>> = Mutex::new(None);
static SUSPENDED: Mutex<bool> = Mutex::new(false);

extern "C" fn on_pressed(id: u32) {
    if let Some(tx) = PRESSED.get() {
        let _ = tx.unbounded_send(id);
    }
}

/// Listen: Quick Open's comes up, a screenshot is taken.
pub fn init(cx: &mut App) {
    let (tx, mut rx) = unbounded();
    let _ = PRESSED.set(tx);
    macos::hot_keys_listen(on_pressed);
    cx.spawn(async move |cx| {
        while let Some(id) = rx.next().await {
            cx.update(|cx| match id {
                QUICK_OPEN => crate::quick_panel::show(None, cx),
                CLIPBOARD => crate::quick_panel::show_mode(crate::quick_panel::Mode::Clipboard, cx),
                EMOJI => crate::quick_panel::show_mode(crate::quick_panel::Mode::Emoji, cx),
                SELECTION => crate::quick_panel::show_selected(cx),
                id => {
                    if !crate::keyboard::pressed(id, cx) && !crate::windows::pressed(id, cx) {
                        crate::shots::pressed(id, cx)
                    }
                }
            });
        }
    })
    .detach();
}

/// Set shortcut `id` to `shortcut` ("shift-cmd-4"; None: none). False if
/// it can't be had (another app has it), or isn't one.
pub fn set(id: u32, shortcut: Option<&str>) -> bool {
    let key = shortcut.and_then(parse);
    {
        let mut wanted = WANTED.lock().unwrap();
        let wanted = wanted.get_or_insert_with(HashMap::new);
        match key {
            Some(key) => wanted.insert(id, key),
            None => wanted.remove(&id),
        };
    }
    if *SUSPENDED.lock().unwrap() {
        return true;
    }
    let had = macos::hot_key_set(id, key);
    had && (key.is_some() || shortcut.is_none())
}

/// Let go of every shortcut for now (while Settings records one: pressed,
/// it'd be taken instead of recorded), or take them again.
pub fn suspend(suspended: bool) {
    let mut now = SUSPENDED.lock().unwrap();
    if *now == suspended {
        return;
    }
    *now = suspended;
    let wanted = WANTED.lock().unwrap().clone().unwrap_or_default();
    for (id, key) in wanted {
        macos::hot_key_set(id, (!suspended).then_some(key));
    }
}

/// The Carbon modifiers' bits.
const CMD: u32 = 0x100;
const SHIFT: u32 = 0x200;
const ALT: u32 = 0x800;
const CTRL: u32 = 0x1000;

/// "shift-cmd-4" as a virtual key code and Carbon modifiers.
pub fn parse(shortcut: &str) -> Option<(u32, u32)> {
    let mut modifiers = 0;
    let mut key = None;
    for part in shortcut.split('-') {
        match part {
            "cmd" => modifiers |= CMD,
            "shift" => modifiers |= SHIFT,
            "alt" => modifiers |= ALT,
            "ctrl" => modifiers |= CTRL,
            "" => key = Some("-"),
            other => key = Some(other),
        }
    }
    Some((key_code(key?)?, modifiers))
}

/// How macOS writes it: "⌃⌥⇧⌘4".
pub fn label(shortcut: &str) -> String {
    let parts: Vec<&str> = shortcut.split('-').collect();
    let has = |m: &str| parts.contains(&m);
    let key = parts.iter().rev().find(|p| !matches!(**p, "cmd" | "shift" | "alt" | "ctrl" | "fn")).copied().unwrap_or("-");
    // ("cmd->", as F2's own keys have it: ⇧⌘.)
    let (key, shifted) = unshifted(key);
    let has = |m: &str| has(m) || (m == "shift" && shifted);
    let key = match key.as_str() {
        "space" => "Space".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "enter" => "↩".to_string(),
        "backspace" => "⌫".to_string(),
        "delete" => "⌦".to_string(),
        "tab" => "⇥".to_string(),
        "escape" => "⎋".to_string(),
        "home" => "↖".to_string(),
        "end" => "↘".to_string(),
        "pageup" => "⇞".to_string(),
        "pagedown" => "⇟".to_string(),
        "" => "-".to_string(),
        k if k.starts_with('f') && k.len() > 1 => k.to_uppercase(),
        k => k.to_uppercase(),
    };
    let mut out = String::new();
    for (m, sign) in [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")] {
        if has(m) {
            out.push_str(sign);
        }
    }
    out + &key
}

/// Whether two shortcuts are the same keys: "shift-cmd-." (as Carbon's
/// are written) and "cmd->" (as GPUI binds it).
pub fn same(a: &str, b: &str) -> bool {
    normal(a) == normal(b)
}

/// ⌃, ⌥, ⇧, ⌘, and the key (unshifted).
fn normal(shortcut: &str) -> (bool, bool, bool, bool, String) {
    let parts: Vec<&str> = shortcut.split('-').collect();
    let key = parts.iter().rev().find(|p| !matches!(**p, "cmd" | "shift" | "alt" | "ctrl" | "fn")).copied().unwrap_or("-");
    let (key, shifted) = unshifted(&if key.is_empty() { "-".to_string() } else { key.to_lowercase() });
    let has = |m: &str| parts.contains(&m);
    (has("ctrl"), has("alt"), has("shift") || shifted, has("cmd"), key)
}

/// A keystroke as a shortcut, if it can be one: with ⌘, ⌃ or ⌥, or a
/// function key.
pub fn from_keystroke(keystroke: &Keystroke) -> Option<String> {
    let (key, shifted) = unshifted(&keystroke.key.to_lowercase());
    key_code(&key)?;
    let m = keystroke.modifiers;
    let function = key.starts_with('f') && key.len() > 1;
    if !(m.platform || m.control || m.alt || function) {
        return None;
    }
    let mut parts = Vec::new();
    for (on, name) in [(m.control, "ctrl"), (m.alt, "alt"), (m.shift || shifted, "shift"), (m.platform, "cmd")] {
        if on {
            parts.push(name);
        }
    }
    parts.push(&key);
    Some(parts.join("-"))
}

/// The key a shifted character is typed with ("$": 4), as on an ANSI
/// keyboard: macOS names a key pressed with ⇧ by what it types.
fn unshifted(key: &str) -> (String, bool) {
    const SHIFTED: [(char, char); 21] = [
        ('!', '1'), ('@', '2'), ('#', '3'), ('$', '4'), ('%', '5'), ('^', '6'), ('&', '7'), ('*', '8'), ('(', '9'), (')', '0'), ('_', '-'),
        ('+', '='), ('{', '['), ('}', ']'), (':', ';'), ('"', '\''), ('<', ','), ('>', '.'), ('?', '/'), ('~', '`'), ('|', '\\'),
    ];
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && let Some(&(_, base)) = SHIFTED.iter().find(|(s, _)| *s == c)
    {
        return (base.to_string(), true);
    }
    (key.to_string(), false)
}

/// The virtual key code (an ANSI keyboard's) of a key as GPUI names it.
pub(crate) fn key_code(key: &str) -> Option<u32> {
    const LETTERS: [u32; 26] = [0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16, 6];
    const DIGITS: [u32; 10] = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25];
    const FUNCTION: [u32; 12] = [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111];
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return match c {
            'a'..='z' => Some(LETTERS[c as usize - 'a' as usize]),
            '0'..='9' => Some(DIGITS[c as usize - '0' as usize]),
            '-' => Some(27),
            '=' => Some(24),
            '[' => Some(33),
            ']' => Some(30),
            ';' => Some(41),
            '\'' => Some(39),
            ',' => Some(43),
            '.' => Some(47),
            '/' => Some(44),
            '\\' => Some(42),
            '`' => Some(50),
            _ => None,
        };
    }
    match key {
        "space" => Some(49),
        "left" => Some(123),
        "right" => Some(124),
        "down" => Some(125),
        "up" => Some(126),
        "enter" => Some(36),
        "backspace" => Some(51),
        "delete" => Some(117),
        "tab" => Some(48),
        "escape" => Some(53),
        "home" => Some(115),
        "end" => Some(119),
        "pageup" => Some(116),
        "pagedown" => Some(121),
        f if f.starts_with('f') => f[1..].parse::<usize>().ok().filter(|n| (1..=12).contains(n)).map(|n| FUNCTION[n - 1]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{label, parse};

    #[test]
    fn shortcuts() {
        assert_eq!(parse("shift-cmd-4"), Some((21, 0x300)));
        assert_eq!(parse("cmd-space"), Some((49, 0x100)));
        assert_eq!(parse("ctrl-alt-o"), Some((31, 0x1800)));
        assert_eq!(parse("alt-f5"), Some((96, 0x800)));
        assert_eq!(parse("cmd-"), Some((27, 0x100)), "the minus key");
        assert_eq!(parse("shift-cmd-é"), None);
        assert_eq!(label("shift-cmd-4"), "⇧⌘4");
        assert_eq!(label("ctrl-alt-shift-cmd-o"), "⌃⌥⇧⌘O");
        assert_eq!(label("cmd-space"), "⌘Space");
        assert_eq!(parse("ctrl-alt-left"), Some((123, 0x1800)));
        assert_eq!(label("ctrl-alt-left"), "⌃⌥←");
        assert_eq!(label("ctrl-alt--"), "⌃⌥-");
        assert_eq!(parse("ctrl-alt--"), Some((27, 0x1800)));
        assert_eq!(super::unshifted("$"), ("4".to_string(), true));
        assert_eq!(super::unshifted("o"), ("o".to_string(), false));
        assert_eq!(label("cmd->"), "⇧⌘.");
        assert_eq!(label("cmd-shift-n"), "⇧⌘N");
        assert_eq!(label("ctrl-shift-tab"), "⌃⇧⇥");
        assert!(super::same("cmd->", "shift-cmd-."));
        assert!(super::same("cmd-shift-n", "shift-cmd-n"));
        assert!(super::same("ctrl-alt--", "alt-ctrl--"));
        assert!(!super::same("cmd-n", "shift-cmd-n"));
        assert!(!super::same("cmd--", "cmd-="));
    }
}

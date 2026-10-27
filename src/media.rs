//! Sound, music and the screen, from Quick Open: "volume 30", "volume up",
//! "mute", "unmute", "brightness 50", "brightness down", "dim"; and Play/
//! Pause, Next Track and Previous Track, as the keyboard's media keys (to
//! whatever's playing). The built-in screen's brightness only: an external
//! one's own buttons set its.

use gpui::App;

/// How loud, or bright: to so much (0 to 1), or a step up or down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    To(f32),
    Up,
    Down,
    /// As it is, with the ways to change it.
    Show,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    Volume(Level),
    Mute(bool),
    Brightness(Level),
}

/// A step up or down.
const STEP: f32 = 0.1;

/// "30", "30%", "to 30", "max", "half", "up", "louder".
fn level(rest: &str, up: &[&str], down: &[&str]) -> Option<Level> {
    let rest = rest.trim().trim_start_matches("to ").trim_start_matches("at ").trim();
    if rest.is_empty() {
        return Some(Level::Show);
    }
    if up.contains(&rest) {
        return Some(Level::Up);
    }
    if down.contains(&rest) {
        return Some(Level::Down);
    }
    match rest {
        "max" | "full" | "100" | "100%" => return Some(Level::To(1.)),
        "half" => return Some(Level::To(0.5)),
        "off" | "0" | "0%" => return Some(Level::To(0.)),
        _ => {}
    }
    let n: f32 = rest.trim_end_matches('%').trim().parse().ok()?;
    (0. ..=100.).contains(&n).then_some(Level::To(n / 100.))
}

/// What's asked, if it's the sound or the screen.
pub fn parse(q: &str) -> Option<Ask> {
    let lower = q.trim().to_lowercase();
    let lower = lower.as_str();
    match lower {
        "mute" | "mute sound" | "mute the sound" | "mute volume" | "silence" => return Some(Ask::Mute(true)),
        "unmute" | "unmute sound" | "unmute the sound" | "unmute volume" => return Some(Ask::Mute(false)),
        "louder" | "turn it up" => return Some(Ask::Volume(Level::Up)),
        "quieter" | "softer" | "turn it down" => return Some(Ask::Volume(Level::Down)),
        "brighter" => return Some(Ask::Brightness(Level::Up)),
        "dimmer" | "dim" | "dim screen" | "dim the screen" | "darker" => return Some(Ask::Brightness(Level::Down)),
        _ => {}
    }
    let after = |prefixes: &[&str]| prefixes.iter().find_map(|p| lower.strip_prefix(p).filter(|r| r.is_empty() || r.starts_with(' ')));
    if let Some(rest) = after(&["set the volume", "set volume", "volume", "vol"]) {
        return level(rest, &["up", "louder", "higher"], &["down", "quieter", "lower", "softer"]).map(Ask::Volume);
    }
    if let Some(rest) = after(&["set the brightness", "set brightness", "screen brightness", "brightness"]) {
        return level(rest, &["up", "higher", "brighter"], &["down", "lower", "dimmer", "darker"]).map(Ask::Brightness);
    }
    None
}

/// "55%".
pub fn percent(v: f32) -> String {
    format!("{}%", (v * 100.).round() as i32)
}

/// Where a level goes from `now`.
pub fn target(level: Level, now: f32) -> f32 {
    match level {
        Level::To(v) => v,
        Level::Up => (now + STEP).min(1.),
        Level::Down => (now - STEP).max(0.),
        Level::Show => now,
    }
}

/// Do it, and say how it is now.
pub fn apply(ask: Ask, cx: &mut App) {
    let (title, detail) = match ask {
        Ask::Volume(level) => match crate::macos::volume(None) {
            None => ("No Sound Output".into(), "There's nothing to set the volume of.".into()),
            Some(now) => {
                let to = target(level, now);
                crate::macos::volume(Some(to));
                // (Setting the volume means hearing it.)
                if to > 0. {
                    crate::macos::muted(Some(false));
                }
                (format!("Volume {}", percent(to)), String::new())
            }
        },
        Ask::Mute(on) => match crate::macos::muted(Some(on)) {
            Some(muted) => (if muted { "Muted".into() } else { "Sound On".into() }, String::new()),
            None => ("Can't Mute".into(), "The sound output can't be muted.".into()),
        },
        Ask::Brightness(level) => match crate::macos::brightness(None) {
            None => ("No Built-In Screen".into(), "Brightness is set here for a Mac's own screen; an external one has its own buttons.".into()),
            Some(now) => {
                let to = target(level, now);
                crate::macos::brightness(Some(to));
                (format!("Brightness {}", percent(to)), String::new())
            }
        },
    };
    crate::shots::note(title, detail, None, cx);
}

/// A media key pressed (with Accessibility: macOS asks the first time).
pub fn press(key: crate::macos::MediaKey, cx: &mut App) {
    if !crate::macos::accessibility(false) {
        crate::macos::accessibility(true);
        return crate::shots::note(
            "Allow F2 to Press the Media Keys".into(),
            format!("{} plays, pauses and skips as the keyboard does, once it's allowed in System Settings > Privacy & Security > Accessibility.", crate::APP_NAME),
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
            cx,
        );
    }
    crate::macos::media_key(key);
}

#[cfg(test)]
mod tests {
    use super::{Ask, Level, parse, target};

    #[test]
    fn asked() {
        assert_eq!(parse("volume 30"), Some(Ask::Volume(Level::To(0.3))));
        assert_eq!(parse("Volume 30%"), Some(Ask::Volume(Level::To(0.3))));
        assert_eq!(parse("set volume to 75"), Some(Ask::Volume(Level::To(0.75))));
        assert_eq!(parse("volume up"), Some(Ask::Volume(Level::Up)));
        assert_eq!(parse("volume"), Some(Ask::Volume(Level::Show)));
        assert_eq!(parse("mute"), Some(Ask::Mute(true)));
        assert_eq!(parse("brightness 50"), Some(Ask::Brightness(Level::To(0.5))));
        assert_eq!(parse("dim"), Some(Ask::Brightness(Level::Down)));
        assert_eq!(parse("volume 300"), None);
        assert_eq!(parse("volumes"), None);
        assert_eq!(parse("volume control app"), None);
    }

    #[test]
    fn steps() {
        assert_eq!(target(Level::Up, 0.95), 1.);
        assert_eq!(target(Level::Down, 0.05), 0.);
    }
}

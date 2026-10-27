//! Keeping the Mac awake, from Quick Open, as Amphetamine does: "keep
//! awake", "keep awake for 2 hours", "keep awake until 5pm", "keep awake
//! until downloads finish", "caffeinate". Its display stays on too. A cup
//! beside the menu bar item says so, and its menu stops it.

use std::sync::Mutex;

use gpui::App;

/// Till when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Until {
    /// Till it's stopped.
    Stopped,
    /// Then (seconds since 1970).
    At(i64),
    /// Till nothing's downloading into Downloads.
    Downloads,
}

static AWAKE: Mutex<Option<Until>> = Mutex::new(None);

/// What's asked: keeping awake, and till when.
pub fn parse(q: &str) -> Option<Until> {
    let lower = q.trim().to_lowercase();
    let rest = [
        "keep my mac awake",
        "keep mac awake",
        "keep the mac awake",
        "keep computer awake",
        "keep screen on",
        "keep the screen on",
        "keep awake",
        "stay awake",
        "caffeinate",
        "don't sleep",
        "dont sleep",
        "no sleep",
    ]
    .iter()
    .find_map(|p| lower.strip_prefix(p).filter(|r| r.is_empty() || r.starts_with(' ')))?
    .trim();
    if rest.is_empty() {
        return Some(Until::Stopped);
    }
    let rest = rest.strip_prefix("for ").unwrap_or(rest);
    if ["until downloads finish", "until downloads are done", "until the download finishes", "until downloads are finished", "while downloading", "until downloaded", "until the downloads finish"].contains(&rest) {
        return Some(Until::Downloads);
    }
    // "until 5pm": through the reminders' way with times.
    if let Some(time) = rest.strip_prefix("until ").or_else(|| rest.strip_prefix("till ")) {
        let due = crate::reminders::parse(&format!("remind me x at {time}"))?.due?;
        return due.timed.then_some(Until::At(due.at));
    }
    // "2 hours", "30 min", "1h".
    let words: Vec<&str> = rest.split_whitespace().collect();
    let (n, unit) = match words[..] {
        [n, unit] => (n.parse::<f64>().ok()?, unit),
        [joined] => {
            let digits = joined.find(|c: char| !c.is_ascii_digit() && c != '.')?;
            (joined[..digits].parse().ok()?, &joined[digits..])
        }
        _ => return None,
    };
    let secs = match unit {
        "m" | "min" | "mins" | "minute" | "minutes" => 60.,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.,
        _ => return None,
    };
    Some(Until::At(crate::timers::now() + (n * secs) as i64))
}

/// "until you stop it", "until 18:30", "until downloads finish".
pub fn till(until: Until) -> String {
    match until {
        Until::Stopped => "until you stop it".into(),
        Until::At(at) => format!("until {}", crate::macos::zone_format(&crate::macos::local_zone(), at, "jmm")),
        Until::Downloads => "until downloads finish".into(),
    }
}

/// As it is: kept awake, till when.
pub fn state() -> Option<Until> {
    *AWAKE.lock().unwrap()
}

/// Whether anything's downloading into Downloads (a browser's file still
/// coming in: ".download", ".crdownload", ".part").
fn downloading() -> bool {
    let downloads = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Downloads");
    std::fs::read_dir(downloads)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| crate::tidy::PARTIAL.contains(&x.to_string_lossy().to_lowercase().as_str())))
}

pub fn start(until: Until, cx: &mut App) {
    if until == Until::Downloads && !downloading() {
        return crate::shots::note("Nothing's Downloading".into(), "There's nothing coming into Downloads to stay awake for.".into(), None, cx);
    }
    if !crate::macos::keep_awake(true) {
        return crate::shots::note("Couldn't Keep the Mac Awake".into(), "macOS didn't let it.".into(), None, cx);
    }
    *AWAKE.lock().unwrap() = Some(until);
    crate::shots::note("Keeping Your Mac Awake".into(), format!("And its screen on, {}.", till(until)), None, cx);
    crate::status_item::refresh(cx);
}

/// Let it sleep again (saying why, if it's not that you stopped it).
pub fn stop(why: Option<&str>, cx: &mut App) {
    crate::macos::keep_awake(false);
    *AWAKE.lock().unwrap() = None;
    crate::shots::note("Your Mac Can Sleep Again".into(), why.unwrap_or("").into(), None, cx);
    crate::status_item::refresh(cx);
}

/// Stopped when it's time, or when the downloads are done (looked at every
/// few seconds).
pub fn tick(cx: &mut App) {
    match state() {
        Some(Until::At(at)) if crate::timers::now() >= at => stop(None, cx),
        Some(Until::Downloads) if crate::timers::now() % 5 == 0 && !downloading() => stop(Some("The downloads are done."), cx),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{Until, parse};

    #[test]
    fn asked() {
        assert_eq!(parse("keep awake"), Some(Until::Stopped));
        assert_eq!(parse("Caffeinate"), Some(Until::Stopped));
        assert_eq!(parse("keep awake until downloads finish"), Some(Until::Downloads));
        assert!(matches!(parse("keep awake for 2 hours"), Some(Until::At(_))));
        assert!(matches!(parse("keep awake 30m"), Some(Until::At(_))));
        assert!(matches!(parse("keep awake until 11pm"), Some(Until::At(_))));
        assert_eq!(parse("keep awake forever and ever"), None);
        assert_eq!(parse("keepass"), None);
    }
}

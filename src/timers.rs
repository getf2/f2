//! Timers, from Quick Open: "timer 10 min", "timer 25m tea", "10 minute
//! timer", "set a timer for 1h30m", "pomodoro". The soonest one counts down
//! beside the menu bar item; when one's done, a sound and a note. "timers"
//! lists them to cancel, as the menu bar item's menu does. Breaks, again
//! and again ("eye break every 20 min", "stand up every hour"); and a
//! stopwatch, counting up in the menu bar.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::App;

#[derive(Clone, Debug, PartialEq)]
pub struct Timer {
    pub id: u64,
    /// What it's for ("Tea"), if said.
    pub label: String,
    pub secs: u64,
    /// When it's done (seconds since 1970).
    pub ends: i64,
    /// An alarm: at a time of day ("alarm 7am"), not counted down.
    pub alarm: bool,
    /// What's done when it's done: rung, or the Mac put to sleep, or the
    /// music stopped (a sleep timer).
    pub then: Then,
    /// Seconds till it's due again, once it's done (a break's), or 0.
    pub every: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Then {
    Ring,
    Sleep,
    StopMusic,
}

/// A sleep timer asked for: "sleep in 30 min", "stop music in 20 minutes",
/// "sleep at 11pm". What, and in how many seconds.
pub fn sleep_asked(q: &str) -> Option<(Then, u64)> {
    sleep_at(q, chrono::Local::now())
}

fn sleep_at(q: &str, now: chrono::DateTime<chrono::Local>) -> Option<(Then, u64)> {
    let lower = q.trim().to_lowercase();
    const LEADS: &[(&str, Then)] = &[
        ("sleep timer ", Then::Sleep),
        ("go to sleep ", Then::Sleep),
        ("sleep mac ", Then::Sleep),
        ("mac sleep ", Then::Sleep),
        ("sleep ", Then::Sleep),
        ("stop music ", Then::StopMusic),
        ("pause music ", Then::StopMusic),
        ("music off ", Then::StopMusic),
        ("stop the music ", Then::StopMusic),
        ("stop playing ", Then::StopMusic),
    ];
    let (lead, then) = LEADS.iter().find(|(l, _)| lower.starts_with(l))?;
    let rest = lower[lead.len()..].trim();
    let rest_words: Vec<&str> = rest.split_whitespace().collect();
    // "in 30 min", "after an hour", "30 min".
    let words = match rest_words.first() {
        Some(&"in") | Some(&"after") | Some(&"for") => &rest_words[1..],
        _ => &rest_words[..],
    };
    if let Some((secs, used)) = duration(words)
        && used == words.len()
        && (60..=12 * 3600).contains(&secs)
    {
        return Some((*then, secs));
    }
    // "at 11pm".
    if rest.starts_with("at ") {
        let said = crate::reminders::split_when(&format!("Sleep {rest}"), now)?;
        let due = said.due.filter(|d| d.timed)?;
        let secs = due.at - now.timestamp();
        return (60..=24 * 3600).contains(&secs).then_some((*then, secs as u64));
    }
    None
}

/// A sleep timer: the Mac put to sleep, or the music stopped, in `secs`.
pub fn start_sleep(then: Then, secs: u64, cx: &mut App) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let label = if then == Then::Sleep { "Sleep" } else { "Stop Music" }.to_string();
    let at = crate::macos::zone_format(&crate::macos::local_zone(), now() + secs as i64, "jmm");
    let what = if then == Then::Sleep { "The Mac sleeps" } else { "The music stops" };
    crate::shots::note(format!("{what} at {at}"), format!("In {}. The menu bar item cancels it.", till(secs)), None, cx);
    TIMERS.lock().unwrap().push(Timer { id, label, secs, ends: now() + secs as i64, alarm: false, then, every: 0 });
    crate::status_item::refresh(cx);
    if then == Then::StopMusic {
        // (Asked now, while you're here, whether F2 may tell Music and
        // Spotify to pause: macOS asks the first time.)
        std::thread::spawn(|| {
            music("get player state");
        });
    }
}

/// Tell Music and Spotify (those running) `command`: whether any of them
/// was playing, before.
fn music(command: &str) -> bool {
    let mut playing = false;
    for app in ["Music", "Spotify"] {
        let script = format!("if application \"{app}\" is running then tell application \"{app}\" to get player state as string");
        let state = std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase()).unwrap_or_default();
        if state == "playing" {
            playing = true;
            if command != "get player state" {
                let _ = std::process::Command::new("/usr/bin/osascript").args(["-e", &format!("tell application \"{app}\" to {command}")]).output();
            }
        }
    }
    playing
}

static TIMERS: Mutex<Vec<Timer>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// "10 min", "1h30m", "90s", "1:30", "25" (minutes): in seconds; what's left
/// over is the label.
fn duration(words: &[&str]) -> Option<(u64, usize)> {
    let unit = |u: &str| -> Option<u64> {
        match u.trim_end_matches([',', '.']) {
            "s" | "sec" | "secs" | "second" | "seconds" => Some(1),
            "m" | "min" | "mins" | "minute" | "minutes" => Some(60),
            "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600),
            _ => None,
        }
    };
    let mut total = 0;
    let mut used = 0;
    while let Some(word) = words.get(used) {
        let word = word.trim_end_matches(',');
        // "1:30": minutes and seconds ("1:30:00": hours too).
        if word.contains(':') {
            let parts: Option<Vec<u64>> = word.split(':').map(|p| p.parse().ok()).collect();
            let parts = parts?;
            total += match parts[..] {
                [m, s] if s < 60 => m * 60 + s,
                [h, m, s] if m < 60 && s < 60 => h * 3600 + m * 60 + s,
                _ => return None,
            };
            used += 1;
            continue;
        }
        let n_end = word.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(word.len());
        let n = match &word[..n_end] {
            "" => match word {
                "a" | "an" | "one" if used + 1 < words.len() && unit(words[used + 1]).is_some() => 1.,
                "half" if words.get(used + 1) == Some(&"an") && words.get(used + 2).is_some_and(|w| unit(w) == Some(3600)) => {
                    total += 1800;
                    used += 3;
                    continue;
                }
                "and" if total > 0 => {
                    used += 1;
                    continue;
                }
                _ => break,
            },
            n => n.parse::<f64>().ok()?,
        };
        // "1h30m": several in one word.
        let mut rest = &word[n_end..];
        if rest.is_empty() {
            match words.get(used + 1).and_then(|u| unit(u)) {
                Some(secs) => {
                    total += (n * secs as f64) as u64;
                    used += 2;
                }
                // (A bare number: minutes.)
                None => {
                    total += (n * 60.) as u64;
                    used += 1;
                }
            }
            continue;
        }
        let mut n = n;
        loop {
            let letters = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
            total += (n * unit(&rest[..letters])? as f64) as u64;
            rest = &rest[letters..];
            if rest.is_empty() {
                break;
            }
            let digits = rest.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(rest.len());
            n = rest[..digits].parse().ok()?;
            rest = &rest[digits..];
            if rest.is_empty() {
                // ("1h30": the rest's minutes.)
                total += (n * 60.) as u64;
                break;
            }
        }
        used += 1;
    }
    (total > 0).then_some((total, used))
}

/// The timer asked for: how long, and what for ("" if unsaid): "timer 10
/// min tea", "set a timer for 1h30m", or "a 10 minute timer for tea".
pub fn parse(q: &str) -> Option<(u64, String)> {
    let lower = q.trim().to_lowercase();
    if matches!(lower.as_str(), "pomodoro" | "focus timer") {
        return Some((25 * 60, "Pomodoro".into()));
    }
    let words: Vec<&str> = lower.split_whitespace().collect();
    let original: Vec<&str> = q.split_whitespace().collect();
    // What's after the duration: what it's for.
    let label = |from: usize| -> String {
        let text = original.get(from..).map_or(String::new(), |w| w.join(" "));
        let text = text.strip_prefix("for ").or_else(|| text.strip_prefix("called ")).unwrap_or(&text).trim();
        let mut chars = text.chars();
        chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
    };
    let starts = |prefix: &str| -> Option<usize> {
        let n = prefix.split(' ').count();
        (words.len() > n && words[..n].join(" ") == prefix).then_some(n)
    };
    let fits = |secs: u64| (1..=24 * 3600).contains(&secs);
    // "timer 10 min", "set a timer for 10 minutes".
    if let Some(n) = ["set a timer for", "set timer for", "start a timer for", "set a timer", "timer for", "timer"].iter().find_map(|p| starts(p)) {
        let (secs, used) = duration(&words[n..])?;
        return fits(secs).then(|| (secs, label(n + used)));
    }
    // "10 minute timer", "a 10 minute timer for tea".
    let n = ["start a", "set a", "a"].iter().find_map(|p| starts(p)).unwrap_or(0);
    let (secs, used) = duration(&words[n..])?;
    (words.get(n + used) == Some(&"timer") && fits(secs)).then(|| (secs, label(n + used + 1)))
}

/// A break asked for, again and again: how often, and what for ("eye break
/// every 20 min", "stand up every hour", "remind me to drink water every
/// 30 minutes", "eye breaks": the 20-20-20 rule's 20 minutes).
pub fn breaks_asked(q: &str) -> Option<(u64, String)> {
    let lower = q.trim().trim_end_matches(['.', '!']).to_lowercase();
    if matches!(lower.as_str(), "eye break" | "eye breaks" | "20-20-20" | "20 20 20" | "eye break reminder" | "eye break reminders" | "rest my eyes") {
        return Some((20 * 60, "Eye break".into()));
    }
    let (what, often) = lower.rsplit_once(" every ")?;
    let words: Vec<&str> = often.split_whitespace().collect();
    let secs = match words[..] {
        ["hour"] => 3600,
        ["half", "hour"] | ["half", "an", "hour"] => 1800,
        _ => match duration(&words)? {
            (secs, used) if used == words.len() => secs,
            _ => return None,
        },
    };
    if !(5 * 60..=4 * 3600).contains(&secs) {
        return None;
    }
    let what = ["remind me to ", "reminder to ", "remind me ", "reminders to ", "remember to "].iter().find_map(|p| what.strip_prefix(p)).unwrap_or(what);
    let what = what.trim_end_matches(" reminders").trim_end_matches(" reminder").trim();
    if what.is_empty() || what.split_whitespace().count() > 6 {
        return None;
    }
    let mut chars = what.chars();
    let label: String = chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
    Some((secs, label))
}

/// A break, every so often, till it's stopped.
pub fn start_breaks(every: u64, label: String, cx: &mut App) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    crate::shots::note(format!("“{label}” every {}", till(every)), "The menu bar item stops them.".into(), None, cx);
    TIMERS.lock().unwrap().push(Timer { id, label, secs: every, ends: now() + every as i64, alarm: false, then: Then::Ring, every });
    crate::status_item::refresh(cx);
}

/// What a break's note says.
fn break_note(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.contains("eye") {
        "Look at something 6 m (20 ft) away for 20 seconds.".into()
    } else if lower.contains("stand") || lower.contains("stretch") || lower.contains("walk") || lower.contains("move") {
        "Up you get: stretch, walk a little.".into()
    } else if lower.contains("water") || lower.contains("drink") {
        "Have a glass of water.".into()
    } else if lower.contains("posture") || lower.contains("sit up") {
        "Sit up straight, shoulders down.".into()
    } else {
        "The menu bar item stops these.".into()
    }
}

/// A stopwatch: since when it's been running (None: paused), and how long
/// it had run before.
struct Stopwatch {
    since: Option<std::time::Instant>,
    before: std::time::Duration,
}

static STOPWATCH: Mutex<Option<Stopwatch>> = Mutex::new(None);

/// What's done to the stopwatch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Watch {
    Start,
    Pause,
    Resume,
    Stop,
}

/// How long the stopwatch has run, and whether it's running (None: there's
/// none).
pub fn stopwatch() -> Option<(std::time::Duration, bool)> {
    STOPWATCH.lock().unwrap().as_ref().map(|w| (w.before + w.since.map_or_else(Default::default, |s| s.elapsed()), w.since.is_some()))
}

pub fn stopwatch_asked(q: &str) -> bool {
    let lower = q.trim().to_lowercase();
    matches!(lower.as_str(), "stopwatch" | "stop watch" | "start stopwatch" | "start a stopwatch" | "start the stopwatch" | "stopwatch start" | "stop stopwatch" | "stop the stopwatch" | "pause stopwatch" | "resume stopwatch" | "count up")
}

/// "1:05.3", "12:40.0", "1:02:03.5".
pub fn stopwatch_said(d: std::time::Duration) -> String {
    format!("{}.{}", countdown(d.as_secs() as i64), d.subsec_millis() / 100)
}

pub fn stopwatch_do(what: Watch, cx: &mut App) {
    {
        let mut watch = STOPWATCH.lock().unwrap();
        match what {
            Watch::Start => *watch = Some(Stopwatch { since: Some(std::time::Instant::now()), before: Default::default() }),
            Watch::Pause => {
                if let Some(w) = watch.as_mut()
                    && let Some(since) = w.since.take()
                {
                    w.before += since.elapsed();
                }
            }
            Watch::Resume => {
                if let Some(w) = watch.as_mut() {
                    w.since.get_or_insert_with(std::time::Instant::now);
                }
            }
            Watch::Stop => {
                if let Some(w) = watch.take() {
                    let ran = w.before + w.since.map_or_else(Default::default, |s| s.elapsed());
                    drop(watch);
                    crate::shots::note("Stopwatch Stopped".into(), stopwatch_said(ran), None, cx);
                    crate::status_item::refresh(cx);
                    return;
                }
            }
        }
    }
    crate::status_item::refresh(cx);
}

/// "clean keyboard", "clean my keyboard for 1 min", "clean the screen":
/// cleaning mode, and for how long (seconds: 30 if it's not said; 10
/// seconds to 5 minutes).
pub fn clean_asked(q: &str) -> Option<u32> {
    let lower = q.trim().trim_end_matches(['.', '!']).to_lowercase();
    const ASKED: &[&str] = &[
        "clean keyboard",
        "clean my keyboard",
        "clean the keyboard",
        "keyboard cleaning",
        "keyboard cleaning mode",
        "cleaning mode",
        "wipe keyboard",
        "wipe my keyboard",
        "wipe the keyboard",
        "lock keyboard",
        "lock the keyboard",
        "disable keyboard",
        "clean screen",
        "clean my screen",
        "clean the screen",
        "clean trackpad",
        "clean my trackpad",
        "clean the trackpad",
    ];
    let rest = ASKED.iter().filter_map(|a| lower.strip_prefix(a).filter(|r| r.is_empty() || r.starts_with(' ')).map(|r| (a.len(), r))).max_by_key(|(n, _)| *n)?.1;
    let rest = rest.trim();
    let rest = rest.strip_prefix("for ").unwrap_or(rest).trim();
    if rest.is_empty() {
        return Some(30);
    }
    // (A bare number would be minutes, as a timer's: say which.)
    let words: Vec<&str> = rest.split_whitespace().collect();
    if !rest.chars().any(char::is_alphabetic) {
        return None;
    }
    let (secs, used) = duration(&words)?;
    (used == words.len()).then_some(secs.clamp(10, 300) as u32)
}

/// "10 minutes", "1 hour 30 minutes", "45 seconds".
pub fn spoken(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let part = |n: u64, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    let parts: Vec<String> = [(h, "hour"), (m, "minute"), (s, "second")].into_iter().filter(|(n, _)| *n > 0).map(|(n, w)| part(n, w)).collect();
    parts.join(" ")
}

/// "9:41", "1:02:03".
pub fn countdown(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The timers going, the soonest first.
pub fn list() -> Vec<Timer> {
    let mut timers = TIMERS.lock().unwrap().clone();
    timers.sort_by_key(|t| t.ends);
    timers
}

pub fn start(secs: u64, label: String, cx: &mut App) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let timer = Timer { id, label, secs, ends: now() + secs as i64, alarm: false, then: Then::Ring, every: 0 };
    let at = crate::macos::zone_format(&crate::macos::local_zone(), timer.ends, "jmm");
    let what = if timer.label.is_empty() { "Timer Started".to_string() } else { format!("“{}” Started", timer.label) };
    crate::shots::note(what, format!("{}, till {at}", spoken(secs)), None, cx);
    TIMERS.lock().unwrap().push(timer);
    crate::status_item::refresh(cx);
}

/// The alarm asked for: when, and what for ("alarm 7am", "wake me at 6:30",
/// "alarm tomorrow 9am gym", "set an alarm for 7").
pub fn alarm_asked(q: &str) -> Option<(i64, String)> {
    alarm_at(q, chrono::Local::now())
}

fn alarm_at(q: &str, now: chrono::DateTime<chrono::Local>) -> Option<(i64, String)> {
    let lower = q.trim().to_lowercase();
    const LEADS: &[&str] = &["set an alarm for ", "set an alarm at ", "set alarm for ", "set alarm at ", "set an alarm ", "alarm for ", "alarm at ", "alarm ", "wake me up at ", "wake me at ", "wake me up ", "wake me "];
    let lead = LEADS.iter().find(|l| lower.starts_with(*l))?;
    let rest = q.trim()[lead.len()..].trim();
    // (A bare "7" is a time here: "at" before it says so. And a title to
    // read the time off: what's left of it, the alarm's label.)
    let said = crate::reminders::split_when(&format!("Alarm at {rest}"), now)?;
    let due = said.due.filter(|d| d.timed)?;
    let label = said.title.strip_prefix("Alarm").unwrap_or(&said.title).trim().to_string();
    let mut chars = label.chars();
    let label: String = chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
    (due.at > now.timestamp() && due.at - now.timestamp() <= 7 * 86_400).then_some((due.at, label))
}

/// How long till an alarm, to the minute: "19 h 17 min", "45 min".
pub fn till(secs: u64) -> String {
    let minutes = secs.div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// An alarm, at `at` (seconds since 1970).
pub fn set_alarm(at: i64, label: String, cx: &mut App) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let secs = (at - now()).max(0) as u64;
    let when = crate::macos::zone_format(&crate::macos::local_zone(), at, "jmm");
    let what = if label.is_empty() { "Alarm Set".to_string() } else { format!("“{label}” Alarm Set") };
    crate::shots::note(what, format!("For {when}, in {}. Keep the Mac awake for it to ring.", till(secs)), None, cx);
    TIMERS.lock().unwrap().push(Timer { id, label, secs, ends: at, alarm: true, then: Then::Ring, every: 0 });
    crate::status_item::refresh(cx);
}

pub fn cancel(id: u64, cx: &mut App) {
    TIMERS.lock().unwrap().retain(|t| t.id != id);
    crate::status_item::refresh(cx);
}

/// The ones done, rung (a sound, a note) and gone.
pub fn tick(cx: &mut App) {
    let now = now();
    let done: Vec<Timer> = {
        let mut timers = TIMERS.lock().unwrap();
        let (done, mut going): (Vec<Timer>, Vec<Timer>) = timers.drain(..).partition(|t| t.ends <= now);
        // (A break's due again.)
        going.extend(done.iter().filter(|t| t.every > 0).map(|t| Timer { ends: now + t.every as i64, ..t.clone() }));
        *timers = going;
        done
    };
    for timer in done {
        if timer.every > 0 {
            crate::macos::play_sound("Purr", 1);
            crate::shots::note(timer.label.clone(), break_note(&timer.label), None, cx);
            continue;
        }
        match timer.then {
            Then::Sleep => {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("/usr/bin/pmset").arg("sleepnow").output();
                });
                continue;
            }
            // (Music and Spotify paused; else, something else playing (a
            // video in a browser): the sound muted.)
            Then::StopMusic => {
                std::thread::spawn(|| {
                    if !music("pause") {
                        crate::macos::muted(Some(true));
                    }
                });
                continue;
            }
            Then::Ring => {}
        }
        if timer.alarm {
            crate::macos::play_sound("Hero", 6);
            let at = crate::macos::zone_format(&crate::macos::local_zone(), timer.ends, "jmm");
            let title = if timer.label.is_empty() { "Alarm".to_string() } else { format!("{}: Alarm", timer.label) };
            crate::shots::note(title, format!("It's {at}"), None, cx);
            continue;
        }
        crate::macos::play_sound("Glass", 3);
        let title = if timer.label.is_empty() { "Time's Up".to_string() } else { format!("{}: Time's Up", timer.label) };
        crate::shots::note(title, format!("{} gone", spoken(timer.secs)), None, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{alarm_at, countdown, parse, spoken};

    #[test]
    fn sleep_timers() {
        use super::Then;
        use chrono::{Local, TimeZone};
        let now = Local.with_ymd_and_hms(2026, 10, 20, 22, 0, 0).unwrap();
        let at = |q: &str| super::sleep_at(q, now);
        assert_eq!(at("sleep in 30 min"), Some((Then::Sleep, 1800)));
        assert_eq!(at("stop music in 1 hour"), Some((Then::StopMusic, 3600)));
        assert_eq!(at("sleep timer 45 minutes"), Some((Then::Sleep, 2700)));
        assert_eq!(at("sleep at 11pm"), Some((Then::Sleep, 3600)));
        assert_eq!(at("sleep"), None, "now: the Sleep command");
        assert_eq!(at("sleep in 10 seconds"), None, "too soon to be a timer");
        assert_eq!(at("sleeping bag"), None);
    }

    #[test]
    fn alarms() {
        use chrono::{Local, TimeZone};
        // (As if it were Tuesday, October 20, 2026, at 14:00.)
        let now = Local.with_ymd_and_hms(2026, 10, 20, 14, 0, 0).unwrap();
        let at = |q: &str| alarm_at(q, now).map(|(t, label)| (Local.timestamp_opt(t, 0).unwrap().format("%a %H:%M").to_string(), label));
        assert_eq!(at("alarm 7am"), Some(("Wed 07:00".into(), String::new())), "gone today: tomorrow's");
        assert_eq!(at("wake me at 6:30"), Some(("Wed 06:30".into(), String::new())));
        assert_eq!(at("alarm 5pm"), Some(("Tue 17:00".into(), String::new())));
        assert_eq!(at("set an alarm for 7"), Some(("Tue 19:00".into(), String::new())));
        assert_eq!(at("alarm friday 9am gym"), Some(("Fri 09:00".into(), "Gym".into())));
        assert_eq!(at("alarm clock"), None);
        assert_eq!((super::till(69_445), super::till(2_700), super::till(7_200)), ("19 h 18 min".into(), "45 min".into(), "2 h".into()));
        assert_eq!(at("alarms"), None);
    }

    #[test]
    fn timers_asked() {
        assert_eq!(parse("timer 10 min"), Some((600, String::new())));
        assert_eq!(parse("timer 10m"), Some((600, String::new())));
        assert_eq!(parse("Timer 25m tea"), Some((1500, "Tea".into())));
        assert_eq!(parse("timer 4 minutes for the eggs"), Some((240, "The eggs".into())));
        assert_eq!(parse("set a timer for 1h30m"), Some((5400, String::new())));
        assert_eq!(parse("timer 1 hour and 15 minutes"), Some((4500, String::new())));
        assert_eq!(parse("timer 90s"), Some((90, String::new())));
        assert_eq!(parse("timer 1:30"), Some((90, String::new())));
        assert_eq!(parse("timer 5"), Some((300, String::new())), "minutes");
        assert_eq!(parse("10 minute timer"), Some((600, String::new())));
        assert_eq!(parse("a 3 minute timer for tea"), Some((180, "Tea".into())));
        assert_eq!(parse("pomodoro"), Some((1500, "Pomodoro".into())));
        assert_eq!(parse("timer"), None);
        assert_eq!(parse("timer app"), None);
        assert_eq!(parse("timeout"), None);
    }

    #[test]
    fn breaks() {
        use super::breaks_asked;
        assert_eq!(breaks_asked("eye breaks"), Some((1200, "Eye break".into())));
        assert_eq!(breaks_asked("eye break every 20 min"), Some((1200, "Eye break".into())));
        assert_eq!(breaks_asked("stand up every hour"), Some((3600, "Stand up".into())));
        assert_eq!(breaks_asked("remind me to drink water every 30 minutes"), Some((1800, "Drink water".into())));
        assert_eq!(breaks_asked("posture reminder every half hour"), Some((1800, "Posture".into())));
        assert_eq!(breaks_asked("blink every 10 seconds"), None, "too often");
        assert_eq!(breaks_asked("every hour"), None);
        assert_eq!(breaks_asked("stopwatch"), None);
        assert!(super::stopwatch_asked("Start stopwatch"));
        assert_eq!(super::stopwatch_said(std::time::Duration::from_millis(65_340)), "1:05.3");
    }

    #[test]
    fn cleaning() {
        assert_eq!(super::clean_asked("clean keyboard"), Some(30));
        assert_eq!(super::clean_asked("clean my keyboard for 1 min"), Some(60));
        assert_eq!(super::clean_asked("clean the screen for 2 minutes"), Some(120));
        assert_eq!(super::clean_asked("clean keyboard 5"), None, "five what?");
        assert_eq!(super::clean_asked("clean keyboards.ts"), None);
        assert_eq!(super::clean_asked("cleanup"), None);
    }

    #[test]
    fn said() {
        assert_eq!(spoken(5400), "1 hour 30 minutes");
        assert_eq!(spoken(45), "45 seconds");
        assert_eq!(countdown(581), "9:41");
        assert_eq!(countdown(3723), "1:02:03");
    }
}

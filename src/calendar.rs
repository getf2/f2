//! The calendar, from Quick Open. Events added as they're said: "lunch with
//! Sara friday at 1", "event dentist oct 30 at 9:30", "standup tomorrow
//! 10-10:15am", "workshop monday 2pm for 3 hours", "holiday on dec 24" (all
//! day); and what's next: "next meeting", "my day", "agenda", each with its
//! video call's link to join. Into, and from, the Mac's calendars
//! (EventKit: macOS asks once).

use chrono::{DateTime, Local, NaiveTime, TimeZone};

#[derive(Clone, Debug, PartialEq)]
pub struct NewEvent {
    pub title: String,
    /// Seconds since 1970.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
}

/// An event coming up (or under way).
pub struct Upcoming {
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub calendar: String,
    pub link: Option<String>,
}

/// What "add" words lead an event ("event dentist tomorrow"): said, it's
/// an event for sure; unsaid, only with a day and a time in it.
const LEADS: &[&str] = &["add event ", "new event ", "add to calendar ", "calendar event ", "event ", "schedule "];

/// Other things' words, never an event's.
const OTHERS: &[&str] = &["remind", "translate", "time in", "timer", "define", "what time", "keep awake", "weather", "sleep ", "stop music", "pause music", "alarm", "wake me"];

/// "for 2 hours", "for 30 min", "for an hour": in minutes, and the words.
fn lasting(words: &[&str]) -> Option<(i64, usize)> {
    if words.first() != Some(&"for") {
        return None;
    }
    let unit = |u: &str| match u.trim_end_matches([',', '.']) {
        "m" | "min" | "mins" | "minute" | "minutes" => Some(1),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(60),
        _ => None,
    };
    match words.get(1..) {
        Some(["half", "an", u, ..]) if unit(u) == Some(60) => Some((30, 4)),
        Some(["an" | "a" | "one", u, ..]) => Some((unit(u)?, 3)),
        Some([n, u, ..]) if unit(u).is_some() => Some(((n.parse::<f64>().ok()? * unit(u)? as f64) as i64, 3)),
        Some([joined, ..]) => {
            let digits = joined.find(|c: char| !c.is_ascii_digit() && c != '.')?;
            Some(((joined[..digits].parse::<f64>().ok()? * unit(&joined[digits..])? as f64) as i64, 2))
        }
        _ => None,
    }
}

/// "3-4pm", "15:00-16:00", "10-10:15am": the start's word (with the end's
/// am or pm, if it has none) and the end's time.
fn range(word: &str) -> Option<(String, NaiveTime)> {
    let (from, to) = word.split_once(['-', '–'])?;
    if from.is_empty() || to.is_empty() || !from.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let (end, _) = crate::reminders::time_of(&[to], true)?;
    let half = ["am", "pm"].into_iter().find(|h| to.ends_with(h));
    let from = match half {
        Some(h) if !from.ends_with("am") && !from.ends_with("pm") => format!("{from}{h}"),
        _ => from.to_string(),
    };
    Some((from, end))
}

/// The event said, if one is: and whether it was said for sure (with "event"
/// or the like before it), or only looks like one.
pub fn parse(q: &str) -> Option<(NewEvent, bool)> {
    parse_at(q, Local::now())
}

fn parse_at(q: &str, now: DateTime<Local>) -> Option<(NewEvent, bool)> {
    let q = q.trim();
    let lower = q.to_lowercase();
    let lead = LEADS.iter().find(|l| lower.starts_with(*l));
    if lead.is_none() && (OTHERS.iter().any(|o| lower.starts_with(o)) || crate::natural::translate(q).is_some()) {
        return None;
    }
    let rest = &q[lead.map_or(0, |l| l.len())..];
    // How long, or till when, taken out first.
    let words: Vec<&str> = rest.split_whitespace().collect();
    let lower_words: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let lower_words: Vec<&str> = lower_words.iter().map(String::as_str).collect();
    let (mut minutes, mut ends): (Option<i64>, Option<NaiveTime>) = (None, None);
    let mut kept: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if let Some((m, n)) = lasting(&lower_words[i..]) {
            minutes = Some(m);
            i += n;
            continue;
        }
        if let Some((from, end)) = range(lower_words[i]) {
            kept.push(from);
            ends = Some(end);
            i += 1;
            continue;
        }
        // "from 3pm to 4pm", "3pm to 4pm".
        if lower_words.get(i + 1) == Some(&"to")
            && let (Some(_), Some((end, n))) = (crate::reminders::time_of(&[lower_words[i]], true), lower_words.get(i + 2..).and_then(|w| crate::reminders::time_of(w, true)))
        {
            if kept.last().is_some_and(|w| w.eq_ignore_ascii_case("from")) {
                kept.pop();
            }
            kept.push("at".into());
            kept.push(lower_words[i].to_string());
            ends = Some(end);
            i += 2 + n;
            continue;
        }
        kept.push(words[i].to_string());
        i += 1;
    }
    let said = crate::reminders::split_when(&kept.join(" "), now)?;
    let due = said.due?;
    // (Unsaid: only with a day and a time, as an event's said.)
    if lead.is_none() && !(said.dated && due.timed) {
        return None;
    }
    let event = if due.timed {
        let start = due.at;
        let end = match (ends, minutes) {
            (Some(end), _) => {
                let day = Local.timestamp_opt(start, 0).single()?.date_naive();
                let mut end = Local.from_local_datetime(&day.and_time(end)).earliest()?.timestamp();
                if end <= start {
                    end += 86_400;
                }
                end
            }
            (None, Some(m)) => start + m * 60,
            (None, None) => start + 3600,
        };
        NewEvent { title: said.title, start, end, all_day: false }
    } else {
        // (A day only: all of it.)
        let day = Local.timestamp_opt(due.at, 0).single()?.date_naive();
        let start = Local.from_local_datetime(&day.and_hms_opt(0, 0, 0)?).earliest()?.timestamp();
        NewEvent { title: said.title, start, end: start + 86_400, all_day: true }
    };
    Some((event, lead.is_some()))
}

/// Whether what's asked is what's on: "next meeting", "my day", "agenda".
pub fn agenda_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    [
        "next meeting",
        "next event",
        "my next meeting",
        "my day",
        "agenda",
        "my agenda",
        "today's events",
        "todays events",
        "events today",
        "events",
        "what's on today",
        "whats on today",
        "what's on",
        "whats on",
        "what's next",
        "whats next",
        "my calendar",
        "calendar today",
        "upcoming events",
        "upcoming",
        "meetings",
        "meetings today",
        "my meetings",
        "schedule today",
        "my schedule",
        "join meeting",
        "join call",
    ]
    .contains(&lower.as_str())
}

/// What's on from now till the end of tomorrow (and what's under way), at
/// most `limit`. None: the calendar's not to be seen (not asked, or not
/// allowed).
pub fn upcoming(limit: usize) -> Option<Vec<Upcoming>> {
    let now = Local::now();
    let end = now.date_naive().succ_opt()?.succ_opt()?.and_hms_opt(0, 0, 0)?;
    let end = Local.from_local_datetime(&end).earliest()?.timestamp();
    let events = crate::macos::events(now.timestamp(), end)?;
    Some(
        events
            .into_iter()
            .take(limit)
            .map(|(start, end, all_day, title, calendar, link)| Upcoming { title, start, end, all_day, calendar, link })
            .collect(),
    )
}

/// "Today at 14:00–15:00", "Tomorrow, all day", "Friday at 13:00".
pub fn when(start: i64, end: i64, all_day: bool) -> String {
    let zone = crate::macos::local_zone();
    let Some(day) = Local.timestamp_opt(start, 0).single() else { return String::new() };
    let days = (day.date_naive() - Local::now().date_naive()).num_days();
    let on = match days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        2..=6 => crate::macos::zone_format(&zone, start, "EEEE"),
        _ => crate::macos::zone_format(&zone, start, "EEEMMMd"),
    };
    if all_day {
        return format!("{on}, all day");
    }
    format!("{on} at {}–{}", crate::macos::zone_format(&zone, start, "jmm"), crate::macos::zone_format(&zone, end, "jmm"))
}

/// "in 25 min", "now", "in 3 h".
pub fn soon(start: i64, end: i64) -> String {
    let now = crate::timers::now();
    if start <= now && now < end {
        return "now".into();
    }
    match (start - now) / 60 {
        m if m < 60 => format!("in {} min", m.max(1)),
        m if m < 24 * 60 => format!("in {} h", (m + 30) / 60),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{NewEvent, agenda_asked, parse_at};
    use chrono::{Local, TimeZone};

    /// As if it were Tuesday, October 20, 2026, at 14:00 here.
    fn at(q: &str) -> Option<(String, String, String, bool, bool)> {
        let now = Local.with_ymd_and_hms(2026, 10, 20, 14, 0, 0).unwrap();
        let f = |t: i64| Local.timestamp_opt(t, 0).unwrap().format("%a %H:%M").to_string();
        parse_at(q, now).map(|(NewEvent { title, start, end, all_day }, sure)| (title, f(start), f(end), all_day, sure))
    }

    #[test]
    fn events() {
        assert_eq!(at("lunch with Sara friday at 1"), Some(("Lunch with Sara".into(), "Fri 13:00".into(), "Fri 14:00".into(), false, false)));
        assert_eq!(at("event dentist tomorrow at 9:30"), Some(("Dentist".into(), "Wed 09:30".into(), "Wed 10:30".into(), false, true)));
        assert_eq!(at("standup tomorrow 10-10:15am"), Some(("Standup".into(), "Wed 10:00".into(), "Wed 10:15".into(), false, false)));
        assert_eq!(at("workshop monday 2pm for 3 hours"), Some(("Workshop".into(), "Mon 14:00".into(), "Mon 17:00".into(), false, false)));
        assert_eq!(at("dinner friday from 7pm to 9pm"), Some(("Dinner".into(), "Fri 19:00".into(), "Fri 21:00".into(), false, false)));
        assert_eq!(at("event holiday on dec 24"), Some(("Holiday".into(), "Thu 00:00".into(), "Fri 00:00".into(), true, true)));
        assert_eq!(at("dinner at 7"), None, "no day: not for sure an event");
        assert_eq!(at("photos from friday"), None);
        assert_eq!(at("remind me to call mom tomorrow at 5pm"), None, "a reminder");
        assert_eq!(at("notes"), None);
        assert_eq!(at("sleep in 30 min"), None, "a sleep timer");
    }

    #[test]
    fn agenda() {
        assert!(agenda_asked("next meeting"));
        assert!(agenda_asked("What's on today?"));
        assert!(!agenda_asked("meeting notes"));
    }
}

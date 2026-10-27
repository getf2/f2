//! Reminders from Quick Open: "remind me to call mom tomorrow at 5pm",
//! "remind me in 20 minutes to check the oven", "reminder pay rent on
//! friday". Into Reminders' default list (macOS asks, the first time): at a
//! time, with an alert then; on a day; or, with no when, with no date.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Weekday};

#[derive(Clone, Debug, PartialEq)]
pub struct Reminder {
    pub title: String,
    pub due: Option<Due>,
}

/// When: a moment (seconds since 1970), and whether it's at that time
/// (else it's for that day).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Due {
    pub at: i64,
    pub timed: bool,
}

/// `text`'s start, if it's `prefix` (any case): what's after.
fn after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len()).filter(|start| start.eq_ignore_ascii_case(prefix)).map(|_| &text[prefix.len()..])
}

/// The reminder asked for, if one is.
pub fn parse(q: &str) -> Option<Reminder> {
    parse_at(q, Local::now())
}

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

fn month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches('.');
    // ("may" only in full: it's a word too.)
    MONTHS.iter().position(|m| *m == word || (word.len() >= 3 && word != "may" && m.starts_with(word) && (word.len() == 3 || word == "sept"))).map(|i| i as u32 + 1)
}

fn weekday(word: &str) -> Option<Weekday> {
    match word {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" | "tues" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" | "thurs" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

/// "25", "25th", "1st": a day of the month.
fn day_of_month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(',');
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &word[digits.len()..];
    let day: u32 = digits.parse().ok()?;
    (["", "st", "nd", "rd", "th"].contains(&suffix) && (1..=31).contains(&day)).then_some(day)
}

/// A time of day: "5pm", "5:30pm", "17:00", and (`bare`, after "at") "5"
/// (in the afternoon, if before 8). With the words it took (one, or two:
/// "5 pm").
pub(crate) fn time_of(words: &[&str], bare: bool) -> Option<(NaiveTime, usize)> {
    let first = *words.first()?;
    match first {
        "noon" | "midday" => return Some((NaiveTime::from_hms_opt(12, 0, 0)?, 1)),
        "midnight" => return Some((NaiveTime::from_hms_opt(23, 59, 0)?, 1)),
        _ => {}
    }
    let (clock, half, used) = match ["am", "pm", "a.m.", "p.m."].iter().find(|h| first.ends_with(**h) && first.len() > h.len()) {
        Some(h) => (&first[..first.len() - h.len()], Some(h.starts_with('p')), 1),
        None => match words.get(1).map(|w| w.trim_end_matches(',')) {
            Some(h @ ("am" | "pm" | "a.m." | "p.m.")) => (first, Some(h.starts_with('p')), 2),
            _ => (first, None, 1),
        },
    };
    let (h, m) = match clock.split_once([':', '.']) {
        Some((h, m)) if m.len() == 2 => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        Some(_) => return None,
        None if half.is_some() || bare => (clock.parse::<u32>().ok()?, 0),
        None => return None,
    };
    let colon = clock.contains([':', '.']);
    let hour = match half {
        Some(pm) if (1..=12).contains(&h) => (h % 12) + if pm { 12 } else { 0 },
        Some(_) => return None,
        // ("at 5": five in the afternoon; "at 9": the morning's.)
        None if !colon && (1..8).contains(&h) => h + 12,
        None => h,
    };
    Some((NaiveTime::from_hms_opt(hour, m, 0)?, used))
}

/// "20 minutes", "an hour", "2h": how long, and the words it took.
fn span(words: &[&str]) -> Option<(Duration, usize)> {
    let unit = |u: &str| -> Option<i64> {
        match u.trim_end_matches(',') {
            "s" | "sec" | "secs" | "second" | "seconds" => Some(1),
            "m" | "min" | "mins" | "minute" | "minutes" => Some(60),
            "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600),
            "d" | "day" | "days" => Some(86_400),
            "w" | "week" | "weeks" => Some(7 * 86_400),
            _ => None,
        }
    };
    let first = *words.first()?;
    if first == "half" && words.get(1) == Some(&"an") && words.get(2).is_some_and(|w| unit(w) == Some(3600)) {
        return Some((Duration::minutes(30), 3));
    }
    let n: Option<f64> = match first {
        "a" | "an" | "one" => Some(1.),
        "two" => Some(2.),
        "three" => Some(3.),
        "five" => Some(5.),
        "ten" => Some(10.),
        "fifteen" => Some(15.),
        "thirty" => Some(30.),
        n => n.parse().ok(),
    };
    if let Some(n) = n {
        let secs = unit(words.get(1)?)?;
        return Some((Duration::seconds((n * secs as f64) as i64), 2));
    }
    // "20min", "2h".
    let digits = first.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let n: f64 = first[..digits].parse().ok()?;
    Some((Duration::seconds((n * unit(&first[digits..])? as f64) as i64), 1))
}

fn parse_at(q: &str, now: DateTime<Local>) -> Option<Reminder> {
    let q = q.trim();
    let prefixes = ["remind me to ", "remind me ", "reminder to ", "reminder: ", "reminder ", "add a reminder to ", "add a reminder ", "add reminder ", "new reminder "];
    let rest = prefixes.iter().find_map(|p| after(q, p))?;
    let said = split_when(rest, now)?;
    Some(Reminder { title: said.title, due: said.due })
}

/// Words, and when they say: the rest of them (its first letter up, its
/// edges' "to"s and "at"s off), when (None: not said), and whether a day
/// was said ("friday", "tomorrow", "in 2 hours"), not just a time. None if
/// there's nothing but the when.
pub struct Said {
    pub title: String,
    pub due: Option<Due>,
    pub dated: bool,
}

/// What `rest` says, and when (see `Said`): "call mom tomorrow at 5pm".
pub fn split_when(rest: &str, now: DateTime<Local>) -> Option<Said> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let lower: Vec<&str> = lower.iter().map(String::as_str).collect();
    let mut used = vec![false; words.len()];
    let today = now.date_naive();
    let (mut date, mut time, mut moment): (Option<NaiveDate>, Option<NaiveTime>, Option<DateTime<Local>>) = (None, None, None);
    let mut part_of_day: Option<u32> = None;
    let mut i = 0;
    while i < words.len() {
        let w = lower[i].trim_end_matches(',');
        let next = lower.get(i + 1).map(|n| n.trim_end_matches(','));
        let mut took = 0;
        match w {
            "in" => {
                if let Some((d, n)) = span(&lower[i + 1..]) {
                    moment = Some(now + d);
                    took = 1 + n;
                }
            }
            "today" => (date, took) = (Some(today), 1),
            "tonight" => (date, part_of_day, took) = (Some(today), Some(20), 1),
            "tomorrow" | "tmrw" | "tmr" => (date, took) = (today.succ_opt(), 1),
            "day" if next == Some("after") && lower.get(i + 2).is_some_and(|w| w.trim_end_matches(',') == "tomorrow") => {
                (date, took) = (today.checked_add_days(chrono::Days::new(2)), 3);
            }
            "morning" => (part_of_day, took) = (Some(9), 1),
            "afternoon" => (part_of_day, took) = (Some(15), 1),
            "evening" => (part_of_day, took) = (Some(18), 1),
            "night" if i > 0 && matches!(lower[i - 1], "tomorrow" | "this") => (part_of_day, took) = (Some(20), 1),
            "next" if next == Some("week") => (date, took) = (today.checked_add_days(chrono::Days::new(7)), 2),
            "next" | "this" if next.and_then(weekday).is_some() => {
                let day = next.and_then(weekday)?;
                let ahead = (7 + day.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64) % 7;
                let ahead = if w == "next" && ahead == 0 { 7 } else { ahead };
                (date, took) = (today.checked_add_signed(Duration::days(ahead)), 2);
            }
            "this" if matches!(next, Some("morning" | "afternoon" | "evening")) => (date, took) = (Some(today), 1),
            w if weekday(w).is_some() && (w.len() > 3 || i > 0 && matches!(lower[i - 1], "on" | "next" | "this" | "by")) => {
                let day = weekday(w)?;
                // (The next one: a week on, if it's today.)
                let ahead = (7 + day.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64 - 1) % 7 + 1;
                (date, took) = (today.checked_add_signed(Duration::days(ahead)), 1);
            }
            w if month(w).is_some() && next.and_then(day_of_month).is_some() => {
                let (m, d) = (month(w)?, next.and_then(day_of_month)?);
                (date, took) = (next_date(today, m, d), 2);
            }
            w if day_of_month(w).is_some() && next.and_then(month).is_some() => {
                let (d, m) = (day_of_month(w)?, next.and_then(month)?);
                (date, took) = (next_date(today, m, d), 2);
            }
            _ => {
                let bare = i > 0 && matches!(lower[i - 1], "at" | "by");
                if let Some((t, n)) = time_of(&lower[i..], bare) {
                    (time, took) = (Some(t), n);
                }
            }
        }
        if took > 0 {
            used[i..i + took].fill(true);
            // ("at 5pm", "on friday": the word before goes with it.)
            if i > 0 && matches!(lower[i - 1], "at" | "on" | "by") {
                used[i - 1] = true;
            }
            i += took;
        } else {
            i += 1;
        }
    }
    // The title: what's left, its edges' "to"s and "at"s off.
    let mut title: Vec<&str> = words.iter().zip(&used).filter(|(_, u)| !**u).map(|(w, _)| *w).collect();
    while title.first().is_some_and(|w| matches!(w.to_lowercase().as_str(), "to" | "at" | "on" | "by" | "and" | "that" | "-" | ",")) {
        title.remove(0);
    }
    while title.last().is_some_and(|w| matches!(w.to_lowercase().as_str(), "to" | "at" | "on" | "by" | "and" | "-" | ",")) {
        title.pop();
    }
    let title = title.join(" ").trim_end_matches(',').to_string();
    let mut chars = title.chars();
    let title: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect())?;
    let local = |d: NaiveDate, t: NaiveTime| Local.from_local_datetime(&d.and_time(t)).earliest();
    let due = match (moment, date, time.or(part_of_day.and_then(|h| NaiveTime::from_hms_opt(h, 0, 0)))) {
        (Some(m), ..) => Some(Due { at: m.timestamp(), timed: true }),
        (None, date, Some(t)) => {
            let mut at = local(date.unwrap_or(today), t)?;
            // (A time that's gone today, without a day: tomorrow's.)
            if date.is_none() && at <= now {
                at = local(today.succ_opt()?, t)?;
            }
            Some(Due { at: at.timestamp(), timed: true })
        }
        (None, Some(d), None) => Some(Due { at: local(d, NaiveTime::from_hms_opt(9, 0, 0)?)?.timestamp(), timed: false }),
        (None, None, None) => None,
    };
    Some(Said { title, due, dated: date.is_some() || moment.is_some() })
}

/// The next `month` `day` from `today` (this year's, or next).
fn next_date(today: NaiveDate, month: u32, day: u32) -> Option<NaiveDate> {
    let this = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if this >= today { Some(this) } else { NaiveDate::from_ymd_opt(today.year() + 1, month, day) }
}

/// What a reminder's still missing, for Quick Open's gray hint once a word
/// is done: when ("tomorrow at 9am"), or the time of its day ("at 9am").
pub fn hint(text: &str) -> Option<String> {
    if !text.ends_with(' ') {
        return None;
    }
    match parse(text)?.due {
        None => Some("tomorrow at 9am".into()),
        Some(due) if !due.timed => Some("at 9am".into()),
        Some(_) => None,
    }
}

/// When it's due, as said: "Today at 17:00", "Tomorrow", "Friday at 9:00",
/// "October 25".
pub fn when(due: &Due) -> String {
    let Some(at) = Local.timestamp_opt(due.at, 0).single() else { return String::new() };
    let today = Local::now().date_naive();
    let days = (at.date_naive() - today).num_days();
    let zone = crate::macos::local_zone();
    let day = match days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        2..=6 => crate::macos::zone_format(&zone, due.at, "EEEE"),
        _ => crate::macos::zone_format(&zone, due.at, "MMMMd"),
    };
    if due.timed { format!("{day} at {}", crate::macos::zone_format(&zone, due.at, "jmm")) } else { day }
}

/// Whether what's due is asked for: "todo", "my reminders", "what do i
/// need to do". ("reminders" alone: the app.)
pub fn todo_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(
        lower.as_str(),
        "todo" | "todos" | "to do" | "to-do" | "to dos" | "to-dos" | "todo list" | "to do list" | "to-do list" | "my todos" | "my to do list" | "my to-do list" | "my reminders" | "my list" | "reminders today" | "today's reminders" | "todays reminders" | "due today" | "what's due" | "whats due" | "what do i need to do" | "what do i have to do" | "할 일" | "할일"
    )
}

/// Add it to Reminders (waits for macOS's asking, the first time: not on
/// the main thread).
pub fn add(reminder: &Reminder) -> Result<(), String> {
    crate::macos::add_reminder(&reminder.title, reminder.due.map(|d| d.at), reminder.due.is_some_and(|d| d.timed))
}

#[cfg(test)]
mod tests {
    use super::{Due, parse_at};
    use chrono::{Local, TimeZone};

    /// As if it were Tuesday, October 20, 2026, at 14:00 here.
    fn at(q: &str) -> Option<(String, Option<(String, bool)>)> {
        let now = Local.with_ymd_and_hms(2026, 10, 20, 14, 0, 0).unwrap();
        parse_at(q, now).map(|r| {
            let due = r.due.map(|Due { at, timed }| (Local.timestamp_opt(at, 0).unwrap().format("%a %b %-d %H:%M").to_string(), timed));
            (r.title, due)
        })
    }

    fn due(title: &str, when: &str, timed: bool) -> Option<(String, Option<(String, bool)>)> {
        Some((title.into(), Some((when.into(), timed))))
    }

    #[test]
    fn whens() {
        assert_eq!(at("remind me to call mom tomorrow at 5pm"), due("Call mom", "Wed Oct 21 17:00", true));
        assert_eq!(at("remind me in 20 minutes to check the oven"), due("Check the oven", "Tue Oct 20 14:20", true));
        assert_eq!(at("reminder pay rent on friday"), due("Pay rent", "Fri Oct 23 09:00", false));
        assert_eq!(at("remind me to water plants at 9"), due("Water plants", "Wed Oct 21 09:00", true), "9 today is gone");
        assert_eq!(at("remind me to take out the trash tonight"), due("Take out the trash", "Tue Oct 20 20:00", true));
        assert_eq!(at("remind me to renew passport on oct 25"), due("Renew passport", "Sun Oct 25 09:00", false));
        assert_eq!(at("remind me to stretch in an hour"), due("Stretch", "Tue Oct 20 15:00", true));
        assert_eq!(at("remind me to call the bank next tuesday at 10:30"), due("Call the bank", "Tue Oct 27 10:30", true));
        assert_eq!(at("remind me buy milk"), Some(("Buy milk".into(), None)));
        assert_eq!(at("remind me tomorrow"), None, "nothing to be reminded of");
        assert_eq!(at("reminders"), None);
        assert_eq!(at("remind me to read on the train at 6"), due("Read on the train", "Tue Oct 20 18:00", true));
        assert!(super::todo_asked("To-do") && super::todo_asked("what do i need to do?") && !super::todo_asked("reminders"));
    }
}

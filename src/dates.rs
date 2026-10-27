//! Date math, for Quick Open: "days until christmas", "how long until march
//! 3", "days since 1 jan", "100 days from today", "in 2 weeks", "30 days
//! ago", "what day is 3 march 2027", "days between 1 jan and 3 mar", "week
//! number", "today's date". Days as this Mac writes them.

use chrono::{Datelike, Days, Local, Months, NaiveDate, TimeZone, Weekday};

/// An answer's row: as shown, what it's of, as copied.
pub struct Answer {
    pub shown: String,
    pub about: String,
    pub copied: String,
}

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

fn month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(['.', ',']);
    MONTHS.iter().position(|m| *m == word || (word.len() >= 3 && m.starts_with(word))).map(|i| i as u32 + 1)
}

fn day_number(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(',');
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let day: u32 = digits.parse().ok()?;
    (["", "st", "nd", "rd", "th"].contains(&&word[digits.len()..]) && (1..=31).contains(&day)).then_some(day)
}

fn year(word: &str) -> Option<i32> {
    let year: i32 = word.trim_end_matches(',').parse().ok()?;
    (1900..=2200).contains(&year).then_some(year)
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

/// Easter Sunday (the Gregorian computus).
fn easter(year: i32) -> Option<NaiveDate> {
    let (a, b, c) = (year % 19, year / 100, year % 100);
    let (d, e) = (b / 4, b % 4);
    let g = (b - (b + 8) / 25 + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

/// The `n`th `weekday` of a month (Thanksgiving: the 4th Thursday of
/// November).
fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n as u8)
}

/// The days that have names, in `year`.
fn named(name: &str, year: i32) -> Option<NaiveDate> {
    let ymd = |m, d| NaiveDate::from_ymd_opt(year, m, d);
    match name {
        "christmas" | "christmas day" | "xmas" => ymd(12, 25),
        "christmas eve" | "xmas eve" => ymd(12, 24),
        "new year" | "new years" | "new year's" | "new years day" | "new year's day" => NaiveDate::from_ymd_opt(year + 1, 1, 1),
        "new years eve" | "new year's eve" => ymd(12, 31),
        "halloween" => ymd(10, 31),
        "valentines" | "valentine's" | "valentines day" | "valentine's day" => ymd(2, 14),
        "easter" => easter(year),
        "thanksgiving" => nth_weekday(year, 11, Weekday::Thu, 4),
        "independence day" | "july 4th" | "4th of july" => ymd(7, 4),
        "summer" => ymd(6, 21),
        "winter" => ymd(12, 21),
        _ => None,
    }
}

/// Which way a date without a year goes: the next one (for "until"), the
/// last (for "since"), or this year's.
#[derive(Clone, Copy, PartialEq)]
enum Lean {
    Ahead,
    Behind,
    ThisYear,
}

/// A date said: "today", "christmas", "march 3", "3 march 2027", "2027-03-03",
/// "friday", "next friday".
fn date_of(text: &str, today: NaiveDate, lean: Lean) -> Option<NaiveDate> {
    let text = text.trim().trim_end_matches(['?', '.', '!']).trim();
    let text = text.strip_prefix("the ").unwrap_or(text);
    match text {
        "today" | "now" => return Some(today),
        "tomorrow" => return today.succ_opt(),
        "yesterday" => return today.pred_opt(),
        _ => {}
    }
    // (A named day: the next, or the last, as leaned.)
    let unnamed = text.strip_suffix(" day").filter(|t| named(t, today.year()).is_some()).unwrap_or(text);
    if let Some(this) = named(unnamed, today.year()) {
        return Some(match lean {
            Lean::Ahead if this < today => named(unnamed, today.year() + 1)?,
            Lean::Behind if this > today => named(unnamed, today.year() - 1)?,
            _ => this,
        });
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(date);
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    if let [w] | ["next", w] | ["this", w] | ["on", w] = words[..]
        && let Some(day) = weekday(w)
    {
        // (The coming one: a week on, if it's today.)
        let ahead = (7 + day.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64) % 7;
        return today.checked_add_days(Days::new(if ahead == 0 { 7 } else { ahead as u64 }));
    }
    let month_day = |a: &str, b: &str| month(a).zip(day_number(b)).or_else(|| month(b).zip(day_number(a)));
    let ((month, day), y) = match words[..] {
        [a, b] => (month_day(a, b)?, None),
        [a, b, c] => (month_day(a, b)?, Some(year(c)?)),
        _ => return None,
    };
    if let Some(year) = y {
        return NaiveDate::from_ymd_opt(year, month, day);
    }
    let this = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    Some(match lean {
        Lean::Ahead if this < today => NaiveDate::from_ymd_opt(today.year() + 1, month, day)?,
        Lean::Behind if this > today => NaiveDate::from_ymd_opt(today.year() - 1, month, day)?,
        _ => this,
    })
}

/// A span said: "100 days", "2 weeks", "3 months", "a year".
fn span(text: &str) -> Option<(u32, &'static str)> {
    let mut words = text.split_whitespace();
    let n = match words.next()? {
        "a" | "an" | "one" => 1,
        "two" => 2,
        "three" => 3,
        "ten" => 10,
        n => n.parse().ok()?,
    };
    let unit = match words.next()?.trim_end_matches(['.', '?']) {
        "day" | "days" => "day",
        "week" | "weeks" => "week",
        "month" | "months" => "month",
        "year" | "years" => "year",
        _ => return None,
    };
    words.next().is_none().then_some((n, unit))
}

/// `date` moved by a span (back, with `back`).
fn moved(date: NaiveDate, (n, unit): (u32, &str), back: bool) -> Option<NaiveDate> {
    match (unit, back) {
        ("day", false) => date.checked_add_days(Days::new(n.into())),
        ("day", true) => date.checked_sub_days(Days::new(n.into())),
        ("week", false) => date.checked_add_days(Days::new(u64::from(n) * 7)),
        ("week", true) => date.checked_sub_days(Days::new(u64::from(n) * 7)),
        ("month", false) => date.checked_add_months(Months::new(n)),
        ("month", true) => date.checked_sub_months(Months::new(n)),
        ("year", false) => date.checked_add_months(Months::new(n * 12)),
        ("year", true) => date.checked_sub_months(Months::new(n * 12)),
        _ => None,
    }
}

/// "Thursday, December 25, 2026", as this Mac writes it.
fn written(date: NaiveDate) -> String {
    let noon = Local.from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap_or_default()).earliest().map_or(0, |t| t.timestamp());
    crate::macos::zone_format(&crate::macos::local_zone(), noon, "EEEEMMMMdy")
}

/// "64 days", "1 day", "9 weeks and 1 day" after.
fn days(n: i64) -> String {
    match n.abs() {
        1 => "1 day".into(),
        n => format!("{} days", crate::listing::thousands(n)),
    }
}

/// "in 133 days", "today", "12 days ago".
fn from_today(date: NaiveDate, today: NaiveDate) -> String {
    match (date - today).num_days() {
        0 => "today".into(),
        1 => "tomorrow".into(),
        -1 => "yesterday".into(),
        n if n > 0 => format!("in {}", days(n)),
        n => format!("{} ago", days(n)),
    }
}

/// `text`'s start, if it's one of `prefixes`: what's after.
fn after<'a>(text: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| text.strip_prefix(p)).map(str::trim)
}

/// The answer to what's typed, if it's date math.
pub fn answer(text: &str) -> Option<Answer> {
    answer_on(text, Local::now().date_naive())
}

fn answer_on(text: &str, today: NaiveDate) -> Option<Answer> {
    let q = text.trim().to_lowercase();
    let q = q.trim_end_matches('?').trim();
    let row = |shown: String, about: String, copied: String| Some(Answer { shown, about, copied });
    // "days until christmas", "how long until march 3".
    if let Some(rest) = after(q, &["how many days until ", "how many days till ", "how many days to ", "how long until ", "how long till ", "days until ", "days till ", "days to ", "countdown to ", "time until "]) {
        let date = date_of(rest, today, Lean::Ahead)?;
        let n = (date - today).num_days();
        let shown = match n {
            0 => "Today".to_string(),
            n if n < 0 => format!("{} ago", days(n)),
            n => days(n),
        };
        let weeks = if n >= 14 { format!(" · {} weeks{}", n / 7, if n % 7 > 0 { format!(" and {}", days(n % 7)) } else { String::new() }) } else { String::new() };
        return row(shown, format!("until {}{weeks}", written(date)), n.to_string());
    }
    // "days since 1 jan".
    if let Some(rest) = after(q, &["how many days since ", "how long since ", "days since "]) {
        let date = date_of(rest, today, Lean::Behind)?;
        let n = (today - date).num_days();
        return row(days(n), format!("since {}", written(date)), n.to_string());
    }
    // "days between 1 jan and 3 mar", "days from 1 jan to 3 mar".
    if let Some(rest) = after(q, &["days between ", "how many days between ", "days from "]) {
        let (a, b) = rest.split_once(" and ").or_else(|| rest.split_once(" to "))?;
        let (a, b) = (date_of(a, today, Lean::ThisYear)?, date_of(b, today, Lean::ThisYear)?);
        let n = (b - a).num_days().abs();
        return row(days(n), format!("from {} to {}", written(a.min(b)), written(a.max(b))), n.to_string());
    }
    // "what day is 3 march 2027", "what day was christmas".
    if let Some(rest) = after(q, &["what day is ", "what day was ", "what day of the week is ", "which day is ", "day of "]) {
        let date = date_of(rest, today, if q.contains(" was ") { Lean::Behind } else { Lean::Ahead })?;
        let day = written(date);
        let name = day.split([',', ' ']).next().unwrap_or("").to_string();
        return row(name.clone(), format!("{day} · {}", from_today(date, today)), name);
    }
    // "100 days from today", "2 weeks from friday", "30 days ago", "in 3 weeks".
    let (span_text, from, back) = if let Some(s) = q.strip_suffix(" ago") {
        (s, today, true)
    } else if let Some(s) = q.strip_prefix("in ") {
        (s, today, false)
    } else if let Some((s, base)) = q.split_once(" from ").or_else(|| q.split_once(" after ")) {
        (s, date_of(base, today, Lean::ThisYear)?, false)
    } else if let Some((s, base)) = q.split_once(" before ") {
        (s, date_of(base, today, Lean::ThisYear)?, true)
    } else {
        ("", today, false)
    };
    if let Some(span) = span(span_text) {
        let date = moved(from, span, back)?;
        let base = if from == today { "today".to_string() } else { written(from) };
        return row(written(date), format!("{span_text} {} {base} · {}", if back { "before" } else { "after" }, from_today(date, today)), date.format("%Y-%m-%d").to_string());
    }
    // "week number", "what week is it".
    if ["week number", "what week is it", "week of the year", "current week", "which week is it"].contains(&q) {
        let week = today.iso_week().week();
        return row(format!("Week {week}"), format!("{} · ISO week of {}", written(today), today.iso_week().year()), week.to_string());
    }
    // "today's date", "what's the date".
    if ["date", "today", "todays date", "today's date", "what's the date", "whats the date", "what is the date", "what's today", "what is today", "what day is it", "what day is today"].contains(&q) {
        let year_days = if NaiveDate::from_ymd_opt(today.year(), 2, 29).is_some() { 366 } else { 365 };
        return row(written(today), format!("Week {} · day {} of {year_days}", today.iso_week().week(), today.ordinal()), today.format("%Y-%m-%d").to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Lean, answer_on, date_of, easter};
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn dates() {
        // A Wednesday.
        let today = d(2026, 10, 21);
        assert_eq!(date_of("christmas", today, Lean::Ahead), Some(d(2026, 12, 25)));
        assert_eq!(date_of("march 3", today, Lean::Ahead), Some(d(2027, 3, 3)), "the next");
        assert_eq!(date_of("march 3", today, Lean::Behind), Some(d(2026, 3, 3)));
        assert_eq!(date_of("3rd march 2028", today, Lean::Ahead), Some(d(2028, 3, 3)));
        assert_eq!(date_of("friday", today, Lean::Ahead), Some(d(2026, 10, 23)));
        assert_eq!(date_of("next wednesday", today, Lean::Ahead), Some(d(2026, 10, 28)));
        assert_eq!(date_of("new year", today, Lean::Ahead), Some(d(2027, 1, 1)));
        assert_eq!(easter(2026), Some(d(2026, 4, 5)));
        assert_eq!(easter(2027), Some(d(2027, 3, 28)));
        assert_eq!(date_of("thanksgiving", today, Lean::Ahead), Some(d(2026, 11, 26)));
    }

    fn shown(q: &str) -> Option<(String, String)> {
        answer_on(q, d(2026, 10, 21)).map(|a| (a.shown, a.copied))
    }

    #[test]
    fn answers() {
        assert_eq!(shown("days until christmas"), Some(("65 days".into(), "65".into())));
        assert_eq!(shown("How many days until new year?"), Some(("72 days".into(), "72".into())));
        assert_eq!(shown("days since jan 1"), Some(("293 days".into(), "293".into())));
        assert_eq!(shown("days between 1 jan and 3 mar").map(|s| s.1), Some("61".into()));
        assert_eq!(shown("100 days from today").map(|s| s.1), Some("2027-01-29".into()));
        assert_eq!(shown("in 2 weeks").map(|s| s.1), Some("2026-11-04".into()));
        assert_eq!(shown("30 days ago").map(|s| s.1), Some("2026-09-21".into()));
        assert_eq!(shown("3 months from christmas").map(|s| s.1), Some("2027-03-25".into()));
        assert_eq!(shown("week number").map(|s| s.1), Some("43".into()));
        assert!(shown("what day is 3 march 2027").is_some());
        assert_eq!(shown("in 20 minutes"), None, "a reminder's, not a date's");
        assert_eq!(shown("days"), None);
        assert_eq!(shown("photos from last week"), None);
    }
}

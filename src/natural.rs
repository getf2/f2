//! Search in plain words: "photos from last week", "big videos", "pdfs
//! from today", "screenshots in downloads", "invoices from march". The
//! words F2 knows become fsearch's filters (type:, ext:, size:, mtime:,
//! in:); the rest stay names to look for. What it understood is said with
//! the results ("Photos · in the last week"), newest (or biggest) first.

use chrono::{Datelike, Local, NaiveDate, TimeZone};

/// A plain-words search, as fsearch's query language has it.
#[derive(Debug, PartialEq)]
pub struct Plain {
    pub query: String,
    /// What it understood: "Photos · in the last week".
    pub about: String,
    pub order: Order,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Order {
    Newest,
    Biggest,
    Best,
}

/// Kinds of files, by the words for them: the filter, and what to call it.
const KINDS: &[(&[&str], &str, &str)] = &[
    // (Photos: what cameras and phones make, not icons and drawings.)
    (&["photo", "photos", "picture", "pictures", "pic", "pics", "image", "images"], "ext:jpg,jpeg,heic,heif,png,gif,webp,tif,tiff,dng,raw,cr2,cr3,nef,arw", "Photos"),
    (&["screenshot", "screenshots"], "ext:png,jpg,heic screenshot", "Screenshots"),
    (&["video", "videos", "movie", "movies", "clip", "clips", "recording", "recordings"], "type:video", "Videos"),
    (&["music", "song", "songs", "audio", "track", "tracks"], "type:audio", "Music"),
    (&["document", "documents", "doc", "docs"], "type:doc", "Documents"),
    (&["pdf", "pdfs"], "ext:pdf", "PDFs"),
    (&["presentation", "presentations", "slides", "keynote", "keynotes", "powerpoint"], "ext:key,ppt,pptx", "Presentations"),
    (&["spreadsheet", "spreadsheets", "excel", "sheets"], "ext:xls,xlsx,numbers,csv", "Spreadsheets"),
    (&["zip", "zips", "archive", "archives"], "type:archive", "Archives"),
    (&["installer", "installers", "dmg", "dmgs"], "ext:dmg,pkg", "Installers"),
    (&["app", "apps", "application", "applications"], "type:app", "Apps"),
    (&["folder", "folders"], "kind:dir", "Folders"),
    (&["font", "fonts"], "type:font", "Fonts"),
];

/// A kind word alone ("photos", "music") is as likely a name to look for
/// (the Photos app, a Music folder): only these mean the kind by themselves.
const ALONE: &[&str] = &["screenshots", "pdfs", "installers", "spreadsheets", "presentations", "videos", "fonts"];

/// The folders "in downloads" and the like mean.
const PLACES: &[(&str, &str, &str)] = &[
    ("downloads", "~/Downloads", "in Downloads"),
    ("desktop", "~/Desktop", "on the Desktop"),
    ("documents", "~/Documents", "in Documents"),
    ("pictures", "~/Pictures", "in Pictures"),
    ("movies", "~/Movies", "in Movies"),
    ("music", "~/Music", "in Music"),
];

/// Words that only join the others ("photos from the beach I took").
const FILLER: &[&str] = &[
    "from", "in", "on", "of", "the", "my", "a", "an", "that", "which", "with", "made", "changed", "modified", "edited", "taken", "saved", "downloaded",
    "created", "files", "file", "all", "any", "some", "i", "me", "show", "find", "for", "at", "since", "during", "were", "was", "are", "ago",
];

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

/// Seconds since a local date's start.
fn age_since(date: NaiveDate, now: chrono::DateTime<Local>) -> i64 {
    let start = Local.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).earliest().unwrap_or(now);
    (now - start).num_seconds().max(1)
}

/// `text` in plain words, as a search fsearch understands; None if it's
/// not (a name, or fsearch's own language already).
pub fn translate(text: &str) -> Option<Plain> {
    translate_at(text, Local::now())
}

fn translate_at(text: &str, now: chrono::DateTime<Local>) -> Option<Plain> {
    if text.contains(':') || text.contains('/') || text.contains('*') || text.trim_start().starts_with(['\'', '^', '!', '.']) {
        return None;
    }
    let words: Vec<String> = text.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).filter(|w| !w.is_empty()).collect();
    let mut filters: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let (mut kind, mut when, mut size, mut place): (Option<&str>, Option<String>, Option<&str>, Option<&str>) = (None, None, None, None);
    let mut order = Order::Best;
    let today = now.date_naive();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].as_str();
        let next = words.get(i + 1).map(String::as_str);
        let prev = i.checked_sub(1).map(|p| words[p].as_str());
        // Times: "today", "this week", "last 3 days", "in march".
        let span: Option<(String, String, usize)> = match (w, next) {
            ("today", _) => Some((format!("mtime:<{}s", age_since(today, now)), "today".into(), 1)),
            ("yesterday", _) => {
                let a = age_since(today, now);
                Some((format!("mtime:{a}s..{}s", a + 86_400), "yesterday".into(), 1))
            }
            ("recent" | "recently" | "lately" | "new" | "newest" | "latest", _) => Some(("mtime:<14d".into(), "lately".into(), 1)),
            ("this", Some("week")) => {
                let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
                Some((format!("mtime:<{}s", age_since(monday, now)), "this week".into(), 2))
            }
            ("this", Some("month")) => Some((format!("mtime:<{}s", age_since(today.with_day(1).unwrap(), now)), "this month".into(), 2)),
            ("this", Some("year")) => Some((format!("mtime:<{}s", age_since(today.with_ordinal(1).unwrap(), now)), "this year".into(), 2)),
            ("last" | "past", Some("week")) => Some(("mtime:<7d".into(), "in the last week".into(), 2)),
            ("last" | "past", Some("month")) => Some(("mtime:<30d".into(), "in the last month".into(), 2)),
            ("last" | "past", Some("year")) => Some(("mtime:<365d".into(), "in the last year".into(), 2)),
            ("last" | "past", Some(n)) if n.parse::<u32>().is_ok() => {
                let n: u32 = n.parse().unwrap();
                let unit = words.get(i + 2).map(String::as_str).unwrap_or("");
                let (code, said) = match unit.trim_end_matches('s') {
                    "day" => ("d", "days"),
                    "week" => ("w", "weeks"),
                    "month" => ("mo", "months"),
                    "hour" => ("h", "hours"),
                    _ => ("", ""),
                };
                (!code.is_empty()).then(|| (format!("mtime:<{n}{code}"), format!("in the last {n} {said}"), 3))
            }
            ("old", _) | ("older", _) => Some(("mtime:>365d".into(), "over a year old".into(), 1)),
            (m, _) if MONTHS.contains(&m) && matches!(prev, Some("in" | "from" | "during" | "since")) => {
                let month = MONTHS.iter().position(|x| *x == m).unwrap() as u32 + 1;
                // (This year's, or last year's if it's still to come.)
                let year = if month > today.month() { today.year() - 1 } else { today.year() };
                let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                let after = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }.unwrap();
                let (newer, older) = (if after > today { 1 } else { age_since(after, now) }, age_since(first, now));
                let name = format!("{}{}", m[..1].to_uppercase(), &m[1..]);
                Some((format!("mtime:{newer}s..{older}s"), format!("in {name}"), 1))
            }
            _ => None,
        };
        if let Some((filter, said, used)) = span {
            if when.is_none() {
                filters.push(filter);
                when = Some(said);
            }
            i += used;
            continue;
        }
        // Places: "in downloads", "on the desktop", "my documents".
        if let Some((_, folder, said)) = PLACES.iter().find(|(word, ..)| *word == w)
            && matches!(prev, Some("in" | "from" | "on" | "my" | "the"))
            && place.is_none()
        {
            filters.push(format!("in:{folder}"));
            place = Some(said);
            i += 1;
            continue;
        }
        // Sizes: "big", "huge", "small".
        let sized = match w {
            "big" | "large" | "biggest" | "largest" => Some(("size:>100mb", "big")),
            "huge" | "giant" | "enormous" => Some(("size:>1gb", "huge")),
            "small" | "tiny" => Some(("size:<1mb", "small")),
            _ => None,
        };
        if let Some((filter, said)) = sized {
            if size.is_none() {
                filters.push(filter.into());
                size = Some(said);
            }
            i += 1;
            continue;
        }
        // Kinds: "photos", "pdfs".
        if let Some((_, filter, said)) = KINDS.iter().find(|(names, ..)| names.contains(&w))
            && kind.is_none()
        {
            filters.push((*filter).into());
            kind = Some(said);
            i += 1;
            continue;
        }
        if !FILLER.contains(&w) {
            names.push(w.to_string());
        }
        i += 1;
    }
    // A kind word alone ("photos", "music") is more likely a name.
    let alone = kind.is_some() && when.is_none() && size.is_none() && place.is_none() && names.is_empty();
    if (kind.is_none() && when.is_none() && size.is_none() && place.is_none()) || (alone && !words.iter().any(|w| ALONE.contains(&w.as_str()))) {
        return None;
    }
    if when.is_some() {
        order = Order::Newest;
    } else if size.is_some_and(|s| s != "small") {
        order = Order::Biggest;
    }
    // "Big photos in Downloads from last week", as it'll be said.
    let mut about = match (size, kind) {
        (Some(size), Some(kind)) => format!("{}{} {}", size[..1].to_uppercase(), &size[1..], kind.to_lowercase()),
        (Some(size), None) => format!("{}{} files", size[..1].to_uppercase(), &size[1..]),
        (None, Some(kind)) => kind.to_string(),
        (None, None) => "Files".into(),
    };
    if !names.is_empty() {
        about += &format!(" named “{}”", names.join(" "));
    }
    if let Some(place) = place {
        about += &format!(" {place}");
    }
    if let Some(when) = when {
        about += &format!(" · {}", if when == "lately" { "changed lately".to_string() } else { format!("changed {when}") });
    }
    let mut query = names;
    query.extend(filters);
    Some(Plain { query: query.join(" "), about, order })
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::{Order, translate_at};

    fn at(text: &str) -> Option<(String, String, Order)> {
        // (Tuesday 20 October 2026, 15:00.)
        let now = Local.with_ymd_and_hms(2026, 10, 20, 15, 0, 0).unwrap();
        translate_at(text, now).map(|p| (p.query, p.about, p.order))
    }

    #[test]
    fn plain_words() {
        let photos = "ext:jpg,jpeg,heic,heif,png,gif,webp,tif,tiff,dng,raw,cr2,cr3,nef,arw";
        assert_eq!(at("photos from last week"), Some((format!("{photos} mtime:<7d"), "Photos · changed in the last week".into(), Order::Newest)));
        assert_eq!(at("big videos"), Some(("size:>100mb type:video".into(), "Big videos".into(), Order::Biggest)));
        assert_eq!(at("pdfs from today"), Some(("ext:pdf mtime:<54000s".into(), "PDFs · changed today".into(), Order::Newest)));
        assert_eq!(at("screenshots in downloads").unwrap().0, "ext:png,jpg,heic screenshot in:~/Downloads");
        // (A month's ages depend on the clocks changing: its shape.)
        let (query, about, order) = at("invoice pdfs from march").unwrap();
        assert!(query.starts_with("invoice ext:pdf mtime:") && query.contains("s.."), "{query}");
        assert_eq!((about.as_str(), order), ("PDFs named “invoice” · changed in March", Order::Newest));
        assert_eq!(at("documents this week").unwrap().0, "type:doc mtime:<140400s", "since Monday");
        assert_eq!(at("photos from the last 3 days").unwrap().0, format!("{photos} mtime:<3d"));
        assert_eq!(at("huge files").unwrap().1, "Huge files");
    }

    #[test]
    fn names_stay_names() {
        for text in ["photos", "music", "Downloads", "invoice", "report 2024", "ext:pdf", "src/main.rs", "'exact", "*.rs"] {
            assert_eq!(at(text), None, "{text}");
        }
        assert!(at("screenshots").is_some(), "a plural kind alone means the kind");
    }
}

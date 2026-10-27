//! Sunrise and sunset, for Quick Open: "sunset", "sunrise in tokyo", "when
//! is sunset in paris", "day length". Worked out here (NOAA's solar
//! equations: to a minute or two), for the city of the Mac's time zone, or
//! the one asked, in its own time.

use chrono::{Datelike, Duration, NaiveDate, Utc};

/// What's asked: sunrise, sunset, or both (and the day's length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Asked {
    Sunrise,
    Sunset,
    Both,
}

/// What's asked, and where (None: here).
pub fn asked(q: &str) -> Option<(Asked, Option<String>)> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    let lower = ["when is ", "what time is ", "when's ", "time of "].iter().find_map(|p| lower.strip_prefix(p)).unwrap_or(&lower).trim().to_string();
    const WORDS: &[(&str, Asked)] = &[
        ("sunrise and sunset", Asked::Both),
        ("sunrise", Asked::Sunrise),
        ("sun rise", Asked::Sunrise),
        ("sunset", Asked::Sunset),
        ("sun set", Asked::Sunset),
        ("dawn", Asked::Sunrise),
        ("dusk", Asked::Sunset),
        ("day length", Asked::Both),
        ("daylight", Asked::Both),
        ("hours of daylight", Asked::Both),
    ];
    for (word, what) in WORDS {
        if lower == *word {
            return Some((*what, None));
        }
        // ("sunset in tokyo", "sunset tokyo", "tokyo sunset".)
        if let Some(rest) = lower.strip_prefix(&format!("{word} ")) {
            let rest = rest.trim();
            let rest = rest.strip_prefix("in ").or_else(|| rest.strip_prefix("at ")).unwrap_or(rest).trim();
            return (!rest.is_empty()).then(|| (*what, Some(rest.to_string())));
        }
        if let Some(rest) = lower.strip_suffix(&format!(" {word}")) {
            let rest = rest.trim().trim_end_matches("'s").trim();
            return (!rest.is_empty() && rest.split_whitespace().count() <= 3).then(|| (*what, Some(rest.to_string())));
        }
    }
    None
}

/// Sunrise and sunset on `date` at latitude `lat`, longitude `lon`, as
/// seconds since 1970: None for one that doesn't come that day (a polar
/// day, or night).
pub fn times(date: NaiveDate, lat: f64, lon: f64) -> (Option<i64>, Option<i64>) {
    let n = date.ordinal() as f64;
    let g = 2. * std::f64::consts::PI / 365. * (n - 1.);
    // (The equation of time, in minutes, and the sun's declination.)
    let eq = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2. * g).cos() - 0.040849 * (2. * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2. * g).cos() + 0.000907 * (2. * g).sin() - 0.002697 * (3. * g).cos() + 0.00148 * (3. * g).sin();
    let lat = lat.to_radians();
    let cos_ha = (90.833f64.to_radians().cos() / (lat.cos() * decl.cos())) - lat.tan() * decl.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return (None, None);
    }
    let ha = cos_ha.acos().to_degrees();
    let midnight = date.and_hms_opt(0, 0, 0).map(|t| t.and_utc().timestamp()).unwrap_or(0);
    let at = |minutes: f64| midnight + (minutes * 60.).round() as i64;
    (Some(at(720. - 4. * (lon + ha) - eq)), Some(at(720. - 4. * (lon - ha) - eq)))
}

/// The next sunrise or sunset there (today's, else tomorrow's), and the
/// day's length, for `zone`'s date. With "Today" or "Tomorrow".
pub fn next(what: Asked, at: (f32, f32), zone: &str) -> Option<(i64, bool)> {
    let now = Utc::now().timestamp();
    let offset = crate::macos::zone_offset(zone, now).unwrap_or(0) as i64;
    let today = chrono::DateTime::from_timestamp(now + offset, 0)?.date_naive();
    for (i, day) in [today, today + Duration::days(1)].into_iter().enumerate() {
        let (rise, set) = times(day, at.0 as f64, at.1 as f64);
        let when = if what == Asked::Sunset { set } else { rise };
        if let Some(when) = when.filter(|w| *w > now) {
            return Some((when, i == 1));
        }
    }
    None
}

/// The day's length there today, in minutes (None: the sun doesn't set,
/// or rise).
pub fn day_length(at: (f32, f32), zone: &str) -> Option<i64> {
    let now = Utc::now().timestamp();
    let offset = crate::macos::zone_offset(zone, now).unwrap_or(0) as i64;
    let today = chrono::DateTime::from_timestamp(now + offset, 0)?.date_naive();
    match times(today, at.0 as f64, at.1 as f64) {
        (Some(rise), Some(set)) => Some((set - rise) / 60),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Asked, asked, times};
    use chrono::NaiveDate;

    #[test]
    fn asking() {
        assert_eq!(asked("sunset"), Some((Asked::Sunset, None)));
        assert_eq!(asked("When is sunrise in Tokyo?"), Some((Asked::Sunrise, Some("tokyo".into()))));
        assert_eq!(asked("reykjavik sunset"), Some((Asked::Sunset, Some("reykjavik".into()))));
        assert_eq!(asked("day length"), Some((Asked::Both, None)));
        assert_eq!(asked("sunset boulevard movie tickets tonight"), Some((Asked::Sunset, Some("boulevard movie tickets tonight".into()))), "a place it may not find: no rows then");
        assert_eq!(asked("sunsets"), None);
    }

    #[test]
    fn worked_out() {
        // Tokyo, 21 October 2026: sunrise about 05:52, sunset about 17:00
        // (UTC+9), give or take a couple of minutes.
        let (rise, set) = times(NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(), 35.69, 139.69);
        let hm = |t: i64| {
            let t = chrono::DateTime::from_timestamp(t + 9 * 3600, 0).unwrap();
            t.format("%H:%M").to_string()
        };
        let (rise, set) = (hm(rise.unwrap()), hm(set.unwrap()));
        assert!(("05:49".."05:56").contains(&rise.as_str()), "{rise}");
        assert!(("16:57".."17:04").contains(&set.as_str()), "{set}");
        // Svalbard in June: the sun doesn't set.
        assert_eq!(times(NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(), 78.2, 15.6), (None, None));
    }
}

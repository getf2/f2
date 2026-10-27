//! Public holidays, for Quick Open: "holidays", "next holiday", "holidays in
//! japan", "iceland holidays". The Mac's region's, or the country asked,
//! from Nager.Date (https://date.nager.at: free, no key; it's sent the
//! country and the year), kept on disk once fetched (a year's don't
//! change).

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Holiday {
    pub date: NaiveDate,
    /// In English, and as it's called there ("Culture Day", "文化の日").
    pub name: String,
    pub local: String,
}

/// What's asked: holidays here (None), or in a country (its name, as
/// typed).
pub fn asked(q: &str) -> Option<Option<String>> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    const HERE: &[&str] = &["holidays", "holiday", "next holiday", "public holidays", "bank holidays", "next public holiday", "next bank holiday", "upcoming holidays", "national holidays", "days off", "祝日", "공휴일"];
    if HERE.contains(&lower.as_str()) {
        return Some(None);
    }
    const BEFORE: &[&str] = &["public holidays in ", "holidays in ", "next holiday in ", "bank holidays in ", "holidays for ", "national holidays in "];
    const AFTER: &[&str] = &[" public holidays", " holidays", " national holidays", " bank holidays", "の祝日"];
    let place = BEFORE.iter().find_map(|p| lower.strip_prefix(p)).or_else(|| AFTER.iter().find_map(|a| lower.strip_suffix(a)))?.trim();
    (!place.is_empty() && place.split_whitespace().count() <= 3).then(|| Some(place.to_string()))
}

/// The country (its code, and name): the Mac's region's, or one by name
/// ("japan", "usa", "japanese").
pub fn country(name: Option<&str>) -> Option<(String, String)> {
    match name {
        None => {
            let code = crate::macos::region_code()?;
            let name = crate::world_time::country_named_by_code(&code).unwrap_or(&code).to_string();
            Some((code, name))
        }
        Some(name) => {
            const ALSO: &[(&str, &str)] = &[
                ("usa", "US"),
                ("us", "US"),
                ("america", "US"),
                ("american", "US"),
                ("uk", "GB"),
                ("britain", "GB"),
                ("british", "GB"),
                ("england", "GB"),
                ("japanese", "JP"),
                ("日本", "JP"),
                ("korean", "KR"),
                ("한국", "KR"),
                ("icelandic", "IS"),
                ("german", "DE"),
                ("french", "FR"),
                ("spanish", "ES"),
                ("italian", "IT"),
                ("canadian", "CA"),
                ("australian", "AU"),
            ];
            let code = ALSO.iter().find(|(n, _)| *n == name).map(|(_, c)| c.to_string()).or_else(|| crate::world_time::country_code(name).map(str::to_string))?;
            let name = crate::world_time::country_named_by_code(&code).unwrap_or(&code).to_string();
            Some((code, name))
        }
    }
}

fn kept(code: &str, year: i32) -> PathBuf {
    crate::settings::data_dir().join("Holidays").join(format!("{code}-{year}.json"))
}

/// A year's, from disk, else fetched (and kept). Waits on the network: not
/// on the main thread.
fn year(code: &str, year: i32) -> Result<Vec<Holiday>, String> {
    let file = kept(code, year);
    let json = match std::fs::read(&file) {
        Ok(json) => json,
        Err(_) => {
            let url = format!("https://date.nager.at/api/v3/PublicHolidays/{year}/{code}");
            let out = std::process::Command::new("/usr/bin/curl").args(["-fsS", "--max-time", "5", &url]).output().map_err(|e| e.to_string())?;
            if !out.status.success() || out.stdout.is_empty() {
                return Err("The holidays couldn't be fetched (no connection, or a country it doesn't know).".into());
            }
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&file, &out.stdout);
            out.stdout
        }
    };
    Ok(read(&json))
}

/// Nager.Date's list, nationwide ones only (not a state's or a county's).
fn read(json: &[u8]) -> Vec<Holiday> {
    let Ok(Value::Array(list)) = serde_json::from_slice::<Value>(json) else { return Vec::new() };
    let mut found: Vec<Holiday> = list
        .iter()
        .filter(|h| h["global"].as_bool() != Some(false))
        .filter_map(|h| {
            let date = NaiveDate::parse_from_str(h["date"].as_str()?, "%Y-%m-%d").ok()?;
            Some(Holiday { date, name: h["name"].as_str()?.to_string(), local: h["localName"].as_str().unwrap_or("").to_string() })
        })
        .collect();
    // (One a day: some are listed twice, by their two names.)
    found.dedup_by(|a, b| a.date == b.date);
    found
}

/// The next `n` from `today` on (this year's, then next year's).
pub fn upcoming(code: &str, today: NaiveDate, n: usize) -> Result<Vec<Holiday>, String> {
    let mut all = year(code, today.year())?;
    if all.iter().filter(|h| h.date >= today).count() < n {
        all.extend(year(code, today.year() + 1).unwrap_or_default());
    }
    Ok(all.into_iter().filter(|h| h.date >= today).take(n).collect())
}

#[cfg(test)]
mod tests {
    use super::{Holiday, asked, read, upcoming};
    use chrono::NaiveDate;

    #[test]
    fn asking() {
        assert_eq!(asked("holidays"), Some(None));
        assert_eq!(asked("Next holiday?"), Some(None));
        assert_eq!(asked("holidays in japan"), Some(Some("japan".into())));
        assert_eq!(asked("iceland holidays"), Some(Some("iceland".into())));
        assert_eq!(asked("日本の祝日"), Some(Some("日本".into())));
        assert_eq!(asked("holiday inn"), None);
    }

    #[test]
    fn read_nationwide() {
        let json = r#"[{"date":"2026-11-03","localName":"文化の日","name":"Culture Day","global":true},
            {"date":"2026-11-11","localName":"Veterans Day","name":"Veterans Day","global":false}]"#;
        assert_eq!(read(json.as_bytes()), [Holiday { date: NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(), name: "Culture Day".into(), local: "文化の日".into() }]);
    }

    /// Japan's, fetched: `cargo test --release -- --ignored japan_fetched`
    #[test]
    #[ignore]
    fn japan_fetched() {
        let next = upcoming("JP", NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(), 3).unwrap();
        println!("{next:?}");
        assert_eq!(next[0].name, "Culture Day");
    }
}

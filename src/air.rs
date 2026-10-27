//! The air, for Quick Open: "air quality", "fine dust", "미세먼지", "aqi",
//! "uv", "air quality in seoul", "서울 미세먼지". Fine dust (PM2.5 and
//! PM10), graded as Korea grades it there (where it's asked after most) and
//! by the US AQI elsewhere, with what to do about it; and the UV index,
//! now and at its highest today. From Open-Meteo's air quality API
//! (https://open-meteo.com: CAMS's data, free, no key; it's sent the city's
//! whereabouts), kept ten minutes.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

/// What's asked: the air (or the UV), here (None) or somewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub place: Option<String>,
    pub uv: bool,
}

pub fn asked(q: &str) -> Option<Asked> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    let lower = lower.trim();
    const AIR: &[&str] = &["air quality", "aqi", "fine dust", "air pollution", "pollution", "smog", "dust", "pm2.5", "pm 2.5", "pm10", "미세먼지", "초미세먼지", "대기질", "공기질", "大気汚染", "空気質", "空气质量"];
    const UV: &[&str] = &["uv", "uv index", "uv today", "sunscreen", "do i need sunscreen", "자외선", "紫外線"];
    let here = |words: &[&str], uv: bool| words.contains(&lower).then_some(Asked { place: None, uv });
    if let Some(asked) = here(AIR, false).or_else(|| here(UV, true)) {
        return Some(asked);
    }
    for (words, uv) in [(AIR, false), (UV, true)] {
        for word in words.iter() {
            // "air quality in tokyo", "tokyo air quality", "서울 미세먼지", "서울의 미세먼지".
            let place = lower
                .strip_prefix(&format!("{word} in "))
                .or_else(|| lower.strip_prefix(&format!("{word} for ")))
                .or_else(|| lower.strip_suffix(&format!("의 {word}")))
                .or_else(|| lower.strip_suffix(&format!(" {word}")))
                .or_else(|| lower.strip_suffix(&format!("の{word}")));
            if let Some(place) = place.map(str::trim).filter(|p| !p.is_empty() && p.split_whitespace().count() <= 3 && !matches!(*p, "today" | "now" | "the")) {
                return Some(Asked { place: Some(place.to_string()), uv });
            }
        }
    }
    None
}

/// The air somewhere, now.
#[derive(Clone, Debug)]
pub struct Air {
    pub place: String,
    pub pm25: f64,
    pub pm10: f64,
    pub aqi: Option<f64>,
    pub uv: f64,
    pub uv_high: f64,
}

static KNOWN: Mutex<Option<HashMap<(i32, i32), (i64, Air)>>> = Mutex::new(None);

/// The air there (waits on the network: not on the main thread).
pub fn fetch(place: &str, at: (f32, f32)) -> Result<Air, String> {
    let key = ((at.0 * 100.) as i32, (at.1 * 100.) as i32);
    if let Some((when, air)) = KNOWN.lock().unwrap().as_ref().and_then(|k| k.get(&key))
        && crate::timers::now() - when < 600
    {
        return Ok(air.clone());
    }
    let url = format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={:.2}&longitude={:.2}&current=us_aqi,pm10,pm2_5,uv_index&hourly=uv_index&forecast_days=1&timezone=auto", at.0, at.1);
    let out = std::process::Command::new("/usr/bin/curl").args(["-fsS", "--max-time", "6", &url]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("The air couldn't be looked up (no connection?).".into());
    }
    let json: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let current = &json["current"];
    let uv_high = json["hourly"]["uv_index"].as_array().map_or(0., |all| all.iter().filter_map(Value::as_f64).fold(0., f64::max));
    let air = Air {
        place: place.to_string(),
        pm25: current["pm2_5"].as_f64().ok_or("No reading there.")?,
        pm10: current["pm10"].as_f64().unwrap_or(0.),
        aqi: current["us_aqi"].as_f64(),
        uv: current["uv_index"].as_f64().unwrap_or(0.),
        uv_high,
    };
    KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, (crate::timers::now(), air.clone()));
    Ok(air)
}

/// How it is: a grade, what to do, and how bad (0 good, to 3).
pub fn grade(air: &Air, korea: bool) -> (String, &'static str, u8) {
    if korea {
        // (Korea's grades: the worse of the two dusts'.)
        let pm25 = match air.pm25 {
            p if p <= 15. => 0,
            p if p <= 35. => 1,
            p if p <= 75. => 2,
            _ => 3,
        };
        let pm10 = match air.pm10 {
            p if p <= 30. => 0,
            p if p <= 80. => 1,
            p if p <= 150. => 2,
            _ => 3,
        };
        let level = pm25.max(pm10);
        let (name, advice) = [("Good", "Fine to be outside"), ("Moderate", "Fine for most; if you're sensitive, take it easy outside"), ("Bad", "Wear a mask (KF94) outside; keep the windows shut"), ("Very bad", "Stay inside if you can; a KF94 mask if not")][level as usize];
        return (name.to_string(), advice, level);
    }
    let aqi = air.aqi.unwrap_or(0.);
    let (name, advice, level) = match aqi {
        a if a <= 50. => ("Good", "Fine to be outside", 0),
        a if a <= 100. => ("Moderate", "Fine for most; if you're sensitive, take it easy outside", 1),
        a if a <= 150. => ("Unhealthy for sensitive groups", "If you're sensitive, spend less time outside", 2),
        a if a <= 200. => ("Unhealthy", "Spend less time outside; a mask helps", 2),
        a if a <= 300. => ("Very unhealthy", "Stay inside if you can", 3),
        _ => ("Hazardous", "Stay inside", 3),
    };
    (format!("{name} · AQI {aqi:.0}"), advice, level)
}

/// The UV index's grade, and what to do.
pub fn uv_grade(uv: f64) -> (&'static str, &'static str) {
    match uv.round() as i64 {
        ..=2 => ("Low", "No need for sunscreen"),
        3..=5 => ("Moderate", "Sunscreen if you're out a while"),
        6..=7 => ("High", "Sunscreen and a hat"),
        8..=10 => ("Very high", "Sunscreen, a hat, and shade at midday"),
        _ => ("Extreme", "Stay out of the midday sun"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Air, Asked, asked, grade};

    #[test]
    fn asking() {
        let here = |uv: bool| Some(Asked { place: None, uv });
        let there = |place: &str, uv: bool| Some(Asked { place: Some(place.into()), uv });
        assert_eq!(asked("air quality"), here(false));
        assert_eq!(asked("미세먼지"), here(false));
        assert_eq!(asked("uv index?"), here(true));
        assert_eq!(asked("air quality in tokyo"), there("tokyo", false));
        assert_eq!(asked("seoul air quality"), there("seoul", false));
        assert_eq!(asked("서울 미세먼지"), there("서울", false));
        assert_eq!(asked("부산의 미세먼지"), there("부산", false));
        assert_eq!(asked("uv in sydney"), there("sydney", true));
        assert_eq!(asked("air quality today"), None, "not a place");
        assert_eq!(asked("dustbin"), None);
        assert_eq!(asked("airpods"), None);
    }

    #[test]
    fn grades() {
        let air = |pm25: f64, pm10: f64, aqi: f64| Air { place: String::new(), pm25, pm10, aqi: Some(aqi), uv: 0., uv_high: 0. };
        assert_eq!(grade(&air(6., 8., 78.), true).0, "Good");
        assert_eq!(grade(&air(40., 60., 110.), true).0, "Bad");
        assert_eq!(grade(&air(6., 8., 78.), false).0, "Moderate · AQI 78");
    }
}

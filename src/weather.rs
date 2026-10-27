//! The weather, for Quick Open: "weather", "weather in tokyo", "reykjavik
//! weather", "forecast", "will it rain"; "東京の天気", "서울 날씨", "北京天气".
//! Now, and the next three days, from Open-Meteo (https://open-meteo.com:
//! free, no key; it's sent the city's whereabouts, to a kilometer or so),
//! kept ten minutes. "weather" alone is the city of the Mac's time zone (no
//! need to ask macOS where the Mac is). In °F or °C as the Mac has them.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

/// How long a forecast's kept.
const KEPT: i64 = 600;

/// The weather somewhere.
#[derive(Clone, Debug)]
pub struct Forecast {
    pub place: String,
    pub now: f64,
    pub feels: f64,
    pub code: i64,
    pub day: bool,
    pub wind: f64,
    /// The next days (today first): weekday, code, high, low, chance of rain.
    pub days: Vec<(String, i64, f64, f64, i64)>,
    pub fahrenheit: bool,
}

/// Where the weather's asked for: Some(None) here, Some(Some(place)).
pub fn asked(q: &str) -> Option<Option<String>> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    let lower = lower.trim();
    const HERE: &[&str] = &["weather", "forecast", "the weather", "weather today", "weather now", "temperature", "how's the weather", "hows the weather", "how is the weather", "is it raining", "will it rain", "will it rain today", "will it rain tomorrow", "天気", "天気予報", "날씨", "天气"];
    if HERE.contains(&lower) {
        return Some(None);
    }
    const BEFORE: &[&str] = &["weather in ", "weather at ", "weather for ", "forecast for ", "forecast in ", "temperature in ", "how's the weather in ", "hows the weather in ", "how is the weather in ", "weather ", "forecast "];
    const AFTER: &[&str] = &[" weather", " forecast", "の天気予報", "の天気", " 天気", "天気", "의 날씨", " 날씨", "날씨", "的天气", " 天气", "天气"];
    let place = BEFORE.iter().find_map(|p| lower.strip_prefix(p)).or_else(|| AFTER.iter().find_map(|a| lower.strip_suffix(a)))?.trim();
    // ("weather in": half typed, not a place called "in".)
    (!place.is_empty() && !matches!(place, "in" | "at" | "for")).then(|| Some(place.to_string()))
}

/// Where: what it's called, and its latitude and longitude (the Mac's time
/// zone's city, for none).
pub fn place(name: Option<&str>) -> Option<(String, (f32, f32))> {
    match name {
        Some(name) => crate::world_time::locate(name),
        None => crate::world_time::here(),
    }
}

static KNOWN: Mutex<Option<HashMap<(i32, i32), (i64, Forecast)>>> = Mutex::new(None);

fn key(at: (f32, f32)) -> (i32, i32) {
    ((at.0 * 100.) as i32, (at.1 * 100.) as i32)
}

/// The forecast there, if it's been fetched lately.
pub fn kept(at: (f32, f32)) -> Option<Forecast> {
    let known = KNOWN.lock().unwrap();
    known.as_ref()?.get(&key(at)).filter(|(when, f)| crate::timers::now() - when < KEPT && f.fahrenheit == crate::macos::fahrenheit()).map(|(_, f)| f.clone())
}

/// Fetch the forecast there (waits on the network: not on the main thread).
pub fn fetch(place: &str, at: (f32, f32)) -> Result<Forecast, String> {
    let fahrenheit = crate::macos::fahrenheit();
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.2}&longitude={:.2}&current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,is_day&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=4{}",
        at.0,
        at.1,
        if fahrenheit { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" }
    );
    let out = std::process::Command::new("/usr/bin/curl").args(["-fsS", "--max-time", "6", &url]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("The weather couldn't be fetched (no connection?).".into());
    }
    let json: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let current = &json["current"];
    let number = |v: &Value| v.as_f64().unwrap_or(0.);
    let daily = &json["daily"];
    let column = |name: &str| daily[name].as_array().cloned().unwrap_or_default();
    let (times, codes, highs, lows, rain) = (column("time"), column("weather_code"), column("temperature_2m_max"), column("temperature_2m_min"), column("precipitation_probability_max"));
    let days = (0..times.len())
        .map(|i| {
            let day = times[i].as_str().unwrap_or("").to_string();
            (day, codes.get(i).and_then(Value::as_i64).unwrap_or(0), highs.get(i).map_or(0., number), lows.get(i).map_or(0., number), rain.get(i).and_then(Value::as_i64).unwrap_or(0))
        })
        .collect();
    let forecast = Forecast {
        place: place.to_string(),
        now: number(&current["temperature_2m"]),
        feels: number(&current["apparent_temperature"]),
        code: current["weather_code"].as_i64().unwrap_or(0),
        day: current["is_day"].as_i64() != Some(0),
        wind: number(&current["wind_speed_10m"]),
        days,
        fahrenheit,
    };
    KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(key(at), (crate::timers::now(), forecast.clone()));
    Ok(forecast)
}

/// What the sky's doing (WMO's codes): words, an SF Symbol, and a color for
/// its tile.
pub fn sky(code: i64, day: bool) -> (&'static str, &'static str, u32) {
    const SUN: u32 = 0xff9f0a;
    const CLOUD: u32 = 0x8e8e93;
    const RAIN: u32 = 0x0a84ff;
    const SNOW: u32 = 0x64d2ff;
    const STORM: u32 = 0x5e5ce6;
    match code {
        0 => ("Clear", if day { "sun.max.fill" } else { "moon.stars.fill" }, if day { SUN } else { STORM }),
        1 => ("Mostly Clear", if day { "sun.max.fill" } else { "moon.fill" }, if day { SUN } else { STORM }),
        2 => ("Partly Cloudy", if day { "cloud.sun.fill" } else { "cloud.moon.fill" }, CLOUD),
        3 => ("Cloudy", "cloud.fill", CLOUD),
        45 | 48 => ("Fog", "cloud.fog.fill", CLOUD),
        51 | 53 | 55 => ("Drizzle", "cloud.drizzle.fill", RAIN),
        56 | 57 | 66 | 67 => ("Freezing Rain", "cloud.sleet.fill", SNOW),
        61 | 63 => ("Rain", "cloud.rain.fill", RAIN),
        65 => ("Heavy Rain", "cloud.heavyrain.fill", RAIN),
        71 | 73 | 77 => ("Snow", "cloud.snow.fill", SNOW),
        75 => ("Heavy Snow", "cloud.snow.fill", SNOW),
        80 | 81 | 82 => ("Showers", "cloud.sun.rain.fill", RAIN),
        85 | 86 => ("Snow Showers", "cloud.snow.fill", SNOW),
        95 => ("Thunderstorm", "cloud.bolt.rain.fill", STORM),
        96 | 99 => ("Thunderstorm, Hail", "cloud.bolt.rain.fill", STORM),
        _ => ("", "cloud.fill", CLOUD),
    }
}

/// "14°".
pub fn degrees(t: f64) -> String {
    format!("{}°", t.round() as i64)
}

/// A day ("2026-10-22") by its name: "Today", "Tomorrow", "Friday".
fn day_name(day: &str, i: usize) -> String {
    match i {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        _ => chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").map_or_else(|_| day.to_string(), |d| d.format("%A").to_string()),
    }
}

/// The forecast, said: the sky now ("14° Partly Cloudy"), beside it (feels,
/// high and low), and a line a day.
pub fn said(f: &Forecast) -> (String, String, Vec<String>) {
    let (words, ..) = sky(f.code, f.day);
    let now = format!("{} {words}", degrees(f.now));
    let today = f.days.first();
    let wind = format!("wind {} {}", f.wind.round() as i64, if f.fahrenheit { "mph" } else { "km/h" });
    let aside = match today {
        Some((_, _, high, low, _)) => format!("feels {} · H {} L {} · {wind}", degrees(f.feels), degrees(*high), degrees(*low)),
        None => format!("feels {} · {wind}", degrees(f.feels)),
    };
    let days = f
        .days
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, (day, code, high, low, rain))| {
            let rain = if *rain > 0 { format!(", {rain}% chance of rain") } else { String::new() };
            format!("{}: {}, {} / {}{rain}", day_name(day, i), sky(*code, true).0, degrees(*high), degrees(*low))
        })
        .collect();
    (now, aside, days)
}

#[cfg(test)]
mod tests {
    use super::asked;

    #[test]
    fn where_asked() {
        assert_eq!(asked("weather"), Some(None));
        assert_eq!(asked("Weather in Tokyo"), Some(Some("tokyo".into())));
        assert_eq!(asked("reykjavik weather"), Some(Some("reykjavik".into())));
        assert_eq!(asked("forecast for new york"), Some(Some("new york".into())));
        assert_eq!(asked("東京の天気"), Some(Some("東京".into())));
        assert_eq!(asked("서울 날씨"), Some(Some("서울".into())));
        assert_eq!(asked("北京天气"), Some(Some("北京".into())));
        assert_eq!(asked("will it rain?"), Some(None));
        assert_eq!(asked("weatherproof"), None);
    }

    #[test]
    fn places() {
        assert!(super::place(Some("tokyo")).is_some_and(|(label, (lat, _))| label.starts_with("Tokyo") && (lat - 35.7).abs() < 0.5));
        assert!(super::place(None).is_some(), "the time zone's city");
    }
}

//! The time anywhere, for Quick Open: "time in tokyo", "tokyo time", "what
//! time is it in reykjavik", "time in japan" or "time in california" (each
//! of its time zones), "pst", "utc+5:30"; and a time somewhere, somewhere
//! else: "3pm in tokyo" (3pm here, there), "3pm tokyo" (Tokyo's 3pm, here),
//! "9:30 london in new york". Some 7,500 cities (assets/cities.tsv, from
//! GeoNames: of 100,000 people and more, and every capital), countries and
//! states by name, and the zones' letters; the times as this Mac writes
//! them, by its own time zones. In Japanese, Chinese and Korean too:
//! "東京の時間", "东京时间", "도쿄 시간", "日本の時間".

use std::sync::OnceLock;

const TABLE: &str = include_str!("../assets/cities.tsv");
/// Rows at most.
const SHOWN: usize = 6;

/// A time, for its row: the time, what it's of, as copied.
pub struct Time {
    pub shown: String,
    pub about: String,
    pub copied: String,
}

struct City {
    name: &'static str,
    key: String,
    /// Its ASCII spelling's, if that's different.
    other: Option<String>,
    /// Its names in Japanese, Chinese and Korean ("東京", "도쿄").
    others: Vec<&'static str>,
    country: &'static str,
    state: &'static str,
    state_key: String,
    zone: &'static str,
    /// Where it is: latitude, longitude.
    at: (f32, f32),
}

struct Country {
    code: &'static str,
    name: &'static str,
    key: String,
}

struct Places {
    /// The biggest first.
    cities: Vec<City>,
    countries: Vec<Country>,
}

/// Somewhere with a time: what to call it, and its zone (a name, or
/// "offset:<seconds>").
#[derive(Clone, Debug, PartialEq)]
struct Place {
    label: String,
    zone: String,
    /// Where it is (latitude, longitude), for a city (or a country's).
    at: Option<(f32, f32)>,
}

/// Other names for cities ("nyc"), and their own names in their languages
/// where GeoNames' are English ("münchen").
const ALIASES: &[(&str, &str)] = &[
    ("nyc", "new york city"),
    ("new york", "new york city"),
    ("ny", "new york city"),
    ("manhattan", "new york city"),
    ("sf", "san francisco"),
    ("san fran", "san francisco"),
    ("bay area", "san francisco"),
    ("silicon valley", "san jose"),
    ("la", "los angeles"),
    ("hollywood", "los angeles"),
    ("dc", "washington"),
    ("washington dc", "washington"),
    ("washington d c", "washington"),
    ("vegas", "las vegas"),
    ("philly", "philadelphia"),
    ("nola", "new orleans"),
    ("hk", "hong kong"),
    ("kl", "kuala lumpur"),
    ("bombay", "mumbai"),
    ("calcutta", "kolkata"),
    ("madras", "chennai"),
    ("bangalore", "bengaluru"),
    ("peking", "beijing"),
    ("canton", "guangzhou"),
    ("saigon", "ho chi minh city"),
    ("hcmc", "ho chi minh city"),
    ("kiev", "kyiv"),
    ("rangoon", "yangon"),
    ("rio", "rio de janeiro"),
    ("cdmx", "mexico city"),
    ("den haag", "the hague"),
    ("munchen", "munich"),
    ("koln", "cologne"),
    ("wien", "vienna"),
    ("praha", "prague"),
    ("roma", "rome"),
    ("milano", "milan"),
    ("firenze", "florence"),
    ("napoli", "naples"),
    ("venezia", "venice"),
    ("torino", "turin"),
    ("lisboa", "lisbon"),
    ("moskva", "moscow"),
    ("kobenhavn", "copenhagen"),
    ("goteborg", "gothenburg"),
    ("warszawa", "warsaw"),
    ("bruxelles", "brussels"),
    ("brussel", "brussels"),
    ("geneve", "geneva"),
    ("athina", "athens"),
    ("beograd", "belgrade"),
    ("bucuresti", "bucharest"),
    ("tokio", "tokyo"),
    ("seul", "seoul"),
    ("bkk", "bangkok"),
    ("rvk", "reykjavik"),
];

/// Countries by other names: name, code.
const COUNTRIES: &[(&str, &str)] = &[
    ("usa", "US"),
    ("us", "US"),
    ("america", "US"),
    ("the us", "US"),
    ("the usa", "US"),
    ("united states of america", "US"),
    ("uk", "GB"),
    ("the uk", "GB"),
    ("britain", "GB"),
    ("great britain", "GB"),
    ("uae", "AE"),
    ("emirates", "AE"),
    ("holland", "NL"),
    ("the netherlands", "NL"),
    ("korea", "KR"),
    ("czech republic", "CZ"),
    ("turkey", "TR"),
    ("the philippines", "PH"),
    ("burma", "MM"),
    ("ivory coast", "CI"),
    ("vatican", "VA"),
    // (Theirs, in Japanese, Chinese and Korean.)
    ("日本", "JP"),
    ("일본", "JP"),
    ("韓国", "KR"),
    ("韩国", "KR"),
    ("한국", "KR"),
    ("中国", "CN"),
    ("中國", "CN"),
    ("중국", "CN"),
    ("台湾", "TW"),
    ("台灣", "TW"),
    ("대만", "TW"),
    ("アメリカ", "US"),
    ("美国", "US"),
    ("美國", "US"),
    ("미국", "US"),
    ("イギリス", "GB"),
    ("英国", "GB"),
    ("英國", "GB"),
    ("영국", "GB"),
    ("カナダ", "CA"),
    ("캐나다", "CA"),
    ("オーストラリア", "AU"),
    ("호주", "AU"),
    ("フランス", "FR"),
    ("프랑스", "FR"),
    ("ドイツ", "DE"),
    ("독일", "DE"),
    ("アイスランド", "IS"),
    ("아이슬란드", "IS"),
    ("香港", "HK"),
    ("홍콩", "HK"),
    ("シンガポール", "SG"),
    ("싱가포르", "SG"),
    ("タイ", "TH"),
    ("태국", "TH"),
    ("ベトナム", "VN"),
    ("베트남", "VN"),
    ("インド", "IN"),
    ("인도", "IN"),
];

/// The zones' letters, and the names of their times: what, zone, called.
const ABBREVIATIONS: &[(&str, &str, &str)] = &[
    ("utc", "UTC", "UTC"),
    ("gmt", "UTC", "GMT"),
    ("zulu", "UTC", "UTC"),
    ("pst", "America/Los_Angeles", "Pacific Time"),
    ("pdt", "America/Los_Angeles", "Pacific Time"),
    ("pt", "America/Los_Angeles", "Pacific Time"),
    ("pacific", "America/Los_Angeles", "Pacific Time"),
    ("mst", "America/Denver", "Mountain Time"),
    ("mdt", "America/Denver", "Mountain Time"),
    ("mt", "America/Denver", "Mountain Time"),
    ("mountain", "America/Denver", "Mountain Time"),
    ("cst", "America/Chicago", "Central Time"),
    ("cdt", "America/Chicago", "Central Time"),
    ("ct", "America/Chicago", "Central Time"),
    ("central", "America/Chicago", "Central Time"),
    ("est", "America/New_York", "Eastern Time"),
    ("edt", "America/New_York", "Eastern Time"),
    ("et", "America/New_York", "Eastern Time"),
    ("eastern", "America/New_York", "Eastern Time"),
    ("akst", "America/Anchorage", "Alaska Time"),
    ("akdt", "America/Anchorage", "Alaska Time"),
    ("hst", "Pacific/Honolulu", "Hawaii Time"),
    ("ast", "America/Halifax", "Atlantic Time"),
    ("nst", "America/St_Johns", "Newfoundland Time"),
    ("bst", "Europe/London", "British Time"),
    ("wet", "Europe/Lisbon", "Western European Time"),
    ("cet", "Europe/Paris", "Central European Time"),
    ("cest", "Europe/Paris", "Central European Time"),
    ("eet", "Europe/Athens", "Eastern European Time"),
    ("eest", "Europe/Athens", "Eastern European Time"),
    ("msk", "Europe/Moscow", "Moscow Time"),
    ("gst", "Asia/Dubai", "Gulf Time"),
    ("ist", "Asia/Kolkata", "India Time"),
    ("ict", "Asia/Bangkok", "Indochina Time"),
    ("wib", "Asia/Jakarta", "Western Indonesia Time"),
    ("sgt", "Asia/Singapore", "Singapore Time"),
    ("hkt", "Asia/Hong_Kong", "Hong Kong Time"),
    ("pht", "Asia/Manila", "Philippine Time"),
    ("jst", "Asia/Tokyo", "Japan Time"),
    ("kst", "Asia/Seoul", "Korea Time"),
    ("awst", "Australia/Perth", "Australian Western Time"),
    ("acst", "Australia/Adelaide", "Australian Central Time"),
    ("acdt", "Australia/Adelaide", "Australian Central Time"),
    ("aest", "Australia/Sydney", "Australian Eastern Time"),
    ("aedt", "Australia/Sydney", "Australian Eastern Time"),
    ("nzst", "Pacific/Auckland", "New Zealand Time"),
    ("nzdt", "Pacific/Auckland", "New Zealand Time"),
    ("wat", "Africa/Lagos", "West Africa Time"),
    ("cat", "Africa/Maputo", "Central Africa Time"),
    ("eat", "Africa/Nairobi", "East Africa Time"),
    ("sast", "Africa/Johannesburg", "South Africa Time"),
    ("brt", "America/Sao_Paulo", "Brasília Time"),
    ("art", "America/Argentina/Buenos_Aires", "Argentina Time"),
];

/// Zones renamed since older macOS's (theirs, if this one doesn't know the
/// new names).
const RENAMED: &[(&str, &str)] = &[
    ("Europe/Kyiv", "Europe/Kiev"),
    ("America/Nuuk", "America/Godthab"),
    ("Pacific/Kanton", "Pacific/Enderbury"),
    ("Asia/Yangon", "Asia/Rangoon"),
    ("America/Ciudad_Juarez", "America/Denver"),
];

/// Lowercase, without accents and punctuation, "st" as "saint":
/// "Reykjavík" "reykjavik", "St. John's" "saint johns".
fn key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let plain = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => "a",
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
            'í' | 'ì' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' | 'ő' => "o",
            'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
            'ý' | 'ÿ' => "y",
            'ç' | 'ć' | 'č' => "c",
            'ñ' | 'ń' | 'ň' => "n",
            'š' | 'ś' | 'ş' | 'ș' => "s",
            'ž' | 'ź' | 'ż' => "z",
            'ł' => "l",
            'ř' => "r",
            'ť' | 'ţ' | 'ț' => "t",
            'ď' | 'đ' | 'ð' => "d",
            'ğ' => "g",
            'þ' => "th",
            'æ' => "ae",
            'œ' => "oe",
            'ß' => "ss",
            '\'' | '’' => "",
            c if c.is_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => " ",
        };
        out.push_str(plain);
    }
    out.split_whitespace().map(|w| if w == "st" { "saint" } else { w }).collect::<Vec<_>>().join(" ")
}

fn places() -> &'static Places {
    static PLACES: OnceLock<Places> = OnceLock::new();
    PLACES.get_or_init(|| {
        let mut places = Places { cities: Vec::new(), countries: Vec::new() };
        for line in TABLE.lines().filter(|l| !l.starts_with('#')) {
            if let Some((code, name)) = line.strip_prefix('@').and_then(|l| l.split_once('\t')) {
                places.countries.push(Country { code, name, key: key(name) });
                continue;
            }
            let f: Vec<&'static str> = line.split('\t').collect();
            if let [name, ascii, country, state, _, zone, lat, lon, ref rest @ ..] = f[..] {
                let key = key(name);
                let other = Some(self::key(ascii)).filter(|k| !ascii.is_empty() && *k != key);
                let others = rest.first().map_or_else(Vec::new, |o| o.split(',').filter(|n| !n.is_empty()).collect());
                let at = (lat.parse().unwrap_or(0.), lon.parse().unwrap_or(0.));
                places.cities.push(City { name, key, other, others, country, state, state_key: self::key(state), zone, at });
            }
        }
        places
    })
}

/// The zone as this macOS calls it, if it knows it.
fn known(zone: &str) -> Option<String> {
    if crate::macos::zone_offset(zone, now()).is_some() {
        return Some(zone.to_string());
    }
    RENAMED.iter().find(|(new, _)| *new == zone).map(|(_, old)| old.to_string()).filter(|old| crate::macos::zone_offset(old, now()).is_some())
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A country's code ("JP"), by its name ("japan").
pub fn country_code(name: &str) -> Option<&'static str> {
    let k = key(name);
    places().countries.iter().find(|c| c.key == k).map(|c| c.code)
}

/// A country's name ("Japan"), by its code.
pub fn country_named_by_code(code: &str) -> Option<&'static str> {
    Some(country_name(code)).filter(|n| !n.is_empty())
}

fn country_name(code: &str) -> &'static str {
    places().countries.iter().find(|c| c.code == code).map_or("", |c| c.name)
}

/// "Portland, Oregon" (in the US, by state), "Paris, France".
fn city_label(city: &City) -> String {
    match city.country {
        "US" if city.state == "District of Columbia" => format!("{}, D.C.", city.name),
        "US" if !city.state.is_empty() && !city.name.starts_with(city.state) => format!("{}, {}", city.name, city.state),
        "US" => city.name.to_string(),
        code => match country_name(code) {
            "" => city.name.to_string(),
            country if country == city.name => country.to_string(),
            country => format!("{}, {country}", city.name),
        },
    }
}

/// Each time of `cities` once (zones of the same time now are one), by its
/// biggest city (they're the biggest first), at most `SHOWN`.
fn by_zone<'a>(cities: impl Iterator<Item = &'a City>) -> Vec<&'a City> {
    let at = now();
    let mut offsets: std::collections::HashMap<&str, Option<i32>> = std::collections::HashMap::new();
    let mut out: Vec<(&City, i32)> = Vec::new();
    for city in cities {
        let offset = *offsets.entry(city.zone).or_insert_with(|| known(city.zone).and_then(|z| crate::macos::zone_offset(&z, at)));
        if let Some(offset) = offset
            && !out.iter().any(|(_, o)| *o == offset)
        {
            out.push((city, offset));
            if out.len() == SHOWN {
                break;
            }
        }
    }
    out.into_iter().map(|(c, _)| c).collect()
}

/// "utc+5:30", "gmt-3": that offset, in seconds.
fn offset(q: &str) -> Option<(i32, String)> {
    let rest = q.strip_prefix("utc").or_else(|| q.strip_prefix("gmt"))?.trim();
    let (sign, rest) = match rest.chars().next()? {
        '+' => (1, &rest[1..]),
        '-' => (-1, &rest[1..]),
        _ => return None,
    };
    let rest = rest.trim();
    let (h, m) = match rest.split_once(':') {
        Some((h, m)) => (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?),
        None if rest.len() == 4 => (rest[..2].parse().ok()?, rest[2..].parse().ok()?),
        None => (rest.parse().ok()?, 0),
    };
    if h > 14 || m > 59 {
        return None;
    }
    let label = if m == 0 { format!("UTC{}{h}", if sign > 0 { '+' } else { '-' }) } else { format!("UTC{}{h}:{m:02}", if sign > 0 { '+' } else { '-' }) };
    Some((sign * (h * 3600 + m * 60), label))
}

/// The places `name` is: here, a zone's letters or offset, cities by that
/// name (and, after one, a state or country: "portland oregon"), a country
/// or a state (each of its zones); or, `prefix` (while it's typed), cities
/// it begins.
fn find(name: &str, prefix: bool) -> Vec<Place> {
    // (An offset's sign, before the key drops it.)
    if let Some((secs, label)) = offset(&name.replace(' ', "")) {
        return vec![Place { label, zone: format!("offset:{secs}"), at: None }];
    }
    let q = key(name);
    if q.is_empty() {
        return Vec::new();
    }
    if matches!(q.as_str(), "here" | "local" | "my time" | "local time") {
        return vec![Place { label: "Here".into(), zone: crate::macos::local_zone(), at: None }];
    }
    if let Some((_, zone, label)) = ABBREVIATIONS.iter().find(|(a, ..)| *a == q) {
        return vec![Place { label: label.to_string(), zone: zone.to_string(), at: None }];
    }
    let places = places();
    // (Another name for a city: only that city, not a state of its name.)
    let alias = ALIASES.iter().find(|(a, _)| *a == q).map(|(_, to)| to.to_string());
    let aliased = alias.is_some();
    let q = alias.unwrap_or(q);
    let named = |c: &City, k: &str| c.key == k || c.other.as_deref() == Some(k) || c.others.contains(&k);
    let mut cities: Vec<&City> = places.cities.iter().filter(|c| named(c, &q)).collect();
    // "portland oregon", "london canada", "paris fr".
    if cities.is_empty() {
        let words: Vec<&str> = q.split(' ').collect();
        for at in (1..words.len()).rev() {
            let (city, of) = (words[..at].join(" "), words[at..].join(" "));
            let code = COUNTRIES.iter().find(|(n, _)| *n == of).map(|(_, c)| *c);
            cities = places
                .cities
                .iter()
                .filter(|c| named(c, &city) && (c.state_key == of || code == Some(c.country) || key(country_name(c.country)) == of || c.country.eq_ignore_ascii_case(&of)))
                .collect();
            if !cities.is_empty() {
                break;
            }
        }
    }
    let mut found: Vec<Place> = by_zone(cities.into_iter()).into_iter().map(|c| Place { label: city_label(c), zone: c.zone.to_string(), at: Some(c.at) }).collect();
    // A country, or a state: its zones, each by its biggest city.
    let code = (!aliased).then(|| COUNTRIES
        .iter()
        .find(|(n, _)| *n == q)
        .map(|(_, c)| *c)
        .or_else(|| places.countries.iter().find(|c| c.key == q).map(|c| c.code))
        .or_else(|| (found.is_empty() && q.len() == 2).then(|| places.countries.iter().find(|c| c.code.eq_ignore_ascii_case(&q)).map(|c| c.code)).flatten())).flatten();
    // (East to west, as they're read on a map: New York, then Los Angeles.)
    let east_first = |mut zones: Vec<&'static City>| {
        let at = now();
        zones.sort_by_key(|c| std::cmp::Reverse(known(c.zone).and_then(|z| crate::macos::zone_offset(&z, at)).unwrap_or(0)));
        zones
    };
    if let Some(code) = code {
        let zones = east_first(by_zone(places.cities.iter().filter(|c| c.country == code)));
        let one = zones.len() == 1;
        found.extend(zones.into_iter().map(|c| Place { label: if one { country_name(code).to_string() } else { city_label(c) }, zone: c.zone.to_string(), at: Some(c.at) }));
    }
    let states = east_first(by_zone(places.cities.iter().filter(|c| !aliased && c.state_key == q && !c.state.is_empty())));
    let one = states.len() == 1;
    found.extend(states.into_iter().map(|c| Place { label: if one && c.country == "US" { c.state.to_string() } else { city_label(c) }, zone: c.zone.to_string(), at: Some(c.at) }));
    if found.is_empty() && prefix && q.len() >= 3 {
        let begun = places.cities.iter().filter(|c| c.key.starts_with(&q) || c.other.as_deref().is_some_and(|o| o.starts_with(&q)) || c.others.iter().any(|o| o.starts_with(q.as_str())));
        found.extend(by_zone(begun).into_iter().take(3).map(|c| Place { label: city_label(c), zone: c.zone.to_string(), at: Some(c.at) }));
    }
    let mut seen = Vec::new();
    found.retain(|p| {
        let new = !seen.contains(&(p.label.clone(), p.zone.clone()));
        seen.push((p.label.clone(), p.zone.clone()));
        new
    });
    // (Zones this macOS knows, as it knows them.)
    found
        .into_iter()
        .filter_map(|p| if p.zone.starts_with("offset:") { Some(p) } else { known(&p.zone).map(|zone| Place { zone, ..p }) })
        .take(SHOWN)
        .collect()
}

/// The place `partial` is the start of ("tok": "Tokyo", "jap": "Japan",
/// "calif": "California"): the biggest city's, or its country's or state's,
/// as written (Quick Open shows the rest after what's typed).
pub fn complete(partial: &str) -> Option<&'static str> {
    let q = partial.trim_start().to_lowercase();
    if q.chars().count() < 2 {
        return None;
    }
    let starts = |name: &str| name.to_lowercase().starts_with(&q);
    // (The biggest first: a city, or the country or state of one.)
    places().cities.iter().find_map(|c| {
        if starts(c.name) {
            Some(c.name)
        } else if starts(country_name(c.country)) {
            Some(country_name(c.country))
        } else if starts(c.state) {
            Some(c.state)
        } else {
            None
        }
    })
}

/// The rest of `whole` after `typed` (any case), if it goes on.
fn rest_of(whole: &str, typed: &str) -> Option<String> {
    let rest: String = whole.chars().skip(typed.chars().count()).collect();
    (whole.to_lowercase().starts_with(&typed.to_lowercase()) && !rest.is_empty()).then_some(rest)
}

/// What can follow, for Quick Open's gray hint: the rest of a place being
/// typed ("time in reyk" → "javík", "3pm lon" → "don"), or somewhere to
/// see a time in ("3pm" → " in london", "3pm london" → " in new york").
pub fn hint(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let space = if lower.ends_with(' ') { "" } else { " " };
    for prefix in ["time in ", "what time is it in ", "the time in ", "time at "] {
        if let Some(partial) = lower.strip_prefix(prefix) {
            return rest_of(complete(partial)?, partial);
        }
    }
    let (_, after) = clock(&lower)?;
    // Somewhere other than here (and than `not`), to see it in.
    let elsewhere = |not: &str| {
        let here = crate::macos::zone_offset(&crate::macos::local_zone(), now());
        [("london", "Europe/London"), ("new york", "America/New_York"), ("tokyo", "Asia/Tokyo")]
            .into_iter()
            .find(|(name, zone)| *name != not && crate::macos::zone_offset(zone, now()) != here)
            .map(|(name, _)| name)
    };
    if after.trim().is_empty() {
        return Some(format!("{space}in {}", elsewhere("")?));
    }
    // After "in" or "to": the place's rest.
    let connector = [" in ", " to "].iter().filter_map(|c| after.rfind(c).map(|at| at + c.len())).max();
    let partial = match (after.strip_prefix("in ").or_else(|| after.strip_prefix("to ")), connector) {
        (_, Some(at)) => &after[at..],
        (Some(partial), None) => partial,
        (None, None) => {
            // A place, whole: somewhere to see its time in.
            if !find(after.trim(), false).is_empty() {
                return Some(format!("{space}in {}", elsewhere(after.trim())?));
            }
            after
        }
    };
    // ("3pm in ": somewhere to type.)
    if partial.trim().is_empty() {
        return lower.ends_with(' ').then(|| elsewhere("").map(String::from)).flatten();
    }
    rest_of(complete(partial)?, partial)
}

/// Where a place is, by its name (a city, or a country's or state's biggest
/// one): what it's called, and its latitude and longitude.
pub fn locate(name: &str) -> Option<(String, (f32, f32))> {
    find(name, true).into_iter().find_map(|p| p.at.map(|at| (p.label, at)))
}

/// Where a place is, and its time zone (as macOS calls it), by its name.
pub fn locate_zoned(name: &str) -> Option<(String, (f32, f32), String)> {
    find(name, true).into_iter().find_map(|p| Some((p.label, p.at?, known(&p.zone).unwrap_or(p.zone))))
}

/// The city of this Mac's time zone (Seoul's, for Asia/Seoul): the biggest
/// there.
pub fn here() -> Option<(String, (f32, f32))> {
    zone_city(&crate::macos::local_zone())
}

/// A time zone's city. (None for UTC's, a Mac set to UTC's: GitHub's.)
fn zone_city(zone: &str) -> Option<(String, (f32, f32))> {
    let city = zone.rsplit('/').next().unwrap_or("").replace('_', " ");
    // (By the zone's city's name, else the zone's biggest city.)
    locate(&city).or_else(|| places().cities.iter().find(|c| known(c.zone).as_deref() == Some(zone)).map(|c| (city_label(c), c.at)))
}

/// A time of day at the start: "3pm", "3:30 pm", "15:00", "9.30am",
/// "noon". (A bare number isn't one: it's a sum's.) Hours and minutes, and
/// what's after.
fn clock(q: &str) -> Option<((u32, u32), &str)> {
    for (word, at) in [("noon", (12, 0)), ("midday", (12, 0)), ("midnight", (0, 0))] {
        if let Some(rest) = q.strip_prefix(word).filter(|r| r.is_empty() || r.starts_with(' ')) {
            return Some((at, rest.trim_start()));
        }
    }
    let digits = q.find(|c: char| !c.is_ascii_digit()).unwrap_or(q.len());
    let mut hour: u32 = q.get(..digits).filter(|d| (1..=2).contains(&d.len()))?.parse().ok()?;
    let mut rest = &q[digits..];
    let mut minute = 0;
    let mut colon = false;
    if let Some(m) = rest.strip_prefix(':').or_else(|| rest.strip_prefix('.')) {
        let m_digits = m.find(|c: char| !c.is_ascii_digit()).unwrap_or(m.len());
        if m_digits != 2 {
            return None;
        }
        minute = m[..2].parse().ok()?;
        rest = &m[2..];
        colon = true;
    }
    let trimmed = rest.trim_start();
    let half = ["am", "a.m.", "a", "pm", "p.m.", "p"].iter().find(|h| trimmed.strip_prefix(**h).is_some_and(|r| r.is_empty() || r.starts_with(' ')));
    match half {
        Some(h) => {
            if !(1..=12).contains(&hour) {
                return None;
            }
            let pm = h.starts_with('p');
            hour = match (hour, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, true) => h + 12,
                (h, false) => h,
            };
            rest = trimmed[h.len()..].trim_start();
        }
        None if colon => rest = trimmed,
        None => return None,
    }
    (hour < 24 && minute < 60).then_some(((hour, minute), rest))
}

/// What's asked the time of: "time in tokyo", "tokyo time", "what time is
/// it in rome". Whether it's asked outright ("time in ...", so a city's
/// first letters do while it's typed), and the place.
fn asked<'a>(q: &'a str) -> Option<(bool, &'a str)> {
    const BEFORE: &[&str] = &[
        "what time is it in ",
        "what time is it at ",
        "what time is it ",
        "what is the time in ",
        "what's the time in ",
        "whats the time in ",
        "what time in ",
        "what day is it in ",
        "current time in ",
        "local time in ",
        "the time in ",
        "time now in ",
        "time in ",
        "time at ",
        "time for ",
        "clock in ",
        "date in ",
        "now in ",
    ];
    // (And in Japanese, Chinese and Korean: "東京の時間", "东京时间", "도쿄 시간".)
    const AFTER: &[&str] = &[
        " time now",
        " local time",
        " time",
        " now",
        " clock",
        "の現在時刻",
        "の時刻",
        "の時間",
        "の時差",
        " 時間",
        "時間",
        "的时间",
        " 时间",
        "时间",
        "의 시간",
        " 시간",
        "시간",
    ];
    // ("time in", "what time is it in": half typed, not India's "in".)
    let real = |p: &'a str| Some(p.trim()).filter(|p| !matches!(*p, "in" | "at" | "for"));
    if let Some(place) = BEFORE.iter().find_map(|b| q.strip_prefix(b)) {
        return real(place).map(|p| (true, p));
    }
    if let Some(found) = AFTER.iter().find_map(|a| q.strip_suffix(a)).or_else(|| q.strip_prefix("time ")) {
        return real(found).map(|p| (false, p));
    }
    None
}

/// The local time somewhere: its time, then where, the day, and how far
/// ahead of here.
fn now_in(place: &Place, here: &str, at: i64) -> Time {
    let shown = crate::macos::zone_format(&place.zone, at, "jmm");
    let day = crate::macos::zone_format(&place.zone, at, "EEEEMMMMd");
    let ahead = crate::macos::zone_offset(&place.zone, at).unwrap_or(0) - crate::macos::zone_offset(here, at).unwrap_or(0);
    Time { about: format!("{} · {day} · {}", place.label, difference(ahead)), copied: shown.clone(), shown }
}

/// "9 hours ahead", "1 hour behind", "5½ hours ahead", "Same time as here".
fn difference(secs: i32) -> String {
    if secs == 0 {
        return "Same time as here".into();
    }
    let way = if secs > 0 { "ahead" } else { "behind" };
    let (h, m) = (secs.abs() / 3600, secs.abs() % 3600 / 60);
    let hours = match (h, m) {
        (0, m) => format!("{m} minutes"),
        (1, 0) => "1 hour".into(),
        (h, 0) => format!("{h} hours"),
        (h, 30) => format!("{h}½ hours"),
        (h, m) => format!("{h} h {m} min"),
    };
    format!("{hours} {way}")
}

/// When it's `clock` today at `from`: that moment.
fn moment(clock: (u32, u32), from: &Place, at: i64) -> i64 {
    let off = |t: i64| i64::from(crate::macos::zone_offset(&from.zone, t).unwrap_or(0));
    let today = (at + off(at)).div_euclid(86_400);
    let wall = today * 86_400 + i64::from(clock.0) * 3600 + i64::from(clock.1) * 60;
    // (Its offset then, not now: across a change of clocks.)
    let guess = wall - off(at);
    wall - off(guess)
}

/// The rows for what's typed, if it asks the time somewhere.
pub fn answer(text: &str) -> Option<Vec<Time>> {
    let lower = text.trim().to_lowercase();
    let lower = ["what is ", "what's ", "whats ", "convert "].iter().find_map(|p| lower.strip_prefix(p)).unwrap_or(&lower);
    let here = crate::macos::local_zone();
    let at = now();
    let local = Place { label: "here".into(), zone: here.clone(), at: None };
    // A time somewhere, somewhere else.
    if let Some((clock, rest)) = clock(lower) {
        let rest = rest.trim().trim_end_matches(" time").trim();
        let (from, to) = if let Some(to) = rest.strip_prefix("in ").or_else(|| rest.strip_prefix("to ")) {
            (vec![local.clone()], find(to, false))
        } else if let Some((from, to)) = rest.rsplit_once(" in ").or_else(|| rest.rsplit_once(" to ")) {
            (find(from.trim_end_matches(" time"), false), find(to, false))
        } else if rest.is_empty() {
            return None;
        } else {
            (find(rest, false), vec![local.clone()])
        };
        let from = from.into_iter().next()?;
        if to.is_empty() {
            return None;
        }
        let moment = moment(clock, &from, at);
        let then = crate::macos::zone_format(&from.zone, moment, "jmm");
        let rows = to
            .iter()
            .map(|place| {
                let shown = crate::macos::zone_format(&place.zone, moment, "jmm");
                let day = crate::macos::zone_format(&place.zone, moment, "EEEEMMMMd");
                let there = if place.zone == here && place.label == "here" { "Here".to_string() } else { place.label.clone() };
                let source = if from.label == "here" { format!("when it's {then} here") } else { format!("when it's {then} in {}", from.label) };
                Time { about: format!("{there} · {day} · {source}"), copied: shown.clone(), shown }
            })
            .collect();
        return Some(rows);
    }
    let (outright, place) = asked(lower)?;
    let found = find(place, outright);
    if found.is_empty() {
        return None;
    }
    Some(found.iter().map(|p| now_in(p, &here, at)).collect())
}

#[cfg(test)]
mod tests {
    use super::{answer, asked, clock, find, key};

    #[test]
    fn keys_drop_accents_and_saints() {
        assert_eq!(key("Reykjavík"), "reykjavik");
        assert_eq!(key("St. John's"), "saint johns");
        assert_eq!(key("  São   Paulo "), "sao paulo");
        assert_eq!(key("Tórshavn"), "torshavn");
    }

    #[test]
    fn clocks() {
        assert_eq!(clock("3pm tokyo"), Some(((15, 0), "tokyo")));
        assert_eq!(clock("3:30 pm in london"), Some(((15, 30), "in london")));
        assert_eq!(clock("12am"), Some(((0, 0), "")));
        assert_eq!(clock("15:00 rome"), Some(((15, 0), "rome")));
        assert_eq!(clock("9.30am"), Some(((9, 30), "")));
        assert_eq!(clock("noon in sydney"), Some(((12, 0), "in sydney")));
        assert_eq!(clock("3 tokyo"), None, "a bare number is a sum's");
        assert_eq!(clock("13pm"), None);
        assert_eq!(clock("paris"), None);
    }

    #[test]
    fn what_is_asked() {
        assert_eq!(asked("time in tokyo"), Some((true, "tokyo")));
        assert_eq!(asked("time in"), None, "half typed");
        assert_eq!(asked("what time is it in"), None);
        assert_eq!(asked("what time is it in new york"), Some((true, "new york")));
        assert_eq!(asked("tokyo time"), Some((false, "tokyo")));
        assert_eq!(asked("time tokyo"), Some((false, "tokyo")));
        assert_eq!(asked("timer"), None);
        assert_eq!(asked("東京の時間"), Some((false, "東京")));
        assert_eq!(asked("도쿄 시간"), Some((false, "도쿄")));
        assert_eq!(asked("时间"), Some((false, "")), "nothing asked of: found nothing");
    }

    fn labels(name: &str) -> Vec<String> {
        find(name, false).into_iter().map(|p| p.label).collect()
    }

    #[test]
    fn zones_cities() {
        let city = |zone| super::zone_city(zone).map(|(label, _)| label);
        assert_eq!(city("Asia/Tokyo").as_deref(), Some("Tokyo, Japan"));
        assert_eq!(city("Atlantic/Reykjavik").as_deref(), Some("Reykjavík, Iceland"));
        assert!(city("America/Argentina/Buenos_Aires").is_some_and(|label| label.starts_with("Buenos Aires")));
        assert_eq!(city("UTC"), None);
    }

    #[test]
    fn places() {
        assert_eq!(labels("tokyo"), ["Tokyo, Japan"]);
        assert_eq!(labels("reykjavik"), ["Reykjavík, Iceland"]);
        assert_eq!(labels("nyc"), ["New York City"]);
        assert_eq!(labels("dc"), ["Washington, D.C."]);
        assert_eq!(labels("portland oregon"), ["Portland, Oregon"]);
        assert_eq!(labels("japan"), ["Japan"]);
        assert!(labels("usa").len() >= 4, "{:?}", labels("usa"));
        assert!(labels("australia").iter().any(|l| l.starts_with("Perth")), "{:?}", labels("australia"));
        assert_eq!(labels("pst"), ["Pacific Time"]);
        assert_eq!(labels("utc+5:30"), ["UTC+5:30"]);
        assert_eq!(labels("california"), ["California"]);
        assert!(labels("texas").len() == 2, "{:?}", labels("texas"));
        assert!(labels("london").iter().any(|l| l == "London, United Kingdom"), "{:?}", labels("london"));
        assert!(labels("xyzzy").is_empty());
        assert_eq!(labels("東京"), ["Tokyo, Japan"]);
        assert_eq!(labels("서울"), ["Seoul, South Korea"]);
        assert_eq!(labels("ロンドン")[0], "London, United Kingdom");
        assert_eq!(labels("日本"), ["Japan"]);
        assert!(labels("미국").len() >= 4);
        // While typing, the first letters do (asked outright).
        assert!(find("tok", true).iter().any(|p| p.label == "Tokyo, Japan"));
        assert!(find("tok", false).is_empty());
    }

    #[test]
    fn completes() {
        assert_eq!(super::complete("tok"), Some("Tokyo"));
        assert_eq!(super::complete("jap"), Some("Japan"));
        assert_eq!(super::complete("reyk"), Some("Reykjavík"));
        assert_eq!(super::complete("t"), None, "a letter's too little");
    }

    #[test]
    fn answers() {
        let rows = answer("time in tokyo").expect("tokyo");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].about.starts_with("Tokyo, Japan · "), "{}", rows[0].about);
        let rows = answer("3pm tokyo in london").expect("a time");
        assert!(rows[0].about.contains("when it's") && rows[0].about.starts_with("London"), "{}", rows[0].about);
        assert!(answer("time machine").is_none());
        assert!(answer("東京の時間").is_some_and(|rows| rows[0].about.starts_with("Tokyo")));
        assert!(answer("도쿄 시간").is_some());
        assert!(answer("3 + 4").is_none());
        assert!(answer("3pm").is_none());
    }
}

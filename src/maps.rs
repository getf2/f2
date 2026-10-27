//! Maps, for Quick Open: directions ("directions to incheon airport",
//! "directions from gangnam to hongdae", "walk to the park", "how do i get
//! to lotte world") and places ("coffee near me", "nearby pharmacy", "map
//! of busan"), opened in Apple Maps, or Google Maps; in Korea, Naver Map
//! and Kakao Map too (Korea's maps know it best).

/// How you'd go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Going {
    Driving,
    Walking,
    Transit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Map {
    Directions { to: String, from: Option<String>, going: Option<Going> },
    /// A place or a kind of place, near here (`near`) or anywhere.
    Find { what: String, near: bool },
}

pub fn asked(q: &str) -> Option<Map> {
    let trimmed = q.trim().trim_end_matches('?');
    let lower = trimmed.to_lowercase();
    let after = |prefix: &str| lower.strip_prefix(prefix).map(|_| trimmed[prefix.len()..].trim().to_string()).filter(|r| !r.is_empty());
    const LEADS: &[(&str, Option<Going>)] = &[
        ("walking directions to ", Some(Going::Walking)),
        ("driving directions to ", Some(Going::Driving)),
        ("transit directions to ", Some(Going::Transit)),
        ("directions to ", None),
        ("directions from ", None),
        ("how do i get to ", None),
        ("how to get to ", None),
        ("how do i get from ", None),
        ("navigate to ", Some(Going::Driving)),
        ("route to ", None),
        ("take me to ", None),
        ("drive to ", Some(Going::Driving)),
        ("walk to ", Some(Going::Walking)),
        ("bus to ", Some(Going::Transit)),
        ("train to ", Some(Going::Transit)),
        ("subway to ", Some(Going::Transit)),
        ("transit to ", Some(Going::Transit)),
    ];
    for (lead, going) in LEADS {
        if let Some(rest) = after(lead) {
            // "from A to B" ("directions from gangnam to hongdae").
            let (from, to) = if lead.ends_with("from ") {
                let at = rest.to_lowercase().rfind(" to ")?;
                (Some(rest[..at].trim().to_string()), rest[at + 4..].trim().to_string())
            } else {
                match rest.to_lowercase().rfind(" from ") {
                    Some(at) => (Some(rest[at + 6..].trim().to_string()), rest[..at].trim().to_string()),
                    None => (None, rest),
                }
            };
            return (!to.is_empty() && from.as_deref() != Some("")).then_some(Map::Directions { to, from, going: *going });
        }
    }
    for suffix in [" near me", " nearby", " around me", " near here", " close by"] {
        if let Some(what) = lower.strip_suffix(suffix).map(|_| trimmed[..trimmed.len() - suffix.len()].trim()).filter(|w| !w.is_empty()) {
            return Some(Map::Find { what: what.to_string(), near: true });
        }
    }
    for lead in ["nearby ", "near me ", "closest ", "nearest "] {
        if let Some(what) = after(lead) {
            return Some(Map::Find { what, near: true });
        }
    }
    // (Not "map …" or "… map": "map network drive", "mind map".)
    for lead in ["map of ", "maps ", "show on map ", "find on map "] {
        if let Some(what) = after(lead) {
            return Some(Map::Find { what, near: false });
        }
    }
    for suffix in [" on the map", " on a map", " on map"] {
        if let Some(what) = lower.strip_suffix(suffix).map(|_| trimmed[..trimmed.len() - suffix.len()].trim()).filter(|w| !w.is_empty() && w.split_whitespace().count() <= 4) {
            return Some(Map::Find { what: what.to_string(), near: false });
        }
    }
    None
}

/// Where to open it: the map's name, the link, and its symbol.
pub fn links(map: &Map, korea: bool) -> Vec<(&'static str, String)> {
    let e = crate::dev::url_encode;
    match map {
        Map::Directions { to, from, going } => {
            let apple_mode = match going {
                Some(Going::Walking) => "&dirflg=w",
                Some(Going::Transit) => "&dirflg=r",
                Some(Going::Driving) => "&dirflg=d",
                None => "",
            };
            let google_mode = match going {
                Some(Going::Walking) => "&travelmode=walking",
                Some(Going::Transit) => "&travelmode=transit",
                Some(Going::Driving) => "&travelmode=driving",
                None => "",
            };
            let from_apple = from.as_ref().map_or(String::new(), |f| format!("&saddr={}", e(f)));
            let from_google = from.as_ref().map_or(String::new(), |f| format!("&origin={}", e(f)));
            let mut links = vec![
                ("Apple Maps", format!("maps://?daddr={}{from_apple}{apple_mode}", e(to))),
                ("Google Maps", format!("https://www.google.com/maps/dir/?api=1&destination={}{from_google}{google_mode}", e(to))),
            ];
            if korea {
                let from_kakao = from.as_ref().map_or(String::new(), |f| format!("&sName={}", e(f)));
                links.insert(0, ("Kakao Map", format!("https://map.kakao.com/?eName={}{from_kakao}", e(to))));
                links.insert(1, ("Naver Map", format!("https://map.naver.com/p/search/{}", e(to))));
            }
            links
        }
        Map::Find { what, .. } => {
            let mut links = vec![("Apple Maps", format!("maps://?q={}", e(what))), ("Google Maps", format!("https://www.google.com/maps/search/?api=1&query={}", e(what)))];
            if korea {
                links.insert(0, ("Naver Map", format!("https://map.naver.com/p/search/{}", e(what))));
                links.insert(1, ("Kakao Map", format!("https://map.kakao.com/?q={}", e(what))));
            }
            links
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Going, Map, asked};

    #[test]
    fn asking() {
        let to = |to: &str, from: Option<&str>, going: Option<Going>| Some(Map::Directions { to: to.into(), from: from.map(Into::into), going });
        assert_eq!(asked("directions to Incheon Airport"), to("Incheon Airport", None, None));
        assert_eq!(asked("directions from gangnam to hongdae"), to("hongdae", Some("gangnam"), None));
        assert_eq!(asked("walk to the park"), to("the park", None, Some(Going::Walking)));
        assert_eq!(asked("how do i get to lotte world?"), to("lotte world", None, None));
        assert_eq!(asked("directions to busan from seoul"), to("busan", Some("seoul"), None));
        assert_eq!(asked("coffee near me"), Some(Map::Find { what: "coffee".into(), near: true }));
        assert_eq!(asked("nearby pharmacy"), Some(Map::Find { what: "pharmacy".into(), near: true }));
        assert_eq!(asked("map of Busan"), Some(Map::Find { what: "Busan".into(), near: false }));
        assert_eq!(asked("eiffel tower on the map"), Some(Map::Find { what: "eiffel tower".into(), near: false }));
        assert_eq!(asked("directions to"), None);
        assert_eq!(asked("maps"), None);
        assert_eq!(asked("roadmap"), None);
        assert_eq!(asked("mind map"), None);
        assert_eq!(asked("map network drive"), None);
    }
}

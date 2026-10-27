//! Package tracking from Quick Open: a tracking number pasted (or "track
//! …") opens the carrier's page. Told apart by their shapes: UPS's "1Z…",
//! USPS's 20 to 22 digits from 9, an international one's "EC123456789JP"
//! (its country's post), FedEx's 12 or 15 digits, DHL's 10, Japan's
//! carriers' (Japan Post, Yamato, Sagawa), Amazon's "TBA…". A number that
//! could be several's: each, the likeliest where the Mac is first; and
//! 17TRACK, which knows most.

/// A carrier's name and its tracking page for `number`.
pub struct Carrier {
    pub name: &'static str,
    pub link: String,
}

/// The number in `q`, if it's one: "track 1Z…" (spaces in it dropped), or
/// one by itself.
fn number_in(q: &str) -> Option<String> {
    let trimmed = q.trim();
    let lower = trimmed.to_lowercase();
    let rest = ["track package ", "track parcel ", "track order ", "tracking ", "track "].iter().find_map(|p| lower.strip_prefix(p).map(|_| &trimmed[p.len()..]));
    let number: String = match rest {
        Some(rest) => rest.chars().filter(|c| !c.is_whitespace() && *c != '-').collect(),
        None if !trimmed.contains(char::is_whitespace) => trimmed.to_string(),
        None => return None,
    };
    let number = number.to_uppercase();
    (number.len() >= 10 && number.len() <= 34 && number.chars().all(|c| c.is_ascii_alphanumeric()) && number.chars().any(|c| c.is_ascii_digit())).then_some(number)
}

/// Where `number` can be tracked, if it's a tracking number's shape. `here`:
/// the Mac's region's currency ("JPY"), to put its carriers first.
pub fn carriers(q: &str, here: &str) -> Vec<Carrier> {
    let Some(n) = number_in(q) else { return Vec::new() };
    let digits = n.chars().all(|c| c.is_ascii_digit());
    let carrier = |name, link: String| Carrier { name, link };
    let ups = || carrier("UPS", format!("https://www.ups.com/track?tracknum={n}"));
    let usps = || carrier("USPS", format!("https://tools.usps.com/go/TrackConfirmAction?tLabels={n}"));
    let fedex = || carrier("FedEx", format!("https://www.fedex.com/fedextrack/?trknbr={n}"));
    let dhl = || carrier("DHL", format!("https://www.dhl.com/global-en/home/tracking/tracking-express.html?submit=1&tracking-id={n}"));
    let japan_post = || carrier("Japan Post", format!("https://trackings.post.japanpost.jp/services/srv/search/direct?reqCodeNo1={n}&locale=en"));
    let yamato = || carrier("Yamato Transport", format!("https://toi.kuronekoyamato.co.jp/cgi-bin/tneko?number01={n}"));
    let sagawa = || carrier("Sagawa Express", format!("https://k2k.sagawa-exp.co.jp/p/web/okurijosearch.do?okurijoNo={n}"));
    let royal_mail = || carrier("Royal Mail", format!("https://www.royalmail.com/track-your-item#/tracking-results/{n}"));
    let any = || carrier("17TRACK", format!("https://t.17track.net/en#nums={n}"));
    let japan = here == "JPY";
    let mut found: Vec<Carrier> = Vec::new();
    let bytes = n.as_bytes();
    if n.starts_with("1Z") && n.len() == 18 {
        found.push(ups());
    } else if n.starts_with("TBA") && n.len() >= 12 {
        found.push(carrier("Amazon", format!("https://track.amazon.com/tracking/{n}")));
    } else if n.len() == 13 && bytes[..2].iter().all(u8::is_ascii_alphabetic) && bytes[2..11].iter().all(u8::is_ascii_digit) && bytes[11..].iter().all(u8::is_ascii_alphabetic) {
        // An international one's: its country's post (the last two letters).
        match &n[11..] {
            "US" => found.push(usps()),
            "JP" => found.push(japan_post()),
            "GB" => found.push(royal_mail()),
            _ => {}
        }
    } else if digits {
        match n.len() {
            20..=22 if n.starts_with('9') => found.extend([usps(), fedex()]),
            20 | 22 => found.push(fedex()),
            15 => found.push(fedex()),
            12 if japan => found.extend([yamato(), japan_post(), sagawa(), fedex()]),
            12 => found.extend([fedex(), yamato(), japan_post()]),
            11 | 13 if japan => found.extend([japan_post(), sagawa()]),
            10 if japan => found.extend([sagawa(), dhl()]),
            10 => found.push(dhl()),
            _ => return Vec::new(),
        }
    } else {
        return Vec::new();
    }
    found.push(any());
    found
}

#[cfg(test)]
mod tests {
    use super::carriers;

    fn names(q: &str, here: &str) -> Vec<&'static str> {
        carriers(q, here).into_iter().map(|c| c.name).collect()
    }

    #[test]
    fn shapes() {
        assert_eq!(names("1Z999AA10123456784", "USD"), ["UPS", "17TRACK"]);
        assert_eq!(names("track 1Z 999 AA1 01 2345 6784", "USD"), ["UPS", "17TRACK"]);
        assert_eq!(names("9400111899223397623910", "USD"), ["USPS", "FedEx", "17TRACK"]);
        assert_eq!(names("EC123456789JP", "USD"), ["Japan Post", "17TRACK"]);
        assert_eq!(names("123456789012", "JPY")[0], "Yamato Transport");
        assert_eq!(names("123456789012", "USD")[0], "FedEx");
        assert_eq!(names("TBA123456789000", "USD"), ["Amazon", "17TRACK"]);
        assert!(names("hello world", "USD").is_empty());
        assert!(names("12345", "USD").is_empty());
        assert!(names("photos2024", "USD").is_empty());
    }
}

//! Flight status, for Quick Open: a flight number ("KE 17", "UA837",
//! "flight oz 102") opens its page on FlightAware, Flightradar24 or Google
//! (today's times, gate and where it is), as tracking numbers open their
//! carrier's. Told by its airline's code: by itself, only an airline's
//! known here (so "am 7" isn't one); after "flight", any.

/// Airlines by their IATA codes.
const AIRLINES: &[(&str, &str)] = &[
    ("AA", "American Airlines"),
    ("AC", "Air Canada"),
    ("AF", "Air France"),
    ("AI", "Air India"),
    ("AK", "AirAsia"),
    ("AS", "Alaska Airlines"),
    ("AV", "Avianca"),
    ("AY", "Finnair"),
    ("AZ", "ITA Airways"),
    ("B6", "JetBlue"),
    ("BA", "British Airways"),
    ("BR", "EVA Air"),
    ("BX", "Air Busan"),
    ("CA", "Air China"),
    ("CI", "China Airlines"),
    ("CM", "Copa Airlines"),
    ("CX", "Cathay Pacific"),
    ("CZ", "China Southern"),
    ("DL", "Delta"),
    ("DY", "Norwegian"),
    ("EI", "Aer Lingus"),
    ("EK", "Emirates"),
    ("ET", "Ethiopian Airlines"),
    ("EY", "Etihad"),
    ("F9", "Frontier"),
    ("FI", "Icelandair"),
    ("FR", "Ryanair"),
    ("GA", "Garuda Indonesia"),
    ("GK", "Jetstar Japan"),
    ("HA", "Hawaiian Airlines"),
    ("HX", "Hong Kong Airlines"),
    ("IB", "Iberia"),
    ("JL", "Japan Airlines"),
    ("JQ", "Jetstar"),
    ("KE", "Korean Air"),
    ("KL", "KLM"),
    ("LA", "LATAM"),
    ("LH", "Lufthansa"),
    ("LJ", "Jin Air"),
    ("LO", "LOT Polish Airlines"),
    ("LX", "Swiss"),
    ("MH", "Malaysia Airlines"),
    ("MM", "Peach"),
    ("MS", "EgyptAir"),
    ("MU", "China Eastern"),
    ("NH", "ANA"),
    ("NK", "Spirit"),
    ("NZ", "Air New Zealand"),
    ("OG", "PLAY"),
    ("OS", "Austrian"),
    ("OZ", "Asiana"),
    ("PR", "Philippine Airlines"),
    ("QF", "Qantas"),
    ("RS", "Air Seoul"),
    ("SA", "South African Airways"),
    ("SK", "SAS"),
    ("SN", "Brussels Airlines"),
    ("SQ", "Singapore Airlines"),
    ("TG", "Thai Airways"),
    ("TK", "Turkish Airlines"),
    ("TP", "TAP Air Portugal"),
    ("TW", "T'way Air"),
    ("U2", "easyJet"),
    ("UA", "United"),
    ("VA", "Virgin Australia"),
    ("VJ", "VietJet"),
    ("VN", "Vietnam Airlines"),
    ("VS", "Virgin Atlantic"),
    ("VY", "Vueling"),
    ("W6", "Wizz Air"),
    ("WN", "Southwest"),
    ("WS", "WestJet"),
    ("ZE", "Eastar Jet"),
    ("5J", "Cebu Pacific"),
    ("6E", "IndiGo"),
    ("7C", "Jeju Air"),
    // (After "flight" only: they're words, or Quick Open's own.)
    ("QR", "Qatar Airways"),
    ("AM", "Aeroméxico"),
];

/// Codes that are words too, or Quick Open's ("qr 8": a QR code), or
/// short for something else ("pr 123", "ci", "os 26", "dl").
const ONLY_AFTER_FLIGHT: &[&str] = &["QR", "AM", "AS", "LA", "HA", "OS", "PR", "CI", "CM", "MM", "DL"];

/// The flight asked about: its airline's code and the number ("KE", "17"),
/// and the airline's name, if it's known.
pub fn asked(q: &str) -> Option<(String, String, Option<&'static str>)> {
    let upper = q.trim().to_uppercase();
    let (rest, said_flight) = match upper.strip_prefix("FLIGHT ").or_else(|| upper.strip_prefix("FLIGHT STATUS ")) {
        Some(rest) => (rest.trim().to_string(), true),
        None => (upper.trim_end_matches(" FLIGHT").trim_end_matches(" FLIGHT STATUS").to_string(), false),
    };
    let compact: String = rest.split_whitespace().collect();
    // ("KE17", "KE017", "7C1101", "UA837A"; "KAL17" too, after "flight".)
    let digits_at = compact.char_indices().skip(2).find(|(_, c)| c.is_ascii_digit()).map(|(i, _)| i)?;
    let (code, number) = compact.split_at(digits_at);
    let number = number.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let fine_code = (code.len() == 2 && code.chars().all(|c| c.is_ascii_alphanumeric()) && code.chars().any(|c| c.is_ascii_alphabetic())) || (said_flight && code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()));
    if !fine_code || number.is_empty() || number.len() > 4 || !number.chars().all(|c| c.is_ascii_digit()) || rest.split_whitespace().count() > 2 {
        return None;
    }
    let airline = AIRLINES.iter().find(|(c, _)| *c == code).map(|(_, name)| *name);
    if !said_flight && (airline.is_none() || ONLY_AFTER_FLIGHT.contains(&code)) {
        return None;
    }
    // ("KE017": KE 17.)
    let number = number.trim_start_matches('0');
    (!number.is_empty()).then(|| (code.to_string(), number.to_string(), airline))
}

/// Where to see it: a site's name and the flight's page there.
pub fn pages(code: &str, number: &str) -> Vec<(&'static str, String)> {
    let flight = format!("{code}{number}");
    vec![
        ("FlightAware", format!("https://www.flightaware.com/live/flight/{flight}")),
        ("Flightradar24", format!("https://www.flightradar24.com/data/flights/{}", flight.to_lowercase())),
        ("Google", format!("https://www.google.com/search?q={flight}+flight+status")),
    ]
}

#[cfg(test)]
mod tests {
    use super::asked;

    #[test]
    fn asking() {
        let flight = |code: &str, number: &str, airline: Option<&'static str>| Some((code.to_string(), number.to_string(), airline));
        assert_eq!(asked("KE 17"), flight("KE", "17", Some("Korean Air")));
        assert_eq!(asked("ua837"), flight("UA", "837", Some("United")));
        assert_eq!(asked("KE017"), flight("KE", "17", Some("Korean Air")));
        assert_eq!(asked("7C 1101"), flight("7C", "1101", Some("Jeju Air")));
        assert_eq!(asked("flight oz 102"), flight("OZ", "102", Some("Asiana")));
        assert_eq!(asked("FI 615 flight"), flight("FI", "615", Some("Icelandair")));
        assert_eq!(asked("flight KAL17"), flight("KAL", "17", None));
        assert_eq!(asked("flight qr 8"), flight("QR", "8", Some("Qatar Airways")));
        assert_eq!(asked("qr 8"), None, "a QR code");
        assert_eq!(asked("am 7"), None);
        assert_eq!(asked("pr 123"), None, "a pull request");
        assert_eq!(asked("flight dl 1"), flight("DL", "1", Some("Delta")));
        assert_eq!(asked("zz 12"), None, "no such airline");
        assert_eq!(asked("ke 12345"), None);
        assert_eq!(asked("ke"), None);
        assert_eq!(asked("iphone 15"), None);
        assert_eq!(asked("mp3"), None);
    }
}

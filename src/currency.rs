//! Currency conversion for Quick Open's calculator: "100 usd in eur",
//! "€20 to krw", "usd to isk" (one of it), and "50 gbp" (into this Mac's
//! currency). At the European Central Bank's reference rates of the day
//! (some 30 currencies), from Frankfurter (https://frankfurter.dev: free,
//! no key), kept in the data folder and fetched again in the background
//! once they're 12 hours old. The very first conversion waits for them, on
//! the search's thread; without them (offline), it isn't answered.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const URL: &str = "https://api.frankfurter.dev/v1/latest";
/// How old rates may be before they're fetched again (the ECB's change once
/// a working day).
const STALE: u64 = 12 * 3600;
/// After a fetch fails, how long before another try.
const RETRY: u64 = 300;

/// What the ECB has rates for.
const CODES: &[&str] = &[
    "AUD", "BRL", "CAD", "CHF", "CNY", "CZK", "DKK", "EUR", "GBP", "HKD", "HUF", "IDR", "ILS", "INR", "ISK", "JPY", "KRW", "MXN", "MYR", "NOK",
    "NZD", "PHP", "PLN", "RON", "SEK", "SGD", "THB", "TRY", "USD", "ZAR",
];

/// Symbols and names, and the currency each is taken for. (Not "pounds":
/// that's a weight. Nor "kr": that's four currencies.)
const ALIASES: &[(&str, &str)] = &[
    ("$", "USD"),
    ("€", "EUR"),
    ("£", "GBP"),
    ("¥", "JPY"),
    ("₩", "KRW"),
    ("₹", "INR"),
    ("₺", "TRY"),
    ("₪", "ILS"),
    ("₱", "PHP"),
    ("฿", "THB"),
    ("dollar", "USD"),
    ("dollars", "USD"),
    ("euro", "EUR"),
    ("euros", "EUR"),
    ("yen", "JPY"),
    ("won", "KRW"),
    ("yuan", "CNY"),
    ("rupee", "INR"),
    ("rupees", "INR"),
    ("franc", "CHF"),
    ("francs", "CHF"),
];

/// Shown without cents.
pub(crate) const WHOLE: &[&str] = &["JPY", "KRW", "ISK", "HUF", "IDR"];

#[derive(Clone, Serialize, Deserialize)]
struct Rates {
    /// The ECB's day for them ("2026-10-19").
    date: String,
    /// When they were fetched, in seconds since 1970.
    fetched: u64,
    /// Per euro (the euro's own 1 too).
    rates: HashMap<String, f64>,
}

pub struct Answer {
    /// "92,345 KRW"
    pub shown: String,
    /// "92345"
    pub copied: String,
    /// "100 USD in KRW · ECB rate of 19 Oct"
    pub about: String,
}

static RATES: Mutex<Option<Rates>> = Mutex::new(None);
/// One fetch at a time: another asking waits for it.
static FETCH: Mutex<()> = Mutex::new(());
static REFRESHING: AtomicBool = AtomicBool::new(false);
/// When a fetch last failed.
static FAILED: AtomicU64 = AtomicU64::new(0);

/// The conversion `text` asks for, if it asks for one (and there are rates).
pub fn answer(text: &str) -> Option<Answer> {
    let (amount, from, to) = parse(&text.trim().to_lowercase())?;
    let rates = rates()?;
    answer_with(amount, from, to, &rates, crate::macos::local_currency().as_deref())
}

fn answer_with(amount: f64, from: &str, to: Option<&str>, rates: &Rates, local: Option<&str>) -> Option<Answer> {
    // (Just "50 gbp": into this Mac's currency, or else dollars, or euros.)
    let to = to.unwrap_or_else(|| match local.and_then(code) {
        Some(local) if local != from => local,
        _ if from != "USD" => "USD",
        _ => "EUR",
    });
    let value = amount / rates.rates.get(from)? * rates.rates.get(to)?;
    let decimals = if WHOLE.contains(&to) { 0 } else { 2 };
    let copied = format!("{value:.decimals$}");
    let day = chrono::NaiveDate::parse_from_str(&rates.date, "%Y-%m-%d").map_or(rates.date.clone(), |d| d.format("%-d %b").to_string());
    Some(Answer {
        shown: format!("{} {to}", crate::calc::grouped(&copied)),
        about: format!("{} {from} in {to} · ECB rate of {day}", crate::calc::grouped(&amount.to_string())),
        copied,
    })
}

/// "100 usd in eur", "$20 to krw", "usd to isk", "50 gbp": how much, of
/// what, into what (None: this Mac's).
fn parse(text: &str) -> Option<(f64, &'static str, Option<&'static str>)> {
    match text.split_once(" in ").or_else(|| text.split_once(" to ")) {
        Some((from, to)) => {
            let (amount, from) = amount_of(from, true)?;
            Some((amount, from, Some(code(to.trim())?)))
        }
        None => amount_of(text, false).map(|(amount, from)| (amount, from, None)),
    }
}

/// "100 usd", "100usd", "usd 100", "$100", "1,000 €" (and, if `bare`, "usd":
/// one).
fn amount_of(text: &str, bare: bool) -> Option<(f64, &'static str)> {
    let text = text.trim();
    let digit = |c: char| c.is_ascii_digit() || c == '.' || c == ',';
    let (number, word) = if text.starts_with(digit) {
        let at = text.find(|c: char| !digit(c)).unwrap_or(text.len());
        (&text[..at], text[at..].trim())
    } else {
        let at = text.find(digit).unwrap_or(text.len());
        (text[at..].trim(), text[..at].trim())
    };
    if !number.chars().all(digit) || (number.is_empty() && !bare) {
        return None;
    }
    let amount = if number.is_empty() { 1. } else { number.replace(',', "").parse().ok()? };
    Some((amount, code(word)?))
}

/// What can follow an amount typed so far, for Quick Open's gray hint:
/// "100 usd" → " in eur", "100 usd in k" → "rw".
pub fn hint(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let target = |from: &str| if from == "EUR" { "usd" } else { "eur" };
    if let Some((_, from)) = amount_of(lower.trim_end(), false) {
        let space = if lower.ends_with(' ') { "" } else { " " };
        return Some(format!("{space}in {}", target(from)));
    }
    // The amount and its currency, then what's typed after.
    let (head, tail) = lower.char_indices().filter(|(_, c)| *c == ' ').map(|(i, _)| (&lower[..i], &lower[i + 1..])).rev().find(|(head, _)| amount_of(head, false).is_some())?;
    let (_, from) = amount_of(head, false)?;
    for whole in [format!("in {}", target(from)), format!("to {}", target(from))] {
        if whole.len() > tail.len() && whole.starts_with(tail) {
            return Some(whole[tail.len()..].to_string());
        }
    }
    let partial = tail.strip_prefix("in ").or_else(|| tail.strip_prefix("to "))?;
    if partial.is_empty() || code(partial).is_some() {
        return None;
    }
    CODES.iter().map(|c| c.to_lowercase()).find(|c| c.starts_with(partial)).map(|c| c[partial.len()..].to_string())
}

/// `amount` of one currency in another, at the ECB's rates (None: one
/// they haven't, or no rates yet).
pub(crate) fn convert(amount: f64, from: &str, to: &str) -> Option<f64> {
    if from == to {
        return Some(amount);
    }
    let rates = rates()?;
    Some(amount / rates.rates.get(from)? * rates.rates.get(to)?)
}

/// The currency `word` names ("usd", "€", "euros"), if the ECB has it.
pub(crate) fn code(word: &str) -> Option<&'static str> {
    let upper = word.to_uppercase();
    CODES.iter().find(|c| **c == upper).copied().or_else(|| ALIASES.iter().find(|(name, _)| *name == word.to_lowercase()).map(|(_, c)| *c))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn file() -> PathBuf {
    crate::settings::data_dir().join("exchange-rates.json")
}

/// The rates: as kept (fetched again in the background if they're old), or
/// fetched now if there are none.
fn rates() -> Option<Rates> {
    {
        let mut held = RATES.lock().unwrap();
        if held.is_none() {
            *held = std::fs::read(file()).ok().and_then(|t| serde_json::from_slice(&t).ok());
        }
        if let Some(rates) = held.as_ref() {
            if now().saturating_sub(rates.fetched) > STALE {
                refresh_soon();
            }
            return Some(rates.clone());
        }
    }
    if now().saturating_sub(FAILED.load(Ordering::SeqCst)) < RETRY {
        return None;
    }
    fetch();
    RATES.lock().unwrap().clone()
}

fn refresh_soon() {
    if now().saturating_sub(FAILED.load(Ordering::SeqCst)) < RETRY || REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        fetch();
        REFRESHING.store(false, Ordering::SeqCst);
    });
}

/// Today's rates, from Frankfurter, kept (here and in the data folder).
fn fetch() {
    let _one = FETCH.lock().unwrap();
    // (Fetched by another while this one waited.)
    if RATES.lock().unwrap().as_ref().is_some_and(|r| now().saturating_sub(r.fetched) <= STALE) {
        return;
    }
    #[derive(Deserialize)]
    struct Latest {
        date: String,
        rates: HashMap<String, f64>,
    }
    let out = Command::new("/usr/bin/curl").args(["-fsS", "--max-time", "5", URL]).output();
    let latest = out.ok().filter(|o| o.status.success()).and_then(|o| serde_json::from_slice::<Latest>(&o.stdout).ok());
    let Some(Latest { date, mut rates }) = latest else {
        eprintln!("couldn't fetch exchange rates from {URL}");
        FAILED.store(now(), Ordering::SeqCst);
        return;
    };
    rates.insert("EUR".into(), 1.);
    let rates = Rates { date, fetched: now(), rates };
    let _ = std::fs::create_dir_all(crate::settings::data_dir());
    if let Ok(json) = serde_json::to_vec(&rates) {
        let _ = std::fs::write(file(), json);
    }
    *RATES.lock().unwrap() = Some(rates);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Rates, answer_with, parse};

    fn ask(text: &str, local: Option<&str>) -> Option<(String, String, String)> {
        let rates = Rates {
            date: "2026-10-19".into(),
            fetched: 0,
            rates: HashMap::from([("EUR".into(), 1.), ("USD".into(), 1.1225), ("KRW".into(), 1492.35), ("GBP".into(), 0.84205), ("ISK".into(), 136.4)]),
        };
        let (amount, from, to) = parse(text)?;
        answer_with(amount, from, to, &rates, local).map(|a| (a.shown, a.copied, a.about))
    }

    #[test]
    fn conversions() {
        assert_eq!(ask("100 usd in eur", None).unwrap().0, "89.09 EUR");
        assert_eq!(ask("$100 to krw", None).unwrap(), ("132,949 KRW".into(), "132949".into(), "100 USD in KRW · ECB rate of 19 Oct".into()));
        assert_eq!(ask("1,000 € in gbp", None).unwrap().0, "842.05 GBP");
        assert_eq!(ask("usd to isk", None).unwrap().0, "122 ISK", "one of it");
        assert_eq!(ask("100usd in euros", None).unwrap().0, "89.09 EUR");
        // Just an amount: into this Mac's currency (or dollars, if it's that).
        assert_eq!(ask("50 gbp", Some("KRW")).unwrap().0, "88,614 KRW");
        assert_eq!(ask("50 krw", Some("KRW")).unwrap().0, "0.04 USD");
        assert_eq!(ask("50 usd", Some("USD")).unwrap().0, "44.54 EUR");
    }

    #[test]
    fn not_conversions() {
        for text in ["usd", "100", "10 pounds in kg", "5 km in mi", "100 abc in eur", "100 usd in abc", "report 2024", "1.2.3 usd"] {
            assert!(ask(text, Some("KRW")).is_none(), "{text}");
        }
    }
}

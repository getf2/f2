//! Prices, for Quick Open: coins ("btc", "bitcoin price", "price of
//! solana", "0.5 eth") from CoinGecko's keyless API (https://coingecko.com:
//! Coinbase's spot price, if it's down), in dollars (as coins are quoted)
//! and the Mac's currency; stocks ("aapl stock", "apple stock", "toyota
//! stock", "$tsla") from Yahoo Finance's chart (no key, but no official
//! API either: it may be delayed, and if it stops answering, that's said),
//! in what they're traded in. Either in another currency if it's said ("btc
//! in krw", "aapl stock in eur"); and by a name, either ("apple price",
//! "kaspa price"), as Yahoo finds it first. Fetched only when asked, and
//! kept a minute.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// How long a price is kept.
const KEPT: Duration = Duration::from_secs(60);

static KNOWN: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);

/// A JSON answer from `url`, kept a minute (`browser`: asked as a browser
/// asks, as Yahoo wants). Waits on the network: not on the main thread.
fn fetch(url: &str, browser: bool) -> Option<Value> {
    if let Some((when, json)) = KNOWN.lock().unwrap().as_ref().and_then(|k| k.get(url))
        && when.elapsed() < KEPT
    {
        return Some(json.clone());
    }
    // (Yahoo turns some away, "Too Many Requests", now and then: its other
    // host, then.)
    let urls = if browser { vec![url.to_string(), url.replacen("query1.", "query2.", 1)] } else { vec![url.to_string()] };
    let json: Value = urls.iter().find_map(|url| {
        let mut curl = Command::new("/usr/bin/curl");
        curl.args(["-fsS", "--max-time", "5"]);
        if browser {
            curl.args(["-A", "Mozilla/5.0"]);
        }
        let out = curl.arg(url).output().ok().filter(|o| o.status.success())?;
        serde_json::from_slice(&out.stdout).ok()
    })?;
    KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(url.to_string(), (Instant::now(), json.clone()));
    Some(json)
}

/// Coins by their symbols (CoinGecko's ids): the common ones, without
/// asking.
const COINS: &[(&str, &str, &str)] = &[
    ("btc", "bitcoin", "Bitcoin"),
    ("eth", "ethereum", "Ethereum"),
    ("sol", "solana", "Solana"),
    ("xrp", "ripple", "XRP"),
    ("doge", "dogecoin", "Dogecoin"),
    ("ada", "cardano", "Cardano"),
    ("bnb", "binancecoin", "BNB"),
    ("usdt", "tether", "Tether"),
    ("usdc", "usd-coin", "USDC"),
    ("dot", "polkadot", "Polkadot"),
    ("ltc", "litecoin", "Litecoin"),
    ("trx", "tron", "TRON"),
    ("avax", "avalanche-2", "Avalanche"),
    ("link", "chainlink", "Chainlink"),
    ("shib", "shiba-inu", "Shiba Inu"),
    ("ton", "the-open-network", "Toncoin"),
    ("xlm", "stellar", "Stellar"),
    ("bch", "bitcoin-cash", "Bitcoin Cash"),
    ("atom", "cosmos", "Cosmos"),
    ("sui", "sui", "Sui"),
    ("pepe", "pepe", "Pepe"),
    ("xmr", "monero", "Monero"),
];

/// What's asked: a coin (its CoinGecko id, name, and how many: "0.5 eth"),
/// a stock (a ticker or a company's name), or either, by a name ("apple
/// price", "price of render").
#[derive(Clone, Debug, PartialEq)]
pub enum Asked {
    Coin { id: String, name: String, amount: f64 },
    Stock(String),
    Named(String),
}

/// What's asked, and in what currency, if it's said ("btc in krw": KRW).
pub fn asked(q: &str) -> Option<(Asked, Option<&'static str>)> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    let (lower, wanted) = match lower.rsplit_once(" in ").or_else(|| lower.rsplit_once(" to ")) {
        Some((what, currency)) if crate::currency::code(currency.trim()).is_some() => (what.trim().to_string(), crate::currency::code(currency.trim())),
        _ => (lower, None),
    };
    asked_what(&lower).map(|what| (what, wanted))
}

fn asked_what(lower: &str) -> Option<Asked> {
    let lower = lower.to_string();
    // Stocks: "aapl stock", "stock aapl", "apple shares", "$tsla".
    if let Some(ticker) = lower.strip_prefix('$').filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')) {
        return Some(Asked::Stock(ticker.to_uppercase()));
    }
    for suffix in [" stock price", " share price", " stock", " shares", " share"] {
        if let Some(name) = lower.strip_suffix(suffix).map(str::trim).filter(|n| !n.is_empty() && n.split_whitespace().count() <= 3) {
            return Some(Asked::Stock(name.to_string()));
        }
    }
    for prefix in ["stock price of ", "stock ", "shares of ", "share price of "] {
        if let Some(name) = lower.strip_prefix(prefix).map(str::trim).filter(|n| !n.is_empty() && n.split_whitespace().count() <= 3) {
            return Some(Asked::Stock(name.to_string()));
        }
    }
    // Coins: "btc", "bitcoin price", "price of eth", "0.5 btc".
    let words = lower.strip_prefix("price of ").or_else(|| lower.strip_suffix(" price")).unwrap_or(&lower).trim();
    let said_price = words.len() != lower.len();
    let (amount, coin) = match words.split_once(' ') {
        Some((n, coin)) if n.replace(',', "").parse::<f64>().is_ok() => (n.replace(',', "").parse::<f64>().ok()?, coin.trim()),
        _ => (1., words),
    };
    // (A symbol that's a word too ("link", "dot"), only with "price".)
    const WORDS: &[&str] = &["link", "ton", "dot", "atom", "sui", "pepe", "ada", "sol"];
    if let Some((_, id, name)) = COINS.iter().find(|(sym, id, name)| coin == *sym || coin == *id || coin == name.to_lowercase())
        && (said_price || amount != 1. || !WORDS.contains(&coin))
    {
        return Some(Asked::Coin { id: id.to_string(), name: name.to_string(), amount });
    }
    (said_price && !coin.is_empty() && coin.split_whitespace().count() <= 3).then(|| Asked::Named(coin.to_string()))
}

/// A price, said: what of, how much (in its currency), the day's change.
#[derive(Clone, Debug)]
pub struct Price {
    pub name: String,
    pub value: f64,
    pub currency: String,
    /// Percent, since yesterday's close (or the last 24 hours).
    pub change: Option<f64>,
    /// Where it's traded, or what else it's worth ("$82,637").
    pub about: String,
    /// Its page: a chart, the news.
    pub link: String,
}

/// "82,637.00", "13,071,453" (whole currencies whole).
pub fn amount(value: f64, currency: &str) -> String {
    let decimals = if crate::currency::WHOLE.contains(&currency) || value >= 10_000. {
        0
    } else if value < 1. {
        6
    } else {
        2
    };
    let text = crate::calc::grouped(&format!("{value:.decimals$}"));
    // (Calc's leaves four digits be: a price is "1,218.36".)
    let int = text.split('.').next().unwrap_or("").trim_start_matches('-');
    match text.find(|c: char| c.is_ascii_digit()) {
        Some(at) if int.len() == 4 => format!("{},{}", &text[..at + 1], &text[at + 1..]),
        _ => text,
    }
}

/// A coin's price, in dollars (or the currency `wanted`), and beside it in
/// the Mac's currency (or dollars).
pub fn coin(id: &str, name: &str, amount_of: f64, wanted: Option<&str>) -> Result<Price, String> {
    let main = wanted.unwrap_or("USD").to_string();
    let local = crate::macos::local_currency().unwrap_or_else(|| "USD".into()).to_uppercase();
    let also = if main != "USD" { "USD".to_string() } else { local };
    // (In what CoinGecko has; else from dollars, at the ECB's rate.)
    let in_ = |json: &Value, code: &str| json[id][code.to_lowercase()].as_f64().or_else(|| crate::currency::convert(json[id]["usd"].as_f64()?, "USD", code));
    let url = format!("https://api.coingecko.com/api/v3/simple/price?ids={id}&vs_currencies=usd,{},{}&include_24hr_change=true", main.to_lowercase(), also.to_lowercase());
    if let Some(json) = fetch(&url, false)
        && let Some(value) = in_(&json, &main)
    {
        let about = match in_(&json, &also) {
            Some(other) if also != main => format!("{} {also} · CoinGecko", amount(other * amount_of, &also)),
            _ => "CoinGecko".into(),
        };
        let change = |code: &str| json[id][format!("{}_24h_change", code.to_lowercase())].as_f64();
        return Ok(Price {
            name: name.into(),
            value: value * amount_of,
            currency: main.clone(),
            change: change(&main).or_else(|| change("USD")),
            about,
            link: format!("https://www.coingecko.com/en/coins/{id}"),
        });
    }
    // (CoinGecko down, or busy: Coinbase's spot price, for the common ones.)
    let symbol = COINS.iter().find(|(_, i, _)| *i == id).map(|(s, ..)| s.to_uppercase()).ok_or("CoinGecko didn't answer.")?;
    let json = fetch(&format!("https://api.coinbase.com/v2/prices/{symbol}-USD/spot"), false).ok_or("Neither CoinGecko nor Coinbase answered (no connection?).")?;
    let usd: f64 = json["data"]["amount"].as_str().and_then(|a| a.parse().ok()).ok_or("Coinbase didn't say.")?;
    let (value, currency) = match crate::currency::convert(usd, "USD", &main) {
        Some(value) => (value, main),
        None => (usd, "USD".to_string()),
    };
    Ok(Price { name: name.into(), value: value * amount_of, currency, change: None, about: "Coinbase".into(), link: format!("https://www.coinbase.com/price/{id}") })
}

/// A coin by name, as CoinGecko knows it: its id and name.
pub fn find_coin(named: &str) -> Option<(String, String)> {
    let json = fetch(&format!("https://api.coingecko.com/api/v3/search?query={}", crate::dev::url_encode(named)), false)?;
    let first = json["coins"].get(0)?;
    Some((first["id"].as_str()?.to_string(), first["name"].as_str()?.to_string()))
}

/// Yahoo's search, for a name.
fn search_url(named: &str) -> String {
    format!("https://query1.finance.yahoo.com/v1/finance/search?q={}&quotesCount=4&newsCount=0", crate::dev::url_encode(named))
}

/// The price of what's named, a company's stock or a coin: whichever Yahoo
/// finds first ("apple": Apple's stock; "render": the coin). True: a coin.
pub fn named(named: &str, wanted: Option<&str>) -> Result<(Price, bool), String> {
    let coin_first = fetch(&search_url(named), true).is_none_or(|json| json["quotes"][0]["quoteType"].as_str() == Some("CRYPTOCURRENCY"));
    let as_coin = || find_coin(named).ok_or_else(|| format!("Nothing called “{named}” found.")).and_then(|(id, name)| coin(&id, &name, 1., wanted)).map(|p| (p, true));
    let as_stock = || stock(named, wanted).map(|p| (p, false));
    if coin_first { as_coin().or_else(|_| as_stock()) } else { as_stock().or_else(|why| as_coin().map_err(|_| why)) }
}

/// A stock's price: the ticker as typed ("AAPL", "7203.T"), else the
/// company asked for by name, as Yahoo finds it. In what it's traded in,
/// or (at the ECB's rate) the currency `wanted`.
pub fn stock(asked: &str, wanted: Option<&str>) -> Result<Price, String> {
    let ticker_like = asked.len() <= 10 && !asked.contains(' ') && asked.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let mut tickers: Vec<String> = Vec::new();
    if ticker_like {
        tickers.push(asked.to_uppercase());
    }
    if let Some(json) = fetch(&search_url(asked), true) {
        for quote in json["quotes"].as_array().into_iter().flatten() {
            if matches!(quote["quoteType"].as_str(), Some("EQUITY" | "ETF" | "INDEX" | "MUTUALFUND"))
                && let Some(symbol) = quote["symbol"].as_str()
                && !tickers.iter().any(|t| t == symbol)
            {
                tickers.push(symbol.to_string());
            }
        }
    }
    // (The listing where the Mac is first, after one typed: Tokyo's for
    // "toyota" in Japan, not New York's.)
    let suffix = match crate::macos::region_code().as_deref() {
        Some("JP") => ".T",
        Some("KR") => ".KS",
        Some("GB") => ".L",
        Some("DE") => ".DE",
        Some("FR") => ".PA",
        Some("CA") => ".TO",
        Some("AU") => ".AX",
        Some("HK") => ".HK",
        Some("IS") => ".IC",
        _ => "",
    };
    if !suffix.is_empty() {
        let typed = usize::from(ticker_like);
        if let Some(at) = tickers.iter().skip(typed).position(|t| t.ends_with(suffix)) {
            let local = tickers.remove(typed + at);
            tickers.insert(typed, local);
        }
    }
    for ticker in tickers.iter().take(3) {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=1d", crate::dev::url_encode(ticker));
        let Some(json) = fetch(&url, true) else { continue };
        let meta = &json["chart"]["result"][0]["meta"];
        let Some(value) = meta["regularMarketPrice"].as_f64() else { continue };
        let previous = meta["chartPreviousClose"].as_f64().or_else(|| meta["previousClose"].as_f64());
        let name = meta["longName"].as_str().or_else(|| meta["shortName"].as_str()).unwrap_or(ticker).to_string();
        let exchange = meta["fullExchangeName"].as_str().or_else(|| meta["exchangeName"].as_str()).unwrap_or("").to_string();
        let traded = meta["currency"].as_str().unwrap_or("USD").to_string();
        // (In another currency, if it's asked: then what it's traded at,
        // beside it.)
        let (shown, currency, also) = match wanted.filter(|w| *w != traded).and_then(|w| Some((crate::currency::convert(value, &traded, w)?, w))) {
            Some((converted, w)) => (converted, w.to_string(), format!("{} {traded} · ", amount(value, &traded))),
            None => (value, traded, String::new()),
        };
        return Ok(Price {
            name: format!("{name} ({})", meta["symbol"].as_str().unwrap_or(ticker)),
            value: shown,
            currency,
            change: previous.filter(|p| *p > 0.).map(|p| (value - p) / p * 100.),
            about: if exchange.is_empty() { format!("{also}Yahoo Finance · may be delayed") } else { format!("{also}{exchange} · Yahoo Finance · may be delayed") },
            link: format!("https://finance.yahoo.com/quote/{}", crate::dev::url_encode(meta["symbol"].as_str().unwrap_or(ticker))),
        });
    }
    Err(if tickers.is_empty() { format!("No stock called “{asked}” found.") } else { "Yahoo Finance didn't answer (no connection, or it's changed).".into() })
}

/// "+1.0% today", "−2.3% in 24 h".
pub fn change(price: &Price, coin: bool) -> String {
    match price.change {
        Some(c) => format!("{}{:.1}% {}", if c >= 0. { "+" } else { "−" }, c.abs(), if coin { "in 24 h" } else { "today" }),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Asked, asked};

    #[test]
    fn asking() {
        let what = |q: &str| asked(q).map(|(what, _)| what);
        let coin = |id: &str, name: &str, amount: f64| Some(Asked::Coin { id: id.into(), name: name.into(), amount });
        assert_eq!(what("btc"), coin("bitcoin", "Bitcoin", 1.));
        assert_eq!(what("Bitcoin price"), coin("bitcoin", "Bitcoin", 1.));
        assert_eq!(what("0.5 eth"), coin("ethereum", "Ethereum", 0.5));
        assert_eq!(what("price of render"), Some(Asked::Named("render".into())));
        assert_eq!(what("apple price"), Some(Asked::Named("apple".into())));
        assert_eq!(what("aapl stock"), Some(Asked::Stock("aapl".into())));
        assert_eq!(what("toyota stock price"), Some(Asked::Stock("toyota".into())));
        assert_eq!(what("$tsla"), Some(Asked::Stock("TSLA".into())));
        assert_eq!(super::amount(1218.364, "USD"), "1,218.36");
        assert_eq!(super::amount(82606.4, "USD"), "82,606");
        assert_eq!(super::amount(0.081394, "USD"), "0.081394");
        assert_eq!(super::amount(1613536.2, "KRW"), "1,613,536");
        assert_eq!(super::amount(108.4, "KRW"), "108");
        assert_eq!(what("linked"), None);
        assert_eq!(what("link"), None, "a word");
        assert_eq!(what("link price"), coin("chainlink", "Chainlink", 1.));
        assert_eq!(what("stockholm"), None);
        assert_eq!(asked("btc in krw"), Some((Asked::Coin { id: "bitcoin".into(), name: "Bitcoin".into(), amount: 1. }, Some("KRW"))));
        assert_eq!(asked("0.5 eth to euros"), Some((Asked::Coin { id: "ethereum".into(), name: "Ethereum".into(), amount: 0.5 }, Some("EUR"))));
        assert_eq!(asked("aapl stock in isk"), Some((Asked::Stock("aapl".into()), Some("ISK"))));
        assert_eq!(asked("btc"), Some((Asked::Coin { id: "bitcoin".into(), name: "Bitcoin".into(), amount: 1. }, None)));
        assert_eq!(asked("100 usd in eur"), None, "currency's");
        assert_eq!(asked("time in tokyo"), None);
        assert_eq!(what("photos"), None);
    }

    /// Asked of CoinGecko and Yahoo: `cargo test --release -- --ignored prices_fetched`
    #[test]
    #[ignore]
    fn prices_fetched() {
        let btc = super::coin("bitcoin", "Bitcoin", 1., None).unwrap();
        println!("{btc:?}");
        assert_eq!(btc.currency, "USD");
        let isk = super::coin("ethereum", "Ethereum", 0.5, Some("ISK")).unwrap();
        println!("{isk:?}");
        let apple = super::stock("apple", None).unwrap();
        println!("{apple:?}");
        let toyota = super::stock("toyota", None).unwrap();
        println!("{toyota:?}");
        let samsung = super::stock("samsung", Some("USD")).unwrap();
        println!("{samsung:?}");
        for name in ["apple", "render", "kaspa"] {
            let (price, coin) = super::named(name, None).unwrap();
            println!("{name}: {} {} {coin}", price.name, price.value);
        }
        assert!(btc.value > 0. && apple.value > 0. && toyota.value > 0. && samsung.value > 0.);
    }
}

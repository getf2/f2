//! The bill, for Quick Open: a tip ("tip 18% on 84", "18% tip on 84.50",
//! "tip on 84": 15, 18 and 20%), and splitting it ("split 120 four ways",
//! "split 120 by 3", "split 84 between 4 with 20% tip"). Worked out here,
//! in cents; without them in won or yen (said with ₩ or ¥, or the Mac's
//! currency and a bill of a thousand or more: 84 is dollars, even here).

/// A line of the answer: as shown, as copied, and what it is.
#[derive(Debug, PartialEq)]
pub struct Line {
    pub shown: String,
    pub copied: String,
    pub about: String,
}

/// What's asked: how much, the tip (percent: None, it's asked what it'd
/// be), among how many, and whether the currency's said to have no cents
/// (₩, ¥: Some(true); $, €, £: Some(false)).
#[derive(Debug, PartialEq)]
struct Bill {
    amount: f64,
    tip: Option<Option<f64>>,
    ways: Option<u32>,
    whole: Option<bool>,
}

const WORDS: &[(&str, u32)] = &[("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9), ("ten", 10), ("eleven", 11), ("twelve", 12)];

fn number(word: &str) -> Option<f64> {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
    WORDS.iter().find(|(w, _)| *w == word).map(|(_, n)| f64::from(*n)).or_else(|| word.replace(',', "").parse().ok().filter(|n: &f64| n.is_finite()))
}

/// What's said of a bill so far: a tip or a split, and its numbers (the
/// bill, the tip's percent, how many), each if said.
struct Said {
    tipping: bool,
    splitting: bool,
    amount: Option<f64>,
    tip: Option<f64>,
    ways: Option<f64>,
    whole: Option<bool>,
}

fn said(q: &str) -> Option<Said> {
    let said = |words: &[&str]| q.to_lowercase().split_whitespace().any(|w| words.contains(&w));
    let whole = if q.contains(['₩', '¥']) || said(&["won", "krw", "yen", "jpy"]) {
        Some(true)
    } else if q.contains(['$', '€', '£']) || said(&["dollars", "dollar", "usd", "euros", "euro", "eur", "pounds", "gbp"]) {
        Some(false)
    } else {
        None
    };
    let lower = q.trim().trim_end_matches('?').to_lowercase().replace(['$', '€', '£', '₩', '¥'], " ");
    // ("18%" a word of its own; "1,200" one number, "84," the end of one.)
    let lower = lower.replace('%', " % ");
    let words: Vec<&str> = lower.split_whitespace().map(|w| w.trim_end_matches([',', ':', '.'])).collect();
    let tipping = words.iter().any(|w| matches!(*w, "tip" | "tips" | "tipping" | "gratuity"));
    let splitting = words.iter().any(|w| matches!(*w, "split" | "divide" | "divided"));
    if !tipping && !splitting {
        return None;
    }
    // Everything else said must be a number or one of these: it's the
    // bill, not "split screen" or "tip of the day".
    const SAID: &[&str] = &["tip", "tips", "tipping", "gratuity", "split", "divide", "divided", "by", "between", "among", "for", "on", "of", "with", "ways", "way", "people", "persons", "us", "a", "the", "bill", "check", "%", "percent", "pct", "and", "in", "what's", "whats", "what", "is", "how", "much", "plus", "into", "each", "won", "krw", "yen", "jpy", "dollars", "dollar", "usd", "euros", "euro", "eur", "pounds", "gbp"];
    let mut amount = None;
    let mut tip = None;
    let mut ways = None;
    for (i, word) in words.iter().enumerate() {
        let next = words.get(i + 1).copied().unwrap_or("");
        let before = if i > 0 { words[i - 1] } else { "" };
        match number(word) {
            Some(n) if matches!(next, "%" | "percent" | "pct") => tip = Some(n),
            Some(n) if matches!(next, "ways" | "way" | "people" | "persons") || matches!(before, "by" | "between" | "among" | "into") => ways = Some(n),
            Some(n) if amount.is_none() => amount = Some(n),
            // ("split 120 4": the second, how many.)
            Some(n) if splitting && ways.is_none() => ways = Some(n),
            Some(_) => return None,
            None if SAID.contains(word) => {}
            None => return None,
        }
    }
    Some(Said { tipping, splitting, amount, tip, ways, whole })
}

fn asked(q: &str) -> Option<Bill> {
    let said = said(q)?;
    let amount = said.amount.filter(|a| *a > 0.)?;
    let ways = said.ways.filter(|w| said.splitting && w.fract() == 0. && (2. ..=100.).contains(w)).map(|w| w as u32);
    if said.splitting && ways.is_none() {
        return None;
    }
    let tip = if said.tipping || said.tip.is_some() { Some(said.tip.filter(|t| (0. ..=100.).contains(t))) } else { None };
    Some(Bill { amount, tip, ways, whole: said.whole })
}

/// What a bill half asked about still wants, with an example: "tip" the
/// bill, "split 120" how many. (Started with "tip" or "split": "tips" is
/// an app.)
pub fn wanting(q: &str) -> Option<String> {
    let lower = q.trim().to_lowercase();
    if !["tip", "split"].iter().any(|w| lower == *w || lower.starts_with(&format!("{w} "))) {
        return None;
    }
    let said = said(q)?;
    let wants_ways = said.splitting && said.ways.is_none();
    match (said.amount, wants_ways) {
        (None, true) => Some("Type the bill, and how many: “split 120 four ways”".into()),
        (None, false) => Some(format!("Type the bill: “tip {} on 84”", percent(said.tip.unwrap_or(18.)))),
        (Some(amount), true) => Some(format!("Type how many: “split {} four ways”", money(amount, amount.fract() == 0.))),
        _ => None,
    }
}

/// "15.12", "99", "1,250.50", "45,000" (no cents in a whole currency).
fn money(value: f64, whole: bool) -> String {
    let rounded = if whole { value.round() } else { (value * 100.).round() / 100. };
    let text = if whole || rounded.fract() == 0. { format!("{rounded:.0}") } else { format!("{rounded:.2}") };
    let grouped = crate::calc::grouped(&text);
    // (Calc's leaves four digits be: money's "1,250".)
    let int = grouped.split('.').next().unwrap_or("");
    if int.len() == 4 { format!("{},{}", &grouped[..1], &grouped[1..]) } else { grouped }
}

fn percent(p: f64) -> String {
    if p.fract() == 0. { format!("{p:.0}%") } else { format!("{p}%") }
}

/// The answer to `q`, if it's about a bill.
pub fn answer(q: &str) -> Option<Vec<Line>> {
    let whole = crate::macos::local_currency().is_some_and(|c| crate::currency::WHOLE.contains(&c.as_str()));
    answer_in(q, whole)
}

/// `local_whole`: the Mac's currency has no cents.
fn answer_in(q: &str, local_whole: bool) -> Option<Vec<Line>> {
    let bill = asked(q)?;
    let whole = bill.whole.unwrap_or(local_whole && bill.amount >= 1000.);
    let m = |v: f64| money(v, whole);
    let line = |shown: String, copied: f64, about: String| Line { shown, copied: m(copied).replace(',', ""), about };
    let amount = bill.amount;
    let tips: Vec<f64> = match bill.tip {
        Some(Some(p)) => vec![p],
        Some(None) => vec![15., 18., 20.],
        None => vec![],
    };
    Some(match (bill.ways, tips.as_slice()) {
        (Some(ways), []) => vec![line(format!("{} each", m(amount / f64::from(ways))), amount / f64::from(ways), format!("{} split {ways} ways", m(amount)))],
        (Some(ways), tips) => tips
            .iter()
            .map(|p| {
                let total = amount * (1. + p / 100.);
                line(format!("{} each", m(total / f64::from(ways))), total / f64::from(ways), format!("{} with a {} tip, {} in all, split {ways} ways", m(amount), percent(*p), m(total)))
            })
            .collect(),
        (None, [p]) => {
            let tip = amount * p / 100.;
            vec![line(format!("{} tip", m(tip)), tip, format!("{} of {}", percent(*p), m(amount))), line(format!("{} in all", m(amount + tip)), amount + tip, format!("{} and a {} tip", m(amount), percent(*p)))]
        }
        (None, tips) => tips
            .iter()
            .map(|p| {
                let tip = amount * p / 100.;
                line(format!("{} tip", m(tip)), tip, format!("{} of {} · {} in all", percent(*p), m(amount), m(amount + tip)))
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Bill, answer_in, asked};

    #[test]
    fn asking() {
        let bill = |amount: f64, tip: Option<Option<f64>>, ways: Option<u32>| Some(Bill { amount, tip, ways, whole: None });
        assert_eq!(asked("tip 18% on 84"), bill(84., Some(Some(18.)), None));
        assert_eq!(asked("18% tip on $84.50"), Some(Bill { amount: 84.5, tip: Some(Some(18.)), ways: None, whole: Some(false) }));
        assert_eq!(asked("tip on 84"), bill(84., Some(None), None));
        assert_eq!(asked("split 120 four ways"), bill(120., None, Some(4)));
        assert_eq!(asked("split 120 by 3"), bill(120., None, Some(3)));
        assert_eq!(asked("split 120 4"), bill(120., None, Some(4)));
        assert_eq!(asked("split 84 between 4 with 20% tip"), bill(84., Some(Some(20.)), Some(4)));
        assert_eq!(asked("split 1,200 3 ways"), bill(1200., None, Some(3)));
        assert_eq!(asked("split screen"), None);
        assert_eq!(asked("tip of the day"), None);
        assert_eq!(asked("split 120"), None, "among how many?");
        assert_eq!(asked("tips"), None);
    }

    #[test]
    fn answers() {
        let shown = |q: &str, whole: bool| answer_in(q, whole).unwrap().into_iter().map(|l| format!("{} | {} | {}", l.shown, l.copied, l.about)).collect::<Vec<_>>();
        assert_eq!(shown("tip 18% on 84", false), ["15.12 tip | 15.12 | 18% of 84", "99.12 in all | 99.12 | 84 and a 18% tip"]);
        assert_eq!(shown("split 120 four ways", false), ["30 each | 30 | 120 split 4 ways"]);
        assert_eq!(shown("split 100000 3 ways", true), ["33,333 each | 33333 | 100,000 split 3 ways"]);
        assert_eq!(shown("tip 18% on 84", true)[0], "15.12 tip | 15.12 | 18% of 84", "84: dollars, even where it's won");
        assert_eq!(shown("split ₩90,000 4 ways", false), ["22,500 each | 22500 | 90,000 split 4 ways"]);
        assert_eq!(shown("split 90000 won 4 ways", false), ["22,500 each | 22500 | 90,000 split 4 ways"]);
        assert_eq!(shown("tip on 50", false)[0], "7.50 tip | 7.50 | 15% of 50 · 57.50 in all");
        assert_eq!(shown("split 84 between 4 with 20% tip", false), ["25.20 each | 25.20 | 84 with a 20% tip, 100.80 in all, split 4 ways"]);
        assert_eq!(shown("split 1250 2 ways", false), ["625 each | 625 | 1,250 split 2 ways"]);
    }
}

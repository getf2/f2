//! Half a question, for Quick Open: the start of one it answers, not one
//! yet ("18% of", "5 km in", "tip", "split 120", "price of"). Shown as what
//! it's going to be ("18% of …", "Type a number to work it out") over the
//! web's search, so it's clear the answer's coming (and not that Quick
//! Open can't).

/// What it's the start of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Sum,
    Conversion,
    Currency,
    Bill,
    Kitchen,
    Pick,
    Date,
    /// One of the commands that's typed after a word (its catalog key:
    /// "ask:price").
    Ask(&'static str),
}

/// What's typed, what's still to be typed (with an example), and what
/// it's the start of.
#[derive(Debug, PartialEq)]
pub struct Unfinished {
    pub typed: String,
    pub wanted: String,
    pub kind: Kind,
}

pub fn asked(text: &str) -> Option<Unfinished> {
    let q = text.trim();
    if q.is_empty() || q.chars().count() > 60 {
        return None;
    }
    let lower = q.to_lowercase();
    let (wanted, kind) = sum(&lower)
        .map(|w| (w, Kind::Sum))
        .or_else(|| conversion(text))
        .or_else(|| crate::bill::wanting(q).map(|w| (w, Kind::Bill)))
        .or_else(|| crate::kitchen::wanting(q).map(|w| (w, Kind::Kitchen)))
        .or_else(|| pick(&lower).map(|w| (w.to_string(), Kind::Pick)))
        .or_else(|| lead(&lower))?;
    Some(Unfinished { typed: q.to_string(), wanted, kind })
}

/// A sum that stops at an operator ("18% of", "12 +", "2^", "sqrt(").
fn sum(lower: &str) -> Option<String> {
    let wanted = "Type a number to work it out".to_string();
    // (A function, or a bracket: what's in it.)
    if lower.ends_with('(') || ["sqrt", "√"].contains(&lower) {
        let head = lower.trim_end_matches('(').trim_end();
        let function = ["sqrt", "√", "log", "ln", "sin", "cos", "tan", "abs", "round", "floor", "ceil"].iter().any(|f| head.ends_with(f));
        let after = head.chars().last().is_some_and(|c| "+-*/^×÷(".contains(c) || c.is_ascii_digit());
        return (function || after).then_some(wanted);
    }
    // (Longest first: "**" before "*".)
    const OPERATORS: &[&str] = &["**", "+", "-", "*", "×", "/", "÷", "^", " x", " of", "%of"];
    let op = OPERATORS.iter().find(|op| lower.ends_with(**op))?;
    let head = lower[..lower.len() - op.len()].trim_end();
    let number = head.replace(',', "").parse::<f64>().is_ok();
    // (What's before it's a sum, or a number: not "iphone x", "covid-".)
    let percent = op.ends_with("of") && (head.ends_with('%') || number);
    let before = number || crate::calc::answer(head).is_some();
    ((before && !op.ends_with("of")) || percent).then_some(wanted)
}

/// A conversion that stops before its unit ("5 km", "5 km in", "100 usd
/// to"): with the calculator's (or the currencies') gray hint as its
/// example.
fn conversion(text: &str) -> Option<(String, Kind)> {
    let q = text.trim();
    let example = |rest: String| {
        let whole = format!("{}{rest}", text.trim_start());
        whole.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    if crate::calc::answer(q).is_none()
        && let Some(rest) = crate::calc::hint(text)
    {
        return Some((format!("Type a unit: “{}”", example(rest)), Kind::Conversion));
    }
    if crate::currency::answer(q).is_none()
        && let Some(rest) = crate::currency::hint(text)
    {
        return Some((format!("Type a currency: “{}”", example(rest)), Kind::Currency));
    }
    None
}

/// "pick", "random number between": the choices to come.
fn pick(lower: &str) -> Option<&'static str> {
    const CHOICES: &[&str] = &["pick", "pick between", "pick from", "pick one", "choose between", "choose from", "pick for me", "help me pick", "help me choose"];
    const NUMBERS: &[&str] = &["random number between", "random number from", "pick a number between", "pick a number from", "random between"];
    if CHOICES.contains(&lower) {
        return Some("Type the choices: “pick sushi, pizza or tacos”");
    }
    NUMBERS.contains(&lower).then_some("Type two numbers: “random number between 1 and 10”")
}

/// What a command typed after a word still wants: "price of", "time in".
const LEADS: &[(&str, Kind, &str)] = &[
    ("price of", Kind::Ask("ask:price"), "Type a stock or a coin: “price of apple”"),
    ("time in", Kind::Ask("ask:time"), "Type a city: “time in tokyo”"),
    ("what time is it in", Kind::Ask("ask:time"), "Type a city: “what time is it in london”"),
    ("weather in", Kind::Ask("ask:weather"), "Type a city: “weather in paris”"),
    ("air quality in", Kind::Ask("ask:air"), "Type a city: “air quality in seoul”"),
    ("holidays in", Kind::Ask("ask:holidays"), "Type a country: “holidays in japan”"),
    ("directions to", Kind::Ask("ask:directions"), "Type a place: “directions to the airport”"),
    ("directions from", Kind::Ask("ask:directions"), "Type two places: “directions from gangnam to hongdae”"),
    ("how do i get to", Kind::Ask("ask:directions"), "Type a place: “how do i get to lotte world”"),
    ("flight", Kind::Ask("ask:flight"), "Type a flight number: “flight KE 17”"),
    ("define", Kind::Ask("ask:define"), "Type a word: “define serendipity”"),
    ("meaning of", Kind::Ask("ask:define"), "Type a word: “meaning of ephemeral”"),
    ("synonyms for", Kind::Ask("ask:synonyms"), "Type a word: “synonyms for happy”"),
    ("spell", Kind::Ask("ask:spell"), "Type a word: “spell accomodate”"),
    ("translate", Kind::Ask("ask:translate"), "Type what to translate: “translate good morning to japanese”"),
    ("how do you say", Kind::Ask("ask:translate"), "Type what to translate: “how do you say thank you in french”"),
    ("remind me to", Kind::Ask("ask:reminder"), "Type what, and when: “remind me to call mom at 5pm”"),
    ("remind me", Kind::Ask("ask:reminder"), "Type what, and when: “remind me to call mom at 5pm”"),
    ("timer", Kind::Ask("ask:timer"), "Type how long: “timer 10 min”"),
    ("alarm", Kind::Ask("ask:alarm"), "Type a time: “alarm 7am”"),
    ("sleep in", Kind::Ask("ask:sleep-timer"), "Type how long: “sleep in 30 min”"),
    ("track", Kind::Ask("ask:track"), "Paste a tracking number"),
    ("event", Kind::Ask("ask:event"), "Type what, and when: “event lunch friday at 1pm”"),
    ("shoe size", Kind::Ask("ask:sizes"), "Type a size: “shoe size us 10 in eu”"),
    ("days until", Kind::Date, "Type a day: “days until christmas”"),
    ("how many days until", Kind::Date, "Type a day: “how many days until christmas”"),
    ("days since", Kind::Date, "Type a day: “days since january 1”"),
    ("weeks until", Kind::Date, "Type a day: “weeks until june 1”"),
];

fn lead(lower: &str) -> Option<(String, Kind)> {
    LEADS.iter().find(|(lead, ..)| *lead == lower).map(|(_, kind, wanted)| (wanted.to_string(), *kind))
}

#[cfg(test)]
mod tests {
    use super::{Kind, asked};

    fn kind(q: &str) -> Option<Kind> {
        asked(q).map(|u| u.kind)
    }

    #[test]
    fn sums() {
        assert_eq!(kind("18% of"), Some(Kind::Sum));
        assert_eq!(kind("18% of "), Some(Kind::Sum));
        assert_eq!(kind("12 +"), Some(Kind::Sum));
        assert_eq!(kind("12*7 -"), Some(Kind::Sum));
        assert_eq!(kind("2^"), Some(Kind::Sum));
        assert_eq!(kind("5 x"), Some(Kind::Sum));
        assert_eq!(kind("1,200 /"), Some(Kind::Sum));
        assert_eq!(kind("sqrt("), Some(Kind::Sum));
        assert_eq!(kind("2 * ("), Some(Kind::Sum));
        assert_eq!(kind("18%"), None, "a sum already");
        assert_eq!(kind("18% of 250"), None);
        assert_eq!(kind("iphone x"), None);
        assert_eq!(kind("covid-"), None);
        assert_eq!(kind("best of"), None);
        assert_eq!(kind("c++"), None);
        assert_eq!(kind("log"), None, "Logic Pro, say");
    }

    #[test]
    fn conversions() {
        let wanted = |q: &str| asked(q).map(|u| u.wanted);
        assert_eq!(wanted("5 km").as_deref(), Some("Type a unit: “5 km in mi”"));
        assert_eq!(wanted("5 km in").as_deref(), Some("Type a unit: “5 km in mi”"));
        assert_eq!(wanted("100 usd to").as_deref(), Some("Type a currency: “100 usd to eur”"));
        assert_eq!(kind("5 km in mi"), None);
    }

    #[test]
    fn others() {
        assert_eq!(kind("tip"), Some(Kind::Bill));
        assert_eq!(kind("tip 18% on"), Some(Kind::Bill));
        assert_eq!(kind("split 120"), Some(Kind::Bill));
        assert_eq!(kind("tip 18% on 84"), None);
        assert_eq!(kind("tips"), None, "the Tips app");
        assert_eq!(kind("1 cup flour in"), Some(Kind::Kitchen));
        assert_eq!(kind("1 cup of"), Some(Kind::Kitchen));
        assert_eq!(kind("pick"), Some(Kind::Pick));
        assert_eq!(kind("price of"), Some(Kind::Ask("ask:price")));
        assert_eq!(kind("Time in"), Some(Kind::Ask("ask:time")));
        assert_eq!(kind("days until"), Some(Kind::Date));
        assert_eq!(kind("safari"), None);
        assert_eq!(kind("weather in paris"), None);
    }
}

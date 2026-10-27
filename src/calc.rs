//! Quick Open's calculator: arithmetic as typed (`12*7`, `2^10`, `(3+4)/2`,
//! `sqrt 2`), percentages (`15% of 80`, `80 + 15%`), and units (`5 km in
//! mi`, `30 c to f`, `2 gb in mb`); currencies are `currency`'s. Only what
//! looks like a sum or a conversion: a bare number, or a word, is a name to
//! look for.

/// The answer to `text`, if it's a sum or a conversion: as shown, and as
/// copied (no thousands separators, the unit dropped).
pub fn answer(text: &str) -> Option<(String, String)> {
    // ("12*7 =", as a calculator's typed; "=12*7", as a spreadsheet's.)
    let text = text.trim().trim_matches('=').trim().to_lowercase();
    if text.is_empty() || !text.chars().any(|c| c.is_ascii_digit() || c == 'π') {
        return None;
    }
    if let Some((value, unit)) = convert(&text) {
        return Some((format!("{} {unit}", shown(value)), plain(value)));
    }
    let value = sum(&text)?;
    value.is_finite().then(|| (shown(value), plain(value)))
}

/// Whether `text` is more than a number: an operator or a function in it.
fn looks_like_sum(text: &str) -> bool {
    let ops = text.chars().filter(|c| "+-*/^×÷%()".contains(*c)).count();
    let words = ["sqrt", "log", "ln", "sin", "cos", "tan", "abs", "round", "floor", "ceil", " of ", "pi", "π"];
    let leading_minus_only = ops == 1 && text.starts_with('-');
    (ops > 0 && !leading_minus_only) || words.iter().any(|w| text.contains(w)) || text.split_whitespace().any(|w| w == "x")
}

fn sum(text: &str) -> Option<f64> {
    if !looks_like_sum(text) {
        return None;
    }
    let tokens = tokens(text)?;
    let mut p = Parser { tokens, at: 0 };
    let value = p.expr()?;
    (p.at == p.tokens.len()).then_some(value)
}

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Num(f64),
    Op(char),
    Word(String),
}

fn tokens(text: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            // ("1,000": its comma is the number's, below.)
            ' ' | '\t' | ',' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || (chars[i] == ',' && chars.get(i + 1).is_some_and(char::is_ascii_digit))) {
                    i += 1;
                }
                let n: String = chars[start..i].iter().filter(|c| **c != ',').collect();
                out.push(Tok::Num(n.parse().ok()?));
            }
            '+' | '-' | '*' | '/' | '^' | '%' | '(' | ')' => {
                // "**" as "^".
                if c == '*' && chars.get(i + 1) == Some(&'*') {
                    out.push(Tok::Op('^'));
                    i += 2;
                    continue;
                }
                out.push(Tok::Op(c));
                i += 1;
            }
            '×' | 'x' if out.last().is_some_and(|t| matches!(t, Tok::Num(_) | Tok::Op(')'))) && !chars.get(i + 1).is_some_and(|c| c.is_alphabetic()) => {
                out.push(Tok::Op('*'));
                i += 1;
            }
            '÷' => {
                out.push(Tok::Op('/'));
                i += 1;
            }
            'π' => {
                out.push(Tok::Num(std::f64::consts::PI));
                i += 1;
            }
            c if c.is_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.as_str() {
                    "pi" => out.push(Tok::Num(std::f64::consts::PI)),
                    "e" => out.push(Tok::Num(std::f64::consts::E)),
                    _ => out.push(Tok::Word(word)),
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Precedence climbing: + -, then * / (and "of", and side by side: "2pi"),
/// then unary minus, then ^ (right to left: -2^2 is -4), then
/// percentages, numbers, brackets and functions.
struct Parser {
    tokens: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.at)
    }

    fn eat_op(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.at += 1;
            return true;
        }
        false
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Word(w)) if w == word) {
            self.at += 1;
            return true;
        }
        false
    }

    fn expr(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        loop {
            let op = if self.eat_op('+') { '+' } else if self.eat_op('-') { '-' } else { return Some(value) };
            let start = self.at;
            let rhs = self.term()?;
            // "80 + 15%": 15% of 80, added.
            let percent = matches!(self.tokens.get(self.at.wrapping_sub(1)), Some(Tok::Op('%'))) && self.at - start == 2;
            let rhs = if percent { value * rhs } else { rhs };
            value = if op == '+' { value + rhs } else { value - rhs };
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        loop {
            if self.eat_op('*') {
                value *= self.unary()?;
            } else if self.eat_op('/') {
                value /= self.unary()?;
            } else if self.eat_word("of") {
                // "15% of 80".
                value *= self.unary()?;
            } else if matches!(self.peek(), Some(Tok::Num(_) | Tok::Op('('))) {
                // "2(3+4)", "2pi".
                value *= self.unary()?;
            } else {
                return Some(value);
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat_op('-') {
            return Some(-self.unary()?);
        }
        if self.eat_op('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.percent()?;
        if self.eat_op('^') {
            return Some(base.powf(self.unary()?));
        }
        Some(base)
    }

    fn percent(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        while self.eat_op('%') {
            value /= 100.;
        }
        Some(value)
    }

    fn atom(&mut self) -> Option<f64> {
        match self.peek()?.clone() {
            Tok::Num(n) => {
                self.at += 1;
                Some(n)
            }
            Tok::Op('(') => {
                self.at += 1;
                let value = self.expr()?;
                self.eat_op(')');
                Some(value)
            }
            Tok::Word(f) => {
                self.at += 1;
                let x = self.unary()?;
                Some(match f.as_str() {
                    "sqrt" => x.sqrt(),
                    "log" => x.log10(),
                    "ln" => x.ln(),
                    "sin" => x.to_radians().sin(),
                    "cos" => x.to_radians().cos(),
                    "tan" => x.to_radians().tan(),
                    "abs" => x.abs(),
                    "round" => x.round(),
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}

/// A unit: its names, its kind, and how many of the kind's base it is (or,
/// for temperatures, which scale).
struct Unit {
    names: &'static [&'static str],
    kind: &'static str,
    factor: f64,
    shown: &'static str,
}

const UNITS: &[Unit] = &[
    Unit { names: &["mm", "millimeter", "millimeters"], kind: "length", factor: 0.001, shown: "mm" },
    Unit { names: &["cm", "centimeter", "centimeters"], kind: "length", factor: 0.01, shown: "cm" },
    Unit { names: &["m", "meter", "meters", "metre", "metres"], kind: "length", factor: 1., shown: "m" },
    Unit { names: &["km", "kilometer", "kilometers", "kilometre", "kilometres"], kind: "length", factor: 1000., shown: "km" },
    Unit { names: &["in", "inch", "inches", "\""], kind: "length", factor: 0.0254, shown: "in" },
    Unit { names: &["ft", "foot", "feet", "'"], kind: "length", factor: 0.3048, shown: "ft" },
    Unit { names: &["yd", "yard", "yards"], kind: "length", factor: 0.9144, shown: "yd" },
    Unit { names: &["mi", "mile", "miles"], kind: "length", factor: 1609.344, shown: "mi" },
    Unit { names: &["mg", "milligram", "milligrams"], kind: "mass", factor: 0.001, shown: "mg" },
    Unit { names: &["g", "gram", "grams"], kind: "mass", factor: 1., shown: "g" },
    Unit { names: &["kg", "kilo", "kilos", "kilogram", "kilograms"], kind: "mass", factor: 1000., shown: "kg" },
    Unit { names: &["oz", "ounce", "ounces"], kind: "mass", factor: 28.349523125, shown: "oz" },
    Unit { names: &["lb", "lbs", "pound", "pounds"], kind: "mass", factor: 453.59237, shown: "lb" },
    Unit { names: &["ml", "milliliter", "milliliters"], kind: "volume", factor: 0.001, shown: "ml" },
    Unit { names: &["l", "liter", "liters", "litre", "litres"], kind: "volume", factor: 1., shown: "l" },
    Unit { names: &["floz"], kind: "volume", factor: 0.0295735, shown: "fl oz" },
    Unit { names: &["cup", "cups"], kind: "volume", factor: 0.2365882, shown: "cups" },
    Unit { names: &["gal", "gallon", "gallons"], kind: "volume", factor: 3.785411784, shown: "gal" },
    Unit { names: &["b", "byte", "bytes"], kind: "data", factor: 1., shown: "bytes" },
    Unit { names: &["kb", "kilobyte", "kilobytes"], kind: "data", factor: 1e3, shown: "KB" },
    Unit { names: &["mb", "megabyte", "megabytes"], kind: "data", factor: 1e6, shown: "MB" },
    Unit { names: &["gb", "gigabyte", "gigabytes"], kind: "data", factor: 1e9, shown: "GB" },
    Unit { names: &["tb", "terabyte", "terabytes"], kind: "data", factor: 1e12, shown: "TB" },
    Unit { names: &["ms", "millisecond", "milliseconds"], kind: "time", factor: 0.001, shown: "ms" },
    Unit { names: &["s", "sec", "secs", "second", "seconds"], kind: "time", factor: 1., shown: "s" },
    Unit { names: &["min", "mins", "minute", "minutes"], kind: "time", factor: 60., shown: "min" },
    Unit { names: &["h", "hr", "hrs", "hour", "hours"], kind: "time", factor: 3600., shown: "h" },
    Unit { names: &["day", "days"], kind: "time", factor: 86400., shown: "days" },
    Unit { names: &["week", "weeks"], kind: "time", factor: 604800., shown: "weeks" },
    Unit { names: &["c", "°c", "celsius"], kind: "temperature", factor: 0., shown: "°C" },
    Unit { names: &["f", "°f", "fahrenheit"], kind: "temperature", factor: 1., shown: "°F" },
    Unit { names: &["k", "kelvin"], kind: "temperature", factor: 2., shown: "K" },
];

/// What a unit's usually turned into ("km": "mi"), by its first name.
fn counterpart(unit: &Unit) -> Option<&'static Unit> {
    let to = match unit.names[0] {
        "mm" | "cm" => "in",
        "m" | "yd" => "ft",
        "km" => "mi",
        "in" => "cm",
        "ft" => "m",
        "mi" => "km",
        "mg" => "g",
        "g" => "oz",
        "kg" => "lb",
        "oz" => "g",
        "lb" => "kg",
        "ml" => "cup",
        "l" => "gal",
        "floz" | "cup" => "ml",
        "gal" => "l",
        "b" => "kb",
        "kb" => "mb",
        "mb" | "tb" => "gb",
        "gb" => "mb",
        "ms" => "s",
        "s" => "min",
        "min" | "day" => "h",
        "h" => "min",
        "week" => "day",
        "c" => "f",
        "f" | "k" => "c",
        _ => return None,
    };
    UNITS.iter().find(|u| u.names[0] == to)
}

/// What can follow a quantity typed so far, for Quick Open's gray hint:
/// "5 km" → " in mi", "30 c" → " to f", "5 kg in po" → "und".
pub fn hint(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let digits = lower.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))?;
    if digits == 0 || !lower.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let rest = lower[digits..].trim_start();
    let (name, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    // (Run together, a letter's rarely a unit: "4k", "5m"; but "30c".)
    if !lower[digits..].starts_with(' ') && name.len() == 1 && !matches!(name, "c" | "f") {
        return None;
    }
    let from = unit(name)?;
    let to = counterpart(from)?;
    // ("5 in in cm" reads as five, not inches: "to" after inches.)
    let word = if from.kind == "temperature" || name == "in" || name == "\"" { "to" } else { "in" };
    if !rest.contains(' ') {
        return Some(format!(" {word} {}", to.names[0]));
    }
    for whole in [format!("{word} {}", to.names[0]), format!("{} {}", if word == "in" { "to" } else { "in" }, to.names[0])] {
        if whole.len() > tail.len() && whole.starts_with(tail) {
            return Some(whole[tail.len()..].to_string());
        }
    }
    // A unit of its kind, by its start.
    let (connector, partial) = tail.split_once(' ')?;
    if !matches!(connector, "in" | "to") || partial.is_empty() || unit(partial).is_some() {
        return None;
    }
    UNITS
        .iter()
        .filter(|u| u.kind == from.kind && !std::ptr::eq(*u, from))
        .flat_map(|u| u.names.iter())
        .find(|n| n.len() > partial.len() && n.starts_with(partial))
        .map(|n| n[partial.len()..].to_string())
}

fn unit(name: &str) -> Option<&'static Unit> {
    let name = name.trim().trim_end_matches('.');
    UNITS.iter().find(|u| u.names.contains(&name))
}

/// "5 km in mi", "30 c to f", "2.5gb in mb": the value and the unit it's in.
fn convert(text: &str) -> Option<(f64, &'static str)> {
    let (from, to) = text.split_once(" in ").or_else(|| text.split_once(" to "))?;
    let to = unit(to)?;
    let from = from.trim();
    let split = from.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == ',' || c == ' ')).filter(|&i| i > 0)?;
    let (number, name) = from.split_at(split);
    let value: f64 = number.replace([',', ' '], "").parse().ok()?;
    let from = unit(name)?;
    if from.kind != to.kind {
        return None;
    }
    let value = if from.kind == "temperature" {
        let celsius = match from.factor as u8 {
            0 => value,
            1 => (value - 32.) * 5. / 9.,
            _ => value - 273.15,
        };
        match to.factor as u8 {
            0 => celsius,
            1 => celsius * 9. / 5. + 32.,
            _ => celsius + 273.15,
        }
    } else {
        value * from.factor / to.factor
    };
    Some((value, to.shown))
}

/// To 10 significant digits, without trailing zeros.
fn plain(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let digits = 10 - (value.abs().log10().floor() as i32 + 1).clamp(-6, 10);
    let s = format!("{:.*}", digits.max(0) as usize, value);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// `plain`, with thousands separated.
fn shown(value: f64) -> String {
    grouped(&plain(value))
}

/// A number as text ("-1234567.5"), its thousands separated
/// ("-1,234,567.5"; not 4 digits: "1234").
pub fn grouped(s: &str) -> String {
    let (sign, s) = s.strip_prefix('-').map_or(("", s), |r| ("-", r));
    let (int, frac) = s.split_once('.').map_or((s, None), |(i, f)| (i, Some(f)));
    if int.len() <= 4 || int.contains('e') {
        return format!("{sign}{s}");
    }
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

#[cfg(test)]
mod tests {
    use super::answer;

    fn copied(text: &str) -> Option<String> {
        answer(text).map(|(_, copy)| copy)
    }

    #[test]
    fn sums() {
        assert_eq!(copied("12*7").as_deref(), Some("84"));
        assert_eq!(copied("12*7 =").as_deref(), Some("84"), "as a calculator's typed");
        assert_eq!(copied("=12*7").as_deref(), Some("84"), "as a spreadsheet's");
        assert_eq!(copied("2^10").as_deref(), Some("1024"));
        assert_eq!(copied("2**3").as_deref(), Some("8"));
        assert_eq!(copied("(3+4)/2").as_deref(), Some("3.5"));
        assert_eq!(copied("1 + 2 * 3").as_deref(), Some("7"));
        assert_eq!(copied("-2^2").as_deref(), Some("-4"));
        assert_eq!(copied("10 / 3").as_deref(), Some("3.333333333"));
        assert_eq!(copied("15% of 80").as_deref(), Some("12"));
        assert_eq!(copied("80 + 15%").as_deref(), Some("92"));
        assert_eq!(copied("80 - 25%").as_deref(), Some("60"));
        assert_eq!(copied("sqrt 16").as_deref(), Some("4"));
        assert_eq!(copied("2pi").map(|s| s[..4].to_string()).as_deref(), Some("6.28"));
        assert_eq!(copied("3 x 4").as_deref(), Some("12"));
        assert_eq!(copied("1,000 * 3").as_deref(), Some("3000"));
        assert_eq!(answer("1234567 * 1").map(|(s, _)| s).as_deref(), Some("1,234,567"));
    }

    #[test]
    fn not_sums() {
        // Names to look for, not sums.
        for text in ["2024", "-5", "tax 2024", "invoice", "IMG_0001", "x", "", "v1.2", "notes 3"] {
            assert_eq!(answer(text), None, "{text}");
        }
        assert_eq!(answer("1/0"), None);
    }

    #[test]
    fn units() {
        assert_eq!(answer("5 km in mi").map(|(s, _)| s).as_deref(), Some("3.106855961 mi"));
        assert_eq!(copied("100 c to f").as_deref(), Some("212"));
        assert_eq!(copied("32 f in c").as_deref(), Some("0"));
        assert_eq!(copied("2 gb in mb").as_deref(), Some("2000"));
        assert_eq!(copied("90 min in h").as_deref(), Some("1.5"));
        assert_eq!(copied("1 lb in kg").as_deref(), Some("0.45359237"));
        assert_eq!(answer("5 km in kg"), None);
    }
}

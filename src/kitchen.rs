//! Kitchen conversions, for Quick Open: cups and spoons to grams and back,
//! for what's measured ("1 cup flour in grams", "200 g butter in cups",
//! "2 tbsp sugar", "½ cup of honey in g"): each weighs its own (a cup of
//! flour 120 g, of sugar 200 g), by the weights bakers go by. Without an
//! ingredient it's the calculator's ("1 cup in ml").

/// US measures, in milliliters; weights, in grams.
const VOLUMES: &[(&[&str], f64, &str)] = &[
    (&["cup", "cups", "c"], 236.588, "cup"),
    (&["tablespoon", "tablespoons", "tbsp", "tbs", "tbl"], 14.787, "tbsp"),
    (&["teaspoon", "teaspoons", "tsp"], 4.929, "tsp"),
    (&["ml", "milliliter", "milliliters", "millilitre", "millilitres"], 1., "ml"),
    (&["l", "liter", "liters", "litre", "litres"], 1000., "l"),
    (&["fl oz", "fluid ounce", "fluid ounces"], 29.574, "fl oz"),
    (&["pint", "pints"], 473.176, "pint"),
    (&["quart", "quarts"], 946.353, "quart"),
];
const WEIGHTS: &[(&[&str], f64, &str)] = &[
    (&["g", "gram", "grams", "gr"], 1., "g"),
    (&["kg", "kilo", "kilos", "kilogram", "kilograms"], 1000., "kg"),
    (&["oz", "ounce", "ounces"], 28.3495, "oz"),
    (&["lb", "lbs", "pound", "pounds"], 453.592, "lb"),
];

/// What's measured, and grams to a US cup (King Arthur's, mostly).
const INGREDIENTS: &[(&[&str], f64)] = &[
    (&["flour", "all-purpose flour", "all purpose flour", "plain flour", "bread flour", "cake flour", "self-rising flour", "self raising flour"], 120.),
    (&["whole wheat flour", "wholemeal flour"], 113.),
    (&["almond flour", "ground almonds"], 96.),
    (&["sugar", "granulated sugar", "white sugar", "caster sugar"], 198.),
    (&["brown sugar"], 213.),
    (&["powdered sugar", "icing sugar", "confectioners sugar", "confectioners' sugar"], 113.),
    (&["butter"], 227.),
    (&["water"], 237.),
    (&["milk"], 242.),
    (&["cream", "heavy cream", "whipping cream"], 238.),
    (&["yogurt", "yoghurt", "greek yogurt"], 227.),
    (&["oil", "vegetable oil", "olive oil", "canola oil"], 200.),
    (&["honey"], 336.),
    (&["maple syrup", "syrup"], 312.),
    (&["rice", "white rice", "uncooked rice"], 198.),
    (&["oats", "rolled oats", "oatmeal"], 89.),
    (&["cocoa", "cocoa powder"], 84.),
    (&["chocolate chips"], 170.),
    (&["salt", "table salt"], 288.),
    (&["cornstarch", "corn starch", "cornflour"], 112.),
    (&["baking powder"], 192.),
    (&["baking soda"], 288.),
    (&["peanut butter"], 270.),
    (&["shredded cheese", "cheese", "grated cheese"], 113.),
    (&["parmesan", "grated parmesan"], 100.),
    (&["nuts", "chopped nuts", "walnuts", "pecans", "almonds"], 113.),
    (&["raisins"], 149.),
];

#[derive(Debug, PartialEq)]
struct Asked {
    amount: f64,
    /// Milliliters (a volume) or grams.
    each: f64,
    volume: bool,
    unit: &'static str,
    ingredient: &'static str,
    grams_a_cup: f64,
    /// What it's wanted in (None: grams, for a volume; cups, for a weight).
    into: Option<(f64, bool, &'static str)>,
}

/// "1", "1.5", "1/2", "1 1/2", "½", "a", "half a".
fn amount(words: &[&str]) -> Option<(f64, usize)> {
    let fraction = |w: &str| -> Option<f64> {
        match w {
            "½" => Some(0.5),
            "¼" => Some(0.25),
            "¾" => Some(0.75),
            "⅓" => Some(1. / 3.),
            "⅔" => Some(2. / 3.),
            "⅛" => Some(0.125),
            _ => {
                let (a, b) = w.split_once('/')?;
                let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
                (b != 0.).then(|| a / b)
            }
        }
    };
    let first = *words.first()?;
    if matches!(first, "a" | "an" | "one") {
        return Some((1., 1));
    }
    if first == "half" {
        return Some((0.5, if words.get(1).is_some_and(|w| matches!(*w, "a" | "an")) { 2 } else { 1 }));
    }
    // ("1½": a number and a fraction, together.)
    if let Some(at) = first.find(['½', '¼', '¾', '⅓', '⅔', '⅛']).filter(|at| *at > 0) {
        return Some((first[..at].parse::<f64>().ok()? + fraction(&first[at..])?, 1));
    }
    let whole = first.parse::<f64>().ok().or_else(|| fraction(first))?;
    match words.get(1).and_then(|w| fraction(w)) {
        Some(part) if first.parse::<f64>().is_ok() && !first.contains('.') => Some((whole + part, 2)),
        _ => Some((whole, 1)),
    }
}

/// A unit at the start of `words` ("cup", "fl oz"): ml or g each, whether
/// it's a volume, its short name, and how many words it took.
fn unit(words: &[&str]) -> Option<(f64, bool, &'static str, usize)> {
    let two = words.get(..2).map(|w| w.join(" "));
    for (list, volume) in [(VOLUMES, true), (WEIGHTS, false)] {
        for (names, each, short) in list {
            if let Some(two) = &two
                && names.contains(&two.as_str())
            {
                return Some((*each, volume, short, 2));
            }
            if names.contains(words.first()?) {
                return Some((*each, volume, short, 1));
            }
        }
    }
    None
}

fn asked(q: &str) -> Option<Asked> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    // "100g" as "100 g".
    let mut spaced = String::new();
    let mut before = ' ';
    for c in lower.chars() {
        if c.is_ascii_alphabetic() && before.is_ascii_digit() {
            spaced.push(' ');
        }
        spaced.push(c);
        before = c;
    }
    let (what, into) = match spaced.rsplit_once(" in ").or_else(|| spaced.rsplit_once(" to ")).or_else(|| spaced.rsplit_once(" into ")) {
        Some((what, into)) => (what.to_string(), Some(into.trim().to_string())),
        None => (spaced.clone(), None),
    };
    let words: Vec<&str> = what.split_whitespace().collect();
    let (amount, used) = amount(&words)?;
    let (each, volume, unit_name, took) = unit(&words[used..])?;
    let rest: Vec<&str> = words[used + took..].iter().copied().skip_while(|w| *w == "of").collect();
    let named = rest.join(" ");
    let (names, grams_a_cup) = INGREDIENTS.iter().find(|(names, _)| names.contains(&named.as_str()))?;
    let into = match into {
        Some(into) => {
            let into_words: Vec<&str> = into.split_whitespace().collect();
            let (each, volume, short, took) = unit(&into_words)?;
            (took == into_words.len()).then_some((each, volume, short))?
        }
        None => return Some(Asked { amount, each, volume, unit: unit_name, ingredient: names[0], grams_a_cup: *grams_a_cup, into: None }),
    };
    Some(Asked { amount, each, volume, unit: unit_name, ingredient: names[0], grams_a_cup: *grams_a_cup, into: Some(into) })
}

/// What a measure half asked about still wants, with an example: "1 cup
/// flour in" a unit, "1 cup of" what's measured.
pub fn wanting(q: &str) -> Option<String> {
    let lower = q.trim().to_lowercase();
    for into in [" in", " to", " into"] {
        if let Some(head) = lower.strip_suffix(into)
            && let Some(a) = asked(head)
        {
            return Some(format!("Type a unit: “{head}{into} {}”", if a.volume { "grams" } else { "cups" }));
        }
    }
    let words: Vec<&str> = lower.split_whitespace().collect();
    let (_, used) = amount(&words)?;
    let (.., took) = unit(&words[used..])?;
    (words[used + took..] == ["of"]).then(|| format!("Type what's measured: “{lower} flour”"))
}

/// "1 ½", "¾", "2", "0.4": cups and spoons as recipes say them.
fn measure(n: f64) -> String {
    const PARTS: &[(f64, &str)] = &[(0., ""), (0.125, "⅛"), (0.25, "¼"), (1. / 3., "⅓"), (0.375, "⅜"), (0.5, "½"), (0.625, "⅝"), (2. / 3., "⅔"), (0.75, "¾"), (0.875, "⅞"), (1., "")];
    let whole = n.trunc();
    let part = n - whole;
    let (value, glyph) = PARTS.iter().min_by(|a, b| (a.0 - part).abs().total_cmp(&(b.0 - part).abs())).copied().unwrap_or((0., ""));
    // (Not near any: as a number.)
    if (value - part).abs() > 0.05 {
        return format!("{:.1}", n).trim_end_matches(".0").to_string();
    }
    let whole = whole + if value == 1. { 1. } else { 0. };
    match (whole as u64, glyph) {
        (0, "") => "0".into(),
        (0, g) => g.into(),
        (w, "") => w.to_string(),
        (w, g) => format!("{w} {g}"),
    }
}

/// How much, said: grams whole (tenths under 10), cups and spoons in
/// fractions.
fn said(value: f64, short: &str) -> String {
    let number = match short {
        "g" | "ml" if value >= 10. => format!("{}", value.round()),
        "g" | "ml" | "kg" | "l" | "oz" | "lb" | "fl oz" => format!("{:.1}", value).trim_end_matches(".0").to_string(),
        _ => measure(value),
    };
    let plural = matches!(short, "cup" | "pint" | "quart") && value > 1.;
    format!("{number} {short}{}", if plural { "s" } else { "" })
}

/// The answer: as shown, as copied, and what it's of.
pub fn answer(q: &str) -> Option<(String, String, String)> {
    let a = asked(q)?;
    let grams_a_ml = a.grams_a_cup / 236.588;
    let grams = if a.volume { a.amount * a.each * grams_a_ml } else { a.amount * a.each };
    let (each, volume, short) = a.into.unwrap_or(if a.volume {
        (1., false, "g")
    } else {
        // (A weight, as cups; a little, as spoons.)
        let cups = grams / grams_a_ml / 236.588;
        if cups >= 0.25 { (236.588, true, "cup") } else if cups * 16. >= 1. { (14.787, true, "tbsp") } else { (4.929, true, "tsp") }
    });
    let value = if volume { grams / grams_a_ml / each } else { grams / each };
    let shown = said(value, short);
    let copied = shown.split(' ').next().unwrap_or("").to_string();
    let about = format!("{} of {}{}", said(a.amount, a.unit), a.ingredient, if a.volume || volume { " · US measures" } else { "" });
    Some((shown, copied, about))
}

#[cfg(test)]
mod tests {
    use super::answer;

    fn shown(q: &str) -> Option<String> {
        answer(q).map(|(shown, _, about)| format!("{shown} | {about}"))
    }

    #[test]
    fn conversions() {
        assert_eq!(shown("1 cup flour in grams").as_deref(), Some("120 g | 1 cup of flour · US measures"));
        assert_eq!(shown("200 g butter in cups").as_deref(), Some("⅞ cup | 200 g of butter · US measures"));
        assert_eq!(shown("2 tbsp sugar").as_deref(), Some("25 g | 2 tbsp of sugar · US measures"));
        assert_eq!(shown("½ cup of honey in g").as_deref(), Some("168 g | ½ cup of honey · US measures"));
        assert_eq!(shown("1 1/2 cups milk in ml").as_deref(), Some("355 ml | 1 ½ cups of milk · US measures"));
        assert_eq!(shown("100g sugar").as_deref(), Some("½ cup | 100 g of sugar · US measures"));
        assert_eq!(shown("10 g salt").as_deref(), Some("1 ⅔ tsp | 10 g of salt · US measures"));
        assert_eq!(shown("a cup of rice in grams").as_deref(), Some("198 g | 1 cup of rice · US measures"));
        assert_eq!(shown("1 lb butter in cups").as_deref(), Some("2 cups | 1 lb of butter · US measures"));
        assert_eq!(shown("1 cup in ml"), None, "the calculator's");
        assert_eq!(shown("1 cup coffee"), None, "not known");
        assert_eq!(shown("flour"), None);
    }
}

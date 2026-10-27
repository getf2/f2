//! Shoe and clothing sizes, for Quick Open: "us 10 shoe in eu", "eu 39
//! shoes", "270 shoes in us" (Korea's millimeters), "women's 8 shoe", "us 8
//! dress in uk", "korean 66 in us", "medium shirt in korean". From the
//! usual charts (Nike's, for shoes): brands differ a little.

struct Table {
    what: &'static str,
    systems: &'static [&'static str],
    rows: &'static [&'static [&'static str]],
}

const MEN_SHOES: Table = Table {
    what: "Men's shoes",
    systems: &["US", "UK", "EU", "JP", "KR"],
    rows: &[
        &["6", "5.5", "38.5", "24", "240"],
        &["6.5", "6", "39", "24.5", "245"],
        &["7", "6", "40", "25", "250"],
        &["7.5", "6.5", "40.5", "25.5", "255"],
        &["8", "7", "41", "26", "260"],
        &["8.5", "7.5", "42", "26.5", "265"],
        &["9", "8", "42.5", "27", "270"],
        &["9.5", "8.5", "43", "27.5", "275"],
        &["10", "9", "44", "28", "280"],
        &["10.5", "9.5", "44.5", "28.5", "285"],
        &["11", "10", "45", "29", "290"],
        &["11.5", "10.5", "45.5", "29.5", "295"],
        &["12", "11", "46", "30", "300"],
        &["12.5", "11.5", "47", "30.5", "305"],
        &["13", "12", "47.5", "31", "310"],
        &["14", "13", "48.5", "32", "320"],
        &["15", "14", "49.5", "33", "330"],
    ],
};

const WOMEN_SHOES: Table = Table {
    what: "Women's shoes",
    systems: &["US", "UK", "EU", "JP", "KR"],
    rows: &[
        &["5", "2.5", "35.5", "22", "220"],
        &["5.5", "3", "36", "22.5", "225"],
        &["6", "3.5", "36.5", "23", "230"],
        &["6.5", "4", "37.5", "23.5", "235"],
        &["7", "4.5", "38", "24", "240"],
        &["7.5", "5", "38.5", "24.5", "245"],
        &["8", "5.5", "39", "25", "250"],
        &["8.5", "6", "40", "25.5", "255"],
        &["9", "6.5", "40.5", "26", "260"],
        &["9.5", "7", "41", "26.5", "265"],
        &["10", "7.5", "42", "27", "270"],
        &["10.5", "8", "42.5", "27.5", "275"],
        &["11", "8.5", "43", "28", "280"],
        &["12", "9.5", "44.5", "29", "290"],
    ],
};

const WOMEN_CLOTHES: Table = Table {
    what: "Women's clothes",
    systems: &["Size", "US", "UK", "EU", "JP", "KR"],
    rows: &[
        &["XXS", "0", "4", "30", "3", "44"],
        &["XS", "2", "6", "32", "5", "44"],
        &["S", "4", "8", "34", "7", "55"],
        &["S", "6", "10", "36", "9", "55"],
        &["M", "8", "12", "38", "11", "66"],
        &["M", "10", "14", "40", "13", "66"],
        &["L", "12", "16", "42", "15", "77"],
        &["L", "14", "18", "44", "17", "77"],
        &["XL", "16", "20", "46", "19", "88"],
    ],
};

const MEN_CLOTHES: Table = Table {
    what: "Men's tops",
    systems: &["Size", "EU", "KR"],
    rows: &[&["XS", "44", "85"], &["S", "46", "90"], &["M", "48", "95"], &["L", "50", "100"], &["XL", "52", "105"], &["XXL", "54", "110"], &["3XL", "56", "115"]],
};

/// The system a word names.
fn system(word: &str) -> Option<&'static str> {
    Some(match word {
        "us" | "usa" | "american" => "US",
        "uk" | "british" => "UK",
        "eu" | "eur" | "euro" | "european" => "EU",
        "jp" | "japan" | "japanese" | "cm" => "JP",
        "kr" | "korea" | "korean" | "mm" => "KR",
        _ => return None,
    })
}

/// A letter size a word is.
fn letter(word: &str) -> Option<&'static str> {
    Some(match word {
        "xxs" => "XXS",
        "xs" | "extra small" => "XS",
        "s" | "small" => "S",
        "m" | "medium" => "M",
        "l" | "large" => "L",
        "xl" | "extra large" => "XL",
        "xxl" | "2xl" => "XXL",
        "xxxl" | "3xl" => "3XL",
        _ => return None,
    })
}

const SHOES: &[&str] = &["shoe", "shoes", "sneaker", "sneakers", "boot", "boots", "heels", "trainers"];
const WOMENS_CLOTHES: &[&str] = &["dress", "dresses", "skirt", "skirts", "blouse", "blouses"];
const CLOTHES: &[&str] = &["shirt", "shirts", "t-shirt", "tshirt", "tee", "top", "tops", "jacket", "jackets", "coat", "sweater", "hoodie", "clothes", "clothing", "dress", "dresses", "skirt", "skirts", "blouse", "blouses"];
const WOMEN: &[&str] = &["women", "women's", "womens", "woman", "ladies", "female", "w"];
const MEN: &[&str] = &["men", "men's", "mens", "man", "male"];

/// A line of the answer: as shown, as copied, and the rest of the chart.
pub type Line = (String, String, String);

pub fn answer(q: &str) -> Option<Vec<Line>> {
    let lower = q.trim().trim_end_matches('?').to_lowercase().replace("’", "'");
    let (what, into) = match lower.rsplit_once(" in ").or_else(|| lower.rsplit_once(" to ")) {
        Some((what, into)) => (what.to_string(), Some(into.trim().trim_end_matches(" size").trim_end_matches(" sizes").to_string())),
        None => (lower.clone(), None),
    };
    // ("27cm", "270mm": the number, then its system.)
    let mut words: Vec<String> = Vec::new();
    for word in what.split_whitespace() {
        match word.find(|c: char| c.is_ascii_alphabetic()).filter(|at| *at > 0 && word[..*at].parse::<f64>().is_ok() && system(&word[*at..]).is_some()) {
            Some(at) => words.extend([word[..at].to_string(), word[at..].to_string()]),
            None => words.push(word.to_string()),
        }
    }
    let has = |list: &[&str]| words.iter().any(|w| list.contains(&w.as_str()));
    let shoes = has(SHOES);
    let clothes = has(CLOTHES);
    let sized = words.iter().any(|w| w == "size" || w == "sizes");
    if !shoes && !clothes && !(sized && into.is_some()) {
        return None;
    }
    let women = has(WOMEN) || (clothes && has(WOMENS_CLOTHES));
    let men = has(MEN) || (clothes && !women && !words.iter().any(|w| w == "top" || w == "tops"));
    // Everything else said must be a size, its system, or one of these.
    let known = |w: &str| system(w).is_some() || letter(w).is_some() || w.parse::<f64>().is_ok() || SHOES.contains(&w) || CLOTHES.contains(&w) || WOMEN.contains(&w) || MEN.contains(&w) || matches!(w, "size" | "sizes" | "a" | "for" | "of" | "what" | "is" | "my" | "convert" | "extra");
    if !words.iter().all(|w| known(w)) {
        return None;
    }
    // The size said, and in what.
    let mut said: Option<(Option<&'static str>, String)> = None;
    for (i, word) in words.iter().enumerate() {
        if let Ok(n) = word.parse::<f64>() {
            let near = [i.checked_sub(1).and_then(|j| words.get(j)), words.get(i + 1)].into_iter().flatten().find_map(|w| system(w));
            said = Some((near, format_number(n)));
            break;
        }
        if let Some(l) = letter(word) {
            said = Some((Some("Size"), l.to_string()));
            break;
        }
    }
    let (said_system, value) = said?;
    let tables: Vec<&Table> = if shoes || (!clothes && sized) {
        match (women, men) {
            (true, false) => vec![&WOMEN_SHOES],
            (false, true) => vec![&MEN_SHOES],
            _ => vec![&MEN_SHOES, &WOMEN_SHOES],
        }
    } else if women {
        vec![&WOMEN_CLOTHES]
    } else if men {
        vec![&MEN_CLOTHES]
    } else {
        vec![&WOMEN_CLOTHES, &MEN_CLOTHES]
    };
    let target = into.as_deref().and_then(|t| system(t).or_else(|| matches!(t, "letter" | "letters" | "us letter").then_some("Size")));
    if into.is_some() && target.is_none() {
        return None;
    }
    let lines: Vec<Line> = tables.into_iter().filter_map(|table| line(table, said_system, &value, target)).collect();
    (!lines.is_empty()).then_some(lines)
}

fn format_number(n: f64) -> String {
    if n.fract() == 0. { format!("{n:.0}") } else { format!("{n}") }
}

/// The system a size is in, when it isn't said: by what it could be.
fn guess(table: &Table, value: &str) -> Option<&'static str> {
    let n: f64 = value.parse().ok()?;
    let order: &[&str] = if table.systems.contains(&"Size") { &["KR", "US", "EU"] } else { &["KR", "EU", "US", "JP"] };
    order.iter().copied().filter(|s| table.systems.contains(s)).find(|s| {
        let column = table.systems.iter().position(|x| x == s).unwrap_or(0);
        let values: Vec<f64> = table.rows.iter().filter_map(|r| r[column].parse().ok()).collect();
        let (low, high) = values.iter().fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        n >= low - 0.5 && n <= high + 0.5
    })
}

fn line(table: &Table, said_system: Option<&'static str>, value: &str, target: Option<&'static str>) -> Option<Line> {
    // (A men's top's US and UK sizes are its letters.)
    let alias = |s: &'static str| if !table.systems.contains(&s) && matches!(s, "US" | "UK") && table.systems.contains(&"Size") { "Size" } else { s };
    let system = alias(said_system.or_else(|| guess(table, value))?);
    let column = table.systems.iter().position(|s| *s == system)?;
    let row = match value.parse::<f64>() {
        Ok(n) => table.rows.iter().filter(|r| r[column].parse::<f64>().is_ok()).min_by(|a, b| {
            let d = |r: &&&[&str]| (r[column].parse::<f64>().unwrap_or(f64::MAX) - n).abs();
            d(a).total_cmp(&d(b))
        })?,
        Err(_) => table.rows.iter().find(|r| r[column] == value)?,
    };
    // (Too far off the chart: not a size of these.)
    if let (Ok(n), Ok(found)) = (value.parse::<f64>(), row[column].parse::<f64>())
        && (n - found).abs() > 1.
    {
        return None;
    }
    let named = |i: usize| -> String {
        match table.systems[i] {
            "Size" => row[i].to_string(),
            "JP" if table.what.ends_with("shoes") => format!("{} cm", row[i]),
            "KR" if table.what.ends_with("shoes") => format!("{} mm", row[i]),
            s => format!("{s} {}", row[i]),
        }
    };
    let all: Vec<String> = (0..table.systems.len()).map(named).collect();
    let target = target.map(alias).and_then(|t| table.systems.iter().position(|s| *s == t));
    let (shown, copied) = match target {
        Some(t) => (named(t), row[t].to_string()),
        None => (all.join(" · "), all.join(" · ")),
    };
    let rest = format!("{} · {} · brands differ a little", table.what, all.join(" · "));
    Some((shown, copied, rest))
}

#[cfg(test)]
mod tests {
    use super::answer;

    fn shown(q: &str) -> Vec<String> {
        answer(q).unwrap_or_default().into_iter().map(|(shown, _, rest)| format!("{shown} | {rest}")).collect()
    }

    #[test]
    fn shoes() {
        assert_eq!(shown("us 10 shoe in eu"), ["EU 44 | Men's shoes · US 10 · UK 9 · EU 44 · 28 cm · 280 mm · brands differ a little", "EU 42 | Women's shoes · US 10 · UK 7.5 · EU 42 · 27 cm · 270 mm · brands differ a little"]);
        assert_eq!(shown("men's us 10 shoes in uk"), ["UK 9 | Men's shoes · US 10 · UK 9 · EU 44 · 28 cm · 280 mm · brands differ a little"]);
        assert_eq!(shown("270 shoes in us")[0], "US 9 | Men's shoes · US 9 · UK 8 · EU 42.5 · 27 cm · 270 mm · brands differ a little");
        assert_eq!(shown("women's 8 shoe")[0], "US 8 · UK 5.5 · EU 39 · 25 cm · 250 mm | Women's shoes · US 8 · UK 5.5 · EU 39 · 25 cm · 250 mm · brands differ a little");
        assert_eq!(shown("eu 39 shoes in us").len(), 2);
        assert_eq!(shown("27cm shoes in us")[0], "US 9 | Men's shoes · US 9 · UK 8 · EU 42.5 · 27 cm · 270 mm · brands differ a little");
        assert!(shown("shoes").is_empty());
        assert!(shown("shoe store near me").is_empty());
    }

    #[test]
    fn clothes() {
        assert_eq!(shown("us 8 dress in uk"), ["UK 12 | Women's clothes · M · US 8 · UK 12 · EU 38 · JP 11 · KR 66 · brands differ a little"]);
        assert_eq!(shown("korean 66 dress in us")[0], "US 8 | Women's clothes · M · US 8 · UK 12 · EU 38 · JP 11 · KR 66 · brands differ a little");
        assert_eq!(shown("medium shirt in korean"), ["KR 95 | Men's tops · M · EU 48 · KR 95 · brands differ a little"]);
        assert_eq!(shown("kr 100 shirt in us"), ["L | Men's tops · L · EU 50 · KR 100 · brands differ a little"]);
        assert!(shown("dress code").is_empty());
    }
}

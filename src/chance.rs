//! Leaving it to chance, for Quick Open: "flip a coin", "roll a die" ("roll
//! d20", "roll 2d6"), "random number 1 to 100", "pick sushi, pizza or
//! tacos". Each time it's typed (or Return on "Again"), a new one.

#[derive(Debug, PartialEq)]
pub enum Chance {
    Coin,
    Dice { count: u32, sides: u32 },
    Number { low: i64, high: i64 },
    Pick(Vec<String>),
}

pub fn asked(q: &str) -> Option<Chance> {
    let lower = q.trim().trim_end_matches(['?', '!', '.']).to_lowercase();
    let lower = lower.trim();
    const COIN: &[&str] = &["flip a coin", "flip coin", "coin flip", "flip the coin", "toss a coin", "coin toss", "heads or tails"];
    if COIN.contains(&lower) {
        return Some(Chance::Coin);
    }
    // Dice: "roll a die", "roll 2 dice", "roll d20", "roll 2d6", "d20".
    let dice = ["roll the dice", "roll a dice", "roll a die", "roll dice", "roll die", "dice", "die roll", "dice roll"];
    if dice.contains(&lower) {
        return Some(Chance::Dice { count: 1, sides: 6 });
    }
    let rest = lower.strip_prefix("roll ").unwrap_or(lower);
    if let Some((count, sides)) = rest.split_once('d').filter(|(c, s)| (c.is_empty() || c.chars().all(|c| c.is_ascii_digit())) && !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())) {
        let count = if count.is_empty() { 1 } else { count.parse().ok()? };
        let sides = sides.parse().ok()?;
        return ((1..=20).contains(&count) && (2..=1000).contains(&sides)).then_some(Chance::Dice { count, sides });
    }
    if let Some(count) = lower.strip_prefix("roll ").and_then(|r| r.strip_suffix(" dice")).and_then(|c| c.trim().parse().ok()).filter(|c| (1..=20).contains(c)) {
        return Some(Chance::Dice { count, sides: 6 });
    }
    if matches!(lower, "random number" | "pick a number") {
        return Some(Chance::Number { low: 1, high: 100 });
    }
    // A number: "random number", "pick a number between 1 and 10",
    // "random 1-6", "random number 1 to 100".
    for lead in ["random number between ", "random number from ", "random number ", "pick a number between ", "pick a number from ", "pick a number ", "random between ", "random "] {
        if let Some(range) = lower.strip_prefix(lead) {
            let range = range.replace(" and ", " ").replace(" to ", " ").replace('-', " ");
            let numbers: Vec<i64> = range.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            let words = range.split_whitespace().count();
            return match numbers[..] {
                [high] if words == 1 && high > 1 => Some(Chance::Number { low: 1, high }),
                [low, high] if words == 2 && low < high => Some(Chance::Number { low, high }),
                _ => None,
            };
        }
    }
    // One of some: "pick sushi, pizza or tacos", "choose between a and b",
    // "pick one: a, b, c", "decide pizza or sushi".
    for lead in ["pick one of ", "pick one: ", "pick one ", "pick between ", "pick from ", "pick ", "choose between ", "choose one of ", "choose one: ", "choose from ", "choose ", "decide between ", "decide ", "help me choose "] {
        if lower.starts_with(lead) {
            let and = lead.contains("between");
            let mut options: Vec<String> = Vec::new();
            for part in q.trim().trim_end_matches(['?', '!', '.'])[lead.len()..].split(',') {
                let part = part.trim();
                // ("a, b or c", "a or b", "between a and b".)
                let mut pieces = vec![part.to_string()];
                for joiner in [" or ", " Or ", " OR ", " and ", " And "] {
                    if joiner.trim().eq_ignore_ascii_case("and") && !and {
                        continue;
                    }
                    pieces = pieces.iter().flat_map(|p| p.split(joiner).map(str::to_string).collect::<Vec<_>>()).collect();
                }
                options.extend(pieces.into_iter().map(|p| p.trim().trim_start_matches("or ").to_string()).filter(|p| !p.is_empty()));
            }
            return (options.len() >= 2 && options.len() <= 20).then_some(Chance::Pick(options));
        }
    }
    None
}

/// A number from 0 till `n`.
fn below(n: u64) -> u64 {
    if n <= u64::from(u32::MAX) {
        u64::from(unsafe { libc::arc4random_uniform(n as u32) })
    } else {
        let mut bytes = [0u8; 8];
        unsafe { libc::arc4random_buf(bytes.as_mut_ptr().cast(), 8) };
        u64::from_le_bytes(bytes) % n
    }
}

/// How it came out: as shown, and what was done.
pub fn outcome(chance: &Chance) -> (String, String) {
    match chance {
        Chance::Coin => ((if below(2) == 0 { "Heads" } else { "Tails" }).into(), "A coin flipped".into()),
        Chance::Dice { count, sides } => {
            let rolls: Vec<u64> = (0..*count).map(|_| below(u64::from(*sides)) + 1).collect();
            let shown = rolls.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
            let what = if *count == 1 { if *sides == 6 { "A die rolled".to_string() } else { format!("A {sides}-sided die rolled") } } else { format!("{count} {sides}-sided dice rolled") };
            let total: u64 = rolls.iter().sum();
            let about = if *count > 1 { format!("{what} · {total} in all") } else { what };
            (shown, about)
        }
        Chance::Number { low, high } => {
            let n = *low + below((*high - *low + 1) as u64) as i64;
            (n.to_string(), format!("A random number from {low} to {high}"))
        }
        Chance::Pick(options) => {
            let chosen = &options[below(options.len() as u64) as usize];
            let mut chosen_shown = chosen.clone();
            if let Some(first) = chosen_shown.get(..1) {
                chosen_shown = first.to_uppercase() + &chosen_shown[1..];
            }
            (chosen_shown, format!("Out of {}", options.join(", ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chance, asked, outcome};

    #[test]
    fn asking() {
        assert_eq!(asked("Flip a coin"), Some(Chance::Coin));
        assert_eq!(asked("heads or tails?"), Some(Chance::Coin));
        assert_eq!(asked("roll a die"), Some(Chance::Dice { count: 1, sides: 6 }));
        assert_eq!(asked("roll d20"), Some(Chance::Dice { count: 1, sides: 20 }));
        assert_eq!(asked("roll 2d6"), Some(Chance::Dice { count: 2, sides: 6 }));
        assert_eq!(asked("roll 3 dice"), Some(Chance::Dice { count: 3, sides: 6 }));
        assert_eq!(asked("random number"), Some(Chance::Number { low: 1, high: 100 }));
        assert_eq!(asked("random number 1 to 10"), Some(Chance::Number { low: 1, high: 10 }));
        assert_eq!(asked("pick a number between 5 and 50"), Some(Chance::Number { low: 5, high: 50 }));
        assert_eq!(asked("random 1-6"), Some(Chance::Number { low: 1, high: 6 }));
        assert_eq!(asked("pick sushi, pizza or tacos"), Some(Chance::Pick(vec!["sushi".into(), "pizza".into(), "tacos".into()])));
        assert_eq!(asked("choose between Rust and Go"), Some(Chance::Pick(vec!["Rust".into(), "Go".into()])));
        assert_eq!(asked("decide pizza or sushi"), Some(Chance::Pick(vec!["pizza".into(), "sushi".into()])));
        assert_eq!(asked("pick salt and pepper or nothing"), Some(Chance::Pick(vec!["salt and pepper".into(), "nothing".into()])));
        assert_eq!(asked("pick"), None);
        assert_eq!(asked("pick up groceries"), None);
        assert_eq!(asked("random wallpaper"), None);
        assert_eq!(asked("diet"), None);
        assert_eq!(asked("dad"), None);
        assert_eq!(asked("rolling stones"), None);
    }

    #[test]
    fn outcomes() {
        for _ in 0..50 {
            let (n, _) = outcome(&Chance::Number { low: 1, high: 6 });
            assert!((1..=6).contains(&n.parse::<i64>().unwrap()));
            assert!(["Heads", "Tails"].contains(&outcome(&Chance::Coin).0.as_str()));
        }
        assert!(outcome(&Chance::Dice { count: 2, sides: 6 }).1.ends_with("in all"));
        assert!(["Sushi", "Pizza"].contains(&outcome(&Chance::Pick(vec!["sushi".into(), "pizza".into()])).0.as_str()));
    }
}

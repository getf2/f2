//! Passwords from Quick Open: "generate password" (or Generate Password,
//! found by "password") makes a strong one, as Safari's are (three groups
//! of six, easy to type on a phone), and a random one of letters, digits
//! and symbols; "password 32" that long; "generate pin" six digits.
//! Copied out of clipboard histories' way (F2's too), and off the clipboard
//! again after a while (see `macos::copy_secret`).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// "abcdef-ghijkl-mnopqr", one capital and one digit in it.
    Strong,
    /// So many characters of letters, digits and symbols.
    Random(usize),
    /// So many digits.
    Pin(usize),
}

/// What's asked for, if a password's asked to be made: "generate password",
/// "new password", "password 32", "generate pin". ("password" alone isn't:
/// it's the command that makes one, so a password isn't just shown.)
pub fn asked(q: &str) -> Option<Vec<Kind>> {
    let lower = q.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    const ASKING: &[&str] = &["generate", "create", "make", "new", "random", "strong", "secure"];
    const FILLER: &[&str] = &["a", "me", "of", "character", "characters", "chars", "long", "digit", "digits", "code"];
    let number = words.iter().find_map(|w| w.parse::<usize>().ok());
    let asking = number.is_some() || words.iter().any(|w| ASKING.contains(w)) || lower == "password generator";
    // (Only the words of asking: "password manager" is an app, "pin
    // screenshot" a command.)
    let what: Vec<&str> = words.iter().copied().filter(|w| !ASKING.contains(w) && !FILLER.contains(w) && w.parse::<usize>().is_err()).collect();
    if !asking {
        return None;
    }
    match what.as_slice() {
        ["pin"] => Some(vec![Kind::Pin(number.unwrap_or(6).clamp(4, 12))]),
        ["password" | "passwords" | "passcode"] | ["password", "generator"] => Some(match number {
            Some(n) => vec![Kind::Random(n.clamp(8, 128))],
            None => vec![Kind::Strong, Kind::Random(20)],
        }),
        _ => None,
    }
}

fn below(n: usize) -> usize {
    (unsafe { libc::arc4random_uniform(n as u32) }) as usize
}

fn pick(from: &[u8]) -> char {
    from[below(from.len())] as char
}

const LOWER: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITS: &[u8] = b"23456789";
const SYMBOLS: &[u8] = b"!@#$%^&*-_=+?";

/// A new one of the kind.
pub fn make(kind: Kind) -> String {
    match kind {
        Kind::Strong => {
            let mut chars: Vec<char> = (0..18).map(|_| pick(LOWER)).collect();
            let capital = below(18);
            let digit = (capital + 1 + below(17)) % 18;
            chars[capital] = pick(UPPER);
            chars[digit] = pick(DIGITS);
            chars.chunks(6).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
        }
        Kind::Random(n) => loop {
            let all: Vec<u8> = [LOWER, UPPER, DIGITS, SYMBOLS].concat();
            let made: String = (0..n).map(|_| pick(&all)).collect();
            // (One of each kind of character.)
            let has = |set: &[u8]| made.bytes().any(|b| set.contains(&b));
            if [LOWER, UPPER, DIGITS, SYMBOLS].iter().all(|set| has(set)) {
                break made;
            }
        },
        Kind::Pin(n) => (0..n).map(|_| pick(b"0123456789")).collect(),
    }
}

/// What it's called, under it.
pub fn about(kind: Kind) -> String {
    match kind {
        Kind::Strong => "Strong, and easy to type on a phone".into(),
        Kind::Random(n) => format!("{n} letters, digits and symbols"),
        Kind::Pin(n) => format!("A {n}-digit PIN"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, asked, make};

    #[test]
    fn asking() {
        assert_eq!(asked("password"), None, "the command that makes one, not one shown");
        assert_eq!(asked("generate password"), Some(vec![Kind::Strong, Kind::Random(20)]));
        assert_eq!(asked("create a strong password"), Some(vec![Kind::Strong, Kind::Random(20)]));
        assert_eq!(asked("new password"), Some(vec![Kind::Strong, Kind::Random(20)]));
        assert_eq!(asked("password 32"), Some(vec![Kind::Random(32)]));
        assert_eq!(asked("pin"), None);
        assert_eq!(asked("generate pin"), Some(vec![Kind::Pin(6)]));
        assert_eq!(asked("pin 4"), Some(vec![Kind::Pin(4)]));
        assert_eq!(asked("new password manager"), None);
        assert_eq!(asked("new pin screenshot"), None);
        assert_eq!(asked("passport"), None);
    }

    #[test]
    fn made() {
        let strong = make(Kind::Strong);
        assert_eq!(strong.len(), 20);
        assert_eq!(strong.matches('-').count(), 2);
        assert!(strong.chars().any(|c| c.is_ascii_uppercase()) && strong.chars().any(|c| c.is_ascii_digit()));
        let random = make(Kind::Random(24));
        assert_eq!(random.chars().count(), 24);
        assert_ne!(make(Kind::Random(24)), random, "new each time");
        assert!(make(Kind::Pin(6)).chars().all(|c| c.is_ascii_digit()));
    }
}

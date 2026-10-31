//! Korean as 2-Set types it: a key is a jamo (by where it is on the
//! keyboard, as macOS's 2-Set has it), and jamo make syllables as they're
//! typed: ㅇ ㅏ ㄴ is 안, and ㄴ ㅕ after it moves on, 안녀. Double
//! consonants take ⇧ (ㄲ), vowels pair (ㅗ ㅏ is ㅘ) and so do finals (ㄹ ㄱ
//! is ㄺ).

const CHO: [char; 19] = ['ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ'];
const JUNG: [char; 21] = ['ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ'];
/// Finals, after none.
const JONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];
const VOWELS: [(char, char, char); 7] = [('ㅗ', 'ㅏ', 'ㅘ'), ('ㅗ', 'ㅐ', 'ㅙ'), ('ㅗ', 'ㅣ', 'ㅚ'), ('ㅜ', 'ㅓ', 'ㅝ'), ('ㅜ', 'ㅔ', 'ㅞ'), ('ㅜ', 'ㅣ', 'ㅟ'), ('ㅡ', 'ㅣ', 'ㅢ')];
const FINALS: [(char, char, char); 11] = [
    ('ㄱ', 'ㅅ', 'ㄳ'),
    ('ㄴ', 'ㅈ', 'ㄵ'),
    ('ㄴ', 'ㅎ', 'ㄶ'),
    ('ㄹ', 'ㄱ', 'ㄺ'),
    ('ㄹ', 'ㅁ', 'ㄻ'),
    ('ㄹ', 'ㅂ', 'ㄼ'),
    ('ㄹ', 'ㅅ', 'ㄽ'),
    ('ㄹ', 'ㅌ', 'ㄾ'),
    ('ㄹ', 'ㅍ', 'ㄿ'),
    ('ㄹ', 'ㅎ', 'ㅀ'),
    ('ㅂ', 'ㅅ', 'ㅄ'),
];

/// The jamo 2-Set has on a key (its virtual key code), ⇧ for ㄲ ㄸ ㅃ ㅆ
/// ㅉ ㅒ ㅖ.
pub fn jamo(code: u16, shift: bool) -> Option<char> {
    let (plain, shifted) = match code {
        12 => ('ㅂ', 'ㅃ'),
        13 => ('ㅈ', 'ㅉ'),
        14 => ('ㄷ', 'ㄸ'),
        15 => ('ㄱ', 'ㄲ'),
        17 => ('ㅅ', 'ㅆ'),
        31 => ('ㅐ', 'ㅒ'),
        35 => ('ㅔ', 'ㅖ'),
        _ => {
            let jamo = match code {
                0 => 'ㅁ',
                1 => 'ㄴ',
                2 => 'ㅇ',
                3 => 'ㄹ',
                4 => 'ㅗ',
                5 => 'ㅎ',
                6 => 'ㅋ',
                7 => 'ㅌ',
                8 => 'ㅊ',
                9 => 'ㅍ',
                11 => 'ㅠ',
                16 => 'ㅛ',
                32 => 'ㅕ',
                34 => 'ㅑ',
                37 => 'ㅣ',
                38 => 'ㅓ',
                40 => 'ㅏ',
                45 => 'ㅜ',
                46 => 'ㅡ',
                _ => return None,
            };
            (jamo, jamo)
        }
    };
    Some(if shift { shifted } else { plain })
}

/// What's typed: a jamo, or something else (punctuation, a digit), as it
/// is.
pub enum Piece<'a> {
    Jamo(char),
    Other(&'a str),
}

/// A syllable being made.
#[derive(Default)]
struct Syllable {
    cho: Option<char>,
    jung: Option<char>,
    jong: Option<char>,
}

impl Syllable {
    /// It, typed: whole, or its jamo alone (and that it isn't whole).
    fn flush(&mut self, out: &mut String) -> bool {
        let s = std::mem::take(self);
        match (s.cho, s.jung) {
            (Some(cho), Some(jung)) => {
                let i = CHO.iter().position(|c| *c == cho).unwrap_or(0) as u32;
                let m = JUNG.iter().position(|c| *c == jung).unwrap_or(0) as u32;
                let f = s.jong.and_then(|j| JONG.iter().position(|c| *c == j)).map_or(0, |f| f as u32 + 1);
                out.extend(char::from_u32(0xAC00 + (i * 21 + m) * 28 + f));
                true
            }
            (None, None) => true,
            (cho, jung) => {
                out.extend(cho.or(jung));
                false
            }
        }
    }
}

/// What `pieces` type in 2-Set: jamo made syllables, the rest as it is;
/// and whether every syllable's whole (no jamo left alone, as ㅋㅋ).
pub fn compose<'a>(pieces: impl IntoIterator<Item = Piece<'a>>) -> (String, bool) {
    let mut out = String::new();
    let mut whole = true;
    let mut s = Syllable::default();
    for piece in pieces {
        let j = match piece {
            Piece::Jamo(j) => j,
            Piece::Other(text) => {
                whole &= s.flush(&mut out);
                out.push_str(text);
                continue;
            }
        };
        if CHO.contains(&j) {
            let pair = s.jong.and_then(|f| FINALS.iter().find(|(a, b, _)| *a == f && *b == j)).map(|p| p.2);
            match (s.cho, s.jung, s.jong) {
                (Some(_), Some(_), None) if JONG.contains(&j) => s.jong = Some(j),
                (Some(_), Some(_), Some(_)) if pair.is_some() => s.jong = pair,
                _ => {
                    whole &= s.flush(&mut out);
                    s.cho = Some(j);
                }
            }
        } else {
            let pair = s.jung.and_then(|v| VOWELS.iter().find(|(a, b, _)| *a == v && *b == j)).map(|p| p.2);
            match (s.cho, s.jung, s.jong) {
                (Some(_), None, _) => s.jung = Some(j),
                (_, Some(_), None) if pair.is_some() => s.jung = pair,
                (Some(_), Some(_), Some(f)) => {
                    // (The final moves on to the vowel: 안 ㅕ is 아녀; 닭 ㅣ
                    // is 달기.)
                    let (keep, moved) = FINALS.iter().find(|(_, _, pair)| *pair == f).map_or((None, f), |&(a, b, _)| (Some(a), b));
                    s.jong = keep;
                    whole &= s.flush(&mut out);
                    s.cho = Some(moved);
                    s.jung = Some(j);
                }
                _ => {
                    whole &= s.flush(&mut out);
                    s.jung = Some(j);
                }
            }
        }
    }
    whole &= s.flush(&mut out);
    (out, whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As typed on ABC's letters (where 2-Set's are).
    fn typed(keys: &str) -> (String, bool) {
        const CODES: &str = "asdfhgzxcv_bqweryt";
        let code = |c: char| -> u16 {
            let c = c.to_ascii_lowercase();
            match c {
                'o' => 31,
                'u' => 32,
                'i' => 34,
                'p' => 35,
                'l' => 37,
                'j' => 38,
                'k' => 40,
                'n' => 45,
                'm' => 46,
                _ => CODES.find(c).unwrap() as u16,
            }
        };
        compose(keys.chars().map(|c| if c.is_ascii_alphabetic() { Piece::Jamo(jamo(code(c), c.is_ascii_uppercase()).unwrap()) } else { Piece::Other(if c == ' ' { " " } else { "?" }) }))
    }

    #[test]
    fn syllables() {
        assert_eq!(typed("dkssudgktpdy"), ("안녕하세요".into(), true));
        assert_eq!(typed("rkatkgkqslek"), ("감사합니다".into(), true));
        assert_eq!(typed("ekfrl"), ("달기".into(), true), "ㄹㄱ, then a vowel: ㄱ moves on");
        assert_eq!(typed("ekfr"), ("닭".into(), true));
        assert_eq!(typed("Rmx"), ("끝".into(), true));
        assert_eq!(typed("ghk"), ("화".into(), true));
        assert_eq!(typed("dhkdy"), ("와요".into(), true), "ㅘ then ㅇ: 왕, and ㅛ moves it on");
        assert_eq!(typed("dhody"), ("왜요".into(), true));
        assert_eq!(typed("zz"), ("ㅋㅋ".into(), false));
        assert_eq!(typed("hello"), ("ㅗ디ㅣㅐ".into(), false));
        assert_eq!(typed("so"), ("내".into(), true));
    }
}

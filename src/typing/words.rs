//! The words Typing knows, to tell which language a word's in: every word
//! form Icelandic has (BÍN's, 3.7 million: "hestanna", "Sigrúnu"),
//! English's (SCOWL's, 115,000 with "don't" and "dog's"), and the 200,000
//! words said most in Korean (in films' and shows' subtitles, with how
//! often: "내가", "거야"). Each an FST (assets/typing/*.fst: 5.8 MB
//! together), looked up where it's built into F2, never read in whole.
//! scripts/typing-words.sh makes them again.

use fst::automaton::{Automaton, Str};
use fst::{IntoStreamer, Map, Set, Streamer};

use super::engine::Lang;

/// How a word reads in a language: nothing out of place in it (letters,
/// with punctuation only round them: quotes, a full stop), and it's a word
/// of that language's (each part of it, "Norður-Ameríka").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score {
    pub clean: bool,
    pub known: bool,
    /// Its letters.
    pub letters: usize,
}

impl Score {
    /// Clean, and known.
    pub fn word(&self) -> bool {
        self.clean && self.known
    }
}

/// What may come before a word, and after it ("src/").
const OPENING: &str = "([{\"'„“‘«‹¿¡*_`";
const CLOSING: &str = ".,;:!?)]}\"'”“’»›…*_`/";

/// Jamo typed alone that are a laugh or a sob: ㅋㅋ, ㅎㅎ, ㅠㅠ, ㅇㅇ.
const LAUGHS: &str = "ㅋㅎㅠㅜㄷㅇㄱ";

/// Whether jamo alone are a laugh or a sob (two or more): ㅋ and ㅎ
/// together, ㅠ and ㅜ, or one of ㄷ ㅇ ㄱ again and again. Not "ㅜㄷ"
/// ("done", typed as Korean).
fn is_laugh(jamo: &str) -> bool {
    let all = |set: &str| jamo.chars().all(|c| set.contains(c));
    let mut chars = jamo.chars();
    let first = chars.next();
    jamo.chars().count() >= 2 && (all("ㅋㅎ") || all("ㅠㅜ") || (first.is_some_and(|f| "ㄷㅇㄱ".contains(f)) && chars.all(|c| Some(c) == first)))
}

fn is_syllable(c: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&c)
}

fn is_jamo(c: char) -> bool {
    ('\u{3131}'..='\u{318E}').contains(&c)
}

pub struct Words {
    english: Set<&'static [u8]>,
    icelandic: Set<&'static [u8]>,
    korean: Map<&'static [u8]>,
}

impl Words {
    /// F2's own.
    pub fn bundled() -> Words {
        static ENGLISH: &[u8] = include_bytes!("../../assets/typing/english.fst");
        static ICELANDIC: &[u8] = include_bytes!("../../assets/typing/icelandic.fst");
        static KOREAN: &[u8] = include_bytes!("../../assets/typing/korean.fst");
        Words {
            english: Set::new(ENGLISH).expect("english.fst"),
            icelandic: Set::new(ICELANDIC).expect("icelandic.fst"),
            korean: Map::new(KOREAN).expect("korean.fst"),
        }
    }

    /// How often a Korean word's said (in the subtitles: "내" 33,774
    /// times), if it's one.
    pub fn said(&self, word: &str) -> Option<u64> {
        self.korean.get(word)
    }

    /// Whether `text` (Korean, with what's round it) is said often enough
    /// to be Korean against an English or Icelandic word: "내" for "so",
    /// not "랙" for "for". A laugh is.
    pub fn said_often(&self, text: &str) -> bool {
        korean_body(text).is_some_and(|(body, _)| body.is_empty() || self.said(body).is_some_and(|n| n >= super::engine::SAID_OFTEN))
    }

    /// Whether `word` is one of `lang`'s (any case; English's with "'s"
    /// on).
    pub fn has(&self, lang: Lang, word: &str) -> bool {
        let word = word.to_lowercase().replace('’', "'");
        match lang {
            Lang::Icelandic => self.icelandic.contains(&word),
            Lang::English => self.english.contains(&word) || word.strip_suffix("'s").is_some_and(|stem| self.english.contains(stem)),
            Lang::Korean => self.said(&word).is_some(),
        }
    }

    /// Whether an Icelandic word begins as `text` does (any case; quotes
    /// or brackets before it aside): "þe" does, "þc" and "srcþ" don't.
    pub fn begins(&self, text: &str) -> bool {
        let start = text.trim_start_matches(|c| OPENING.contains(c)).to_lowercase();
        !start.is_empty() && self.icelandic.search(Str::new(&start).starts_with()).into_stream().next().is_some()
    }

    /// How `text` (a word as typed, with what's round it) reads in `lang`.
    pub fn score(&self, text: &str, lang: Lang) -> Score {
        let letters = text.chars().filter(|c| c.is_alphabetic()).count();
        if lang == Lang::Korean {
            return self.korean_score(text, letters);
        }
        // (Hangul's no English or Icelandic.)
        if text.chars().any(|c| is_syllable(c) || is_jamo(c)) {
            return Score { clean: false, known: false, letters };
        }
        if is_address(text) {
            return Score { clean: true, known: true, letters };
        }
        let english = lang == Lang::English;
        let (Some(start), Some(end)) = (text.find(char::is_alphabetic), text.char_indices().rev().find(|(_, c)| c.is_alphabetic())) else {
            // (Punctuation alone, or digits: "-", "2-3".)
            return Score { clean: true, known: false, letters: 0 };
        };
        let end = end.0 + end.1.len_utf8();
        let (before, word, after) = (&text[..start], &text[start..end], &text[end..]);
        // (English's "-v", "--all", "#tag", "@name", "/compact".)
        let opened = before.chars().all(|c| OPENING.contains(c) || (english && "-—#@".contains(c))) || (english && before == "/");
        let mut clean = opened && after.chars().all(|c| CLOSING.contains(c));
        let mut known = true;
        // (Parts: "Norður-Ameríka", "t.d.", "og/eða".)
        let parts: &[char] = &['-', '.', '/'];
        for part in word.split(parts) {
            // (English's apostrophes inside: "don't", "o'clock".)
            let fine = part.starts_with(char::is_alphabetic)
                && part.ends_with(char::is_alphabetic)
                && part.chars().all(|c| c.is_alphabetic() || (english && (c == '\'' || c == '’')));
            if !fine {
                clean = false;
                known = false;
            } else if !self.has(lang, part) {
                known = false;
            }
        }
        Score { clean, known, letters }
    }

    /// How `text` reads in Korean: whole syllables (a laugh after them, or
    /// alone, too: ㅋㅋ), with digits or quotes before them, punctuation
    /// after.
    fn korean_score(&self, text: &str, letters: usize) -> Score {
        match korean_body(text) {
            Some((body, edges)) => Score { clean: edges, known: body.is_empty() || self.said(body).is_some(), letters },
            None => Score { clean: !text.chars().any(char::is_alphabetic), known: false, letters },
        }
    }
}

/// A Korean word's syllables (none, for a laugh alone: ㅋㅋ), and whether
/// what's round them is fine (digits or quotes before, punctuation after);
/// None if it isn't whole syllables, a laugh after them at most.
fn korean_body(text: &str) -> Option<(&str, bool)> {
    let hangul = |c: char| is_syllable(c) || is_jamo(c);
    let (start, end) = (text.find(hangul)?, text.char_indices().rev().find(|(_, c)| hangul(*c))?);
    let end = end.0 + end.1.len_utf8();
    let (before, word, after) = (&text[..start], &text[start..end], &text[end..]);
    let edges = before.chars().all(|c| OPENING.contains(c) || c.is_ascii_digit()) && after.chars().all(|c| CLOSING.contains(c));
    let body = word.trim_end_matches(|c| LAUGHS.contains(c));
    let laugh = &word[body.len()..];
    ((laugh.is_empty() || is_laugh(laugh)) && body.chars().all(is_syllable)).then_some((body, edges))
}

/// An email address or a web address, which isn't one language's.
fn is_address(text: &str) -> bool {
    let text = text.trim_end_matches(|c| CLOSING.contains(c));
    if text.contains("://") || text.starts_with("www.") {
        return true;
    }
    let Some((name, place)) = text.split_once('@') else { return false };
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || "._+-".contains(c))
        && place.contains('.')
        && !place.starts_with('.')
        && !place.ends_with('.')
        && place.chars().all(|c| c.is_alphanumeric() || ".-".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known() {
        let w = Words::bundled();
        for word in ["það", "Það", "ég", "hestanna", "Sigrúnu", "að", "fæ"] {
            assert!(w.has(Lang::Icelandic, word), "{word}");
        }
        for word in ["the", "don't", "dog's", "a", "I", "Iceland"] {
            assert!(w.has(Lang::English, word), "{word}");
        }
        assert!(!w.has(Lang::English, "takk"));
        assert!(!w.has(Lang::Icelandic, "the"));
    }

    #[test]
    fn scores() {
        let w = Words::bundled();
        let s = |text, lang| w.score(text, lang);
        assert!(s("það", Lang::Icelandic).word());
        assert!(s("„það?“", Lang::Icelandic).word());
        assert!(s("Norður-Ameríka", Lang::Icelandic).word());
        assert!(!s("/a[", Lang::English).clean, "a slash and a bracket round a word");
        assert!(s("/compact", Lang::English).word(), "a command");
        assert!(!s("/'in", Lang::English).clean);
        assert!(w.begins("þe") && w.begins("(Þe") && !w.begins("þc") && !w.begins("srcþ"));
        assert!(s("안녕", Lang::Korean).word() && s("안녕하세요?", Lang::Korean).word() && s("3시", Lang::Korean).clean);
        assert!(s("ㅋㅋㅋ", Lang::Korean).word() && s("좋아ㅋㅋ", Lang::Korean).word() && s("ㅠㅠ", Lang::Korean).word());
        assert!(!s("애ㅜㄷ", Lang::Korean).clean, "\"done\" as Korean: no laugh");
        assert!(!s("ㅗ디ㅣㅐ", Lang::Korean).clean && !s("ㅗ디ㅣㅐ", Lang::English).clean, "jamo alone: neither");
        assert!(!s("hello", Lang::Korean).clean);
        assert!(w.said("내").is_some_and(|n| n > 10_000) && w.said("랙").is_none_or(|n| n < 100));
        assert!(!s("f;ri", Lang::English).clean, "a semicolon inside");
        assert!(s("f;", Lang::English).word(), "after a word, as in code");
        assert!(s("don't", Lang::English).word());
        assert!(!s("don´t", Lang::Icelandic).clean, "an accent alone, inside");
        assert!(s("and/or", Lang::English).word());
        assert!(s("dagur@gmail.com", Lang::English).word());
        assert!(!s("dagur\"gmail.com", Lang::Icelandic).clean);
        assert_eq!(s(";", Lang::English), Score { clean: true, known: false, letters: 0 });
        assert!(!s("Gu[mundsd'ottir", Lang::English).word());
    }

    /// Builds assets/typing/*.fst from the lists scripts/typing-words.sh
    /// downloads and writes (in $TYPING_WORDS), one word a line (Korean's
    /// with a tab and how often it's said).
    #[test]
    #[ignore]
    fn build() {
        let dir = std::path::PathBuf::from(std::env::var("TYPING_WORDS").expect("TYPING_WORDS: run scripts/typing-words.sh"));
        let out = |name: &str| std::io::BufWriter::new(std::fs::File::create(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("assets/typing/{name}.fst"))).unwrap());
        for name in ["english", "icelandic"] {
            let list = std::fs::read_to_string(dir.join(format!("{name}.txt"))).unwrap();
            let mut words: Vec<&str> = list.lines().filter(|w| !w.is_empty()).collect();
            words.sort_unstable();
            words.dedup();
            let mut set = fst::SetBuilder::new(out(name)).unwrap();
            for word in words {
                set.insert(word).unwrap();
            }
            set.finish().unwrap();
        }
        let list = std::fs::read_to_string(dir.join("korean.txt")).unwrap();
        let mut words: Vec<(&str, u64)> = list.lines().filter_map(|l| l.split_once('\t')).map(|(w, n)| (w, n.parse().unwrap())).collect();
        words.sort_unstable();
        words.dedup_by(|a, b| a.0 == b.0);
        let mut map = fst::MapBuilder::new(out("korean")).unwrap();
        for (word, said) in words {
            map.insert(word, said).unwrap();
        }
        map.finish().unwrap();
    }
}

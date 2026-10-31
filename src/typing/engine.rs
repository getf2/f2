//! Typing's engine: keys in, what to type out. Keys type as on ABC, all
//! but those Icelandic has letters on: [ (ð), ; (æ), / (þ), - (ö), and '
//! then a vowel (á é í ó ú ý). As a word ends (Space, Return, Tab, ⌘↩),
//! it's read every way those can go, and typed the way that's a word:
//! "/a[" is "það", "and/or" and "src/main.rs" stay. The language of the
//! last word that said is the one words lean to: in Icelandic, those keys
//! show their letters as they're typed, where that begins an Icelandic
//! word ("þe", not "srcþ"); everything else is always ABC's. Delete just
//! after takes a change back; the shortcut switches by hand. Nothing here
//! touches macOS: src/typing.rs and src/macos/typing.m do.
//!
//! Korean, as 2-Set types it (`hangul`), is a third way to read every word:
//! "dkssud" is "안녕". In Korean, syllables are made as they're typed, and
//! a word that's English or Icelandic instead goes back as it ends. Each
//! of Icelandic and Korean can be off.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::hangul::{self, Piece};
use super::words::Words;

/// Modifiers, as layouts read keys with them (UCKeyTranslate's bits).
pub const SHIFT: u8 = 0x02;
pub const CAPS: u8 = 0x04;
pub const OPTION: u8 = 0x08;

pub const RETURN: u16 = 36;
pub const TAB: u16 = 48;
pub const SPACE: u16 = 49;
pub const DELETE: u16 = 51;
pub const ENTER: u16 = 76;

/// How long a switch by hand holds before words switch it again.
const PAUSE: Duration = Duration::from_secs(5);

/// How often a Korean word must be said to be one against an English or
/// Icelandic word ("so" is 내, said 33,774 times; "for" isn't 랙, said 6).
pub const SAID_OFTEN: u64 = 100;

/// Faces, typed as on ABC whatever language it is (";)" isn't "æ)").
const FACES: [&str; 24] = [
    ":)", ":-)", ";)", ";-)", ":(", ":-(", ":'(", ":D", ";D", ":-D", ":P", ";P", ":p", ";p", ":-P", ":/", ":O", ":o", "xD", "XD", ":3", "<3", "-_-", ":|",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    English,
    Icelandic,
    Korean,
}

impl Lang {
    /// In the order the shortcut goes through them.
    const ALL: [Lang; 3] = [Lang::English, Lang::Icelandic, Lang::Korean];
}

/// A key pressed: its virtual key code, and the modifiers it's read with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
    pub code: u16,
    pub mods: u8,
}

/// A keyboard layout as macOS reads keys with it (UCKeyTranslate): what a
/// key types, by the dead key waiting (0: none), the key and its
/// modifiers, and the dead key it leaves waiting.
#[derive(Default)]
pub struct Layout {
    keys: HashMap<(u32, u16, u8), (String, u32)>,
}

#[derive(serde::Deserialize)]
struct Table {
    keys: Vec<(u32, u16, u8, String, u32)>,
}

impl Layout {
    /// As src/macos/typing.m gives it.
    pub fn parse(json: &str) -> Option<Layout> {
        let table: Table = serde_json::from_str(json).ok()?;
        Some(Layout { keys: table.keys.into_iter().map(|(state, code, mods, text, next)| ((state, code, mods), (text, next))).collect() })
    }

    /// What `key` types by itself, and the dead key it leaves.
    fn alone(&self, key: Key) -> Option<&(String, u32)> {
        self.keys.get(&(0, key.code, key.mods))
    }
}

/// One letter, nothing else.
fn is_letter(text: &str) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic())
}

/// A key of a word, or two (' then a vowel): what ABC types, and the
/// Icelandic letter, if it's one Icelandic has a letter on.
struct Unit {
    abc: String,
    icelandic: Option<String>,
    /// With ⇧: after letters, ABC's ?, :, _ or {.
    shifted: bool,
    /// ' last in the word: a vowel may be to come (nothing shows yet, in
    /// Icelandic).
    waiting: bool,
    keys: usize,
}

/// The word with `letters` (which of its keys Icelandic has letters on are
/// those letters, in order), `hide` hiding ' waiting.
fn render(units: &[Unit], letters: &[bool], hide: bool) -> String {
    let mut out = String::new();
    let mut n = 0;
    for u in units {
        let text = match &u.icelandic {
            Some(letter) => {
                n += 1;
                if letters.get(n - 1).copied().unwrap_or(false) { letter.as_str() } else { u.abc.as_str() }
            }
            None if u.waiting && hide => "",
            None => u.abc.as_str(),
        };
        out.push_str(text);
    }
    out
}

/// Every way `n` keys can each go (with more than ten: all ABC's, or all
/// letters).
fn every(n: usize) -> Vec<Vec<bool>> {
    if n > 10 {
        return vec![vec![false; n], vec![true; n]];
    }
    (0..1u32 << n).map(|mask| (0..n).map(|i| mask & (1 << i) != 0).collect()).collect()
}

/// What to do with a key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Out {
    /// Let it through to the app.
    Pass,
    /// Keep it from the app: take back `back` characters, type `text`,
    /// then (`key`) the key itself after all.
    Type { back: usize, text: String, key: bool },
}

impl Out {
    fn keep() -> Out {
        Out::Type { back: 0, text: String::new(), key: false }
    }
}

/// A word: its keys, which of them are Icelandic letters, and what shows.
#[derive(Clone, Default)]
struct Word {
    keys: Vec<Key>,
    letters: Vec<bool>,
    shown: String,
}

/// A word changed as it ended, while Delete can still take that back: as
/// it showed, and the language it was.
struct Undo {
    was: Word,
    lang: Lang,
    typed: String,
}

pub struct Engine {
    /// Keys as the Mac's own layout reads them (ABC's), none while it isn't
    /// one for English (the Icelandic layout itself, Korean): then keys are
    /// left alone.
    english: Option<Layout>,
    icelandic: Layout,
    words: Words,
    /// The language words lean to, and show their letters in as they're
    /// typed.
    pub lang: Lang,
    /// The languages it types besides English.
    icelandic_on: bool,
    korean_on: bool,
    /// Words may switch it to Icelandic by themselves (not in Terminal or
    /// a code editor: there "a[" is code more often than "að").
    pub auto: bool,
    /// The app in front, and the language each app was left in.
    app: String,
    langs: HashMap<String, Lang>,
    /// The word being typed.
    word: Word,
    /// It began a line, as far as F2 knows (after Return, a click, another
    /// app): "/" there is a command or a path.
    fresh: bool,
    /// Keys of it F2 can't follow were typed (⌥'s, a layout's dead keys:
    /// the app's own): it's left be.
    lost: bool,
    /// Kept as it shows when it ends (Delete took a change back).
    held: bool,
    undo: Option<Undo>,
    /// The word just ended with a space (nothing since): the shortcut
    /// switches it, Delete goes back into it.
    last: Option<Word>,
    paused_until: Option<Instant>,
}

impl Engine {
    pub fn new(english: Option<Layout>, icelandic: Layout, words: Words) -> Engine {
        Engine {
            english,
            icelandic,
            words,
            lang: Lang::English,
            icelandic_on: true,
            korean_on: false,
            auto: true,
            app: String::new(),
            langs: HashMap::new(),
            word: Word::default(),
            fresh: true,
            lost: false,
            held: false,
            undo: None,
            last: None,
            paused_until: None,
        }
    }

    /// The Mac's layout changed (None: it isn't one for English).
    pub fn set_english(&mut self, english: Option<Layout>) {
        self.english = english;
        self.forget();
    }

    /// The languages it types besides English (Icelandic, Korean).
    pub fn set_langs(&mut self, icelandic: bool, korean: bool) {
        if (icelandic, korean) == (self.icelandic_on, self.korean_on) {
            return;
        }
        self.icelandic_on = icelandic;
        self.korean_on = korean;
        self.forget();
        if !self.on(self.lang) {
            self.lang = Lang::English;
        }
    }

    fn on(&self, lang: Lang) -> bool {
        match lang {
            Lang::English => true,
            Lang::Icelandic => self.icelandic_on,
            Lang::Korean => self.korean_on,
        }
    }

    /// Another app's in front: the language it was left in (English, the
    /// first time).
    pub fn set_app(&mut self, app: &str, auto: bool) {
        self.forget();
        if app == self.app {
            return;
        }
        let was = std::mem::replace(&mut self.app, app.to_string());
        self.langs.insert(was, self.lang);
        self.lang = self.langs.get(app).copied().filter(|lang| self.on(*lang)).unwrap_or(Lang::English);
        self.auto = auto;
        self.paused_until = None;
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    /// Forget the word: the text cursor moved (a click, an arrow), or
    /// something F2 can't follow happened.
    pub fn forget(&mut self) {
        self.word = Word::default();
        self.fresh = true;
        self.lost = false;
        self.held = false;
        self.undo = None;
        self.last = None;
    }

    fn paused(&self, now: Instant) -> bool {
        self.paused_until.is_some_and(|until| now < until)
    }

    /// Whether words may switch it to `lang`: only to English in Terminal
    /// or a code editor.
    fn may(&self, lang: Lang) -> bool {
        lang == Lang::English || self.auto || self.lang == lang
    }

    /// The Icelandic letter on a key ABC has punctuation on (ð, Ð, æ, þ, ö).
    fn letter(&self, key: Key) -> Option<String> {
        if key.mods & OPTION != 0 || !self.icelandic_on {
            return None;
        }
        let (abc, dead) = self.english.as_ref()?.alone(key)?;
        let (text, next) = self.icelandic.alone(key)?;
        (*dead == 0 && *next == 0 && !abc.chars().any(char::is_alphabetic) && is_letter(text)).then(|| text.clone())
    }

    /// The accent Icelandic has on a key ABC has punctuation on ('), as
    /// the dead key it leaves waiting.
    fn accent(&self, key: Key) -> Option<u32> {
        if key.mods & (OPTION | SHIFT) != 0 || !self.icelandic_on {
            return None;
        }
        let (abc, dead) = self.english.as_ref()?.alone(key)?;
        let (_, next) = self.icelandic.alone(key)?;
        (*dead == 0 && *next != 0 && !abc.chars().any(char::is_alphabetic)).then_some(*next)
    }

    /// The word's keys, as units.
    fn units(&self, keys: &[Key]) -> Vec<Unit> {
        let Some(english) = &self.english else { return Vec::new() };
        let abc = |key: Key| english.alone(key).map_or(String::new(), |(text, _)| text.clone());
        let mut units = Vec::new();
        let mut i = 0;
        while i < keys.len() {
            let key = keys[i];
            if let Some(state) = self.accent(key) {
                let next = keys.get(i + 1).copied();
                // (' then a vowel: á. Else it's ABC's '.)
                let accented = next.and_then(|n| self.icelandic.keys.get(&(state, n.code, n.mods))).filter(|(text, after)| *after == 0 && is_letter(text));
                if let (Some(next), Some((letter, _))) = (next, accented) {
                    units.push(Unit { abc: abc(key) + &abc(next), icelandic: Some(letter.clone()), shifted: false, waiting: false, keys: 2 });
                    i += 2;
                    continue;
                }
                units.push(Unit { abc: abc(key), icelandic: None, shifted: false, waiting: next.is_none(), keys: 1 });
                i += 1;
                continue;
            }
            units.push(Unit { abc: abc(key), icelandic: self.letter(key), shifted: key.mods & SHIFT != 0, waiting: false, keys: 1 });
            i += 1;
        }
        units
    }

    /// Which of the word's keys show Icelandic letters as it's typed (in
    /// Icelandic): the way that begins an Icelandic word with the most of
    /// them ("þe", "dög"); if none does, as they were, and a new one as it
    /// most often is ("/" and "-" inside a word ABC's: "src/", "e-"). One
    /// alone at a word's start waits for the next key; "/" beginning a
    /// line (a command, a path), and ⇧'s after letters (? : _ {), are
    /// ABC's.
    fn live(&self, units: &[Unit], was: &[bool]) -> Vec<bool> {
        let either: Vec<usize> = (0..units.len()).filter(|&i| units[i].icelandic.is_some()).collect();
        if self.lang != Lang::Icelandic {
            return vec![false; either.len()];
        }
        let after_letters = |i: usize| units[..i].iter().any(|u| u.abc.chars().any(char::is_alphabetic));
        let abc_only: Vec<bool> = either
            .iter()
            .map(|&i| {
                let first = !after_letters(i);
                (first && i + 1 == units.len()) || (first && self.fresh && units[i].abc == "/") || (units[i].shifted && !first)
            })
            .collect();
        let free: Vec<usize> = (0..either.len()).filter(|&n| !abc_only[n]).collect();
        let mut best: Option<((usize, usize), Vec<bool>)> = None;
        if free.len() <= 8 {
            for pick in every(free.len()) {
                let mut letters = vec![false; either.len()];
                for (k, &n) in free.iter().enumerate() {
                    letters[n] = pick[k];
                }
                if !self.words.begins(&render(units, &letters, true)) {
                    continue;
                }
                let key = (letters.iter().filter(|l| **l).count(), letters.iter().zip(was).filter(|(a, b)| a == b).count());
                if best.as_ref().is_none_or(|(b, _)| key > *b) {
                    best = Some((key, letters));
                }
            }
        }
        if let Some((_, letters)) = best {
            return letters;
        }
        either
            .iter()
            .enumerate()
            .map(|(n, &i)| !abc_only[n] && was.get(n).copied().unwrap_or_else(|| !matches!(units[i].abc.as_str(), "/" | "-")))
            .collect()
    }

    /// A key pressed, with `mods` as layouts read it, and `command` if ⌘
    /// or ⌃ is held.
    pub fn key(&mut self, code: u16, mods: u8, command: bool, now: Instant) -> Out {
        let Some(english) = &self.english else {
            self.forget();
            return Out::Pass;
        };
        if command {
            // (⌘↩ sends, in some apps: the word's typed right first.)
            if matches!(code, RETURN | ENTER) {
                return self.end(false, now);
            }
            self.forget();
            return Out::Pass;
        }
        let key = Key { code, mods };
        match code {
            DELETE => self.delete(mods),
            // (⌥Space is a shortcut more often than a space: Dictation's.)
            SPACE | RETURN | TAB | ENTER if mods & OPTION == 0 => self.end(code == SPACE, now),
            _ if english.alone(key).is_some() && self.icelandic.alone(key).is_some() => self.typed(key),
            _ => {
                self.forget();
                Out::Pass
            }
        }
    }

    /// The word as 2-Set types it (keys that aren't letters as ABC types
    /// them), and whether its syllables are whole.
    fn korean(&self, keys: &[Key]) -> (String, bool) {
        let Some(english) = &self.english else { return (String::new(), false) };
        hangul::compose(keys.iter().map(|key| match hangul::jamo(key.code, key.mods & SHIFT != 0) {
            Some(jamo) => Piece::Jamo(jamo),
            None => Piece::Other(english.alone(*key).map_or("", |(text, _)| text.as_str())),
        }))
    }

    /// The word begins a line with "/": a command or a path, ABC's in
    /// Korean too.
    fn command(&self, keys: &[Key]) -> bool {
        self.fresh && keys.first().and_then(|key| self.english.as_ref()?.alone(*key)).is_some_and(|(text, _)| text == "/")
    }

    /// The word typed in `lang` as it ends (' waiting typed), with
    /// `letters` in Icelandic.
    fn reading(&self, lang: Lang, keys: &[Key], units: &[Unit], letters: &[bool]) -> String {
        match lang {
            Lang::Korean if !self.command(keys) => self.korean(keys).0,
            Lang::Icelandic => render(units, letters, false),
            _ => render(units, &[], false),
        }
    }

    /// The word as it shows now, from its keys.
    fn show(&mut self) -> String {
        if self.lang == Lang::Korean && !self.command(&self.word.keys) {
            return self.korean(&self.word.keys).0;
        }
        let units = self.units(&self.word.keys);
        self.word.letters = self.live(&units, &self.word.letters);
        render(&units, &self.word.letters, self.lang == Lang::Icelandic)
    }

    /// A key that types.
    fn typed(&mut self, key: Key) -> Out {
        self.undo = None;
        self.last = None;
        if self.lost {
            return Out::Pass;
        }
        let Some(english) = &self.english else { return Out::Pass };
        let alone = english.alone(key).cloned();
        // (⌥'s keys, and a layout's dead keys, are the app's to keep.)
        if key.mods & OPTION != 0 || alone.as_ref().is_none_or(|(_, dead)| *dead != 0) {
            self.word = Word::default();
            self.lost = true;
            return Out::Pass;
        }
        self.word.keys.push(key);
        let shown = self.show();
        let (back, text) = edit(&self.word.shown, &shown);
        self.word.shown = shown;
        // (What ABC types anyway goes through as it is.)
        if back == 0 && alone.is_some_and(|(abc, _)| abc == text) {
            return Out::Pass;
        }
        Out::Type { back, text, key: false }
    }

    /// Delete: a change taken back, or the word one shorter.
    fn delete(&mut self, mods: u8) -> Out {
        if let Some(undo) = self.undo.take() {
            return self.take_back(undo);
        }
        let last = self.last.take();
        if mods & OPTION != 0 || self.lost {
            self.forget();
            return Out::Pass;
        }
        if self.word.keys.is_empty() {
            // (The space after a word: back in that word.)
            match last {
                Some(last) => self.word = last,
                None => self.forget(),
            }
            return Out::Pass;
        }
        // (In Icelandic, á goes whole, its ' too, and the rest as they
        // showed; in Korean a jamo, as 2-Set has it: 안 is 아 again.)
        let gone = if self.lang == Lang::Icelandic { self.units(&self.word.keys).last().map_or(1, |u| u.keys) } else { 1 };
        self.word.keys.truncate(self.word.keys.len().saturating_sub(gone));
        let shown = if self.lang == Lang::Korean {
            self.show()
        } else {
            let units = self.units(&self.word.keys);
            self.word.letters.truncate(units.iter().filter(|u| u.icelandic.is_some()).count());
            render(&units, &self.word.letters, self.lang == Lang::Icelandic)
        };
        let (back, text) = edit(&self.word.shown, &shown);
        self.word.shown = shown;
        match (back, text.is_empty()) {
            (1, true) => Out::Pass,
            // (' waiting, which showed nothing.)
            (0, true) => Out::keep(),
            _ => Out::Type { back, text, key: false },
        }
    }

    /// The word ends: typed the way it's a word.
    fn end(&mut self, space: bool, now: Instant) -> Out {
        self.undo = None;
        self.last = None;
        if self.word.keys.is_empty() || self.lost {
            self.forget();
            self.fresh = !space;
            return Out::Pass;
        }
        let word = std::mem::take(&mut self.word);
        let held = std::mem::replace(&mut self.held, false);
        let units = self.units(&word.keys);
        let before = self.lang;
        let finished = self.reading(before, &word.keys, &units, &word.letters);
        let (typed, letters, lang) = if held || self.paused(now) { (finished.clone(), word.letters.clone(), None) } else { self.choose(&word.keys, &units, &word.letters) };
        self.fresh = !space;
        if let Some(lang) = lang {
            self.lang = lang;
        }
        if space {
            self.last = Some(Word { keys: word.keys.clone(), letters, shown: typed.clone() });
        }
        if typed == word.shown {
            return Out::Pass;
        }
        let (back, text) = edit(&word.shown, &typed);
        // (Changed, not just finished: ' left waiting typed.)
        if space && typed != finished {
            self.undo = Some(Undo { was: word, lang: before, typed });
        }
        Out::Type { back, text, key: true }
    }

    /// How the word's typed, of every way it can be read (Icelandic's
    /// letters, or not; Korean's syllables), which of its keys are
    /// Icelandic letters in it, and the language it says the word's in
    /// (None: it says nothing).
    fn choose(&self, keys: &[Key], units: &[Unit], letters: &[bool]) -> (String, Vec<bool>, Option<Lang>) {
        let either = units.iter().filter(|u| u.icelandic.is_some()).count();
        let shown = self.reading(self.lang, keys, units, letters);
        let abc = render(units, &[], false);
        let abc_letters = abc.chars().filter(|c| c.is_alphabetic()).count();
        // (Punctuation alone, "--" or ";" but in Icelandic, and faces:
        // ABC's.)
        if FACES.contains(&abc.as_str()) || (abc_letters == 0 && (self.lang != Lang::Icelandic || abc.chars().count() > 1)) {
            return (abc, vec![false; either], None);
        }
        // (Each way: what it types, its Icelandic letters, and whether
        // it's Korean's.)
        let mut ways: Vec<(String, Vec<bool>, bool)> = every(either).into_iter().map(|l| (render(units, &l, false), l, false)).collect();
        if self.korean_on && !self.command(keys) {
            ways.push((self.korean(keys).0, vec![false; either], true));
        }
        let words: Vec<Vec<Lang>> = ways.iter().map(|(text, _, _)| Lang::ALL.into_iter().filter(|lang| self.on(*lang) && self.words.score(text, *lang).word()).collect()).collect();
        let latin = words.iter().flatten().any(|lang| *lang != Lang::Korean);
        // (The best, then what shows, then the most of the language it's
        // in.)
        let best = ways
            .into_iter()
            .zip(words)
            .map(|((text, letters, korean), mut langs)| {
                if latin && !self.words.said_often(&text) {
                    langs.retain(|lang| *lang != Lang::Korean);
                }
                let (mut rank, lang) = self.rank(&text, korean, &langs);
                // (In English, only a word changes what's typed.)
                if lang.is_some_and(|lang| !self.may(lang)) || (rank == 1 && self.lang == Lang::English) {
                    rank = 0;
                }
                let count = letters.iter().filter(|l| **l).count() as i64;
                let lean = match self.lang {
                    Lang::English => -count - 100 * korean as i64,
                    Lang::Icelandic => count - 100 * korean as i64,
                    Lang::Korean => 100 * korean as i64 - count,
                };
                (rank, text == shown, lean, text, letters, lang)
            })
            .max_by_key(|(rank, shows, lean, ..)| (*rank, *shows, *lean));
        match best {
            Some((rank, _, _, text, letters, lang)) if rank > 0 => {
                // (A letter alone says nothing of the language: "a", "I".)
                let says = rank >= 2 && (text != abc || abc_letters >= 2);
                (text, letters, lang.filter(|_| says))
            }
            _ => (shown, letters.to_vec(), None),
        }
    }

    /// How good a way to type a word `text` is (a Korean one if `korean`),
    /// of the languages it's a word of: 3 a word of the language it's in
    /// now, 2 of another, 1 nothing out of place in it (in Icelandic, as an
    /// Icelandic word could begin: not "srcþ"; in Korean, whole
    /// syllables), 0 else; and the language it's a word of.
    fn rank(&self, text: &str, korean: bool, langs: &[Lang]) -> (u8, Option<Lang>) {
        if langs.contains(&self.lang) {
            return (3, Some(self.lang));
        }
        if let Some(&lang) = langs.first() {
            return (2, Some(lang));
        }
        let plausible = if korean {
            self.words.score(text, Lang::Korean).clean
        } else {
            let e = self.words.score(text, Lang::English);
            let i = self.words.score(text, Lang::Icelandic);
            // (A letter alone that's no word: "þ" for "/".)
            (e.clean || (self.icelandic_on && i.clean && self.words.begins(text))) && e.letters != 1
        };
        (plausible as u8, None)
    }

    /// The shortcut: the next language (English, Icelandic, Korean, those
    /// on), for the word being typed or the one just ended (with its
    /// space). Words don't switch it back for a while.
    pub fn switch(&mut self, now: Instant) -> Out {
        self.undo = None;
        let on: Vec<Lang> = Lang::ALL.into_iter().filter(|lang| self.on(*lang)).collect();
        let at = on.iter().position(|lang| *lang == self.lang).unwrap_or(0);
        self.lang = on[(at + 1) % on.len()];
        self.paused_until = Some(now + PAUSE);
        if self.english.is_none() {
            return Out::keep();
        }
        if !self.word.keys.is_empty() && !self.lost {
            self.word.letters.clear();
            let shown = self.show();
            let (back, text) = edit(&self.word.shown, &shown);
            self.word.shown = shown;
            return Out::Type { back, text, key: false };
        }
        if let Some(mut last) = self.last.take() {
            let units = self.units(&last.keys);
            let either = units.iter().filter(|u| u.icelandic.is_some()).count();
            // (In Icelandic: the way that's an Icelandic word, with the
            // most letters.)
            last.letters = match self.lang {
                Lang::Icelandic => every(either)
                    .into_iter()
                    .max_by_key(|letters| (self.words.score(&render(&units, letters, false), Lang::Icelandic).word(), letters.iter().filter(|l| **l).count()))
                    .unwrap_or_default(),
                _ => vec![false; either],
            };
            let typed = match self.lang {
                Lang::Korean => self.korean(&last.keys).0,
                _ => render(&units, &last.letters, false),
            };
            let (back, text) = edit(&format!("{} ", last.shown), &format!("{typed} "));
            last.shown = typed;
            self.last = Some(last);
            return Out::Type { back, text, key: false };
        }
        Out::keep()
    }

    /// Delete just after a word changed as it ended: as it showed, and the
    /// language it was, back in the word (kept as it is when it ends).
    fn take_back(&mut self, undo: Undo) -> Out {
        self.last = None;
        let (back, text) = edit(&format!("{} ", undo.typed), &undo.was.shown);
        self.word = undo.was;
        self.lang = undo.lang;
        self.lost = false;
        self.held = true;
        Out::Type { back, text, key: false }
    }
}

/// `to` typed where `from` shows: how many characters to take back, and
/// what to type then.
fn edit(from: &str, to: &str) -> (usize, String) {
    let same = from.chars().zip(to.chars()).take_while(|(a, b)| a == b).count();
    (from.chars().count() - same, to.chars().skip(same).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(id: &str) -> Layout {
        // (Read once: TIS isn't for many threads at once, as tests run.)
        static TABLES: std::sync::OnceLock<HashMap<&str, String>> = std::sync::OnceLock::new();
        let tables = TABLES.get_or_init(|| {
            ["com.apple.keylayout.ABC", "com.apple.keylayout.Icelandic"].into_iter().map(|id| (id, crate::macos::typing_layout(Some(id)).expect(id))).collect()
        });
        Layout::parse(&tables[id]).expect(id)
    }

    fn engine() -> Engine {
        Engine::new(Some(layout("com.apple.keylayout.ABC")), layout("com.apple.keylayout.Icelandic"), Words::bundled())
    }

    /// With Korean on too.
    fn all_three() -> Engine {
        let mut e = engine();
        e.set_langs(true, true);
        e
    }

    /// The key (not the keypad's) ABC types `c` with, without ⌥ or ⇪.
    fn key_for(abc: &Layout, c: char) -> Key {
        let ((_, code, mods), _) = abc
            .keys
            .iter()
            .filter(|((state, code, mods), (text, _))| *state == 0 && *code <= 50 && mods & (CAPS | OPTION) == 0 && text.chars().eq([c]))
            .min_by_key(|((_, code, mods), _)| (*mods, *code))
            .expect("a key for it");
        Key { code: *code, mods: *mods }
    }

    /// A text field: what ends up in it as `typed` (ABC's characters,
    /// "\u{8}" for Delete, "\u{1}" for the shortcut, "\u{2}" for ⌘↩) is
    /// typed.
    fn type_in(e: &mut Engine, typed: &str) -> String {
        let abc = layout("com.apple.keylayout.ABC");
        let now = Instant::now();
        let mut field = String::new();
        for c in typed.chars() {
            let (out, alone) = match c {
                '\u{1}' => (e.switch(now), String::new()),
                '\u{8}' => (e.key(DELETE, 0, false, now), String::new()),
                ' ' => (e.key(SPACE, 0, false, now), " ".to_string()),
                '\n' => (e.key(RETURN, 0, false, now), "\n".to_string()),
                '\u{2}' => (e.key(RETURN, 0, true, now), "\n".to_string()),
                c => {
                    let key = key_for(&abc, c);
                    (e.key(key.code, key.mods, false, now), c.to_string())
                }
            };
            match out {
                Out::Pass if c == '\u{8}' => _ = field.pop(),
                Out::Pass => field.push_str(&alone),
                Out::Type { back, text, key } => {
                    for _ in 0..back {
                        field.pop();
                    }
                    field.push_str(&text);
                    if key {
                        field.push_str(&alone);
                    }
                }
            }
        }
        field
    }

    #[test]
    fn icelandic_on_abc() {
        let mut e = engine();
        assert_eq!(type_in(&mut e, "/a[ er gott "), "það er gott ");
        assert_eq!(e.lang, Lang::Icelandic);
        assert_eq!(type_in(&mut e, "'eg f;ri\n"), "ég færi\n");
        assert_eq!(type_in(&mut e, "Nor[ur-Amer'ika h-fn "), "Norður-Ameríka höfn ");
        assert_eq!(type_in(&mut e, "/a[? ?etta "), "það? Þetta ");
        assert_eq!(type_in(&mut e, "the /a[\u{2}"), "the það\n", "⌘↩ ends a word too");
    }

    #[test]
    fn letters_show_as_typed() {
        let mut e = engine();
        type_in(&mut e, "takk ");
        assert_eq!(e.lang, Lang::Icelandic);
        assert_eq!(type_in(&mut e, "/"), "/", "alone, it waits");
        assert_eq!(type_in(&mut e, "a["), "það", "then a letter that begins a word");
        assert_eq!(type_in(&mut e, " sj'"), " sj", "' waits for a vowel");
        assert_eq!(type_in(&mut e, "a d-g"), "á dög");
        assert_eq!(type_in(&mut e, "um src/"), "um src/", "no word begins srcþ");
        assert_eq!(type_in(&mut e, " e-"), " e-");
    }

    #[test]
    fn slashes() {
        let mut e = engine();
        type_in(&mut e, "/a[ ");
        let typed = "src/typing.rs /compact mbl.is/frettir km/h ~/Downloads 1/2 ";
        assert_eq!(type_in(&mut e, typed), typed);
        assert_eq!(type_in(&mut e, "og/e[a "), "og/eða ");
        assert_eq!(type_in(&mut e, "sj'a src/ og /etta "), "sjá src/ og þetta ");
        // (Beginning a line: a command, until it's a word.)
        assert_eq!(type_in(&mut e, "\n/exit\n/etta er gott "), "\n/exit\nþetta er gott ");
    }

    #[test]
    fn a_message() {
        let mut e = engine();
        let typed = "/a[ src/typing.rs og /etta er gott, sj'a km/h mbl.is/frettir 1/2\n/compact\n/etta er f;ri\n";
        assert_eq!(type_in(&mut e, typed), "það src/typing.rs og þetta er gott, sjá km/h mbl.is/frettir 1/2\n/compact\nþetta er færi\n");
    }

    #[test]
    fn the_rest_is_abc() {
        let mut e = engine();
        type_in(&mut e, "/a[ ");
        assert_eq!(e.lang, Lang::Icelandic);
        let typed = "2-3 (a) :) ;) @dagur 50% x=y? \"svo\" - e-mail don't you? and/or ";
        assert_eq!(type_in(&mut e, typed), typed);
    }

    #[test]
    fn english_stays() {
        let mut e = engine();
        let typed = "don't a[i] x = a[0]; f; - ; dagur@gmail.com 'A /usr/bin ";
        assert_eq!(type_in(&mut e, typed), typed);
        assert_eq!(e.lang, Lang::English);
    }

    #[test]
    fn words_say_which() {
        let mut e = engine();
        assert_eq!(type_in(&mut e, "takk fyrir "), "takk fyrir ");
        assert_eq!(e.lang, Lang::Icelandic, "words only Icelandic has");
        assert_eq!(type_in(&mut e, "f; ; "), "fæ æ ");
        assert_eq!(type_in(&mut e, "the "), "the ");
        assert_eq!(e.lang, Lang::English);
        assert_eq!(type_in(&mut e, "don't "), "don't ");
    }

    #[test]
    fn delete() {
        let mut e = engine();
        // (Delete just after: back as typed, and kept so.)
        assert_eq!(type_in(&mut e, "a[ \u{8}"), "a[");
        assert_eq!(e.lang, Lang::English);
        assert_eq!(type_in(&mut e, " "), " ");
        let mut e = engine();
        assert_eq!(type_in(&mut e, "/a[ /a\u{8}e"), "það þe");
        // (' waiting goes with nothing to take back; á goes whole.)
        assert_eq!(type_in(&mut e, "'\u{8}a"), "a");
        assert_eq!(type_in(&mut e, "'a\u{8}"), "");
    }

    #[test]
    fn by_hand() {
        let mut e = engine();
        // ("'A" is English's too: the shortcut, just after.)
        assert_eq!(type_in(&mut e, "'A \u{1}morgun "), "Á morgun ");
        assert_eq!(e.lang, Lang::Icelandic);
        let mut e = engine();
        assert_eq!(type_in(&mut e, "Gu[\u{1}mundsd'ottir "), "Guðmundsdóttir ");
    }

    #[test]
    fn korean_on_abc() {
        let mut e = all_three();
        assert_eq!(type_in(&mut e, "dkssudgktpdy "), "안녕하세요 ");
        assert_eq!(e.lang, Lang::Korean);
        assert_eq!(type_in(&mut e, "rkatkgkqslek. so zz "), "감사합니다. 내 ㅋㅋ ");
        // (English again, by its words; "so" English there.)
        assert_eq!(type_in(&mut e, "hello so "), "hello so ");
        assert_eq!(e.lang, Lang::English);
        assert_eq!(type_in(&mut e, "/a[ er gott en dkssud "), "það er gott en 안녕 ");
        assert_eq!(e.lang, Lang::Korean);
        // (Said rarely, Korean's no word against English's: "for" isn't 랙.)
        assert_eq!(type_in(&mut e, "for "), "for ");
    }

    #[test]
    fn a_korean_message() {
        let mut e = all_three();
        let typed = "dkssudgktpdy wjsms ekrnfmdPdy. dhsmf skfTlrk whgspdy zz meeting 3tldp gkfRkdy? ok whgdkdy ";
        assert_eq!(type_in(&mut e, typed), "안녕하세요 저는 다구르예요. 오늘 날씨가 좋네요 ㅋㅋ meeting 3시에 할까요? ok 좋아요 ");
    }

    #[test]
    fn english_back_on_return() {
        let mut e = all_three();
        assert_eq!(type_in(&mut e, "rkatkgkqslek done\n"), "감사합니다 done\n");
    }

    #[test]
    fn hangul_as_typed() {
        let mut e = all_three();
        type_in(&mut e, "dkssud ");
        assert_eq!(type_in(&mut e, "dkssu"), "안녀", "syllables made as they're typed");
        // (Delete takes a jamo, as 2-Set does.)
        let mut e = all_three();
        type_in(&mut e, "dkssud ");
        assert_eq!(type_in(&mut e, "dkssu\u{8}\u{8}\u{8}"), "아");
    }

    #[test]
    fn three_by_hand() {
        let mut e = all_three();
        // (English, Icelandic, Korean, and round.)
        assert_eq!(type_in(&mut e, "gks\u{1}"), "gks");
        assert_eq!(e.lang, Lang::Icelandic);
        assert_eq!(type_in(&mut e, "\u{1}"), "한");
        assert_eq!(e.lang, Lang::Korean);
        assert_eq!(type_in(&mut e, "\u{1}"), "gks");
        assert_eq!(e.lang, Lang::English);
        // (The word just typed.)
        let mut e = all_three();
        assert_eq!(type_in(&mut e, "so \u{1}\u{1}"), "내 ");
    }

    #[test]
    fn korean_leaves_the_rest() {
        let mut e = all_three();
        type_in(&mut e, "/a[ ");
        let typed = "2-3 (a) :) ;) @dagur 50% x=y? \"svo\" - e-mail don't you? and/or ";
        assert_eq!(type_in(&mut e, typed), typed);
        let mut e = all_three();
        let typed = "don't a[i] x = a[0]; f; - ; dagur@gmail.com 'A /usr/bin the to do go ";
        assert_eq!(type_in(&mut e, typed), typed);
        assert_eq!(e.lang, Lang::English);
        // (Off, it's as before.)
        let mut e = engine();
        assert_eq!(type_in(&mut e, "dkssud "), "dkssud ");
    }

    #[test]
    fn code_apps() {
        let mut e = engine();
        e.set_app("com.apple.Terminal", false);
        assert_eq!(type_in(&mut e, "a[ "), "a[ ");
        e.set_app("com.apple.MobileSMS", true);
        assert_eq!(type_in(&mut e, "a[ "), "að ");
        e.set_app("com.apple.Terminal", false);
        assert_eq!(e.lang, Lang::English, "each app's own");
    }
}

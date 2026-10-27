//! Words, for Quick Open: spelling ("spell accomodate": "accommodate", as
//! the Mac's spell checker has it), synonyms ("synonym for happy", from
//! macOS's thesaurus), and counting the words in what's copied ("word
//! count").

/// The word whose spelling's asked: "spell necesary", "how do you spell
/// rhythm", "spelling of …".
pub fn spelling_asked(q: &str) -> Option<String> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    const LEADS: &[&str] = &["how do you spell ", "how to spell ", "how do i spell ", "spelling of ", "spelling for ", "spellcheck ", "spell check ", "spelling ", "spell "];
    let lead = LEADS.iter().find(|l| lower.starts_with(*l))?;
    let word = q.trim().trim_end_matches('?').get(lead.len()..)?.trim();
    (!word.is_empty() && word.split_whitespace().count() <= 3).then(|| word.to_string())
}

/// The word whose synonyms are asked: "synonym for happy", "synonyms of
/// big", "another word for walk", "happy synonym".
pub fn synonyms_asked(q: &str) -> Option<String> {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    const LEADS: &[&str] = &[
        "synonyms for ", "synonym for ", "synonyms of ", "synonym of ", "other words for ", "another word for ", "a word for ", "words for ", "thesaurus ", "synonyms ", "synonym ",
    ];
    let word = match LEADS.iter().find(|l| lower.starts_with(*l)) {
        Some(lead) => lower[lead.len()..].trim().to_string(),
        None => lower.strip_suffix(" synonyms").or_else(|| lower.strip_suffix(" synonym"))?.trim().to_string(),
    };
    (!word.is_empty() && word.split_whitespace().count() <= 3).then_some(word)
}

/// A thesaurus entry, read: its part of speech ("adjective"), and each
/// sense's synonyms (the plain ones: not those marked informal, dated,
/// British…) with its opposites.
#[derive(Debug, PartialEq)]
pub struct Entry {
    pub part: String,
    pub senses: Vec<(Vec<String>, Vec<String>)>,
}

const PARTS: &[&str] = &["noun", "verb", "adjective", "adverb", "preposition", "conjunction", "pronoun", "exclamation", "phrase"];

/// Read the thesaurus's text: "happy adjective 1 Melissa looked happy.
/// contented, cheerful…; …; informal chirpy… ANTONYMS sad. 2 …".
pub fn read(text: &str) -> Option<Entry> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let at = words.iter().position(|w| PARTS.contains(w))?;
    let part = words[at].to_string();
    // (Its first part of speech only: "walk verb … noun 1 …".)
    let mut rest: Vec<&str> = Vec::new();
    for (i, w) in words[at + 1..].iter().enumerate() {
        if PARTS.contains(w) && words.get(at + 2 + i).is_some_and(|n| *n == "1") {
            break;
        }
        rest.push(w);
    }
    // Senses: from each number to the next.
    let mut senses_text: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut expected = 1;
    for w in rest {
        if w.parse::<usize>().ok() == Some(expected) {
            if !current.is_empty() {
                senses_text.push(current.join(" "));
            }
            current.clear();
            expected += 1;
            continue;
        }
        current.push(w);
    }
    if !current.is_empty() {
        senses_text.push(current.join(" "));
    }
    let senses = senses_text
        .iter()
        .filter_map(|sense| {
            // (After its example, to the end, or its opposites.)
            let (body, opposites) = sense.split_once("ANTONYMS").map_or((sense.as_str(), ""), |(a, b)| (a, b));
            let list = body.split_once(". ").map_or(body, |(_, list)| list);
            let synonyms = plain(list);
            (!synonyms.is_empty()).then(|| (synonyms, plain(opposites)))
        })
        .collect::<Vec<_>>();
    (!senses.is_empty()).then_some(Entry { part, senses })
}

/// The plain words of a list: up to the first marked ones ("informal
/// chirpy…"), each trimmed.
fn plain(list: &str) -> Vec<String> {
    const MARKS: &[&str] = &["informal", "formal", "dated", "rare", "literary", "archaic", "technical", "humorous", "derogatory", "vulgar", "offensive", "Medicine"];
    let mut out = Vec::new();
    for group in list.trim().trim_end_matches('.').split(';') {
        let first = group.split_whitespace().next().unwrap_or("");
        if MARKS.contains(&first) || first.chars().next().is_some_and(char::is_uppercase) {
            break;
        }
        for word in group.split(',') {
            let word = word.trim().trim_end_matches('.');
            if !word.is_empty() && !out.iter().any(|w| w == word) {
                out.push(word.to_string());
            }
        }
    }
    out
}

/// Whether the words in what's copied are to be counted.
pub fn count_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(
        lower.as_str(),
        "word count" | "count words" | "words count" | "character count" | "count characters" | "char count" | "letter count" | "count letters" | "how many words"
    )
}

/// Words, characters (and without spaces), lines, in `text`. (Each CJK
/// character a word: they're written without spaces.)
pub fn count(text: &str) -> (usize, usize, usize, usize) {
    let cjk = |c: char| matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff);
    let words = text.split_whitespace().map(|w| { let n = w.chars().filter(|c| cjk(*c)).count(); if n > 0 { n + usize::from(w.chars().any(|c| !cjk(c) && c.is_alphanumeric())) } else { 1 } }).sum();
    let chars = text.chars().count();
    let letters = text.chars().filter(|c| !c.is_whitespace()).count();
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    (words, chars, letters, lines)
}

/// A way to change text: its case, or its spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Upper,
    Lower,
    Title,
    Sentence,
    /// Spaces run together one, tabs too; each line's edges trimmed.
    Spaces,
    /// Lines joined into sentences (paragraphs kept).
    LineBreaks,
}

impl Tool {
    pub const ALL: [Tool; 6] = [Tool::Upper, Tool::Lower, Tool::Title, Tool::Sentence, Tool::Spaces, Tool::LineBreaks];

    /// "UPPERCASE", "Remove Line Breaks".
    pub fn title(self) -> &'static str {
        match self {
            Tool::Upper => "UPPERCASE",
            Tool::Lower => "lowercase",
            Tool::Title => "Title Case",
            Tool::Sentence => "Sentence case",
            Tool::Spaces => "Clean Up Spaces",
            Tool::LineBreaks => "Remove Line Breaks",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Tool::Upper | Tool::Lower | Tool::Title | Tool::Sentence => "textformat",
            Tool::Spaces => "text.justify",
            Tool::LineBreaks => "text.append",
        }
    }

    pub fn apply(self, text: &str) -> String {
        match self {
            Tool::Upper => text.to_uppercase(),
            Tool::Lower => text.to_lowercase(),
            Tool::Title => title_case(text),
            Tool::Sentence => sentence_case(text),
            Tool::Spaces => text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n").trim().to_string(),
            Tool::LineBreaks => text
                .split("\n\n")
                .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }
}

/// The tools asked for by name: "uppercase", "title case", "remove line
/// breaks"; all of them for "text tools".
pub fn tools_asked(q: &str) -> Vec<Tool> {
    let lower = q.trim().to_lowercase();
    let lower = lower.strip_prefix("make ").or_else(|| lower.strip_prefix("convert to ")).unwrap_or(&lower).trim();
    match lower {
        "uppercase" | "upper case" | "all caps" | "caps" | "capitals" => vec![Tool::Upper],
        "lowercase" | "lower case" => vec![Tool::Lower],
        "title case" | "titlecase" | "capitalize" | "capitalise" => vec![Tool::Title],
        "sentence case" => vec![Tool::Sentence],
        "clean up spaces" | "remove extra spaces" | "trim spaces" | "trim" | "fix spaces" => vec![Tool::Spaces],
        "remove line breaks" | "join lines" | "remove newlines" | "one line" => vec![Tool::LineBreaks],
        "text tools" | "change case" | "case" | "transform text" | "clean text" | "clean up text" => Tool::ALL.to_vec(),
        _ => Vec::new(),
    }
}

/// "The Lord of the Rings": each word's first letter up, but for small
/// words in the middle.
fn title_case(text: &str) -> String {
    const SMALL: &[&str] = &["a", "an", "the", "and", "but", "or", "nor", "for", "so", "yet", "on", "at", "to", "by", "of", "in", "up", "as", "via", "vs"];
    text.lines()
        .map(|line| {
            let words: Vec<&str> = line.split(' ').collect();
            let last = words.len().saturating_sub(1);
            words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    let lower = w.to_lowercase();
                    if i != 0 && i != last && SMALL.contains(&lower.trim_matches(|c: char| !c.is_alphanumeric())) {
                        return lower;
                    }
                    let mut chars = lower.chars();
                    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// "This is one. And another." from "THIS IS ONE. AND ANOTHER."
fn sentence_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for c in text.to_lowercase().chars() {
        if start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.push(c);
            if matches!(c, '.' | '!' | '?' | '\n') {
                start = true;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Tool, count, read, spelling_asked, synonyms_asked, tools_asked};

    #[test]
    fn tools() {
        assert_eq!(Tool::Title.apply("the lord of the rings"), "The Lord of the Rings");
        assert_eq!(Tool::Sentence.apply("HELLO THERE. HOW ARE YOU?"), "Hello there. How are you?");
        assert_eq!(Tool::Spaces.apply("  too   many \t spaces  \n here  "), "too many spaces\nhere");
        assert_eq!(Tool::LineBreaks.apply("a line\nbroken in\nthree\n\nnext"), "a line broken in three\n\nnext");
        assert_eq!(Tool::Upper.apply("straße"), "STRASSE");
        assert_eq!(tools_asked("Make uppercase"), [Tool::Upper]);
        assert_eq!(tools_asked("text tools").len(), 6);
        assert!(tools_asked("upper").is_empty());
    }

    #[test]
    fn asked() {
        assert_eq!(spelling_asked("spell accomodate"), Some("accomodate".into()));
        assert_eq!(spelling_asked("How do you spell rhythm?"), Some("rhythm".into()));
        assert_eq!(spelling_asked("spellbound"), None);
        assert_eq!(synonyms_asked("synonym for happy"), Some("happy".into()));
        assert_eq!(synonyms_asked("another word for big"), Some("big".into()));
        assert_eq!(synonyms_asked("walk synonyms"), Some("walk".into()));
        assert_eq!(synonyms_asked("synonymous"), None);
    }

    #[test]
    fn thesaurus() {
        let text = "happy adjective 1 Melissa came in looking happy and excited. contented, content, cheerful; cock-a-hoop, in good spirits; informal chirpy, tickled pink; British English informal chuffed. ANTONYMS sad. 2 we will be happy to advise you. willing, glad, ready; disposed, inclined; informal game. ANTONYMS unwilling. 3 by a happy coincidence | a happy choice. fortunate, lucky. ANTONYMS unfortunate.";
        let entry = read(text).unwrap();
        assert_eq!(entry.part, "adjective");
        assert_eq!(entry.senses.len(), 3);
        assert_eq!(entry.senses[0].0, ["contented", "content", "cheerful", "cock-a-hoop", "in good spirits"]);
        assert_eq!(entry.senses[0].1, ["sad"]);
        assert_eq!(entry.senses[1].0, ["willing", "glad", "ready", "disposed", "inclined"]);
        assert_eq!(entry.senses[2].0, ["fortunate", "lucky"]);
        let walk = read("walk verb 1 the men walked along. stroll, saunter, amble. 2 he walked home. go by foot. noun 1 a walk in the park. stroll, saunter.").unwrap();
        assert_eq!((walk.part.as_str(), walk.senses.len()), ("verb", 2), "the verb's only");
    }

    #[test]
    fn counted() {
        assert_eq!(count("The quick brown fox.\n\nJumps over."), (6, 33, 27, 2));
        assert_eq!(count("東京に行きます").0, 7, "a word a character");
    }
}

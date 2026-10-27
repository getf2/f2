//! A word in the Dictionary, for Quick Open: "define serendipity", "meaning
//! of ephemeral", "ephemeral meaning", "what does gauche mean". macOS's
//! own text for it (from the dictionaries Dictionary has on), set out a
//! little: the word and how it's said, then what it means, sense by sense;
//! its origins and the rest are Dictionary's, a Return away.

/// A word as the Dictionary has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub word: String,
    /// How it's said ("ˌserənˈdipədē").
    pub said: String,
    /// What it means: a sense a line.
    pub senses: String,
}

/// The word asked about, if one is.
pub fn word(q: &str) -> Option<String> {
    let lower = q.trim().to_lowercase();
    const BEFORE: &[&str] = &["define ", "define: ", "definition of ", "meaning of ", "what is the meaning of ", "what's the meaning of ", "dictionary ", "look up ", "what does "];
    const AFTER: &[&str] = &[" meaning", " definition", " mean", " means"];
    let mut word = BEFORE.iter().find_map(|p| lower.strip_prefix(p))?.trim().trim_end_matches('?').trim();
    if lower.starts_with("what does ") {
        word = AFTER.iter().find_map(|a| word.strip_suffix(a))?.trim();
    }
    let word = word.trim_end_matches('?').trim();
    // (A word, or a few: "ice cream", "laissez-faire".)
    let fits = !word.is_empty() && word.split_whitespace().count() <= 3 && word.chars().all(|c| c.is_alphabetic() || " -'’".contains(c));
    fits.then(|| word.to_string())
}

/// "ephemeral meaning", "serendipity definition": the word, asked after.
pub fn word_after(q: &str) -> Option<String> {
    let lower = q.trim().to_lowercase();
    let word = [" meaning", " definition", " define"].iter().find_map(|a| lower.strip_suffix(a))?.trim();
    let fits = !word.is_empty() && word.split_whitespace().count() <= 2 && word.chars().all(|c| c.is_alphabetic() || " -'’".contains(c));
    fits.then(|| word.to_string())
}

/// The word in the Dictionary, set out.
pub fn define(word: &str) -> Option<Definition> {
    crate::macos::define(word).map(|text| set_out(&text))
}

/// Parts of speech, where a word's senses start over.
const PARTS: &[&str] = &["noun", "verb", "adjective", "adverb", "pronoun", "preposition", "conjunction", "exclamation", "determiner", "abbreviation", "prefix", "suffix", "symbol"];

/// The Dictionary's text ("serendipity | ˌserənˈdipədē | noun the
/// occurrence of ... ORIGIN 1754: ..."): the word, how it's said, and its
/// senses a line each, the origin and what's after left out.
fn set_out(text: &str) -> Definition {
    let (word, rest) = text.split_once(" | ").unwrap_or(("", text));
    let (said, rest) = match rest.split_once(" | ") {
        Some((said, rest)) if said.chars().count() < 60 => (said.trim(), rest),
        _ => ("", rest),
    };
    // (The word, without its syllables beside it: "ser·en·dip·i·ty".)
    let word = word.split_whitespace().filter(|w| !w.contains('·')).collect::<Vec<_>>().join(" ");
    let said = said.to_string();
    // (Up to the word's origin, phrases, and the rest.)
    let end = ["ORIGIN", "DERIVATIVES", "PHRASES", "PHRASAL VERBS", "USAGE"]
        .iter()
        .filter_map(|marker| rest.find(&format!(" {marker} ")).or_else(|| rest.find(&format!(" {marker}:"))))
        .min()
        .unwrap_or(rest.len());
    let rest = without_forms(rest[..end].trim());
    let rest = rest.as_str();
    let mut lines: Vec<String> = Vec::new();
    let words: Vec<&str> = rest.split(' ').collect();
    let mut line = String::new();
    // (Senses go 1, 2, 3, starting over at each part of speech: a number
    // in an example, "ran 42 marathons", isn't one.)
    let mut sense = 1;
    for (i, w) in words.iter().enumerate() {
        // A new line at each sense ("1", "2"), and each part of speech after
        // the first.
        let numbered = w.parse::<u32>() == Ok(sense) && words.get(i + 1).is_some_and(|n| n.starts_with(|c: char| c.is_lowercase() || c == '['));
        let part = PARTS.contains(w) && i > 0 && words[i - 1].ends_with(['.', ')', ']']);
        if numbered {
            sense += 1;
        }
        if part {
            sense = 1;
        }
        // (Not between a part of speech and its first sense: "noun 1 ...".)
        if (numbered || part) && !line.trim().is_empty() && !(numbered && PARTS.contains(&line.trim())) {
            lines.push(line.trim().to_string());
            line.clear();
        }
        line.push_str(w);
        line.push(' ');
    }
    if !line.trim().is_empty() {
        lines.push(line.trim().to_string());
    }
    Definition { word, said, senses: lines.join("\n") }
}

/// Without the bracketed forms and how they're said ("(, running |
/// ˈrəniNG |)"), as noisy as they're rare to need.
fn without_forms(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut group = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '(' => {
                depth += 1;
                group.push(c);
            }
            ')' if depth > 0 => {
                depth -= 1;
                group.push(c);
                if depth == 0 {
                    if !group.contains('|') {
                        out.push_str(&group);
                    }
                    group.clear();
                }
            }
            c if depth > 0 => group.push(c),
            c => out.push(c),
        }
    }
    out.push_str(&group);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{set_out, word, word_after};

    #[test]
    fn words_asked() {
        assert_eq!(word("define serendipity").as_deref(), Some("serendipity"));
        assert_eq!(word("Meaning of ice cream").as_deref(), Some("ice cream"));
        assert_eq!(word("what does gauche mean?").as_deref(), Some("gauche"));
        assert_eq!(word("define 2+2"), None);
        assert_eq!(word("defined"), None);
        assert_eq!(word_after("ephemeral meaning").as_deref(), Some("ephemeral"));
        assert_eq!(word_after("meaning"), None);
    }

    #[test]
    fn set_out_as_dictionary_has_it() {
        let d = set_out("serendipity ser·en·dip·i·ty | ˌserənˈdipədē | noun the occurrence and development of events by chance in a happy or beneficial way: a fortunate stroke of serendipity. ORIGIN 1754: coined by Horace Walpole.");
        assert_eq!(d.word, "serendipity");
        assert_eq!(d.said, "ˌserənˈdipədē");
        assert_eq!(d.senses, "noun the occurrence and development of events by chance in a happy or beneficial way: a fortunate stroke of serendipity.");
        let d = set_out("run | rən | verb (runs, running) [no object] 1 move at a speed faster than a walk: the dog ran. 2 pass or cause to pass quickly: she ran her fingers through her hair. noun 1 an act or spell of running: a cross-country run. ORIGIN Old English.");
        assert_eq!(d.senses.lines().count(), 4, "{}", d.senses);
        assert!(d.senses.lines().nth(3).unwrap().starts_with("noun 1 an act"));
        let d = set_out("run | rən | verb (runs) (, running | ˈrəniNG |) (past; ran | ran |) [no object] 1 move fast: the dog ran | he has run 42 marathons. 2 pass quickly.");
        assert_eq!(d.senses, "verb (runs) [no object]\n1 move fast: the dog ran | he has run 42 marathons.\n2 pass quickly.");
    }
}

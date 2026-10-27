//! Emoji and symbols for Quick Open, as Raycast's picker: found by their
//! names and the words Unicode gives them ("thumbs", "+1", "check"), the
//! ones used lately first. The table (`assets/emoji.tsv`) is made from
//! Unicode's emoji list and CLDR's annotations, plus arrows, keys, math,
//! typography and currency.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const TABLE: &str = include_str!("../assets/emoji.tsv");
/// How many lately used are kept.
const KEPT: usize = 27;

pub struct Emoji {
    pub text: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    words: &'static str,
}

pub fn all() -> &'static [Emoji] {
    static ALL: OnceLock<Vec<Emoji>> = OnceLock::new();
    ALL.get_or_init(|| {
        TABLE
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|line| {
                let mut f = line.split('\t');
                Some(Emoji { text: f.next()?, name: f.next()?, group: f.next()?, words: f.next().unwrap_or("") })
            })
            .collect()
    })
}

/// Those matching `query`, best first.
pub fn find(query: &str) -> Vec<&'static Emoji> {
    let query = query.trim().to_lowercase();
    let mut found: Vec<(f32, usize, &Emoji)> = all()
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let named = crate::apps::name_score(e.name, &query);
            // (Any word of its, or any of its keywords: "+1", "thumbs up".)
            let worded = e.words.split(" | ").flat_map(|w| std::iter::once(w).chain(w.split(' '))).filter_map(|w| crate::apps::name_score(w, &query)).fold(None, |b: Option<f32>, s| Some(b.map_or(s, |b| b.max(s))));
            // (Its name counts most: "thumbs" is 👍's name's start, and only a
            // word of 👎's.)
            let score = match (named.map(|s| s + 20.), worded.map(|s| s - 2.)) {
                (Some(a), Some(b)) => a.max(b),
                (a, b) => a.or(b)?,
            };
            // (Good matches only: subsequences find too much here.)
            (score >= 40.).then_some((score, i, e))
        })
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, e)| e).collect()
}

/// The groups, in Unicode's order, with what's in each.
pub fn groups() -> Vec<(&'static str, Vec<&'static Emoji>)> {
    let mut groups: Vec<(&'static str, Vec<&'static Emoji>)> = Vec::new();
    for e in all() {
        match groups.last_mut() {
            Some((group, list)) if *group == e.group => list.push(e),
            _ => groups.push((e.group, vec![e])),
        }
    }
    groups
}

fn file() -> PathBuf {
    crate::settings::data_dir().join("emoji-lately.json")
}

static LATELY: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Those used lately, newest first.
pub fn lately() -> Vec<&'static Emoji> {
    let mut lately = LATELY.lock().unwrap();
    let list = lately.get_or_insert_with(|| std::fs::read(file()).ok().and_then(|t| serde_json::from_slice(&t).ok()).unwrap_or_default());
    list.iter().filter_map(|text| all().iter().find(|e| e.text == text)).collect()
}

/// Remember that `text` was used.
pub fn used(text: &str) {
    let mut lately = LATELY.lock().unwrap();
    let list = lately.get_or_insert_with(|| std::fs::read(file()).ok().and_then(|t| serde_json::from_slice(&t).ok()).unwrap_or_default());
    list.retain(|t| t != text);
    list.insert(0, text.to_string());
    list.truncate(KEPT);
    if let Ok(json) = serde_json::to_vec(&*list) {
        let _ = std::fs::write(file(), json);
    }
}

#[cfg(test)]
mod tests {
    use super::{all, find, groups};

    #[test]
    fn finds_by_name_and_word() {
        assert!(all().len() > 1900);
        assert_eq!(find("thumbs up").first().map(|e| e.text), Some("👍"));
        assert_eq!(find("thumbs").first().map(|e| e.text), Some("👍"), "Unicode's order when names tie");
        assert!(find("+1").iter().any(|e| e.text == "👍"), "by a keyword");
        assert!(find("check").iter().take(6).any(|e| e.text == "✓" || e.text == "✅"));
        assert!(find("command").iter().any(|e| e.text == "⌘"));
        assert!(find("zzqqxx").is_empty());
        assert_eq!(groups().first().map(|g| g.0), Some("Smileys & Emotion"));
        assert_eq!(groups().last().map(|g| g.0), Some("Text Symbols"));
    }
}

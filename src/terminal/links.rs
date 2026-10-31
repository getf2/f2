//! What ⌘-click opens: a link a program made (OSC 8, as `ls --hyperlink`
//! and gcc make them; see terminal.rs), else a URL in the text, else a path
//! that's there (from where the program runs: `src/main.rs:42` as a
//! compiler prints it, `~/Downloads`, git's `a/src/x.rs`).

use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Target {
    Url(String),
    /// As printed, without its line and column (`:42:5`).
    Path(String),
}

const SCHEMES: [&str; 5] = ["https://", "http://", "file://", "ftp://", "mailto:"];

/// The link in `line` (its characters) at `at`, and where it is in it (end
/// not in it).
pub fn at(line: &[char], at: usize) -> Option<(Range<usize>, Target)> {
    if at >= line.len() || line[at].is_whitespace() {
        return None;
    }
    url(line, at).or_else(|| path(line, at))
}

/// The run of characters round `at` that `keep` keeps.
fn run(line: &[char], at: usize, keep: impl Fn(char) -> bool) -> Range<usize> {
    let mut start = at;
    while start > 0 && keep(line[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end < line.len() && keep(line[end]) {
        end += 1;
    }
    start..end
}

fn url(line: &[char], at: usize) -> Option<(Range<usize>, Target)> {
    let around = run(line, at, |c| !c.is_whitespace() && !"\"'<>`{}|^\\".contains(c));
    // (Where the last scheme before it starts, and how long that is.)
    let (start, scheme) = (around.start..=at).rev().find_map(|i| {
        let scheme = SCHEMES.iter().find(|s| line[i..around.end].iter().copied().take(s.len()).eq(s.chars()))?;
        Some((i, scheme.len()))
    })?;
    let mut end = around.end;
    // (Not the sentence's full stop, or the bracket round it.)
    loop {
        let text = &line[start..end];
        let count = |c: char| text.iter().filter(|&&t| t == c).count();
        match line[end - 1] {
            '.' | ',' | ':' | ';' | '!' | '?' => end -= 1,
            ')' if count(')') > count('(') => end -= 1,
            ']' if count(']') > count('[') => end -= 1,
            _ => break,
        }
    }
    let url: String = line[start..end].iter().collect();
    (at < end && end - start > scheme).then_some((start..end, Target::Url(url)))
}

fn path(line: &[char], at: usize) -> Option<(Range<usize>, Target)> {
    let around = run(line, at, |c| !c.is_whitespace() && !"\"'<>`()[]{}|,;".contains(c));
    let mut end = around.end;
    while end > around.start && matches!(line[end - 1], '.' | ':') {
        end -= 1;
    }
    if at >= end {
        return None;
    }
    let token: String = line[around.start..end].iter().collect();
    // (Its line and column, as compilers print them: not the file's name.)
    let mut path = token.as_str();
    for _ in 0..2 {
        if let Some((before, after)) = path.rsplit_once(':')
            && !after.is_empty()
            && after.chars().all(|c| c.is_ascii_digit())
        {
            path = before;
        }
    }
    (!path.is_empty()).then(|| (around.start..end, Target::Path(path.to_string())))
}

/// Where a path printed as `text` is, from `cwd` (where the program runs),
/// if it's there.
pub fn resolve(text: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let full = |text: &str| -> Option<PathBuf> {
        let path = if text == "~" {
            home()?
        } else if let Some(rest) = text.strip_prefix("~/") {
            home()?.join(rest)
        } else if text.starts_with('/') {
            PathBuf::from(text)
        } else {
            cwd?.join(text)
        };
        path.exists().then_some(path)
    };
    // (git diff's `a/src/x.rs` and `b/src/x.rs`.)
    full(text).or_else(|| text.strip_prefix("a/").or_else(|| text.strip_prefix("b/")).and_then(full))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(text: &str, at: &str) -> Option<(String, Target)> {
        let line: Vec<char> = text.chars().collect();
        let i = text[..text.find(at).unwrap()].chars().count();
        super::at(&line, i).map(|(r, t)| (line[r].iter().collect(), t))
    }

    #[test]
    fn urls() {
        let url = |s: &str| Target::Url(s.to_string());
        assert_eq!(link("see https://getf2.com/features.", "getf2"), Some(("https://getf2.com/features".into(), url("https://getf2.com/features"))));
        assert_eq!(
            link("(https://en.wikipedia.org/wiki/F2_(disambiguation))", "wiki"),
            Some(("https://en.wikipedia.org/wiki/F2_(disambiguation)".into(), url("https://en.wikipedia.org/wiki/F2_(disambiguation)")))
        );
        assert_eq!(link("<http://localhost:3000/a?b=c>", "local").map(|l| l.0), Some("http://localhost:3000/a?b=c".into()));
        assert_eq!(link("https:// alone", "https").map(|l| l.1), Some(Target::Path("https://".into())), "no URL: a path, if it's there");
    }

    #[test]
    fn paths() {
        let path = |s: &str| Target::Path(s.to_string());
        assert_eq!(link("error: --> src/main.rs:42:5", "main"), Some(("src/main.rs:42:5".into(), path("src/main.rs"))));
        assert_eq!(link("modified:   assets/terminal/themes.tsv", "themes"), Some(("assets/terminal/themes.tsv".into(), path("assets/terminal/themes.tsv"))));
        assert_eq!(link("open ~/Downloads.", "Down"), Some(("~/Downloads".into(), path("~/Downloads"))));
        assert_eq!(link("(Cargo.toml)", "Cargo"), Some(("Cargo.toml".into(), path("Cargo.toml"))));
        assert_eq!(link("a  b", " b").map(|l| l.0), None, "a space");
    }

    #[test]
    fn resolved() {
        let dir = std::env::temp_dir().join(format!("f2-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/x.rs"), "").unwrap();
        assert_eq!(resolve("src/x.rs", Some(&dir)), Some(dir.join("src/x.rs")));
        assert_eq!(resolve("b/src/x.rs", Some(&dir)), Some(dir.join("src/x.rs")), "git's b/");
        assert_eq!(resolve("src/nope.rs", Some(&dir)), None);
        assert_eq!(resolve("src/x.rs", None), None, "no folder to go from");
        let _ = std::fs::remove_dir_all(dir);
    }
}

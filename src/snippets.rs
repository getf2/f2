//! Snippets, as Raycast's: text kept to paste, found by name in Quick Open,
//! or typed as its keyword in any app (";date"), which then becomes the
//! text: the keyword is taken back (⌫), the text pasted, and the clipboard
//! put back as it was. "{clipboard}", "{date}", "{time}", "{datetime}",
//! "{day}" and "{uuid}" are filled in; the text cursor goes where
//! "{cursor}" is.
//!
//! Hearing keywords takes Accessibility (macOS's key monitors), and hears
//! nothing in password fields: macOS keeps those from every app. Only the
//! last few characters typed are kept, to match against keywords.

use std::sync::Mutex;
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::App;

use crate::macos;
use crate::settings::{Settings, Snippet};

/// The snippets, as their keywords were given to the listener (by index).
static LISTENING: Mutex<Vec<Snippet>> = Mutex::new(Vec::new());
static TYPED: std::sync::OnceLock<UnboundedSender<usize>> = std::sync::OnceLock::new();

extern "C" fn on_match(index: i32) {
    if let Some(tx) = TYPED.get() {
        let _ = tx.unbounded_send(index as usize);
    }
}

/// `text` with its placeholders filled in, and how many characters follow
/// "{cursor}" (to go back over), if it has one.
pub fn resolve(text: &str) -> (String, usize) {
    let now = chrono::Local::now();
    let mut out = text.to_string();
    let fills = [
        ("{date}", now.format("%Y-%m-%d").to_string()),
        ("{time}", now.format("%H:%M").to_string()),
        ("{datetime}", now.format("%Y-%m-%d %H:%M").to_string()),
        ("{day}", now.format("%A").to_string()),
    ];
    for (name, value) in fills {
        out = out.replace(name, &value);
    }
    while out.contains("{uuid}") {
        out = out.replacen("{uuid}", &crate::dev::uuid(), 1);
    }
    if out.contains("{clipboard}") {
        out = out.replace("{clipboard}", &macos::clipboard_text().unwrap_or_default());
    }
    match out.find("{cursor}") {
        Some(at) => {
            let after = out[at + "{cursor}".len()..].chars().count();
            (out.replacen("{cursor}", "", 1), after)
        }
        None => (out, 0),
    }
}

/// Paste `text` into the app in front, then go back `back` characters, and
/// put the clipboard back as it was. (Off the main thread: it waits.)
pub fn paste_in(text: String, back: usize) {
    crate::clips::pause(true);
    macos::clipboard_save();
    macos::copy_text(&text);
    macos::post_paste();
    std::thread::sleep(Duration::from_millis(60));
    if back > 0 {
        macos::post_left(back);
    }
    // (Long enough for the app to have pasted.)
    std::thread::sleep(Duration::from_millis(350));
    macos::clipboard_restore();
    crate::clips::ignore_this();
    crate::clips::pause(false);
}

/// The keyword typed: taken back, and the snippet pasted for it.
fn expand(snippet: Snippet) {
    let typed = snippet.keyword.chars().count();
    let (text, back) = resolve(&snippet.text);
    std::thread::spawn(move || {
        macos::type_backspaces(typed);
        std::thread::sleep(Duration::from_millis(25));
        paste_in(text, back);
    });
}

/// Listen for the keywords Settings have (again when they change, and once
/// Accessibility's allowed).
pub fn init(cx: &mut App) {
    let (tx, mut rx) = unbounded();
    let _ = TYPED.set(tx);
    cx.spawn(async move |_| {
        while let Some(index) = rx.next().await {
            let snippet = LISTENING.lock().unwrap().get(index).cloned();
            if let Some(snippet) = snippet {
                expand(snippet);
            }
        }
    })
    .detach();
    let mut listening: Option<(Vec<Snippet>, bool)> = None;
    let mut listen = move |cx: &mut App| {
        let s = Settings::get(cx);
        let trusted = macos::accessibility(false);
        let wanted: Vec<Snippet> = if s.snippet_expansion { s.snippets.clone() } else { Vec::new() };
        if listening.as_ref() == Some(&(wanted.clone(), trusted)) {
            return;
        }
        // (Afresh once trusted: monitors made before may hear nothing.)
        if listening.as_ref().is_some_and(|(_, was)| !was && trusted) {
            macos::snippets_watch(&[], on_match);
        }
        let keywords: Vec<String> = wanted.iter().map(|s| s.keyword.trim().to_string()).collect();
        *LISTENING.lock().unwrap() = wanted.clone();
        macos::snippets_watch(&keywords, on_match);
        listening = Some((wanted, trusted));
    };
    listen(cx);
    let listen = std::rc::Rc::new(std::cell::RefCell::new(listen));
    let again = listen.clone();
    cx.observe_global::<Settings>(move |cx| (again.borrow_mut())(cx)).detach();
    // (Accessibility may be allowed later: looked at now and then.)
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            cx.update(|cx| (listen.borrow_mut())(cx));
        }
    })
    .detach();
}

/// Those matching `query` (lowercase), best first: by name, or by keyword.
pub fn find<'a>(snippets: &'a [Snippet], query: &str) -> Vec<(usize, &'a Snippet)> {
    let mut found: Vec<(f32, usize, &Snippet)> = snippets
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let keyword = s.keyword.to_lowercase();
            let by_keyword = (!keyword.is_empty() && keyword.starts_with(query)).then_some(90.);
            let score = match (crate::apps::name_score(&s.name, query), by_keyword) {
                (Some(a), Some(b)) => a.max(b),
                (a, b) => a.or(b)?,
            };
            (score >= 40.).then_some((score, i, s))
        })
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    found.into_iter().map(|(_, i, s)| (i, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::{find, resolve};
    use crate::settings::Snippet;

    #[test]
    fn placeholders() {
        let (text, back) = resolve("Hi {cursor}, see you");
        assert_eq!((text.as_str(), back), ("Hi , see you", 9));
        let (dated, _) = resolve("{date}");
        assert_eq!(dated.len(), 10);
        assert_eq!(resolve("{uuid} {uuid}").0.len(), 73, "each its own");
        let snippets = vec![
            Snippet { name: "Email Signature".into(), keyword: ";sig".into(), text: "—".into() },
            Snippet { name: "Shrug".into(), keyword: ";shrug".into(), text: "¯\\\\_(ツ)_/¯".into() },
        ];
        assert_eq!(find(&snippets, "email").first().map(|f| f.0), Some(0));
        assert_eq!(find(&snippets, ";sh").first().map(|f| f.0), Some(1));
        assert!(find(&snippets, "zz").is_empty());
    }
}

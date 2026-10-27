//! Translation, for Quick Open: "translate hola", "translate good morning
//! to japanese", "how do you say thank you in french", "thank you in
//! italian". Apple's, on this Mac (macOS 26, once the languages are
//! downloaded: System Settings > General > Language & Region > Translation
//! Languages), into this Mac's language unless another's asked; what Apple
//! doesn't translate (Icelandic, say), Google Translate does, in the
//! browser.

/// A language: its code (BCP 47, as Apple's and Google's take it), its
/// name, whether Apple translates it, and what else it's called.
#[derive(Debug, PartialEq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    apple: bool,
    names: &'static [&'static str],
}

const fn lang(code: &'static str, name: &'static str, apple: bool, names: &'static [&'static str]) -> Language {
    Language { code, name, apple, names }
}

const LANGUAGES: &[Language] = &[
    // Apple's (macOS 26's).
    lang("ar", "Arabic", true, &["arabic"]),
    lang("zh-Hans", "Chinese", true, &["chinese", "mandarin", "simplified chinese"]),
    lang("zh-Hant", "Traditional Chinese", true, &["traditional chinese"]),
    lang("da", "Danish", true, &["danish", "dansk"]),
    lang("nl", "Dutch", true, &["dutch", "nederlands", "flemish"]),
    lang("en", "English", true, &["english"]),
    lang("fr", "French", true, &["french", "francais", "français"]),
    lang("de", "German", true, &["german", "deutsch"]),
    lang("hi", "Hindi", true, &["hindi"]),
    lang("id", "Indonesian", true, &["indonesian", "bahasa"]),
    lang("it", "Italian", true, &["italian", "italiano"]),
    lang("ja", "Japanese", true, &["japanese", "nihongo"]),
    lang("ko", "Korean", true, &["korean"]),
    lang("nb", "Norwegian", true, &["norwegian", "norsk", "bokmal", "bokmål"]),
    lang("pl", "Polish", true, &["polish", "polski"]),
    lang("pt", "Portuguese", true, &["portuguese", "portugues", "português", "brazilian"]),
    lang("ru", "Russian", true, &["russian"]),
    lang("es", "Spanish", true, &["spanish", "espanol", "español", "castilian"]),
    lang("sv", "Swedish", true, &["swedish", "svenska"]),
    lang("th", "Thai", true, &["thai"]),
    lang("tr", "Turkish", true, &["turkish", "turkce", "türkçe"]),
    lang("uk", "Ukrainian", true, &["ukrainian"]),
    lang("vi", "Vietnamese", true, &["vietnamese"]),
    // Google's.
    lang("is", "Icelandic", false, &["icelandic", "islenska", "íslenska"]),
    lang("fi", "Finnish", false, &["finnish", "suomi"]),
    lang("el", "Greek", false, &["greek"]),
    lang("he", "Hebrew", false, &["hebrew"]),
    lang("cs", "Czech", false, &["czech"]),
    lang("hu", "Hungarian", false, &["hungarian", "magyar"]),
    lang("ro", "Romanian", false, &["romanian"]),
    lang("bg", "Bulgarian", false, &["bulgarian"]),
    lang("hr", "Croatian", false, &["croatian"]),
    lang("sr", "Serbian", false, &["serbian"]),
    lang("bs", "Bosnian", false, &["bosnian"]),
    lang("sk", "Slovak", false, &["slovak"]),
    lang("sl", "Slovenian", false, &["slovenian", "slovene"]),
    lang("et", "Estonian", false, &["estonian"]),
    lang("lv", "Latvian", false, &["latvian"]),
    lang("lt", "Lithuanian", false, &["lithuanian"]),
    lang("ga", "Irish", false, &["irish", "gaelic"]),
    lang("cy", "Welsh", false, &["welsh"]),
    lang("lb", "Luxembourgish", false, &["luxembourgish"]),
    lang("mt", "Maltese", false, &["maltese"]),
    lang("sq", "Albanian", false, &["albanian"]),
    lang("mk", "Macedonian", false, &["macedonian"]),
    lang("be", "Belarusian", false, &["belarusian"]),
    lang("hy", "Armenian", false, &["armenian"]),
    lang("ka", "Georgian", false, &["georgian"]),
    lang("az", "Azerbaijani", false, &["azerbaijani", "azeri"]),
    lang("kk", "Kazakh", false, &["kazakh"]),
    lang("uz", "Uzbek", false, &["uzbek"]),
    lang("mn", "Mongolian", false, &["mongolian"]),
    lang("ca", "Catalan", false, &["catalan"]),
    lang("eu", "Basque", false, &["basque"]),
    lang("gl", "Galician", false, &["galician"]),
    lang("la", "Latin", false, &["latin"]),
    lang("eo", "Esperanto", false, &["esperanto"]),
    lang("fa", "Persian", false, &["persian", "farsi"]),
    lang("ur", "Urdu", false, &["urdu"]),
    lang("bn", "Bengali", false, &["bengali", "bangla"]),
    lang("pa", "Punjabi", false, &["punjabi"]),
    lang("gu", "Gujarati", false, &["gujarati"]),
    lang("mr", "Marathi", false, &["marathi"]),
    lang("ta", "Tamil", false, &["tamil"]),
    lang("te", "Telugu", false, &["telugu"]),
    lang("kn", "Kannada", false, &["kannada"]),
    lang("ml", "Malayalam", false, &["malayalam"]),
    lang("ne", "Nepali", false, &["nepali"]),
    lang("si", "Sinhala", false, &["sinhala", "sinhalese"]),
    lang("km", "Khmer", false, &["khmer", "cambodian"]),
    lang("lo", "Lao", false, &["lao", "laotian"]),
    lang("my", "Burmese", false, &["burmese"]),
    lang("ms", "Malay", false, &["malay"]),
    lang("tl", "Filipino", false, &["filipino", "tagalog"]),
    lang("sw", "Swahili", false, &["swahili"]),
    lang("am", "Amharic", false, &["amharic"]),
    lang("so", "Somali", false, &["somali"]),
    lang("ha", "Hausa", false, &["hausa"]),
    lang("yo", "Yoruba", false, &["yoruba"]),
    lang("ig", "Igbo", false, &["igbo"]),
    lang("zu", "Zulu", false, &["zulu"]),
    lang("xh", "Xhosa", false, &["xhosa"]),
    lang("af", "Afrikaans", false, &["afrikaans"]),
    lang("ht", "Haitian Creole", false, &["haitian", "haitian creole", "creole"]),
    lang("haw", "Hawaiian", false, &["hawaiian"]),
    lang("mi", "Maori", false, &["maori", "māori"]),
    lang("sm", "Samoan", false, &["samoan"]),
    lang("yi", "Yiddish", false, &["yiddish"]),
];

/// The language called `name` ("japanese", "español").
fn named(name: &str) -> Option<&'static Language> {
    let name = name.trim().to_lowercase();
    LANGUAGES.iter().find(|l| l.names.contains(&name.as_str()))
}

/// The language of a code ("pt-BR": Portuguese; "zh-Hant-TW": Traditional
/// Chinese; "no": Norwegian).
pub fn by_code(code: &str) -> Option<&'static Language> {
    let lower = code.to_lowercase();
    if lower.starts_with("zh") {
        let traditional = lower.contains("hant") || lower.ends_with("-tw") || lower.ends_with("-hk") || lower.ends_with("-mo");
        return LANGUAGES.iter().find(|l| l.code == if traditional { "zh-Hant" } else { "zh-Hans" });
    }
    let base = lower.split(['-', '_']).next().unwrap_or("");
    let base = match base {
        "no" | "nn" => "nb",
        "iw" => "he",
        "fil" => "tl",
        base => base,
    };
    LANGUAGES.iter().find(|l| l.code == base)
}

/// The language being typed at the end of a translation ("translate hi to
/// jap": "japanese"), for Quick Open to show the rest of.
pub fn complete(q: &str) -> Option<&'static str> {
    let lower = q.to_lowercase();
    if !["translate ", "how do you say ", "how do i say ", "how to say "].iter().any(|p| lower.starts_with(p)) {
        return None;
    }
    let partial = [" to ", " into ", " in "].iter().filter_map(|w| lower.rfind(w).map(|at| &lower[at + w.len()..])).min_by_key(|p| p.len())?;
    if partial.is_empty() {
        return None;
    }
    LANGUAGES.iter().map(|l| l.names[0]).find(|name| name.starts_with(partial))
}

/// What a translation's still missing, for Quick Open's gray hint once a
/// word is done: the language to translate into ("to spanish"), or, after
/// "to", a language.
pub fn hint(text: &str) -> Option<String> {
    if !text.ends_with(' ') {
        return None;
    }
    let lower = text.to_lowercase();
    let saying = ["how do you say ", "how do i say ", "how to say "].iter().any(|p| lower.starts_with(p));
    if !saying && !lower.starts_with("translate ") {
        return None;
    }
    if [" to ", " into ", " in "].iter().any(|w| lower.ends_with(w)) {
        return Some("spanish".into());
    }
    let asked = request(text)?;
    asked.to.is_none().then(|| if saying { "in french".into() } else { "to spanish".into() })
}

/// What's asked: the text, and from what into what, if said.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub text: String,
    pub from: Option<&'static str>,
    pub to: Option<&'static str>,
}

/// `text`'s start, if it's `prefix` (any case): what's after.
fn after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len()).filter(|start| start.eq_ignore_ascii_case(prefix)).map(|_| &text[prefix.len()..])
}

/// "... to japanese": the text before, and the language at the end, after
/// one of `words`.
fn ending_in<'a>(text: &'a str, words: &[&str]) -> Option<(&'a str, &'static Language)> {
    // (A space before, so it's found at the start too: "to french", with
    // nothing to translate yet.)
    let lower = format!(" {}", text.to_lowercase());
    words.iter().find_map(|word| {
        let at = lower.rfind(&format!(" {word} "))?;
        let language = named(&lower[at + word.len() + 2..])?;
        // (Lowercasing keeps byte offsets for what's typed here, near enough:
        // checked, so a slice is never cut mid-character.)
        text.get(..at.saturating_sub(1)).map(|before| (before, language))
    })
}

/// What's asked to be translated: "translate X" (to, into or from a
/// language, said after), "how do you say X in L", "X in L".
pub fn request(q: &str) -> Option<Request> {
    let q = q.trim();
    let asked = ["translate ", "how do you say ", "how do i say ", "how to say "].iter().find_map(|p| after(q, p));
    let mut text = asked.unwrap_or(q);
    let mut to = None;
    if let Some((before, language)) = ending_in(text, &["to", "into", "in"]) {
        (text, to) = (before, Some(language.code));
    }
    let mut from = None;
    if let Some((before, language)) = ending_in(text, &["from"]) {
        (text, from) = (before, Some(language.code));
    }
    // (Without "translate", only with a language to translate into: "thank
    // you in japanese".)
    if asked.is_none() && to.is_none() {
        return None;
    }
    let text = text.trim().trim_matches(['"', '“', '”', '\'', '‘', '’']).trim();
    (!text.is_empty()).then(|| Request { text: text.to_string(), from, to })
}

/// How it'll be translated: by Apple, here, or by Google, in the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    Apple { text: String, from: &'static Language, to: &'static Language },
    Google { text: String, from: Option<&'static Language>, to: Option<&'static Language> },
}

/// Whether this macOS translates outside SwiftUI (26 and later).
fn apple_here() -> bool {
    let version = crate::macos::os_version();
    let major = version.split_whitespace().nth(1).and_then(|v| v.split('.').next()).and_then(|m| m.parse::<u32>().ok());
    major.is_some_and(|m| m >= 26)
}

/// What it's in (said, or guessed) and what it goes into (said, or this
/// Mac's first language that isn't it, or English), and who translates it.
pub fn plan(request: &Request) -> Plan {
    let preferred: Vec<&'static Language> = crate::macos::preferred_languages().iter().filter_map(|c| by_code(c)).collect();
    let from = request.from.and_then(by_code).or_else(|| crate::macos::language_of(&request.text).and_then(|c| by_code(&c)));
    let to = request.to.and_then(by_code).or_else(|| {
        let english = by_code("en");
        preferred.iter().copied().chain(english).find(|l| from.is_none_or(|f| f.code != l.code))
    });
    match (from, to) {
        (Some(from), Some(to)) if from.apple && to.apple && from.code != to.code && apple_here() => Plan::Apple { text: request.text.clone(), from, to },
        _ => Plan::Google { text: request.text.clone(), from, to },
    }
}

/// "Spanish → English", "→ Japanese".
pub fn direction(from: Option<&Language>, to: Option<&Language>) -> String {
    match (from, to) {
        (Some(f), Some(t)) => format!("{} → {}", f.name, t.name),
        (None, Some(t)) => format!("→ {}", t.name),
        (Some(f), None) => format!("{} →", f.name),
        (None, None) => String::new(),
    }
}

/// Google Translate's page for it.
pub fn google(text: &str, from: Option<&Language>, to: Option<&Language>) -> String {
    let code = |l: Option<&Language>, or: &str| l.map_or(or.to_string(), |l| l.code.replace("zh-Hans", "zh-CN").replace("zh-Hant", "zh-TW"));
    format!("https://translate.google.com/?sl={}&tl={}&text={}&op=translate", code(from, "auto"), code(to, "en"), crate::dev::url_encode(text))
}

/// How Apple's translating went.
pub enum Outcome {
    Done(String),
    /// Its languages aren't downloaded yet.
    NotDownloaded,
    Failed(String),
}

/// Translate it, here (waits: not on the main thread).
pub fn run(text: &str, from: &Language, to: &Language) -> Outcome {
    match crate::macos::translate(text, from.code, to.code, 8.) {
        Ok(done) => Outcome::Done(done),
        Err(why) if why == "not-installed" => Outcome::NotDownloaded,
        Err(why) if why == "timed-out" => Outcome::Failed("Translating took too long.".into()),
        Err(why) => Outcome::Failed(why),
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, by_code, request};

    fn asked(text: &str, from: Option<&'static str>, to: Option<&'static str>) -> Option<Request> {
        Some(Request { text: text.into(), from, to })
    }

    #[test]
    fn requests() {
        assert_eq!(request("translate hola"), asked("hola", None, None));
        assert_eq!(request("Translate good morning to Japanese"), asked("good morning", None, Some("ja")));
        assert_eq!(request("translate bonjour from french into german"), asked("bonjour", Some("fr"), Some("de")));
        assert_eq!(request("how do you say thank you in italian"), asked("thank you", None, Some("it")));
        assert_eq!(request("thank you in icelandic"), asked("thank you", None, Some("is")));
        assert_eq!(request("translate “where is the station?”"), asked("where is the station?", None, None));
        assert_eq!(request("photos in spain"), None, "not a language");
        assert_eq!(request("time in japan"), None);
        assert_eq!(request("translate"), None);
        assert_eq!(request("translate to french"), None, "nothing to translate yet");
    }

    #[test]
    fn completes() {
        assert_eq!(super::complete("translate hello to jap"), Some("japanese"));
        assert_eq!(super::complete("how do you say thanks in fr"), Some("french"));
        assert_eq!(super::complete("translate hello to "), None);
        assert_eq!(super::complete("hello to jap"), None, "only in a translation");
    }

    #[test]
    fn codes() {
        assert_eq!(by_code("pt-BR").map(|l| l.name), Some("Portuguese"));
        assert_eq!(by_code("zh-Hant-TW").map(|l| l.code), Some("zh-Hant"));
        assert_eq!(by_code("zh-CN").map(|l| l.code), Some("zh-Hans"));
        assert_eq!(by_code("no").map(|l| l.code), Some("nb"));
        assert_eq!(by_code("is-IS").map(|l| l.name), Some("Icelandic"));
    }
}

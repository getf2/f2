//! Reaching people from Quick Open: "call mom", "facetime sara", "message
//! dad", "email jun". Found in Contacts (macOS asks, the first time), then
//! called through the iPhone (or FaceTime), messaged in Messages, emailed
//! in Mail: the Mac's own apps, by their links.

/// How someone's reached.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Way {
    Call,
    FaceTime,
    FaceTimeAudio,
    Message,
    Email,
}

impl Way {
    /// "Call", "FaceTime", "Message", "Email".
    pub fn verb(self) -> &'static str {
        match self {
            Way::Call => "Call",
            Way::FaceTime => "FaceTime",
            Way::FaceTimeAudio => "FaceTime Audio",
            Way::Message => "Message",
            Way::Email => "Email",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Way::Call | Way::FaceTimeAudio => "phone.fill",
            Way::FaceTime => "video.fill",
            Way::Message => "message.fill",
            Way::Email => "envelope.fill",
        }
    }

    /// Whether it goes to a number (else an email address), or either.
    fn takes(self, email: bool) -> bool {
        match self {
            Way::Call => !email,
            Way::Email => email,
            Way::FaceTime | Way::FaceTimeAudio | Way::Message => true,
        }
    }

    /// The link that does it.
    pub fn link(self, value: &str, email: bool) -> String {
        // (A number's digits and +: "(555) 010-1234" isn't a link's.)
        let to = if email { value.trim().to_string() } else { value.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect() };
        match self {
            Way::Call => format!("tel:{to}"),
            Way::FaceTime => format!("facetime:{to}"),
            Way::FaceTimeAudio => format!("facetime-audio:{to}"),
            Way::Message if email => format!("imessage:{to}"),
            Way::Message => format!("sms:{to}"),
            Way::Email => format!("mailto:{to}"),
        }
    }
}

/// What's asked: how, and who ("call mom": Call, "mom").
pub fn asked(q: &str) -> Option<(Way, String)> {
    let lower = q.trim().to_lowercase();
    const WAYS: &[(&str, Way)] = &[
        ("facetime audio ", Way::FaceTimeAudio),
        ("facetime ", Way::FaceTime),
        ("video call ", Way::FaceTime),
        ("call ", Way::Call),
        ("phone ", Way::Call),
        ("ring ", Way::Call),
        ("message ", Way::Message),
        ("imessage ", Way::Message),
        ("text ", Way::Message),
        ("sms ", Way::Message),
        ("email ", Way::Email),
        ("e-mail ", Way::Email),
        ("mail ", Way::Email),
        ("write to ", Way::Email),
    ];
    let (prefix, way) = WAYS.iter().find(|(p, _)| lower.starts_with(p))?;
    let who = q.trim()[prefix.len()..].trim();
    (!who.is_empty()).then(|| (*way, who.to_string()))
}

/// `text`, if it's a phone number: "090 1234 5678", "+1 (555) 010-1234".
pub fn phone_number(text: &str) -> Option<String> {
    let t = text.trim();
    let digits = t.chars().filter(char::is_ascii_digit).count();
    let phoney = t.chars().all(|c| c.is_ascii_digit() || matches!(c, ' ' | '+' | '-' | '(' | ')' | '.'));
    let starts = t.starts_with(|c: char| c.is_ascii_digit() || c == '+' || c == '(');
    (phoney && starts && (7..=15).contains(&digits) && t.matches('+').count() <= 1).then(|| t.to_string())
}

/// A row's worth: whom, how (their label and number or address), and the
/// link that does it.
pub struct Reach {
    pub title: String,
    pub about: String,
    pub link: String,
}

/// Ways to reach who's named: the name as typed, else the first words of
/// it ("text mom running late": mom), at most `limit`. None: Contacts can't
/// be looked at.
pub fn find(way: Way, who: &str, limit: usize) -> Option<Vec<Reach>> {
    let words: Vec<&str> = who.split_whitespace().collect();
    for n in (1..=words.len().min(4)).rev() {
        let name = words[..n].join(" ");
        let found = crate::macos::contacts_find(&name, 6)?;
        let reaches: Vec<Reach> = found
            .into_iter()
            .filter(|r| way.takes(r.email))
            .take(limit)
            .map(|r| Reach {
                title: format!("{} {}", way.verb(), r.name),
                about: if r.label.is_empty() { r.value.clone() } else { format!("{} · {}", r.label, r.value) },
                link: way.link(&r.value, r.email),
            })
            .collect();
        if !reaches.is_empty() {
            return Some(reaches);
        }
    }
    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{Way, asked};

    #[test]
    fn ways() {
        assert_eq!(asked("call mom"), Some((Way::Call, "mom".into())));
        assert_eq!(asked("FaceTime Sara Lee"), Some((Way::FaceTime, "Sara Lee".into())));
        assert_eq!(asked("facetime audio dad"), Some((Way::FaceTimeAudio, "dad".into())));
        assert_eq!(asked("text jun running late"), Some((Way::Message, "jun running late".into())));
        assert_eq!(asked("email boss"), Some((Way::Email, "boss".into())));
        assert_eq!(asked("call"), None);
        assert_eq!(asked("calling card"), None);
    }

    #[test]
    fn numbers() {
        use super::phone_number;
        assert_eq!(phone_number("090 1234 5678").as_deref(), Some("090 1234 5678"));
        assert!(phone_number("+1 (555) 010-1234").is_some());
        assert!(phone_number("mom").is_none());
        assert!(phone_number("12345").is_none(), "too short");
        assert!(phone_number("1+2+3+4+5+6+7").is_none());
    }

    #[test]
    fn links() {
        assert_eq!(Way::Call.link("+1 (555) 010-1234", false), "tel:+15550101234");
        assert_eq!(Way::Message.link("sara@example.com", true), "imessage:sara@example.com");
        assert_eq!(Way::Message.link("090 1234 5678", false), "sms:09012345678");
        assert_eq!(Way::Email.link("sara@example.com", true), "mailto:sara@example.com");
    }
}

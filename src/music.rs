//! Music, for Quick Open: what's playing ("now playing", "what's
//! playing"), in Music or Spotify, with its artwork, to pause, skip or love;
//! and playing something ("play taylor swift", "play chill playlist"): the
//! best match in your Music library (a playlist by that name, an artist's,
//! an album's, else what Music's search finds), or a search in Apple Music
//! or Spotify. Music and Spotify are asked by Apple Events (macOS asks you
//! once, for each): only when they're open, and only when you ask.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Player {
    Music,
    Spotify,
}

impl Player {
    pub fn name(self) -> &'static str {
        match self {
            Player::Music => "Music",
            Player::Spotify => "Spotify",
        }
    }

    pub fn bundle(self) -> &'static str {
        match self {
            Player::Music => "com.apple.Music",
            Player::Spotify => "com.spotify.client",
        }
    }

    /// Where the app is (Spotify's: where it's installed, if it is).
    pub fn app(self) -> Option<PathBuf> {
        match self {
            Player::Music => Some(PathBuf::from("/System/Applications/Music.app")),
            Player::Spotify => {
                let home = std::env::var("HOME").unwrap_or_default();
                [PathBuf::from("/Applications/Spotify.app"), Path::new(&home).join("Applications/Spotify.app")].into_iter().find(|p| p.exists())
            }
        }
    }
}

/// What can be done to what's playing.
#[derive(Clone, Debug, PartialEq)]
pub enum Do {
    PlayPause,
    /// Play (what was playing, or whatever's next).
    Resume,
    Next,
    Previous,
    /// The song playing marked a favorite (Music's).
    Love,
    /// The best match in the Music library, played.
    Play(String),
}

/// The players open, Music first.
pub fn open_players() -> Vec<Player> {
    let running = crate::macos::running_apps();
    let open = |name: &str| running.iter().any(|(_, _, path)| path.file_name().is_some_and(|n| n == name));
    [(Player::Music, "Music.app"), (Player::Spotify, "Spotify.app")].into_iter().filter(|(_, app)| open(app)).map(|(p, _)| p).collect()
}

pub fn now_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches(['?', '!', '.']).to_lowercase().replace('’', "'");
    const ASKED: &[&str] = &[
        "now playing",
        "what's playing",
        "whats playing",
        "what is playing",
        "what's playing now",
        "playing now",
        "current song",
        "current track",
        "this song",
        "what song is playing",
        "what song is this playing",
        "song playing",
        "what am i listening to",
        "what's this song",
        "song name",
    ];
    ASKED.contains(&lower.as_str())
}

/// "play taylor swift", "play chill playlist", "play 1989 on spotify":
/// what to play, and in what if it's said. ("play" alone is Play/Pause's;
/// "play music" resumes: `Do::Resume`.)
pub fn play_asked(q: &str) -> Option<(String, Option<Player>)> {
    let q = q.trim();
    let lower = q.to_lowercase();
    if !lower.starts_with("play ") || resume_asked(q) {
        return None;
    }
    let mut what = q[5..].trim().to_string();
    let mut said = None;
    for (suffix, player) in [(" on spotify", Player::Spotify), (" in spotify", Player::Spotify), (" on apple music", Player::Music), (" in apple music", Player::Music), (" in music", Player::Music)] {
        if what.to_lowercase().ends_with(suffix) {
            what.truncate(what.len() - suffix.len());
            said = Some(player);
            break;
        }
    }
    let what = what.trim().to_string();
    let plain = what.to_lowercase();
    if what.is_empty() || what.chars().count() > 60 || matches!(plain.as_str(), "pause" | "/pause" | "next" | "previous" | "store" | "date" | "button") {
        return None;
    }
    Some((what, said))
}

/// "play music", "play some music", "resume music".
pub fn resume_asked(q: &str) -> bool {
    matches!(q.trim().to_lowercase().as_str(), "play music" | "play some music" | "play something" | "resume music" | "resume playing" | "play my music")
}

/// A playlist asked for by name: "chill playlist", "playlist chill", "my
/// chill playlist" are "chill".
fn playlist_named(what: &str) -> String {
    let words: Vec<&str> = what.split_whitespace().filter(|w| !matches!(w.to_lowercase().as_str(), "playlist" | "my" | "the")).collect();
    if words.is_empty() { what.to_string() } else { words.join(" ") }
}

/// A string for AppleScript, quoted.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// An AppleScript, run (by osascript: macOS counts its Apple Events as
/// F2's). What it returns, or why not.
fn script(source: &str) -> Result<String, String> {
    let out = Command::new("/usr/bin/osascript").args(["-e", source]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// What's playing (or paused).
#[derive(Clone, Debug, PartialEq)]
pub struct Playing {
    pub player: Player,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// How long, and how far in (seconds).
    pub length: u32,
    pub at: u32,
    /// Whether it's a favorite (Music's; Spotify doesn't say).
    pub loved: Option<bool>,
    pub artwork: Option<PathBuf>,
}

/// What's playing, if anything, or why it can't be told.
#[derive(Clone, Debug, PartialEq)]
pub enum Now {
    Playing(Playing),
    /// The player's open, but stopped.
    Stopped(Player),
    /// Neither's open.
    NotOpen,
    /// macOS would ask first: F2 hasn't been allowed to ask it yet.
    Ask(Player),
    /// It's been said no, in System Settings.
    Denied(Player),
    /// It was asked, and said something F2 didn't know (in the log).
    Failed(Player),
}

/// Where artwork's kept: one file a song (GPUI keeps images by their path).
fn artwork_dir() -> PathBuf {
    crate::settings::data_dir().join("Now Playing")
}

/// The artwork's file for a song, its name its hash, the others gone.
fn artwork_file(song: &str, bytes: &[u8]) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let kind = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        "jpg"
    } else {
        return None;
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    song.hash(&mut hasher);
    let dir = artwork_dir();
    let file = dir.join(format!("{:016x}.{kind}", hasher.finish()));
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten().filter(|e| e.path() != file) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    std::fs::write(&file, bytes).ok()?;
    Some(file)
}

/// Music's song: "state, title, artist, album, length, at, loved", its
/// artwork written to `art`.
fn music_now(art: &Path) -> Result<Option<Playing>, String> {
    let out = script(&music_now_script(art))?;
    if out == "stopped" {
        return Ok(None);
    }
    let parts: Vec<&str> = out.split('\t').collect();
    let [state, title, artist, album, length, at, loved, ..] = parts.as_slice() else { return Err("Music said something else.".into()) };
    let song = format!("{title}\t{artist}\t{album}");
    let artwork = std::fs::read(art).ok().and_then(|bytes| artwork_file(&song, &bytes));
    let _ = std::fs::remove_file(art);
    Ok(Some(Playing {
        player: Player::Music,
        playing: *state == "playing",
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        length: length.parse().unwrap_or(0),
        at: at.parse().unwrap_or(0),
        loved: match *loved {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        artwork,
    }))
}

fn music_now_script(art: &Path) -> String {
    format!(
        r#"tell application "Music"
	set s to player state as text
	if s is "stopped" then return "stopped"
	set t to current track
	set l to ""
	try
		set l to (favorited of t) as text
	end try
	set out to s & tab & (name of t) & tab & (artist of t) & tab & (album of t) & tab & ((round (duration of t)) as integer) & tab & ((round (player position)) as integer) & tab & l
	set d to missing value
	try
		set d to raw data of artwork 1 of t
	end try
end tell
if d is not missing value then
	try
		set f to open for access (POSIX file {art}) with write permission
		set eof f to 0
		write d to f
		close access f
	end try
end if
return out"#,
        art = quoted(&art.to_string_lossy())
    )
}

/// Spotify's song, its artwork fetched (Spotify gives its address). (Its
/// numbers rounded here, not by Spotify: `round` told to Spotify fails,
/// "Can't make player position into type real".)
const SPOTIFY_NOW: &str = r#"tell application "Spotify"
	set s to player state as text
	if s is "stopped" then return "stopped"
	set t to current track
	set n to name of t
	set a to artist of t
	set al to album of t
	set d to 0
	set p to 0
	set u to ""
	try
		set d to duration of t
	end try
	try
		set p to player position
	end try
	try
		set u to artwork url of t
	end try
end tell
set secs to 0
set pos to 0
try
	set secs to (round (d / 1000)) as integer
end try
try
	set pos to (round p) as integer
end try
if u is missing value then set u to ""
return s & tab & (n as text) & tab & (a as text) & tab & (al as text) & tab & secs & tab & pos & tab & u"#;

fn spotify_now() -> Result<Option<Playing>, String> {
    let out = script(SPOTIFY_NOW)?;
    if out == "stopped" {
        return Ok(None);
    }
    let parts: Vec<&str> = out.split('\t').collect();
    let [state, title, artist, album, length, at, art, ..] = parts.as_slice() else { return Err("Spotify said something else.".into()) };
    let song = format!("{title}\t{artist}\t{album}");
    let artwork = art.starts_with("https://").then(|| Command::new("/usr/bin/curl").args(["-fsS", "--max-time", "4", art]).output().ok()).flatten().filter(|o| o.status.success()).and_then(|o| artwork_file(&song, &o.stdout));
    Ok(Some(Playing {
        player: Player::Spotify,
        playing: *state == "playing",
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        length: length.parse().unwrap_or(0),
        at: at.parse().unwrap_or(0),
        loved: None,
        artwork,
    }))
}

/// What's playing: the player that's playing (else one that's paused).
/// Asks the players open only, and only those F2 may ask. (Waits on them:
/// not on the main thread.)
pub fn now() -> Now {
    let open = open_players();
    if open.is_empty() {
        return Now::NotOpen;
    }
    let mut paused = None;
    let mut stopped = None;
    let mut blocked = None;
    let mut failed = None;
    for player in open {
        match crate::macos::automation(player.bundle(), false) {
            0 => {}
            -1744 => {
                blocked.get_or_insert(Now::Ask(player));
                continue;
            }
            -600 => continue,
            _ => {
                blocked.get_or_insert(Now::Denied(player));
                continue;
            }
        }
        let found = match player {
            Player::Music => music_now(&std::env::temp_dir().join(format!("f2-artwork-{}", std::process::id()))),
            Player::Spotify => spotify_now(),
        };
        match found {
            Ok(Some(p)) if p.playing => return Now::Playing(p),
            Ok(Some(p)) => {
                paused.get_or_insert(p);
            }
            Ok(None) => {
                stopped.get_or_insert(player);
            }
            Err(why) => {
                eprintln!("couldn't ask {} what's playing: {why}", player.name());
                failed.get_or_insert(Now::Failed(player));
            }
        }
    }
    paused.map(Now::Playing).or(blocked).or(failed).or(stopped.map(Now::Stopped)).unwrap_or(Now::NotOpen)
}

/// "1:05", "1:02:03".
pub fn clock(seconds: u32) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Do it, in `player`: a note's title and detail if there's something to
/// say. (Waits on the player: not on the main thread.)
pub fn run(player: Player, what: &Do) -> Option<(String, String)> {
    let app = player.name();
    let tell = |verb: &str| script(&format!("tell application \"{app}\" to {verb}")).map(|_| ());
    match what {
        Do::PlayPause => tell("playpause").err().map(|why| (format!("Couldn't Ask {app}"), why)),
        Do::Resume => tell("play").err().map(|why| (format!("Couldn't Ask {app}"), why)),
        Do::Next => tell("next track").err().map(|why| (format!("Couldn't Ask {app}"), why)),
        Do::Previous => tell("previous track").err().map(|why| (format!("Couldn't Ask {app}"), why)),
        Do::Love => {
            let out = script(LOVE);
            Some(match out.as_deref().map(|o| o.split_once('\t')) {
                Ok(Some((title, artist))) => (format!("Loved “{title}”"), artist.to_string()),
                Ok(None) => ("Nothing's Playing".into(), "Music isn't playing anything to love.".into()),
                Err(why) => ("Couldn't Love It".into(), why.clone()),
            })
        }
        Do::Play(what) => Some(play_in_music(what)),
    }
}

const LOVE: &str = r#"tell application "Music"
	if player state is stopped then return "none"
	set t to current track
	try
		set favorited of t to true
	on error
		set loved of t to true
	end try
	return (name of t) & tab & (artist of t)
end tell"#;

/// The best match in the Music library, played: a playlist by that name,
/// an artist's song, an album's, else what Music's search finds first.
fn play_in_music(what: &str) -> (String, String) {
    play_in_music_said(script(&play_script(what)), what)
}

fn play_script(what: &str) -> String {
    format!(
        r#"set q to {q}
set p to {p}
tell application "Music"
	try
		set l to (first user playlist whose name is p)
		play l
		return "playlist" & tab & (name of l)
	end try
	set found to (every track of library playlist 1 whose artist is q)
	if found is {{}} then set found to (every track of library playlist 1 whose album is q)
	if found is {{}} then set found to (search library playlist 1 for q)
	if found is {{}} then return "none"
	set t to item 1 of found
	play t
	return "track" & tab & (name of t) & tab & (artist of t)
end tell"#,
        q = quoted(what),
        p = quoted(&playlist_named(what))
    )
}

fn play_in_music_said(out: Result<String, String>, what: &str) -> (String, String) {
    match out {
        Ok(out) => {
            let parts: Vec<&str> = out.split('\t').collect();
            match parts.as_slice() {
                ["playlist", name] => (format!("Playing “{name}”"), "Your playlist, in Music".into()),
                ["track", title, artist] => (format!("Playing “{title}”"), format!("{artist} · from your library")),
                _ => (format!("Nothing Called “{what}”"), "Not in your Music library: search Apple Music for it instead.".into()),
            }
        }
        Err(why) if why.contains("-1743") => ("F2 Can't Ask Music".into(), "Allow it in System Settings > Privacy & Security > Automation.".into()),
        Err(why) => ("Couldn't Play It".into(), why),
    }
}

/// Apple Music's search (in Music), and Spotify's (in Spotify).
pub fn search_links(what: &str) -> (String, String) {
    let e = crate::dev::url_encode(what);
    (format!("music://music.apple.com/search?term={e}"), format!("spotify:search:{e}"))
}

#[cfg(test)]
mod tests {
    use super::{LOVE, Player, SPOTIFY_NOW, clock, music_now_script, now_asked, play_asked, play_script, playlist_named};

    /// The scripts compile, by the apps' dictionaries (no Apple Events
    /// sent: nothing's asked of Music or Spotify).
    #[test]
    fn scripts() {
        let compiles = |source: &str| {
            let out = std::env::temp_dir().join(format!("f2-test-{}.scpt", std::process::id()));
            let done = std::process::Command::new("/usr/bin/osacompile").args(["-o"]).arg(&out).args(["-e", source]).output().unwrap();
            let _ = std::fs::remove_file(&out);
            assert!(done.status.success(), "{}\n{source}", String::from_utf8_lossy(&done.stderr));
        };
        compiles(&music_now_script(std::path::Path::new("/tmp/f2 \"art\"")));
        compiles(LOVE);
        compiles(&play_script("Taylor \"Swift\" \\ playlist"));
        if Player::Spotify.app().is_some() {
            compiles(SPOTIFY_NOW);
        }
    }

    #[test]
    fn asking() {
        assert!(now_asked("What's playing?"));
        assert!(now_asked("now playing"));
        assert_eq!(play_asked("play Taylor Swift"), Some(("Taylor Swift".into(), None)));
        assert_eq!(play_asked("play 1989 on spotify"), Some(("1989".into(), Some(Player::Spotify))));
        assert_eq!(play_asked("play"), None);
        assert_eq!(play_asked("play music"), None, "picking up where it was");
        assert!(super::resume_asked("Play music"));
        assert_eq!(play_asked("play pause"), None);
        assert_eq!(play_asked("playground"), None);
        assert_eq!(playlist_named("my chill playlist"), "chill");
        assert_eq!(playlist_named("playlist Road Trip"), "Road Trip");
        assert_eq!(clock(65), "1:05");
        assert_eq!(clock(3723), "1:02:03");
    }
}

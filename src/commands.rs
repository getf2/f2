//! Quick Open's commands, as Raycast's: the system's (Lock Screen, Sleep,
//! Restart, Empty Trash, Toggle Dark Mode, Wi-Fi, Bluetooth and Do Not
//! Disturb…), System Settings' panes,
//! quitting apps ("quit safari"), Finder2's own tools, a typed URL, and
//! searching the web for what's typed.

use std::path::PathBuf;

use gpui::App;

use crate::macos;

#[derive(Clone)]
pub enum Icon {
    /// An SF Symbol, white on a tile of a color (as Raycast and System
    /// Settings show theirs): its kind's, see `SYSTEM` and the rest.
    Tile(&'static str, u32),
    /// An app's (or a file's) own icon.
    Of(PathBuf),
    /// A picture (an extension's own icon).
    Image(PathBuf),
}

/// The tiles' colors, by what a command is: the system's, Finder2's own,
/// the web's, the calculator's. (System Settings' panes have theirs, as in
/// its sidebar.)
pub const SYSTEM: u32 = 0xff453a;
pub const FINDER2: u32 = 0x0a84ff;
pub const WEB: u32 = 0x30b0c7;
pub const CALCULATOR: u32 = 0xff9f0a;
pub const SCREENSHOTS: u32 = 0xbf5af2;
pub const CLIPBOARD: u32 = 0x30d158;
pub const EMOJI: u32 = 0xffb800;
pub const WINDOWS: u32 = 0x5e5ce6;
pub const DEV: u32 = 0xa2845e;
pub const QUICKLINKS: u32 = 0x32ade6;
pub const SNIPPETS: u32 = 0xff6482;
pub const TIME: u32 = 0x34aadc;
pub const DICTIONARY: u32 = 0xa2382f;
pub const TRANSLATE: u32 = 0x007aff;
pub const REMINDERS: u32 = 0xff9500;
pub const TIMER: u32 = 0xff9f0a;
pub const AWAKE: u32 = 0xa2845e;
pub const MUSIC: u32 = 0xff2d55;
pub const CALENDAR: u32 = 0xff3b30;
pub const WEATHER: u32 = 0x0a84ff;
pub const QR: u32 = 0x636366;
pub const CONTACTS: u32 = 0x8e8e93;
pub const PHONE: u32 = 0x34c759;
pub const PASSWORDS: u32 = 0x5e5ce6;
pub const BATTERY: u32 = 0x34c759;
pub const BLUETOOTH: u32 = 0x0a84ff;
pub const NOTES: u32 = 0xffcc00;
pub const NETWORK: u32 = 0x0a84ff;
pub const WORDS: u32 = 0xa2382f;
pub const PACKAGE: u32 = 0xa2845e;

#[derive(Clone)]
pub enum Run {
    Lock,
    Sleep,
    Restart,
    ShutDown,
    LogOut,
    EmptyTrash,
    ToggleDarkMode,
    WiFi,
    /// Bluetooth on or off; `sure`: off even with a keyboard or mouse on
    /// it (it was asked).
    Bluetooth {
        sure: bool,
    },
    DoNotDisturb,
    /// Into Reminders.
    Remind(crate::reminders::Reminder),
    /// A timer: how long, and what for.
    Timer {
        secs: u64,
        label: String,
    },
    CancelTimer(u64),
    KeepAwake(crate::awake::Until),
    StopAwake,
    /// The volume, muting, the built-in screen's brightness.
    Sound(crate::media::Ask),
    /// A media key: play/pause, next, previous.
    Media(crate::macos::MediaKey),
    /// Into the calendar.
    AddEvent(crate::calendar::NewEvent),
    /// The calendar asked for (macOS asks).
    CalendarAccess,
    /// Contacts asked for (macOS asks), to find who's called.
    ContactsAccess,
    /// One of your Shortcuts, by name.
    Shortcut(String),
    /// A password onto the clipboard, out of clipboard histories' way.
    CopySecret(String),
    /// Sound out of this output (CoreAudio's id, and its name).
    Output { id: u32, name: String },
    /// A Bluetooth device connected (or disconnected).
    Connect { address: String, name: String, connect: bool },
    /// Into Apple Notes.
    AddNote(String),
    /// The internet's speed tested (macOS's `networkQuality`).
    SpeedTest,
    /// A Wi-Fi network's password looked up (macOS asks for yours), then
    /// shown with the code a phone joins by.
    WifiPassword(String),
    /// Location asked for: only so macOS says which Wi-Fi network it is.
    LocationAccess,
    /// An alarm at a time (seconds since 1970), and what for.
    Alarm { at: i64, label: String },
    /// What's copied, pasted without its formatting (Quick Open does it).
    PastePlain,
    /// Night Shift on or off.
    NightShift,
    /// The desktop's icons hidden or shown (Finder restarts for it).
    DesktopIcons,
    /// Mission Control's Show Desktop.
    ShowDesktop,
    /// A sleep timer: the Mac asleep, or the music stopped, in so long.
    SleepTimer { then: crate::timers::Then, secs: u64 },
    /// A break, every so often (seconds), till it's stopped.
    Breaks { every: u64, label: String },
    /// The stopwatch started, paused, resumed or stopped.
    Stopwatch(crate::timers::Watch),
    /// Reminders asked for (macOS asks), to list what's due.
    RemindersAccess,
    /// A reminder checked off (its identifier, and title).
    ReminderDone { id: String, title: String },
    /// Music or Spotify asked to do something: play, skip, love a song.
    Music(crate::music::Player, crate::music::Do),
    /// F2 allowed to ask Music or Spotify (macOS asks), then what's playing
    /// shown.
    MusicAccess(crate::music::Player),
    /// Cleaning mode: keys and clicks off for so many seconds.
    CleanKeyboard(u32),
    /// Text pasted over what's selected (a text tool's), as it is.
    PasteText(String),
    /// The text selected in the app in front, its spelling fixed in place.
    FixSpelling,
    /// An app's window brought forward (Quick Open's Windows list).
    FocusWindow { pid: i32, id: u32 },
    /// An app quit, then opened again.
    RestartApp { pid: i32, path: PathBuf, name: String },
    /// The text selected in the app Quick Open came up over: what to do
    /// with it (Quick Open lists it).
    UseSelection,
    EjectAll,
    /// A System Settings pane, by its URL's identifier.
    Settings(&'static str),
    Quit { pid: i32, force: bool },
    QuitAll(Vec<i32>),
    Url(String),
    DiskUsage,
    FreeUpSpace,
    /// A folder's loose files into folders of their kinds.
    TidyUp(PathBuf),
    /// An app and what it leaves (see `uninstall`).
    Uninstall(PathBuf),
    Trash,
    NewWindow,
    Preferences,
    QuickOpenSettings,
    Capture(crate::shots::Action),
    /// The colors picked (see `colors`).
    Colors,
    /// The images selected in F2, resized (see `resize`).
    Resize,
    /// The extension store (see `store`).
    Store,
    EditLastShot,
    /// Quick Open's clipboard history, or its emoji.
    Mode(crate::quick_panel::Mode),
    ClearClipboard,
    Window(crate::windows::Place),
    /// A quicklink, its "{query}" filled: a web address or a path.
    Quicklink(String),
    /// Quick Open's field set to this (a quicklink's keyword, to type what
    /// it looks for after).
    Fill(String),
    /// A snippet's text, to paste (Quick Open does).
    Snippet(String),
    /// What's listening on a port, stopped.
    Stop { pid: i32, name: String, port: u16 },
    DevTool(crate::dev::Tool),
    Uuid,
    Timestamp,
    /// Onto the clipboard (a branch's name).
    CopyText(String),
    /// Settings, at a section ("snippets", "quicklinks").
    SettingsAt(&'static str),
    /// A server (a Host of ~/.ssh/config, or one connected to), in a
    /// window, or in a terminal over SSH.
    Server(String),
    SshTerminal(String),
    AddServer,
    SendFeedback,
    Welcome,
    DualPane,
    About,
    CheckForUpdates,
    ShowLog,
    /// An extension's command (by their ids): its page in Quick Open.
    Extension { ext: String, command: String },
    /// Where extensions go, in a window.
    ExtensionsFolder,
    /// Settings › Extensions, a new one's name asked for.
    NewExtension,
}

#[derive(Clone)]
pub struct Command {
    pub title: String,
    /// Beside the title, grayer: what it's from ("Finder2"), if anything.
    pub subtitle: String,
    /// On the right: what it is ("Command", "Website").
    pub kind: &'static str,
    pub icon: Icon,
    pub run: Run,
    /// Asked first (a second Return does it), for what can't be undone.
    pub confirm: Option<String>,
    /// What Recent remembers it by (see `by_key`); None: not remembered
    /// (quitting an app, searching the web).
    pub key: Option<String>,
}

/// What a command can also be had by, for Quick Open to show beside it.
pub enum Keys {
    /// A shortcut in any app ("shift-cmd-4").
    Shortcut(String),
    /// A keyword typed in Quick Open (a quicklink's, a snippet's).
    Keyword(String),
}

/// The shortcut Settings give `command`, or the keyword it's typed by
/// (None: neither).
pub fn keys(command: &Command, settings: &crate::settings::Settings) -> Option<Keys> {
    use crate::quick_panel::Mode;
    let set = |s: Option<&String>| s.filter(|s| !s.trim().is_empty()).map(|s| Keys::Shortcut(s.clone()));
    let word = |w: &str| (!w.trim().is_empty()).then(|| Keys::Keyword(w.trim().to_string()));
    if let Some(shortcut) = set(command.key.as_ref().and_then(|k| settings.shortcuts.get(k))) {
        return Some(shortcut);
    }
    match &command.run {
        Run::Capture(action) => action.shortcut(&settings.shots).filter(|s| !s.is_empty()).map(|s| Keys::Shortcut(s.to_string())),
        Run::Window(place) => set(settings.window_shortcuts.get(place.key())),
        Run::Mode(Mode::Clipboard) => set(settings.clipboard_shortcut.as_ref()),
        Run::Mode(Mode::Emoji) => set(settings.emoji_shortcut.as_ref()),
        Run::Quicklink(link) => settings.quicklinks.iter().find(|q| &q.link == link).and_then(|q| word(&q.keyword)),
        // (A quicklink that takes a query: its keyword, typed for it.)
        Run::Fill(text) => settings.quicklinks.iter().find(|q| q.keyword.trim() == text.trim()).and_then(|q| word(&q.keyword)),
        Run::Snippet(text) => settings.snippets.iter().find(|s| &s.text == text).and_then(|s| word(&s.keyword)),
        Run::Extension { ext, command } => crate::extensions::by_id(ext).and_then(|e| e.spec(command).and_then(|s| s.keyword.as_deref().and_then(word))),
        _ => None,
    }
}

/// Name, words it's also found by, icon, what it does.
fn system() -> Vec<(&'static str, &'static str, &'static str, Run)> {
    vec![
        ("Lock Screen", "lock", "lock.fill", Run::Lock),
        ("Sleep", "sleep", "moon.zzz.fill", Run::Sleep),
        ("Restart", "restart reboot", "arrow.clockwise", Run::Restart),
        ("Shut Down", "shutdown power off turn off", "power", Run::ShutDown),
        ("Log Out", "logout sign out", "rectangle.portrait.and.arrow.right", Run::LogOut),
        ("Toggle Dark Mode", "dark light appearance mode theme", "circle.lefthalf.filled", Run::ToggleDarkMode),
        ("Eject All Disks", "eject unmount disks drives", "eject.fill", Run::EjectAll),
        ("Show Desktop", "show desktop hide windows peek clear", "menubar.dock.rectangle", Run::ShowDesktop),
    ]
}

/// Turned on and off: name, words, icon and its color (System Settings'),
/// what it does.
fn toggles() -> Vec<(&'static str, &'static str, &'static str, u32, Run)> {
    vec![
        ("Toggle Wi-Fi", "wifi wi-fi wireless internet network turn on off|turn wifi on|turn wifi off|wifi on|wifi off|turn on wifi|turn off wifi", "wifi", BLUE, Run::WiFi),
        (
            "Toggle Bluetooth",
            "bluetooth airpods headphones turn on off|turn bluetooth on|turn bluetooth off|bluetooth on|bluetooth off|turn on bluetooth|turn off bluetooth",
            "dot.radiowaves.left.and.right",
            BLUE,
            Run::Bluetooth { sure: false },
        ),
        (
            "Toggle Do Not Disturb",
            "do not disturb dnd focus notifications silence quiet turn on off|turn on do not disturb|turn off do not disturb|dnd on|dnd off|do not disturb on|do not disturb off",
            "moon.fill",
            INDIGO,
            Run::DoNotDisturb,
        ),
        (
            "Toggle Night Shift",
            "night shift warm screen blue light yellow evening eyes turn on off|turn night shift on|turn night shift off|night shift on|night shift off|turn on night shift|turn off night shift",
            "moon.stars.fill",
            0xff9f0a,
            Run::NightShift,
        ),
        (
            "Toggle Desktop Icons",
            "hide show desktop icons files clean screen sharing presentation|hide desktop icons|show desktop icons|hide icons|show icons|clean desktop",
            "eye.slash.fill",
            GRAPHITE,
            Run::DesktopIcons,
        ),
    ]
}

/// Finder2's own: name, words, icon, what it does.
fn finder2() -> Vec<(&'static str, &'static str, &'static str, Run)> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    vec![
        ("Tidy Up Desktop", "tidy up clean desktop organize sort screenshots mess", "sparkles.rectangle.stack", Run::TidyUp(home.join("Desktop"))),
        ("Tidy Up Downloads", "tidy up clean downloads organize sort installers mess", "sparkles.rectangle.stack", Run::TidyUp(home.join("Downloads"))),
        ("Folder Rules", "folder rules automatic automate hazel organize keep tidy downloads desktop move sort trash", "wand.and.stars", Run::SettingsAt("rules")),
        ("Disk Usage", "disk space storage daisydisk usage", "chart.pie.fill", Run::DiskUsage),
        ("Free Up Space", "free up space clean up storage full disk delete junk cleaner installers caches backups", "sparkles", Run::FreeUpSpace),
        ("Dual Pane", "dual pane split two panes side by side commander", "rectangle.split.2x1.fill", Run::DualPane),
        ("New F2 Window", "new window finder finder2", "macwindow", Run::NewWindow),
        ("F2 Settings", "settings preferences finder2 f2", "gearshape.fill", Run::Preferences),
        ("Quick Open Settings", "quick open launcher font text size shortcut hotkey settings", "textformat.size", Run::QuickOpenSettings),
        ("About F2", "about version f2 finder2 licenses acknowledgements", "info.circle.fill", Run::About),
        ("Check for Updates", "update upgrade new version sparkle", "arrow.down.circle.fill", Run::CheckForUpdates),
        ("Show Log File", "log logs console errors crash debug", "doc.text.fill", Run::ShowLog),
    ]
}

/// System Settings' panes: name, words, the pane's URL identifier, and
/// its symbol and color in System Settings' sidebar.
const PANES: &[(&str, &str, &str, &str, u32)] = &[
    ("Wi-Fi", "wifi wireless internet", "com.apple.wifi-settings-extension", "wifi", BLUE),
    ("Bluetooth", "bluetooth airpods headphones", "com.apple.BluetoothSettings", "wave.3.right", BLUE),
    ("Network", "network ethernet vpn", "com.apple.Network-Settings.extension", "network", BLUE),
    ("Displays", "display monitor resolution brightness night shift", "com.apple.Displays-Settings.extension", "sun.max.fill", BLUE),
    ("Sound", "sound volume audio output input", "com.apple.Sound-Settings.extension", "speaker.wave.3.fill", PINK),
    ("Notifications", "notifications alerts", "com.apple.Notifications-Settings.extension", "bell.badge.fill", RED),
    ("Focus", "focus do not disturb", "com.apple.Focus-Settings.extension", "moon.fill", INDIGO),
    ("Battery", "battery power energy", "com.apple.Battery-Settings.extension", "battery.100percent", GREEN),
    ("Appearance", "appearance dark light accent", "com.apple.Appearance-Settings.extension", "circle.lefthalf.filled", GRAPHITE),
    ("Desktop & Dock", "desktop dock stage manager hot corners", "com.apple.Desktop-Settings.extension", "menubar.dock.rectangle", GRAPHITE),
    ("Wallpaper", "wallpaper background", "com.apple.Wallpaper-Settings.extension", "photo.fill", CYAN),
    ("Screen Saver", "screen saver", "com.apple.ScreenSaver-Settings.extension", "sparkles.tv", CYAN),
    ("Lock Screen", "lock screen", "com.apple.Lock-Screen-Settings.extension", "lock.fill", GRAPHITE),
    ("Keyboard", "keyboard shortcuts input sources", "com.apple.Keyboard-Settings.extension", "keyboard.fill", GRAY),
    ("Trackpad", "trackpad gestures", "com.apple.Trackpad-Settings.extension", "hand.point.up.left.fill", GRAY),
    ("Mouse", "mouse", "com.apple.Mouse-Settings.extension", "computermouse.fill", GRAY),
    ("Privacy & Security", "privacy security permissions", "com.apple.settings.PrivacySecurity.extension", "hand.raised.fill", BLUE),
    ("Full Disk Access", "full disk access permissions", "com.apple.preference.security?Privacy_AllFiles", "externaldrive.fill", BLUE),
    ("Software Update", "software update macos upgrade", "com.apple.Software-Update-Settings.extension", "gear.badge", GRAY),
    ("Storage", "storage space", "com.apple.settings.Storage", "internaldrive.fill", GRAY),
    ("General", "general about", "com.apple.systempreferences.GeneralSettings", "gearshape.fill", GRAY),
    ("Login Items", "login items startup", "com.apple.LoginItems-Settings.extension", "list.bullet", GRAY),
    ("Users & Groups", "users groups accounts", "com.apple.Users-Groups-Settings.extension", "person.2.fill", BLUE),
    ("Touch ID & Password", "touch id password fingerprint", "com.apple.Touch-ID-Settings.extension", "touchid", PINK),
    ("Passwords", "passwords keychain", "com.apple.Passwords-Settings.extension", "key.fill", GRAY),
    ("Internet Accounts", "internet accounts mail", "com.apple.Internet-Accounts-Settings.extension", "at", BLUE),
    ("Accessibility", "accessibility zoom voiceover", "com.apple.Accessibility-Settings.extension", "accessibility", BLUE),
    ("Control Center", "control center menu bar", "com.apple.ControlCenter-Settings.extension", "switch.2", GRAY),
    ("Spotlight", "spotlight", "com.apple.Spotlight-Settings.extension", "magnifyingglass", GRAY),
    ("Siri", "siri apple intelligence", "com.apple.Siri-Settings.extension", "waveform", PURPLE),
    ("Screen Time", "screen time", "com.apple.Screen-Time-Settings.extension", "hourglass", INDIGO),
    ("Sharing", "sharing airplay file sharing", "com.apple.Sharing-Settings.extension", "square.and.arrow.up.fill", GRAY),
    ("Time Machine", "time machine backup", "com.apple.Time-Machine-Settings.extension", "clock.arrow.circlepath", MINT),
    ("Printers & Scanners", "printers scanners print", "com.apple.Print-Scan-Settings.extension", "printer.fill", GRAY),
    ("Date & Time", "date time clock", "com.apple.Date-Time-Settings.extension", "clock.fill", BLUE),
    ("Language & Region", "language region locale", "com.apple.Localization-Settings.extension", "globe", BLUE),
    ("Startup Disk", "startup disk boot", "com.apple.Startup-Disk-Settings.extension", "internaldrive", GRAY),
];

// System Settings' sidebar colors.
const BLUE: u32 = 0x0a84ff;
const PINK: u32 = 0xff2d55;
const RED: u32 = 0xff3b30;
const INDIGO: u32 = 0x5e5ce6;
const GREEN: u32 = 0x34c759;
const GRAPHITE: u32 = 0x48484a;
const CYAN: u32 = 0x32ade6;
const GRAY: u32 = 0x8e8e93;
const PURPLE: u32 = 0xbf5af2;
const MINT: u32 = 0x00c7be;

/// How well `query` (lowercase) matches a name or any of its words. After
/// a "|" in `words`, whole phrases it's said by, each as good as its name
/// ("hide desktop icons", "turn wifi off").
pub fn matches(name: &str, words: &str, query: &str) -> Option<f32> {
    let (words, phrases) = words.split_once('|').unwrap_or((words, ""));
    let named = phrases.split('|').filter(|p| !p.is_empty()).filter_map(|p| crate::apps::name_score(p, query)).chain(crate::apps::name_score(name, query)).reduce(f32::max);
    let worded = words.split(' ').filter_map(|w| crate::apps::name_score(w, query)).fold(None, |best: Option<f32>, s| Some(best.map_or(s, |b| b.max(s))));
    // (A name match over a word's: a name starting with the query (80)
    // over a word that's it exactly ("clipboard": Clipboard History, not
    // Format JSON, which works on the clipboard).)
    match (named, worded.map(|s| s - 25.)) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// Every command there is but quitting apps and the web's, in order: the
/// system's, Finder2's, then System Settings' panes. Each with the words
/// it's also found by.
/// The words a server's two commands are found by (kept: made once a
/// server).
fn server_words(host: &str) -> (&'static str, &'static str) {
    static MADE: std::sync::Mutex<Vec<(String, &'static str, &'static str)>> = std::sync::Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap();
    if let Some((_, a, b)) = made.iter().find(|(h, _, _)| h == host) {
        return (a, b);
    }
    let open: &'static str = Box::leak(format!("{host} server sftp ssh files remote connect open").into_boxed_str());
    let ssh: &'static str = Box::leak(format!("{host} ssh terminal shell server remote login|ssh {host}").into_boxed_str());
    made.push((host.to_string(), open, ssh));
    (open, ssh)
}

fn catalog() -> Vec<(&'static str, Command)> {
    let bin = macos::bin();
    let command = |key: String, title: String, subtitle: &str, kind, icon: Icon, run: Run| Command {
        title,
        subtitle: subtitle.into(),
        kind,
        icon,
        run,
        confirm: None,
        key: Some(key),
    };
    let mut all = Vec::new();
    for (name, words, symbol, run) in system() {
        all.push((words, command(name.into(), name.into(), "System", "Command", Icon::Tile(symbol, SYSTEM), run)));
    }
    for (name, words, symbol, color, run) in toggles() {
        let key = format!("toggle:{}", name.trim_start_matches("Toggle ").to_lowercase());
        all.push((words, command(key, name.into(), "System", "Command", Icon::Tile(symbol, color), run)));
    }
    // What's typed after a word: a reminder, a word to look up, a text to
    // translate, a city.
    let ask = |key: &str, title: &str, subtitle: &str, symbol, color, fill: &str| command(key.into(), title.into(), subtitle, "Command", Icon::Tile(symbol, color), Run::Fill(fill.into()));
    all.push(("reminder remind me add todo task reminders", ask("ask:reminder", "Add Reminder", "Reminders", "checklist", REMINDERS, "remind me to ")));
    all.push(("define definition dictionary meaning word look up", ask("ask:define", "Define a Word", "Dictionary", "character.book.closed.fill", DICTIONARY, "define ")));
    all.push(("translate translation language translator", ask("ask:translate", "Translate", "Translation", "translate", TRANSLATE, "translate ")));
    all.push(("world clock time zone zones city cities time in", ask("ask:time", "World Clock", "Time in a city", "clock.fill", TIME, "time in ")));
    all.push(("timer countdown stopwatch alarm egg kitchen", ask("ask:timer", "Start Timer", "Timer", "timer", TIMER, "timer ")));
    all.push(("next meeting calendar agenda events today schedule my day join call", ask("ask:agenda", "Next Meeting", "Calendar", "calendar", CALENDAR, "next meeting")));
    all.push(("add event calendar new schedule appointment meeting", ask("ask:event", "Add Event", "Calendar", "calendar.badge.plus", CALENDAR, "event ")));
    all.push(("weather forecast temperature rain sunny", ask("ask:weather", "Weather", "Here, or anywhere", "cloud.sun.fill", WEATHER, "weather")));
    all.push(("qr code make generate share phone link qrcode", ask("ask:qr", "QR Code", "Of a link or text", "qrcode", QR, "qr ")));
    all.push(("password generate random strong secure pin passcode", ask("ask:password", "Generate Password", "Strong, and kept out of clipboard history", "key.fill", PASSWORDS, "generate password")));
    all.push(("battery charge power airpods level percent", ask("ask:battery", "Battery", "The Mac's, and AirPods', mouse's…", "battery.75percent", BATTERY, "battery")));
    all.push(("sound output audio speakers headphones airpods switch device", ask("ask:output", "Sound Output", "Where the sound goes", "speaker.wave.2.fill", MUSIC, "sound output")));
    all.push(("connect bluetooth airpods headphones pair device", ask("ask:connect", "Connect Bluetooth Device", "AirPods, headphones, speakers", "dot.radiowaves.left.and.right", BLUETOOTH, "connect ")));
    all.push(("latest download downloads newest last recent file", ask("ask:downloads", "Latest Downloads", "Newest in Downloads", "arrow.down.circle.fill", FINDER2, "latest download")));
    all.push(("note new notes jot write memo idea", ask("ask:note", "New Note", "Into Notes", "note.text", NOTES, "note ")));
    all.push(("wifi wi-fi password share guests network key qr join", ask("ask:wifi", "Wi-Fi Password", "To share, with a code to scan", "wifi", NETWORK, "wifi password")));
    all.push(("alarm wake me up clock morning", ask("ask:alarm", "Set Alarm", "At a time: “alarm 7am”", "alarm.fill", TIMER, "alarm ")));
    all.push(("spell spelling spellcheck how do you spell word", ask("ask:spell", "Check Spelling", "“spell accomodate”", "a.magnify", WORDS, "spell ")));
    all.push(("synonym synonyms thesaurus another word for similar", ask("ask:synonyms", "Synonyms", "From the thesaurus", "character.book.closed.fill", WORDS, "synonyms for ")));
    all.push(("word count words characters count letters length", ask("ask:count", "Word Count", "Of what you copied", "number", WORDS, "word count")));
    all.push(("sleep timer later music off stop bedtime night|sleep timer|stop music later", ask("ask:sleep-timer", "Sleep Timer", "The Mac asleep, or the music stopped, later", "moon.zzz.fill", TIMER, "sleep in ")));
    all.push(("text tools uppercase lowercase title case capitalize line breaks spaces clean|text tools|change case", ask("ask:text-tools", "Text Tools", "UPPERCASE, Title Case… of what you copied", "textformat", WORDS, "text tools")));
    all.push(("air quality fine dust pollution aqi uv index sunscreen smog|air quality|fine dust", ask("ask:air", "Air Quality", "Fine dust and UV, here or anywhere", "aqi.medium", WEATHER, "air quality")));
    all.push(("tip calculator split bill check restaurant gratuity|split the bill|tip calculator", ask("ask:tip", "Tip & Split the Bill", "“tip 18% on 84”, “split 120 four ways”", "percent", CALCULATOR, "tip ")));
    all.push(("flip coin heads tails toss random|flip a coin|heads or tails", ask("ask:coin", "Flip a Coin", "Heads or tails", "circle.lefthalf.filled", CALCULATOR, "flip a coin")));
    all.push(("roll die dice random d20|roll a die|roll dice", ask("ask:dice", "Roll a Die", "Or “roll 2d6”, “roll d20”", "die.face.5.fill", CALCULATOR, "roll a die")));
    all.push(("pick choose decide random choice for me|pick for me|random number", ask("ask:pick", "Pick for Me", "“pick sushi, pizza or tacos”, “random number 1 to 10”", "hand.point.right.fill", CALCULATOR, "pick ")));
    all.push(("flight status plane airline arrival departure track flight|flight status", ask("ask:flight", "Flight Status", "Type a flight number: “KE 17”", "airplane", WEB, "flight ")));
    all.push(("directions map maps route navigate near me nearby|directions to|near me", ask("ask:directions", "Directions", "Apple Maps, Google Maps", "map.fill", PHONE, "directions to ")));
    all.push(("kitchen cooking baking recipe cups grams tablespoon teaspoon flour sugar butter convert|cups to grams|grams to cups", ask("ask:kitchen", "Kitchen Conversions", "“1 cup flour in grams”, “200 g butter in cups”", "fork.knife", CALCULATOR, "1 cup flour in grams")));
    all.push(("shoe shoes size sizes clothing clothes dress shirt convert us eu uk korean|shoe size|clothing size", ask("ask:sizes", "Shoe & Clothing Sizes", "“us 10 shoe in eu”, “us 8 dress in uk”", "shoe.fill", CONTACTS, "shoe size ")));
    all.push(("todo to do list reminders today due check off my reminders tasks|to do|my reminders", ask("ask:todo", "To-Do", "Today's reminders, to check off", "checklist", REMINDERS, "todo")));
    all.push(("stopwatch stop watch count up time lap|stopwatch", ask("ask:stopwatch", "Stopwatch", "Counts up in the menu bar", "stopwatch", TIMER, "stopwatch")));
    all.push(("break breaks eye stand up stretch water posture every rest reminder|eye breaks|break reminders", ask("ask:breaks", "Break Reminders", "“eye break every 20 min”, “stand up every hour”", "figure.walk", TIMER, "eye breaks")));
    all.push(("price prices stock stocks share shares market crypto bitcoin btc ethereum eth coin|stock price|bitcoin price", ask("ask:price", "Price", "Of a stock or a coin: “apple price”, “btc”", "chart.line.uptrend.xyaxis", 0x30d158, "price of ")));
    all.push(("about this mac model serial number specs chip memory system info|about this mac|serial number", ask("ask:about", "About This Mac", "Model, chip, memory, serial number", "laptopcomputer", SYSTEM, "about this mac")));
    all.push(("my ip address network public local|my ip|ip address", ask("ask:ip", "My IP Address", "Here, and to the internet", "network", NETWORK, "my ip")));
    all.push(("holidays public bank national day off next holiday|next holiday|public holidays", ask("ask:holidays", "Holidays", "The next public holidays", "gift.fill", CALENDAR, "holidays")));
    all.push(("restart relaunch reopen app|restart an app", ask("ask:restart", "Restart an App", "Quit it and open it again", "arrow.clockwise.circle.fill", SYSTEM, "restart ")));
    all.push(("disk space storage free space left how much|disk space|free space", ask("ask:storage", "Disk Space", "How much is free", "internaldrive.fill", FINDER2, "storage")));
    all.push(("track package parcel tracking delivery shipment order ups fedex usps dhl", ask("ask:track", "Track Package", "Paste its tracking number", "shippingbox.fill", PACKAGE, "track ")));
    all.push(("call phone message text facetime email contact person mom", ask("ask:call", "Call Someone", "Or message, FaceTime, email", "phone.fill", PHONE, "call ")));
    all.push(("slow slowing busy cpu memory ram usage activity task manager processes fan hot apps|what's slowing my mac|why is my mac slow|task manager", ask("ask:busy", "What's Slowing My Mac", "The apps using the most, to quit", "gauge.with.dots.needle.67percent", SYSTEM, "what's slowing my mac")));
    all.push(("internet down online offline connection wifi wi-fi network check working router|is the internet down|am i online", ask("ask:online", "Is the Internet Down?", "Checks Wi-Fi, the router and the internet", "wifi.exclamationmark", NETWORK, "is the internet down")));
    all.push(("now playing song track music spotify current what's playing listening|now playing|what's playing", ask("ask:now-playing", "Now Playing", "In Music or Spotify", "music.note", MUSIC, "now playing")));
    all.push(("play music song songs artist album playlist spotify library listen", ask("ask:play", "Play a Song or Artist", "From your library: “play taylor swift”", "play.circle.fill", MUSIC, "play ")));
    let thing = |key: &str, title: &str, subtitle: &str, words, symbol, color, run| (words, command(key.into(), title.into(), subtitle, "Command", Icon::Tile(symbol, color), run));
    all.push(thing("pomodoro", "Pomodoro", "A 25-minute timer", "pomodoro focus timer 25 work", "timer", TIMER, Run::Timer { secs: 25 * 60, label: "Pomodoro".into() }));
    all.push(thing("speed-test", "Speed Test", "Your internet's download and upload", "speed test internet speedtest wifi fast slow connection bandwidth", "speedometer", NETWORK, Run::SpeedTest));
    all.push(thing("selection", "Use Selected Text", "In the app you were in: translate, define, search it", "selected selection text highlight translate define look up", "character.cursor.ibeam", WORDS, Run::UseSelection));
    all.push(thing("fix-spelling", "Fix Spelling", "Of the text selected in the app you were in", "fix spelling spellcheck spell check typos misspelled misspellings correct selected selection text|fix typos|fix my spelling|correct spelling", "textformat.abc.dottedunderline", WORDS, Run::FixSpelling));
    all.push(thing("paste-plain", "Paste as Plain Text", "What you copied, without its formatting", "paste plain text unformatted match style clean strip formatting", "doc.plaintext", CLIPBOARD, Run::PastePlain));
    all.push(thing("awake", "Keep Awake", "Until you stop it", "keep awake caffeinate amphetamine prevent sleep screen on stay awake", "cup.and.saucer.fill", AWAKE, Run::KeepAwake(crate::awake::Until::Stopped)));
    all.push(thing("media:play", "Play/Pause", "Music", "play pause music song resume playing spotify stop", "playpause.fill", MUSIC, Run::Media(macos::MediaKey::Play)));
    all.push(thing("media:next", "Next Track", "Music", "next skip song track forward", "forward.fill", MUSIC, Run::Media(macos::MediaKey::Next)));
    all.push(thing("music:love", "Love This Song", "In Music", "love like heart favorite favourite song track music", "heart.fill", MUSIC, Run::Music(crate::music::Player::Music, crate::music::Do::Love)));
    all.push(thing("clean-keyboard", "Clean Keyboard", "Keys and clicks off for 30 seconds, to wipe it", "clean cleaning keyboard wipe lock disable keys keyboard trackpad dust", "keyboard", SYSTEM, Run::CleanKeyboard(30)));
    all.push(thing("media:previous", "Previous Track", "Music", "previous back song track last rewind", "backward.fill", MUSIC, Run::Media(macos::MediaKey::Previous)));
    all.push(thing("sound:mute", "Mute", "Sound", "mute sound silence quiet audio", "speaker.slash.fill", PINK, Run::Sound(crate::media::Ask::Mute(true))));
    all.push(thing("sound:up", "Volume Up", "Sound", "volume up louder sound audio", "speaker.wave.3.fill", PINK, Run::Sound(crate::media::Ask::Volume(crate::media::Level::Up))));
    all.push(thing("sound:down", "Volume Down", "Sound", "volume down quieter softer sound audio", "speaker.wave.1.fill", PINK, Run::Sound(crate::media::Ask::Volume(crate::media::Level::Down))));
    all.push(thing("screen:up", "Brightness Up", "Built-in screen", "brightness up brighter screen display", "sun.max.fill", BLUE, Run::Sound(crate::media::Ask::Brightness(crate::media::Level::Up))));
    all.push(thing("screen:down", "Brightness Down", "Built-in screen", "brightness down dim dimmer darker screen display", "sun.min.fill", BLUE, Run::Sound(crate::media::Ask::Brightness(crate::media::Level::Down))));
    all.push((
        "empty trash bin rubbish",
        Command {
            confirm: Some(format!("Erase what's in the {bin} for good? Return again to empty it.")),
            ..command("empty-trash".into(), format!("Empty {bin}"), "System", "Command", Icon::Tile("trash.fill", SYSTEM), Run::EmptyTrash)
        },
    ));
    for (name, words, symbol, run) in finder2() {
        all.push((words, command(name.into(), name.into(), crate::APP_NAME, "Command", Icon::Tile(symbol, FINDER2), run)));
    }
    all.push(("trash bin rubbish", command("trash".into(), bin.into(), crate::APP_NAME, "Command", Icon::Tile("trash.fill", FINDER2), Run::Trash)));
    for action in crate::shots::Action::ALL {
        let words = match action {
            crate::shots::Action::Area => "screenshot capture area region selection snip",
            crate::shots::Action::Window => "screenshot capture window",
            crate::shots::Action::Screen => "screenshot capture screen fullscreen display",
            crate::shots::Action::Text => "ocr text recognize screenshot capture copy",
            crate::shots::Action::Color => "color colour picker eyedropper hex pick",
            crate::shots::Action::Qr => "qr code scan read barcode qrcode",
            crate::shots::Action::RecordScreen => "record screen recording video capture movie mp4 screencast stop|record screen|screen recording|stop recording",
            crate::shots::Action::RecordArea => "record area region recording video capture movie mp4 screencast stop|record area|record part of the screen|stop recording",
            crate::shots::Action::Scrolling => "scrolling screenshot capture long full page whole page scroll stitch|scrolling screenshot|scrolling capture|long screenshot|full page screenshot",
        };
        let key = format!("shot:{}", action.title());
        let kind = if matches!(action, crate::shots::Action::RecordScreen | crate::shots::Action::RecordArea) { "Screen Recording" } else { "Screenshot" };
        all.push((words, command(key, action.title().into(), kind, "Command", Icon::Tile(action.symbol(), SCREENSHOTS), Run::Capture(action))));
    }
    all.push((
        "colors colours picked palette swatches hex rgb hsl oklch history saved eyedropper",
        command("colors".into(), "Colors".into(), "The colors you've picked", "Command", Icon::Tile("swatchpalette.fill", SCREENSHOTS), Run::Colors),
    ));
    all.push((
        "resize images photos pictures smaller scale shrink downsize dimensions width height pixels",
        command("resize".into(), "Resize Images".into(), "The images selected in F2", "Command", Icon::Tile("arrow.down.right.and.arrow.up.left", FINDER2), Run::Resize),
    ));
    for place in crate::windows::Place::ALL {
        let key = format!("window:{}", place.key());
        all.push((place.words(), command(key, place.title().into(), "Window", "Command", Icon::Tile(place.symbol(), WINDOWS), Run::Window(place))));
    }
    for tool in crate::dev::Tool::ALL {
        let (title, words, symbol) = tool.about();
        all.push((words, command(format!("dev:{title}"), title.into(), "Developer", "Command", Icon::Tile(symbol, DEV), Run::DevTool(tool))));
    }
    let dev = |key: &str, title: &str, words, symbol, run| (words, command(key.into(), title.into(), "Developer", "Command", Icon::Tile(symbol, DEV), run));
    all.push(dev("dev:uuid", "Generate UUID", "uuid guid generate random id", "number", Run::Uuid));
    all.push(dev("dev:timestamp", "Copy Unix Timestamp", "timestamp unix epoch now time seconds", "clock", Run::Timestamp));
    all.push(dev("dev:ports", "Listening Ports", "ports listening kill port server process localhost", "network", Run::Fill("ports".into())));
    all.push((
        "snippet new create add text expansion",
        command("snippet-new".into(), "Create Snippet".into(), crate::APP_NAME, "Command", Icon::Tile("text.quote", SNIPPETS), Run::SettingsAt("snippets")),
    ));
    all.push((
        "quicklink new create add link bookmark",
        command("quicklink-new".into(), "Create Quicklink".into(), crate::APP_NAME, "Command", Icon::Tile("link.badge.plus", QUICKLINKS), Run::SettingsAt("quicklinks")),
    ));
    if crate::dictation::available() {
        all.push((
            "dictation dictate voice speech talk speak microphone mic transcribe speech to text voice typing|dictation settings|voice typing",
            command("dictation".into(), "Dictation Settings".into(), crate::APP_NAME, "Command", Icon::Tile("mic.fill", crate::dictation::COLOR), Run::SettingsAt("dictation")),
        ));
        if let Some(text) = crate::dictation::last() {
            let shown: String = text.chars().take(60).collect();
            let shown = if shown.len() < text.len() { format!("{shown}…") } else { shown };
            all.push((
                "paste last dictation again dictated voice speech said|paste last dictation|paste dictation",
                command("dictation:last".into(), "Paste Last Dictation".into(), &shown, "Command", Icon::Tile("mic.fill", crate::dictation::COLOR), Run::PasteText(text)),
            ));
        }
    }
    all.push((
        "keyboard shortcuts hotkeys hot keys keybinds keybindings key bindings bind keys|keyboard shortcuts|set a shortcut",
        command("keyboard".into(), "Keyboard Shortcuts".into(), crate::APP_NAME, "Command", Icon::Tile("keyboard.fill", FINDER2), Run::SettingsAt("keyboard")),
    ));
    all.push((
        "add server ssh sftp remote new server connect host|add server|new server",
        command("add-server".into(), "Add Server".into(), crate::APP_NAME, "Command", Icon::Tile("server.rack", FINDER2), Run::AddServer),
    ));
    // (Servers: ~/.ssh/config's Hosts, and others connected to.)
    for host in crate::remote::hosts_now() {
        let words = server_words(&host);
        all.push((
            words.0,
            command(format!("server:{host}"), format!("Open {host}"), "Server · SFTP", "Command", Icon::Tile("server.rack", FINDER2), Run::Server(host.clone())),
        ));
        all.push((
            words.1,
            command(format!("ssh:{host}"), format!("SSH to {host}"), "Server · in a terminal", "Command", Icon::Tile("terminal.fill", FINDER2), Run::SshTerminal(host)),
        ));
    }
    all.push((
        "search index rebuild reindex indexing spotlight missing files not found full disk access|search index|rebuild index",
        command("search-index".into(), "Search Index".into(), crate::APP_NAME, "Command", Icon::Tile("doc.text.magnifyingglass", FINDER2), Run::SettingsAt("search")),
    ));
    all.push((
        "window management settings drag windows move resize windows keys gap snap|drag windows|window settings",
        command("window-settings".into(), "Window Settings".into(), crate::APP_NAME, "Command", Icon::Tile("macwindow.on.rectangle", WINDOWS), Run::SettingsAt("windows")),
    ));
    all.push((
        "send feedback report a bug bug report suggestion idea contact support help|send feedback|report a bug",
        command("send-feedback".into(), "Send Feedback".into(), crate::APP_NAME, "Command", Icon::Tile("envelope.fill", FINDER2), Run::SendFeedback),
    ));
    all.push((
        "welcome onboarding permissions setup getting started tour",
        command("welcome".into(), format!("Welcome to {}", crate::APP_NAME), crate::APP_NAME, "Command", Icon::Tile("hand.wave.fill", FINDER2), Run::Welcome),
    ));
    all.push((
        "switch window windows switcher alt tab all open windows|switch window|switch windows|window switcher",
        command("windows".into(), "Switch Window".into(), "Every window open, front to back", "Command", Icon::Tile("macwindow.on.rectangle", WINDOWS), Run::Mode(crate::quick_panel::Mode::Windows)),
    ));
    all.push((
        "clipboard history paste copied pasteboard",
        command("clipboard".into(), "Clipboard History".into(), crate::APP_NAME, "Command", Icon::Tile("doc.on.clipboard", CLIPBOARD), Run::Mode(crate::quick_panel::Mode::Clipboard)),
    ));
    all.push((
        "screenshot history screenshots captures recent taken",
        command("screenshots".into(), "Screenshot History".into(), crate::APP_NAME, "Command", Icon::Tile("photo.on.rectangle", SCREENSHOTS), Run::Mode(crate::quick_panel::Mode::Screenshots)),
    ));
    all.push((
        "emoji symbols smiley characters arrows special",
        command("emoji".into(), "Emoji & Symbols".into(), crate::APP_NAME, "Command", Icon::Tile("face.smiling", EMOJI), Run::Mode(crate::quick_panel::Mode::Emoji)),
    ));
    all.push((
        "clear clipboard history forget",
        Command {
            confirm: Some("Forget everything copied? Return again to clear the history.".into()),
            ..command("clear-clipboard".into(), "Clear Clipboard History".into(), crate::APP_NAME, "Command", Icon::Tile("trash.fill", CLIPBOARD), Run::ClearClipboard)
        },
    ));
    all.push((
        "edit last screenshot annotate markup",
        command("shot:edit-last".into(), "Edit Last Screenshot".into(), "Screenshot", "Command", Icon::Tile("pencil.tip", SCREENSHOTS), Run::EditLastShot),
    ));
    for &(name, words, id, symbol, color) in PANES {
        all.push((words, command(format!("settings:{id}"), name.into(), "", "System Settings", Icon::Tile(symbol, color), Run::Settings(id))));
    }
    all
}

/// A toggle's command as things are: "Turn Wi-Fi Off" (it's on). And for
/// Bluetooth, asked first if a keyboard or mouse would stop working (only
/// once F2 may use Bluetooth: else macOS would ask as it's typed).
pub fn live(command: &mut Command) {
    let said = |on: bool| if on { "on" } else { "off" };
    match command.run {
        // (Kept awake: the way to stop it.)
        Run::KeepAwake(_) if crate::awake::state().is_some() => {
            command.title = "Stop Keeping Awake".into();
            command.subtitle = crate::awake::state().map(crate::awake::till).unwrap_or_default();
            command.run = Run::StopAwake;
        }
        Run::Sound(crate::media::Ask::Mute(true)) if macos::muted(None) == Some(true) => {
            command.title = "Unmute".into();
            command.run = Run::Sound(crate::media::Ask::Mute(false));
        }
        Run::Sound(crate::media::Ask::Volume(_)) => {
            if let Some(v) = macos::volume(None) {
                command.subtitle = format!("Now {}", crate::media::percent(v));
            }
        }
        Run::Sound(crate::media::Ask::Brightness(_)) => {
            if let Some(v) = macos::brightness(None) {
                command.subtitle = format!("Now {}", crate::media::percent(v));
            }
        }
        // (Recording: the way to stop it.)
        Run::Capture(crate::shots::Action::RecordScreen | crate::shots::Action::RecordArea) => {
            if let Some(secs) = crate::shots::record::elapsed() {
                command.title = "Stop Recording".into();
                command.subtitle = format!("Recording for {}", crate::shots::record::clock(secs));
            }
        }
        Run::NightShift => match macos::night_shift(None) {
            Some(on) => {
                command.title = format!("Turn Night Shift {}", if on { "Off" } else { "On" });
                command.subtitle = format!("Night Shift is {}", said(on));
            }
            None => command.subtitle = "Not on this Mac's screen".into(),
        },
        Run::DesktopIcons => {
            let shown = macos::desktop_icons(None);
            command.title = if shown { "Hide Desktop Icons" } else { "Show Desktop Icons" }.into();
            command.subtitle = if shown { "Handy before sharing your screen" } else { "They're hidden now" }.into();
        }
        Run::WiFi => {
            if let Some(on) = macos::wifi(None) {
                command.title = format!("Turn Wi-Fi {}", if on { "Off" } else { "On" });
                command.subtitle = format!("Wi-Fi is {}", said(on));
            }
        }
        Run::Bluetooth { .. } if macos::bluetooth_allowed() == Some(true) => {
            let on = macos::bluetooth(None);
            command.title = format!("Turn Bluetooth {}", if on { "Off" } else { "On" });
            command.subtitle = format!("Bluetooth is {}", said(on));
            command.run = Run::Bluetooth { sure: true };
            let inputs = if on { macos::bluetooth_inputs() } else { Vec::new() };
            if !inputs.is_empty() {
                command.confirm = Some(format!("{} would stop working: Return again to turn it off.", inputs.join(" and ")));
            }
        }
        _ => {}
    }
}

/// Every command (see `catalog`), and System Settings' panes A to Z.
pub fn every() -> (Vec<Command>, Vec<Command>) {
    let (mut panes, commands): (Vec<Command>, Vec<Command>) = catalog().into_iter().map(|(_, c)| c).partition(|c| matches!(c.run, Run::Settings(_)));
    panes.sort_by(|a, b| a.title.cmp(&b.title));
    (commands, panes)
}

/// The command Recent remembered by `key` (a website's, too).
pub fn by_key(key: &str) -> Option<Command> {
    if let Some(typed) = key.strip_prefix("url:") {
        return url(typed);
    }
    if key.starts_with("ext:") {
        return crate::extensions::by_key(key);
    }
    catalog().into_iter().map(|(_, c)| c).find(|c| c.key.as_deref() == Some(key))
}

/// The commands matching `query`, best first, with how well (as
/// `apps::name_score` scores: 100 its very name).
pub fn find_scored(query: &str, limit: usize) -> Vec<(f32, Command)> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    if let Some(quitting) = quit(&query) {
        return quitting.into_iter().map(|c| (100., c)).collect();
    }
    if let Some(restarting) = restart(&query) {
        return restarting.into_iter().take(limit).map(|c| (100., c)).collect();
    }
    if let Some(removing) = uninstall(&query) {
        return removing.into_iter().take(limit).map(|c| (100., c)).collect();
    }
    let mut found: Vec<(f32, Command)> = catalog().into_iter().filter_map(|(words, c)| Some((matches(&c.title, words, &query)?, c))).collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    // (Only good matches: "sl" isn't worth Sleep and Spotlight and Lock.)
    let best = found.first().map_or(0., |f| f.0);
    found.into_iter().filter(|(s, _)| *s >= 40. || *s >= best - 1.).take(limit).collect()
}

/// "uninstall zoom": the apps installed (not macOS's own), to uninstall.
fn uninstall(query: &str) -> Option<Vec<Command>> {
    let rest = query.strip_prefix("uninstall")?;
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let rest = rest.trim();
    let mut found: Vec<(f32, Command)> = crate::apps::installed()
        .iter()
        .filter(|app| !app.path.starts_with("/System"))
        .filter_map(|app| {
            let s = if rest.is_empty() { 0. } else { crate::apps::name_score(&app.name, rest)? };
            let command = Command {
                title: format!("Uninstall {}", app.name),
                subtitle: "Its app and what it leaves".into(),
                kind: "Command",
                icon: Icon::Of(app.path.clone()),
                run: Run::Uninstall(app.path.clone()),
                confirm: None,
                key: None,
            };
            Some((s, command))
        })
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    Some(found.into_iter().map(|(_, c)| c).collect())
}

/// "quit safari", "force quit zed", "quit all": the apps running, to quit.
fn quit(query: &str) -> Option<Vec<Command>> {
    let (force, rest) = if let Some(rest) = query.strip_prefix("force quit") {
        (true, rest)
    } else if let Some(rest) = query.strip_prefix("quit") {
        (false, rest)
    } else {
        return None;
    };
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let rest = rest.trim();
    let running = macos::running_apps();
    let verb = if force { "Force Quit" } else { "Quit" };
    let mut found: Vec<(f32, Command)> = running
        .iter()
        .filter_map(|(pid, name, path)| {
            let s = if rest.is_empty() { Some(0.) } else { crate::apps::name_score(name, rest) }?;
            let title = format!("{verb} {name}");
            Some((s, Command { title, subtitle: "Running".into(), kind: "Command", icon: Icon::Of(path.clone()), run: Run::Quit { pid: *pid, force }, confirm: None, key: None }))
        })
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut commands: Vec<Command> = found.into_iter().map(|(_, c)| c).collect();
    if !force && (rest.is_empty() || "all apps".starts_with(rest)) && !running.is_empty() {
        let pids: Vec<i32> = running.iter().map(|(pid, ..)| *pid).collect();
        let n = pids.len();
        commands.insert(
            0,
            Command {
                title: "Quit All Apps".into(),
                subtitle: format!("{n} running"),
                kind: "Command",
                icon: Icon::Tile("xmark", SYSTEM),
                run: Run::QuitAll(pids),
                confirm: Some(format!("Quit all {n} apps (they may ask to save)? Return again to quit them.")),
                key: None,
            },
        );
    }
    Some(commands)
}

/// "restart safari": the running apps by that name, to quit and open again.
fn restart(query: &str) -> Option<Vec<Command>> {
    let rest = query.strip_prefix("restart ").or_else(|| query.strip_prefix("relaunch ")).or_else(|| query.strip_prefix("reopen "))?.trim();
    if rest.is_empty() {
        return None;
    }
    let mut found: Vec<(f32, Command)> = macos::running_apps()
        .into_iter()
        .filter(|(pid, ..)| *pid != std::process::id() as i32)
        .filter_map(|(pid, name, path)| {
            let score = crate::apps::name_score(&name, rest)?;
            let title = format!("Restart {name}");
            Some((score, Command { title, subtitle: "Quit and opened again".into(), kind: "Command", icon: Icon::Of(path.clone()), run: Run::RestartApp { pid, path, name }, confirm: None, key: None }))
        })
        .collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    Some(found.into_iter().map(|(_, c)| c).collect())
}

/// A typed web address: "github.com", "https://…".
pub fn url(query: &str) -> Option<Command> {
    let q = query.trim();
    if q.contains(char::is_whitespace) || q.is_empty() {
        return None;
    }
    let lower = q.to_lowercase();
    let web = lower.starts_with("http://") || lower.starts_with("https://");
    let host = lower.split('/').next().unwrap_or("");
    let tld = host.rsplit('.').next().unwrap_or("");
    let domain = host.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) && !host.starts_with('.');
    if !(web || domain) {
        return None;
    }
    let address = if web { q.to_string() } else { format!("https://{q}") };
    Some(Command {
        title: format!("Open {q}"),
        subtitle: String::new(),
        kind: "Website",
        icon: Icon::Tile("safari.fill", WEB),
        run: Run::Url(address),
        confirm: None,
        key: Some(format!("url:{q}")),
    })
}

/// Search the web for what's typed.
pub fn web(query: &str) -> Command {
    let q = query.trim();
    let encoded: String = q
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    Command {
        title: format!("Search the web for “{q}”"),
        subtitle: "Google".into(),
        kind: "Web Search",
        icon: Icon::Tile("globe", WEB),
        run: Run::Url(format!("https://www.google.com/search?q={encoded}")),
        confirm: None,
        key: None,
    }
}

/// Do it. Whether Finder2 comes forward for it (a window of its own).
pub fn run(run: Run, cx: &mut App) -> bool {
    let osascript = |script: &'static str| {
        std::thread::spawn(move || {
            let _ = std::process::Command::new("/usr/bin/osascript").args(["-e", script]).output();
        });
    };
    // (What waits on the system, off the main thread; then a note of how it
    // went.)
    let later = |work: Box<dyn FnOnce() -> crate::toggles::Note + Send>, cx: &mut App| {
        cx.spawn(async move |cx| {
            let (title, detail, link) = cx.background_executor().spawn(async move { work() }).await;
            cx.update(|cx| crate::shots::note(title, detail, link, cx));
        })
        .detach();
    };
    match run {
        Run::Lock => {
            if !macos::lock_screen() {
                eprintln!("couldn't lock the screen");
            }
        }
        Run::Sleep => {
            std::thread::spawn(|| {
                let _ = std::process::Command::new("/usr/bin/pmset").arg("sleepnow").output();
            });
        }
        // (The system asks first, as from the Apple menu.)
        Run::Restart => osascript("tell application \"loginwindow\" to «event aevtrrst»"),
        Run::ShutDown => osascript("tell application \"loginwindow\" to «event aevtrsdn»"),
        Run::LogOut => osascript("tell application \"loginwindow\" to «event aevtlogo»"),
        Run::ToggleDarkMode => osascript("tell application \"System Events\" to tell appearance preferences to set dark mode to not dark mode"),
        Run::WiFi => later(Box::new(crate::toggles::wifi), cx),
        Run::Bluetooth { sure } => later(Box::new(move || crate::toggles::bluetooth(sure)), cx),
        Run::DoNotDisturb => later(Box::new(crate::toggles::do_not_disturb), cx),
        Run::Timer { secs, label } => crate::timers::start(secs, label, cx),
        Run::CancelTimer(id) => {
            crate::timers::cancel(id, cx);
            crate::shots::note("Timer Cancelled".into(), String::new(), None, cx);
        }
        Run::KeepAwake(until) => crate::awake::start(until, cx),
        Run::StopAwake => crate::awake::stop(None, cx),
        Run::Sound(ask) => crate::media::apply(ask, cx),
        Run::Media(key) => crate::media::press(key, cx),
        Run::AddEvent(event) => later(
            Box::new(move || match macos::add_event(&event.title, event.start, event.end, event.all_day) {
                Ok(()) => ("Added to Calendar".into(), format!("{} · {}", event.title, crate::calendar::when(event.start, event.end, event.all_day)), None),
                Err(why) if why == "denied" => (
                    "Calendar Is Off Limits".into(),
                    format!("{} may not add events: System Settings > Privacy & Security > Calendars can allow it.", crate::APP_NAME),
                    Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"),
                ),
                Err(why) => ("Couldn't Add the Event".into(), why, None),
            }),
            cx,
        ),
        Run::CalendarAccess => later(
            Box::new(|| {
                if macos::calendar_request() {
                    ("Calendar Allowed".into(), "Type “next meeting” or “my day” to see what's on.".into(), None)
                } else {
                    (
                        "Calendar Is Off Limits".into(),
                        format!("System Settings > Privacy & Security > Calendars can allow {}.", crate::APP_NAME),
                        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"),
                    )
                }
            }),
            cx,
        ),
        Run::ContactsAccess => later(
            Box::new(|| {
                if macos::contacts_request() {
                    ("Contacts Allowed".into(), "Type it again: “call …”, “message …”, “email …”.".into(), None)
                } else {
                    (
                        "Contacts Is Off Limits".into(),
                        format!("System Settings > Privacy & Security > Contacts can allow {}.", crate::APP_NAME),
                        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts"),
                    )
                }
            }),
            cx,
        ),
        Run::Shortcut(name) => later(Box::new(move || crate::quick_actions::run_named(&name)), cx),
        Run::CopySecret(secret) => {
            macos::copy_secret(&secret, 90.);
            crate::shots::note("Password Copied".into(), "Kept out of clipboard history, and off the clipboard in 90 seconds.".into(), None, cx);
        }
        Run::Output { id, name } => {
            if macos::set_audio_output(id) {
                crate::shots::note(format!("Sound on {name}"), String::new(), None, cx);
            } else {
                crate::shots::note(format!("Couldn't Play Sound on {name}"), String::new(), None, cx);
            }
        }
        Run::Connect { address, name, connect } => later(Box::new(move || crate::devices::connect(&address, &name, connect)), cx),
        Run::AddNote(text) => later(Box::new(move || crate::notes::add(&text)), cx),
        Run::SpeedTest => {
            if !crate::net::testing() {
                crate::shots::note("Testing Your Internet".into(), "About 20 seconds: it says when it's done.".into(), None, cx);
            }
            later(Box::new(crate::net::speed_test), cx);
        }
        Run::WifiPassword(network) => {
            cx.spawn(async move |cx| {
                let looked = cx.background_executor().spawn({
                    let network = network.clone();
                    async move { crate::net::look_up(&network) }
                });
                match looked.await {
                    // (Shown in Quick Open: the password, and the code to join by.)
                    Ok(_) => cx.update(|cx| crate::quick_panel::show_query("wifi password".into(), cx)),
                    Err(why) => cx.update(|cx| crate::shots::note(format!("No Password for “{network}”"), why, None, cx)),
                }
            })
            .detach();
        }
        Run::LocationAccess => {
            macos::location_request();
            if macos::location_access() == Some(false) {
                cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_LocationServices");
            }
        }
        Run::Alarm { at, label } => crate::timers::set_alarm(at, label, cx),
        Run::Remind(reminder) => later(
            Box::new(move || match crate::reminders::add(&reminder) {
                Ok(()) => ("Added to Reminders".into(), format!("{}{}", reminder.title, reminder.due.map_or(String::new(), |d| format!(" · {}", crate::reminders::when(&d)))), None),
                Err(why) if why == "denied" => (
                    "Reminders Is Off Limits".into(),
                    format!("{} may not add reminders: System Settings > Privacy & Security > Reminders can allow it.", crate::APP_NAME),
                    Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders"),
                ),
                Err(why) => ("Couldn't Add the Reminder".into(), why, None),
            }),
            cx,
        ),
        Run::EmptyTrash => {
            std::thread::spawn(|| {
                if let Err(e) = crate::finder::erase_trash() {
                    eprintln!("couldn't empty the Trash: {e}");
                }
            });
        }
        Run::EjectAll => {
            let volumes: Vec<PathBuf> = std::fs::read_dir("/Volumes")
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| macos::volume_flags(p).1)
                .collect();
            std::thread::spawn(move || {
                for volume in volumes {
                    if let Err(e) = macos::eject(&volume) {
                        eprintln!("couldn't eject {}: {e}", volume.display());
                    }
                }
            });
        }
        Run::Settings(id) => cx.open_url(&format!("x-apple.systempreferences:{id}")),
        Run::Quit { pid, force } => {
            macos::quit_app(pid, force);
        }
        Run::QuitAll(pids) => {
            for pid in pids {
                macos::quit_app(pid, false);
            }
        }
        Run::Url(address) => cx.open_url(&address),
        Run::DiskUsage => {
            crate::disk_usage::open(cx);
            return true;
        }
        Run::FreeUpSpace => {
            crate::disk_usage::free_up_space(cx);
            return true;
        }
        Run::TidyUp(folder) => {
            crate::browser::tidy_up(folder, cx);
            return true;
        }
        Run::Uninstall(app) => {
            crate::uninstall::open(app, cx);
            return true;
        }
        Run::Trash => {
            crate::browser::show_trash(cx);
            return true;
        }
        Run::NewWindow => {
            crate::browser::open(crate::finder::new_window_folder(cx), None, cx);
            cx.activate(true);
            return true;
        }
        Run::Preferences => {
            crate::settings_window::open(cx);
            cx.activate(true);
            return true;
        }
        Run::Capture(action) => {
            // (Once the panel's gone from the screen.)
            cx.spawn(async move |cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                cx.update(|cx| crate::shots::capture(action, cx));
            })
            .detach();
        }
        Run::Mode(mode) => crate::quick_panel::show_mode(mode, cx),
        Run::Colors => crate::quick_panel::show_native(crate::colors::extension(), "colors", serde_json::Value::Null, cx),
        Run::Store => crate::quick_panel::show_native(crate::store::extension(), "browse", serde_json::Value::Null, cx),
        Run::Resize => {
            let paths = crate::resize::selected(cx);
            crate::quick_panel::show_native(crate::resize::extension(), "resize", crate::resize::args(&paths), cx)
        }
        Run::Extension { ext, command } => crate::quick_panel::show_extension(&ext, &command, cx),
        Run::ExtensionsFolder => {
            let folder = crate::extensions::folder();
            let _ = std::fs::create_dir_all(&folder);
            crate::browser::open(folder, None, cx);
            cx.activate(true);
            return true;
        }
        Run::NewExtension => {
            crate::settings_window::new_extension(cx);
            cx.activate(true);
            return true;
        }
        Run::ClearClipboard => crate::clips::clear(),
        Run::Quicklink(link) => {
            if link.contains("://") {
                cx.open_url(&link);
            } else {
                let path = crate::finder::expand(&link);
                if path.is_dir() && !crate::listing::is_package(&path) {
                    crate::browser::open(path, None, cx);
                    cx.activate(true);
                    return true;
                }
                cx.open_with_system(&path);
            }
        }
        Run::Fill(_) | Run::Snippet(_) | Run::PastePlain | Run::UseSelection | Run::PasteText(_) => {}
        Run::FixSpelling => crate::quick_panel::fix_spelling(cx),
        Run::FocusWindow { pid, id } => {
            // (Off the main thread: it asks the app, F2 too for its own.
            // And Quick Open not giving the keyboard back: that app's
            // coming forward.)
            std::thread::spawn(move || macos::focus_window(pid, id));
            return true;
        }
        Run::SleepTimer { then, secs } => crate::timers::start_sleep(then, secs, cx),
        Run::Breaks { every, label } => crate::timers::start_breaks(every, label, cx),
        Run::Stopwatch(what) => crate::timers::stopwatch_do(what, cx),
        Run::RemindersAccess => later(
            Box::new(|| {
                if macos::reminders_request() {
                    ("Reminders Allowed".into(), "Type “todo” to see what's due.".into(), None)
                } else {
                    (
                        "Reminders Is Off Limits".into(),
                        format!("System Settings > Privacy & Security > Reminders can allow {}.", crate::APP_NAME),
                        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders"),
                    )
                }
            }),
            cx,
        ),
        Run::Music(player, what) => {
            cx.spawn(async move |cx| {
                let said = cx.background_executor().spawn(async move { crate::music::run(player, &what) }).await;
                if let Some((title, detail)) = said {
                    cx.update(|cx| crate::shots::note(title, detail, None, cx));
                }
            })
            .detach();
        }
        Run::MusicAccess(player) => {
            cx.spawn(async move |cx| {
                let allowed = cx.background_executor().spawn(async move { macos::automation(player.bundle(), true) }).await;
                cx.update(|cx| match allowed {
                    0 => crate::quick_panel::show_query("now playing".into(), cx),
                    _ => crate::shots::note(
                        format!("{} Can't Ask {}", crate::APP_NAME, player.name()),
                        format!("System Settings > Privacy & Security > Automation can allow it, under {}.", crate::APP_NAME),
                        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"),
                        cx,
                    ),
                });
            })
            .detach();
        }
        Run::CleanKeyboard(seconds) => {
            if !macos::accessibility(false) {
                macos::accessibility(true);
                crate::shots::note(
                    format!("Allow {} to Turn the Keyboard Off", crate::APP_NAME),
                    "Cleaning mode needs Accessibility, in System Settings > Privacy & Security.".into(),
                    Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
                    cx,
                );
            } else {
                // (Once Quick Open's gone, and Return's let go.)
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
                    cx.update(|cx| {
                        if !macos::clean_keyboard(seconds) {
                            crate::shots::note("Couldn't Turn the Keyboard Off".into(), "macOS didn't let it: check Accessibility in System Settings.".into(), None, cx);
                        }
                    });
                })
                .detach();
            }
        }
        Run::ReminderDone { id, title } => later(
            Box::new(move || match macos::reminder_done(&id) {
                Ok(()) => ("Checked Off".into(), title, None),
                Err(why) => ("Couldn't Check It Off".into(), why, None),
            }),
            cx,
        ),
        Run::RestartApp { pid, path, name } => later(
            Box::new(move || {
                macos::quit_app(pid, false);
                // (Till it's gone: it may ask to save first.)
                let started = std::time::Instant::now();
                while unsafe { libc::kill(pid, 0) } == 0 {
                    if started.elapsed() > std::time::Duration::from_secs(20) {
                        return (format!("{name} Didn't Quit"), "It may be asking about something unsaved.".into(), None);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                let _ = std::process::Command::new("/usr/bin/open").arg(&path).status();
                (format!("Restarted {name}"), String::new(), None)
            }),
            cx,
        ),
        Run::NightShift => match macos::night_shift(None) {
            Some(on) => {
                macos::night_shift(Some(!on));
                crate::shots::note(format!("Night Shift {}", if on { "Off" } else { "On" }), String::new(), None, cx);
            }
            None => crate::shots::note("No Night Shift Here".into(), "This Mac's screen hasn't it.".into(), None, cx),
        },
        Run::DesktopIcons => {
            let shown = macos::desktop_icons(None);
            std::thread::spawn(move || {
                macos::desktop_icons(Some(!shown));
            });
            crate::shots::note(if shown { "Desktop Icons Hidden" } else { "Desktop Icons Shown" }.into(), "Finder restarts to take it.".into(), None, cx);
        }
        Run::ShowDesktop => {
            macos::show_desktop();
        }
        Run::Stop { pid, name, port } => {
            crate::dev::stop(pid);
            crate::shots::note(format!("Stopped {name}"), format!("What was listening on port {port} (pid {pid})."), None, cx);
        }
        Run::DevTool(tool) => {
            let title = tool.about().0;
            match macos::clipboard_text().ok_or_else(|| "There's no text on the clipboard.".to_string()).and_then(|t| tool.apply(&t)) {
                Ok(done) => {
                    macos::copy_text(&done);
                    let first: String = done.lines().next().unwrap_or("").chars().take(80).collect();
                    crate::shots::note(format!("{title}: Copied"), first, None, cx);
                }
                Err(e) => crate::shots::note(format!("Couldn't {}", title.to_lowercase()), e, None, cx),
            }
        }
        Run::Uuid => {
            let id = crate::dev::uuid();
            macos::copy_text(&id);
            crate::shots::note("Copied a UUID".into(), id, None, cx);
        }
        Run::Timestamp => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()).to_string();
            macos::copy_text(&now);
            crate::shots::note("Copied the Unix Time".into(), now, None, cx);
        }
        Run::CopyText(text) => {
            macos::copy_text(&text);
            crate::shots::note(format!("Copied {text}"), String::new(), None, cx);
        }
        Run::Welcome => {
            crate::onboarding::open(cx);
            return true;
        }
        Run::SendFeedback => {
            crate::reports::open_feedback(cx);
            return true;
        }
        Run::DualPane => {
            crate::browser::toggle_dual_pane(cx);
            return true;
        }
        Run::About => {
            crate::about::show();
            return true;
        }
        Run::CheckForUpdates => crate::about::check(cx),
        Run::ShowLog => crate::about::show_log(cx),
        Run::SettingsAt(section) => {
            crate::settings_window::open_at(section, cx);
            cx.activate(true);
            return true;
        }
        Run::Server(host) => {
            crate::browser::open_server(crate::sftp::Dest { host, port: None, options: Vec::new() }, cx);
            cx.activate(true);
            return true;
        }
        Run::AddServer => {
            crate::browser::add_server(cx);
            return true;
        }
        Run::SshTerminal(host) => {
            let dest = crate::sftp::Dest { host, port: None, options: Vec::new() };
            if let Some(tool) = crate::tools::chosen(crate::tools::Kind::Terminal, crate::settings::Settings::get(cx))
                && let Err(e) = crate::remote::terminal(&dest, None, &tool.app)
            {
                eprintln!("ssh to {}: {e}", dest.host);
            }
        }
        Run::Window(place) => {
            // (Once the panel's gone, and the window in front is the app's
            // own again.)
            cx.spawn(async move |cx| {
                macos::given_back(cx.background_executor()).await;
                cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
                cx.update(|cx| crate::windows::apply(place, cx));
            })
            .detach();
        }
        Run::EditLastShot => {
            crate::shots::edit_last(cx);
            return true;
        }
        Run::QuickOpenSettings => {
            crate::settings_window::open_quick_open(cx);
            cx.activate(true);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{Keys, Run, keys};
    use crate::settings::Settings;

    fn shown(run: Run, settings: &Settings) -> Option<String> {
        shown_as(None, run, settings)
    }

    fn shown_as(key: Option<&str>, run: Run, settings: &Settings) -> Option<String> {
        let command = super::Command { title: String::new(), subtitle: String::new(), kind: "", icon: super::Icon::Tile("", 0), run, confirm: None, key: key.map(Into::into) };
        keys(&command, settings).map(|k| match k {
            Keys::Shortcut(s) => format!("shortcut {s}"),
            Keys::Keyword(k) => format!("keyword {k}"),
        })
    }

    #[test]
    fn names_rank_over_words() {
        let history = super::matches("Clipboard History", "clipboard history paste copied pasteboard", "clipboard").unwrap();
        let json = super::matches("Format JSON", "json format pretty print beautify clipboard", "clipboard").unwrap();
        assert!(history > json, "{history} > {json}");
        // A word still finds what its name doesn't.
        assert!(super::matches("Format JSON", "json format pretty print beautify clipboard", "beautify").is_some());
    }

    #[test]
    fn commands_show_their_shortcuts_and_keywords() {
        let mut settings = Settings::default();
        settings.window_shortcuts.insert(crate::windows::Place::LeftHalf.key().into(), "ctrl-alt-left".into());
        settings.clipboard_shortcut = Some("shift-cmd-v".into());
        assert_eq!(shown(Run::Window(crate::windows::Place::LeftHalf), &settings).as_deref(), Some("shortcut ctrl-alt-left"));
        assert_eq!(shown(Run::Window(crate::windows::Place::RightHalf), &settings), None, "none set");
        assert_eq!(shown(Run::Mode(crate::quick_panel::Mode::Clipboard), &settings).as_deref(), Some("shortcut shift-cmd-v"));
        assert_eq!(shown(Run::Snippet("¯\\_(ツ)_/¯".into()), &settings).as_deref(), Some("keyword ;shrug"));
        let link = settings.quicklinks.iter().find(|q| !q.keyword.is_empty() && !q.link.contains("{query}")).map(|q| (q.link.clone(), q.keyword.clone()));
        if let Some((link, keyword)) = link {
            assert_eq!(shown(Run::Quicklink(link), &settings), Some(format!("keyword {keyword}")));
        }
        assert_eq!(shown(Run::Lock, &settings), None);
        // (Any command, by its key, in Settings' Keyboard.)
        settings.shortcuts.insert("Lock Screen".into(), "ctrl-alt-l".into());
        assert_eq!(shown_as(Some("Lock Screen"), Run::Lock, &settings).as_deref(), Some("shortcut ctrl-alt-l"));
    }

    #[test]
    fn fix_spelling_is_found() {
        let found: Vec<String> = super::find_scored("fix spelling", 5).into_iter().map(|(_, c)| c.title).collect();
        assert_eq!(found.first().map(String::as_str), Some("Fix Spelling"), "{found:?}");
    }

    /// Every command Settings' Keyboard lists can be found again by its key
    /// (as a shortcut runs it), and no two share one.
    #[test]
    fn keys_find_their_commands() {
        let (commands, panes) = super::every();
        let mut seen = std::collections::HashSet::new();
        for c in commands.iter().chain(&panes) {
            let key = c.key.as_deref().expect("every command has a key");
            assert!(seen.insert(key.to_string()), "two commands are {key}");
            assert_eq!(super::by_key(key).map(|c| c.title), Some(c.title.clone()), "{key}");
        }
    }
}

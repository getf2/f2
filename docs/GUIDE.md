# F2 in detail

How each part of F2 works, and what it asks the internet for (and sends).
[FEATURES.md](../FEATURES.md) is the plain list of everything, and
[getf2.com/features](https://getf2.com/features) the same for everyone.

**Windows.** Windows and tabs (⌘N, ⌘T, ⌘-double-click a folder), each
remembering where it was. A sidebar of Favorites (yours: drag to reorder,
drop folders on "Favorites" to add, ⌃⌘T, Remove from Sidebar), Recents,
AirDrop, iCloud Drive, disks with eject buttons, cloud drives
(Dropbox, Google Drive, OneDrive), the network, and Tags. A toolbar you can
customize (right-click it), a path bar, a status bar with free space, a
preview pane (⇧⌘P), and a menu bar like Finder's with Settings (⌘,).
**Quick Open** floats over any app, as Raycast and Spotlight do: ⌘Space
from anywhere (Spotlight's, once that's turned off in System Settings;
Settings > Quick Open picks another, or records your own, and its font and
text size), ⌘P in a window, or the menu bar item. It opens on the screen
with the pointer, or the active window's, or the main one (Settings >
Quick Open). Dragged by its bottom bar (or its field while it's empty), it
goes anywhere on the screen, snapping to the middle and to where it
opened, and opens there from then on (Settings' Center It Again puts it
back). Simple mode shows only the field until you type; with files turned
off (Settings > Quick Open), files and folders are found only after "f "
("f budget"), which finds only them either way.
It's a launcher, as Raycast is, and it answers as you type:

**Answers.** A sum is worked out (`12*7`, `80 + 15%`, `sqrt 2`, `5 km in
mi`, `30 c to f`; Return copies the answer), and money converted (`100 usd
in eur`, `€20 to krw`; `$50`: into your Mac's currency) at the European
Central Bank's rates of the day, from [Frankfurter](https://frankfurter.dev),
kept and fetched again twice a day. The time anywhere: "time in tokyo",
"reykjavik time", "time in australia" or "time in texas" (each of its time
zones, east to west), "pst", "utc+5:30"; and a time from one place in
another: "3pm in tokyo" (your 3pm there), "3pm tokyo" (Tokyo's here),
"9:30 london in new york". It knows some 7,500 cities (from
[GeoNames](https://www.geonames.org)), countries, states and zones'
letters, and writes times as your Mac does. Dates: "days until christmas",
"100 days from today", "what day is 3 march 2027", "days since jan 1",
"week number". "sunset", "sunrise in tokyo", "day length": worked out here,
in that place's time. Words: "define serendipity", "meaning of …", "what
does … mean" (macOS's Dictionary; Return opens it there), "spell
accomodate" (in English), "synonym for happy" (macOS's thesaurus), "word
count" (of what's copied). Translation: "translate hola", "translate good
morning to japanese", "how do you say thank you in french", "thank you in
italian". Apple's, on your Mac (macOS 26; the languages downloaded once, in
System Settings > General > Language & Region > Translation Languages),
into your Mac's language unless you say; what Apple doesn't translate
(Icelandic, say) opens in Google Translate. The weather: "weather" (in the
city of your Mac's time zone), "weather in tokyo", "東京の天気", "서울
날씨": now and the next three days, from [Open-Meteo](https://open-meteo.com)
(sent the city, kept ten minutes). Public holidays: "holidays", "next
holiday", "holidays in japan", "iceland holidays", "日本の祝日": the next
ones, here (the Mac's region) or there, from
[Nager.Date](https://date.nager.at) (sent the country and year; kept once
fetched). Prices: "btc", "eth price", "0.5 sol", "price of dogecoin"
(in dollars, and your Mac's currency beside them, from
[CoinGecko](https://www.coingecko.com); Coinbase's if it's down), "aapl
stock", "samsung stock", "$tsla" (from Yahoo Finance, in what it's traded
in: it may be delayed; your country's listing first); in another currency
if you say ("btc in krw", "aapl stock in eur"); or just by name ("apple
price", "kaspa price": a stock or a coin, as Yahoo finds it first). Fetched only when asked,
sent the coin or the company, kept a minute. "qr <link or text>" shows a QR code to
scan with your phone ("qr" alone: of what you copied last); Scan QR Code,
with the screenshots, reads one off the screen. A tracking number (UPS,
USPS, FedEx, DHL, Amazon, Japan Post, Yamato, Sagawa…) opens its carrier's
page; a flight number ("KE 17", "UA837", "flight oz 102"), its page on
FlightAware, Flightradar24 or Google. The air: "air quality", "fine dust",
"미세먼지", "uv", "air quality in tokyo": fine dust (graded as Korea grades
it, in Korea; by the US AQI elsewhere), what to do about it, and the UV
index, from [Open-Meteo](https://open-meteo.com) (CAMS's data). Maps:
"directions to incheon airport", "walk to the park", "coffee near me",
"map of busan" open Apple Maps or Google Maps (and, in Korea, Naver Map
and Kakao Map). The bill: "tip 18% on 84", "split 120 four ways", "split
84 between 4 with 20% tip". In the kitchen: "1 cup flour in grams", "200 g
butter in cups", "2 tbsp sugar" (each ingredient weighs its own). Sizes:
"us 10 shoe in eu", "270 shoes in us", "us 8 dress in uk", "medium shirt
in korean". Chance: "flip a coin", "roll 2d6", "random number 1 to 100",
"pick sushi, pizza or tacos" (Return on Again for another).

**Colors.** Those Pick Color picked are kept: "colors" in Quick Open lists
them, each with a big swatch, every way it's written (HEX, RGB, HSL, HSB,
OKLCH, SwiftUI, NSColor) to copy, and its contrast on white and on black
(WCAG's). A color typed there, `#ff8800`, `rgb(255 136 0)`, `hsl(…)`,
`oklch(…)` or "color tomato", is shown every way it's written too.

**Your day.** Reminders: "remind me to call mom tomorrow at 5pm", "remind
me in 20 minutes to check the oven", "reminder pay rent on friday" go into
Reminders' default list (with an alert at a time; macOS asks once). The
calendar: "lunch with Sara friday at 1", "event dentist oct 30 at 9:30",
"standup tomorrow 10-10:15am", "workshop monday 2pm for 3 hours" become
events; "next meeting", "my day", "agenda" list today's and tomorrow's,
Return joining a Zoom, Meet or Teams call. Timers: "timer 10 min", "timer
25m tea", "a 3 minute timer for the eggs", "pomodoro"; the soonest counts
down beside the menu bar item (its menu cancels), and a sound and a note
say when one's done. "stopwatch" counts up beside it (Pause, Resume, Stop
in Quick Open; Stop in its menu). Breaks: "eye break every 20 min" (or
"eye breaks"), "stand up every hour", "remind me to drink water every 30
minutes": a soft sound and a note each time, till the menu bar item (or
"breaks") stops them. What's due: "todo", "my reminders", "what do i need
to do" list today's reminders and overdue ones, then those with no date;
Return checks one off. Alarms: "alarm 7am", "wake me at 6:30" (rung as a
timer is, while the Mac's awake: ⏰ and its time beside the menu bar
item). Sleep timers: "sleep in 30 min", "sleep at 11pm" put the Mac to
sleep then; "stop music in 45 minutes" pauses Music and Spotify then
(else, a video in a browser, say, mutes the sound). People: "call mom" (through your iPhone), "facetime sara",
"message dad", "email jun", from Contacts (macOS asks once); a number
too ("call 090 1234 5678", or one typed or copied: "call" alone). "note buy
milk" goes into Notes (macOS asks once whether F2 may control it).

**Your Mac.** "about this mac": its model, chip, memory, macOS and serial
number, to copy (for a support call). "storage": how much of the disk is
free, Free Up Space a Return away. "my ip": the Mac's address here and
to the internet (from ipify). "restart safari": quit and opened again.
"what's slowing my mac" ("why is my mac slow", "task manager"): how busy
it is (the processor, the memory), then the apps using the most, measured
as Activity Monitor measures (an app's helpers and tabs counted as its),
Return quitting one. "is the internet down" ("am i online"): checked a
step at a time (Wi-Fi or a cable, the router, names, Apple's servers, as
macOS checks: a Wi-Fi network's sign-in page is told apart), then what's
wrong in plain words and what to do. "clean keyboard" ("clean my keyboard
for 1 min", "clean the screen"): keys and clicks off for 30 seconds (with
Accessibility), a panel counting down, its Done the one place a click goes
through. Music: "now playing" ("what's playing"): the song in Music or
Spotify with its artwork, to pause, skip, love (Music's) or look up its
lyrics; "play taylor swift", "play chill playlist": the best match in your
Music library (a playlist by that name, an artist's, an album's, else what
Music's search finds), or Apple Music's or Spotify's search; "play music"
picks up where it was. F2 asks Music and Spotify only when they're open and
you ask (macOS asks you once for each).
"volume 30", "volume up", "mute", "brightness 50" (the
built-in screen's), and Play/Pause, Next Track, Previous Track as the
media keys. "speakers", "airpods", "sound output": where the sound goes,
Return switching it; "connect airpods", "disconnect …": Bluetooth devices,
as Control Center connects them. "battery": the Mac's, and the AirPods',
mouse's and keyboard's (as System Information has them). "speed test": the
internet's download and upload, by macOS's own test. "wifi password": a
network's password (macOS asks for yours), with a code a phone's camera
joins by; which network you're on, macOS tells apps only once Location's
allowed, so without it, those joined before. "keep awake", "keep awake for
2 hours", "until 5pm", "until downloads finish" keep the Mac and its
screen awake (a cup beside the menu bar item). Commands: Lock Screen,
Sleep, Restart, Shut Down, Log Out (the system asks first), Empty Trash
(asked first, here), Toggle Dark Mode, Wi-Fi and Bluetooth on and off
(Bluetooth asks first if a keyboard or mouse is on it), Do Not Disturb
(through a shortcut F2 brings, "F2 Do Not Disturb": Shortcuts asks to add
it the first time, as macOS lets apps change Focus no other way), Night
Shift, the desktop's icons hidden or shown (before sharing your screen),
Show Desktop, Eject All Disks; whole phrases find them ("turn wifi off",
"night shift on", "dnd off"). System Settings' panes by name ("wifi",
"displays", "sound"…); "quit safari", "force quit …", "quit all"; your
Shortcuts by name ("shortcut …" too); F2's own (Disk Usage, the Trash, a
new window, Settings). "latest download": the newest in Downloads.
Generate Password (found by "password"; or "generate password") makes a
strong one, as Safari's are, and a random one ("password 32", "generate
pin"): Return copies it marked as password managers mark theirs, so
clipboard histories (F2's too) leave it out, and it's off the clipboard
after 90 seconds. Paste as Plain Text pastes what's copied without its
formatting; text tools ("uppercase", "title case", "sentence case",
"clean up spaces", "remove line breaks", or "text tools" for all) change
what's copied, Return copying it changed.

**Selected text, anywhere.** Select text in any app, then press the
shortcut Settings > Quick Open records (or choose Use Selected Text): it's
read through Accessibility, or copied with ⌘C where an app doesn't say
(the clipboard put back), and Quick Open lists what to do with it:
translate it (if it's in another language than your Mac's), define it,
open its link, track it, work it out, search it, count it, make a QR code
of it, copy it as plain text, or change it where it is (UPPERCASE,
Title Case, its spaces or line breaks cleaned up: pasted over it).

A web address opens; anything else can be searched for on the web. A path
("~/Do…") goes to that folder. ⌘K lists what else can be done with a
result: show it in F2, open it in your terminal (⌘T) or editor (⌘E), Quick
Look (⌘Y), copy its path (⇧⌘C), send it by AirDrop or email, set an
image as the desktop picture, move it to
the Trash, quit a running app, its Quick Actions. What's done (copied, added, a timer rung) is said in a
note at the screen's corner, looking as macOS's notifications do.

As you type, it shows in gray what can follow, and Tab takes it: an
example ("time" → "in tokyo", "translate" → "good morning to japanese",
"photos from" → "last week"); what a conversion or a time goes into
("5 km" → "in mi", "100 usd" → "in eur", "3pm" → "in london"); what a
reminder or a translation still needs, after a space ("tomorrow at 9am",
"to spanish"); the rest of the city, language, unit or currency you're
typing ("time in reyk" → "javík", "100 usd in k" → "rw"); or the rest of
the result picked, as Spotlight does ("saf" → "ari", "quit sa" → "fari",
"~/Doc" → "uments/"). Input
methods (Korean, Japanese, Chinese) compose in it as anywhere: F2 takes the
focus while it's up, and gives it back to the app you were in. Apps are
found by their names in the languages your Mac is set to or has keyboards
for too ("メモ", "메모": Notes; hiragana for katakana, "めも"), and shown by
the name Finder gives them; full-width letters and digits ("１００ｕｓｄ")
count as plain ones; and the time is asked in Japanese, Chinese or Korean
as well ("東京の時間", "东京时间", "도쿄 시간", "日本の時間").

Half a question shows what it's going to be, over the web's search, so
it's clear the answer's coming: "18% of" is a Calculator row ("18% of …",
"Type a number to work it out"); so are "12 +", "5 km" and "100 usd to",
"tip", "split 120", "1 cup flour in", "pick", and the start of a command
("price of", "time in", "weather in", "directions to", "flight"). It isn't
picked: Return still does what the row under it does. An answer that comes
from the internet (a price, the air, your public IP) shows the rest at
once, and in its place a row with a light passing over where it'll be (a
light runs along the field's edge too, as for the weather and
translations) until it's here; if it's quick, it's just there. And "12*7="
works out, as a calculator's typed.

It searches in plain words too: "photos from last week", "big videos",
"pdfs from this month", "screenshots in downloads", "invoices from march"
become fsearch's filters (kinds of file, sizes, when they changed, where
they are; the other words names to look for), the newest (or biggest)
first, without what's made rather than yours (apps' insides, build
output); the heading says what was understood ("Photos · changed in the
last week"). A window's search field takes them as well.

It launches apps, as Raycast does: what's in the Applications folders
(and the system's own) comes first, found by the start of its name or of a
word in it, its initials ("vsc"), or its letters in order, the ones you
open most first; Return opens it. Below them, it goes to anything on the
Mac by a few letters of its name, typos forgiven (yours: not the system's,
apps' data in ~/Library, nor what's in dependencies, caches and hidden
folders, unless the query is a path or starts with "."), what
you've worked on lately first: fsearch's ranking reordered
by what you opened here and your recent commits, as fsearch's own daemon
does. Return opens it (a folder in F2, in the window it came from; a
file in its app), ⌘Return shows it in its folder; escape or a click
elsewhere puts it away, and the app you were in stays in front.

**Clipboard History** (a command there, or a shortcut Settings >
Clipboard records) lists what's been copied, newest first: text, links,
images and files, with all of the one picked beside the list; ⌘K pins
one (an address, a bank number), listed first and never let go of, Clear
Clipboard History too. It skips
what password managers mark as secret, and keeps 200 (or 50, or 1000).
**Emoji & Symbols** is a grid of Unicode's emoji (to 17.0), the ones used
lately first, and arrows, keys, math, typography and currency, found by
name or Unicode's words for them ("+1", "check"); ":" and a word does it
from the search. Return on either pastes it into the app you were in
(that takes Accessibility, asked for once; until then, and if Settings
say so, it's copied), ⌘Return only copies it. macOS asks once whether
F2 may read the clipboard, as it starts.

**Quicklinks** open a site or a folder by name or keyword; with
`{query}` in the link, what's typed after the keyword goes there ("gh
rust" searches GitHub). **Snippets** are text to paste, found by name, or
typed as their keyword in any app (";date"), which then becomes the
text: the keyword's taken back, the text pasted, the clipboard put back.
They fill in `{clipboard}`, `{date}`, `{time}`, `{datetime}`, `{day}`,
`{uuid}`, and put the cursor at `{cursor}`. Hearing keywords takes
Accessibility, keeps only the last few characters typed, and hears
nothing in password fields. Both are set in Settings > Quicklinks and
Snippets.

**Dictation** (Settings > Dictation, on macOS 26), after Hex's: hold its
shortcut in any app, speak, and let go; what you said is pasted where
you're typing, the clipboard put back after. The shortcut is a key held
alone (Right ⌥ at first; Right ⌘, Fn, a function key) or two together
(⌃⌥, ⌥Space), picked or recorded by holding them. Held with another key
(⌥E), it's the app's, and nothing's heard. Pressed twice, it keeps
listening hands-free until it's pressed again; or, set to Press, it's
pressed once to start and again to finish. Escape stops, pasting
nothing; ten minutes is the most. A pill at the bottom of the screen
(beside the notch, with the notch on) moves with your voice, then waves
while the words are worked out. Apple's
speech recognition does that on the Mac (SpeechAnalyzer), in the Mac's
language or the one picked, macOS downloading its model the first time;
nothing is sent, and only the last text is kept, for Quick Open's Paste
Last Dictation. It listens through Sound settings' input, unless that's
Bluetooth headphones (AirPods switch to call quality while their
microphone's on): then the Mac's own, unless the lid's closed. Settings
can say Sound settings' input whatever it is, or one microphone. It
takes the microphone, and Accessibility (to hear the shortcut and
paste).

**The notch** (Settings > Notch, off at first), as Alcove has it: its
black grows out to the sides to show a timer counting down, the
stopwatch, a screen recording, dictation listening, or what Music or
Spotify is playing (bars moving beside its artwork). F2's notes come down
from it instead of the corner, and wait while pointed at; a click opens
what one links to, or puts it away. Pointed at, it opens: what's playing,
with play, pause and skip (media keys, with Accessibility); all that's
going on, each with a button to stop it; and the last three notes, each
with an × to dismiss it. A screen without a notch gets one, from the
middle of the menu bar, only while there's something to show (or, with
that off, its notes go to the corner). What's playing is what Music and
Spotify announce to the Mac as they play; its artwork F2 asks the player
for when the song changes, only if it may already (as "now playing"
asks), Spotify's from the address Spotify gives (its own server). It's
drawn by Core Animation in a window that's clear around it (clicks go
through), the notch's size while nothing's going on.

**For developers:** "kill port 3000" or "ports" lists what's listening
(servers: apps and macOS's own only when their port's asked for, and
then it asks first), and Return stops it. Repos are found by name
(Return: on GitHub; ⌘E, ⌘T: the editor, the terminal), and the repo
you're in (a F2 window's, or the editor's file's) has Open Repo on
GitHub, Open Pull Requests and Copy Branch Name. Format or minify JSON,
encode and decode Base64 and URLs (the clipboard's text), generate a
UUID, copy the Unix time; a Unix timestamp typed reads as its date.

With nothing typed it lists everything, to scroll through as Raycast
does: Recent (what you chose there last: apps, files, folders, commands,
websites, newest first, saved as you choose them), the sidebar's
Favorites, the commands, every app, and System Settings' panes.

**Extensions** add commands to Quick Open: its Extension Store (or
Settings > Extensions > Browse the Store…) lists those in
[getf2/extensions](https://github.com/getf2/extensions), each with what
it does, the websites it may reach and what it asks for, to install,
update or take out. Wikipedia, Hacker News, MDN, Can I Use, Tailwind CSS,
Homebrew, npm, crates.io, PyPI, Lorem Ipsum and Kaomoji need no account;
GitHub, Linear, Jira, Notion, Todoist, Vercel and Cloudflare a token of
yours, kept in the keychain. Each runs in F2's sandbox, macOS's
JavaScriptCore shut in a process of its own: it can't open a file or run
a program, and reaches only the websites its manifest names. The store's
list is signed, and each file is checked against it as it's installed.
New Extension makes one of your own: [EXTENSIONS.md](../EXTENSIONS.md)
says how.

**For code.** Git's status shows beside names, as editors show it: M, A,
U (untracked), D, ! (conflict) for files, a dot for a folder with changes
inside, kept up as files change (fsearch's git status, read only for the
repos you look at). Open in Terminal (⌥⌘T) and Open in Editor (⌥⌘E) take
the selection, or the folder, to your terminal and editor (Ghostty, cmux,
iTerm, Warp…; Zed, Cursor, VS Code…: chosen in Settings, else the first
installed), also from the right-click menu.

**Gallery view** (⌘4), as Finder's: a big preview of what's selected
(its Quick Look thumbnail: images, PDFs, video), its name and what it is,
and the folder's thumbnails in a strip along the bottom; ← → go along it.
A toolbar too narrow for its tools (a pane of two) leaves out the least
needed first.

**Archives** (.zip, .tar and its .gz, .bz2 and .xz, .7z, .rar) open like
folders: unpacked into a cache (`~/Library/Caches/F2/Archives`, cleared
after a day unopened), the archive's name in the title and path bar, a bar
saying it's a copy, and Extract. Right-click > Extract unpacks beside it as
Archive Utility does (one item as itself, several in a folder named after
the archive); with two panes, Extract to Other Pane. Compress makes a zip.

**Disk images** (.dmg, .iso, .sparseimage, .sparsebundle…) open in F2,
attached with `hdiutil`, so they open with Finder quit too (macOS's
DiskImageMounter leaves an image's window to Finder). A locked one's
password is asked in F2's dialog; a downloaded one is checked through
first; one with a license to agree to goes to DiskImageMounter, which
shows it. The volume shows as its maker laid it out for Finder (its
.DS_Store): a window its size with no toolbar or sidebar, their picture
or color behind, each icon where they put it, the labels dark or light
to read over it. Any folder given a picture or color in Finder's View
Options shows it too. With an app alone in the image, a bar offers
Install (a copy in Applications), then Open, Eject, or Eject and Move to
Trash. Settings > General can make F2 what opens .dmg files from other
apps (macOS asks you to confirm), and back.

**Dual pane** (View > Show or Hide Dual Pane, ⌥⌘S): a window shows two
folders side by side, as Total Commander and ForkLift do, the sidebar on
the left. Tab goes from one to the other (the one with the keyboard shows
its selection in color); F5 copies what's selected to the other pane's
folder, F6 moves it there, asking about names taken as a paste does. Each
pane has its own tabs (⌘T ⌘W ⌃⇥ are the pane's with the keyboard; ⌘W on
the right pane's last closes it). ⌥⌘U swaps the panes; ⌥⌘O (or a
folder's right-click menu) opens the folder selected, else this one, in
the other pane. New windows open the way the last was left, in Home or
wherever Settings > General says (any folder).
A list too narrow for its columns leaves out the last ones, Name first.

**Disk Usage** (from the menu bar item, or Window > Disk Usage) shows what
takes up a disk as DaisyDisk does: a ring chart of the folder shown, each
ring a level deeper and each slice as big as what it holds, beside a
list of what's in it, biggest first. Click a slice to go in, the middle
(or ⌘↑, escape) to come out; right-click to show an item in F2 or
move it to the Trash. The startup disk comes from fsearch's index (all
of it in about 35 ms, a quarter second the first time); other disks are
scanned. Files of 64 MB and more count what they take on the disk, not
their length: a sparse disk image (Docker's Docker.raw says 64 GB and
holds 3; a VM's) is asked for what's allocated. Smaller files count their
length, as the index has it; APFS clones are each counted in full. Get
Info says what a sparse or compressed file takes on disk.

**Uninstall App** (an app's right-click menu, its ⌘K in Quick Open, or
"uninstall zoom" there): the app and what it leaves in your Library when
it's only dragged to the Trash (Application Support, Caches, Preferences,
its container and group containers, saved state, logs, login agents),
found by its bundle identifier and name, each with its size and ticked;
all to the Trash at once (Put Back works). Not macOS's own apps, nor F2; a
running app is quit first.

**Tidy Up** (View menu, a folder's right-click menu, or Quick Open's Tidy
Up Desktop and Tidy Up Downloads): a folder's loose files sorted into
folders of their kinds (Screenshots, Images, Videos, Audio, Documents,
Installers, Archives); folders, kinds it doesn't know and downloads still
coming in stay put, and ⌘Z puts it all back. Settings > Screenshots can
keep the Desktop tidy: screenshots on it moved into its Screenshots folder
as they're taken.

**Folder Rules** (Settings > Folder Rules, a folder's right-click menu,
or Quick Open), as Hazel's: a folder kept in order on its own. A rule says
what in a folder it's for (kinds, a word in the name, a size, how long ago
it was added or opened) and what's done with it: sorted into folders by
kind, moved to a folder, or put in the Trash. Its form says what it would
do right now; it touches only files loose in the folder that have finished
arriving, replaces nothing, and keeps what it did in Lately, each with
Undo.

**Free Up Space** (Clean Up's first page, or Quick Open): what's safe to
let go of, a card each with what it holds: installers (.dmg, .pkg) in
Downloads, what's been in Downloads for over six months (by when it came
in) and not opened in that time (by macOS's Date Last Opened), iPhone and
iPad backups (by device), apps' caches (not the system's), the Trash
(emptied on a second click), large files and copies. Review opens the
list, installers and caches ticked to begin with, Tick All for the rest;
downloads and large files say when each was last opened.

**Clean Up**, under the disks in Disk Usage: **Large Files** (100 MB and
more on the disk, the biggest first, an app's data marked as such) and
**Duplicates** (identical files of 1 MB and more in your home folder:
the same length in the index, then the same first and last 16 KB, then the
same SHA-256; the oldest of each kept, and APFS clones of one another said
to free next to nothing, by what each holds of its own). Tick, then Move to
Trash (Put Back works). Left out: the system's, the insides of apps and
libraries (Photos'), the Trash, and for copies ~/Library, hidden folders
and code's (.git, node_modules, a Rust project's target); files only in
iCloud aren't read, nor downloaded.

**Screenshots**, as BetterShot and CleanShot take them, from a shortcut
in any app, the menu bar item or Quick Open: an area (macOS's crosshair;
Space switches it to a window), a window, the screen the pointer's on,
the text in an area (read by Vision and copied), a long page whole
(**Scrolling Capture**: drag out an area, and F2 scrolls it and puts the
steps together where they overlap), or a color under the pointer (**Pick
Color**: copied as HEX, or as Settings > Screenshots say: RGB, HSL, HSB,
OKLCH, SwiftUI or NSColor). Each is copied, shown in the corner (drag
it into an app; over it, Edit, Copy, Save, Pin), and saved to a folder
if Settings say so. Pinned, it floats over everything (drag it about,
double-click to edit). The editor has arrows, lines, rectangles,
ellipses, a pen, a highlighter, text, numbered steps, blur and crop, in
eight colors and three widths; marks can be picked, moved and deleted,
and undone. A background (ten gradients, with padding, round corners and
a shadow) sets a screenshot off. ⌘C copies it, ⌘S saves it, Done (⌘↩)
copies it and closes. Settings > Screenshots records the shortcuts
(BetterShot's at first: ⇧⌘4 an area, ⇧⌘3 the screen, ⇧⌘O text); macOS
keeps ⇧⌘3 and ⇧⌘4 for its own screenshots until they're turned off in
System Settings > Keyboard > Keyboard Shortcuts > Screenshots, which
Settings point out. The first one asks for Screen Recording.

**Screen recording**: Record Screen (the one the pointer's on) or Record
Area (dragged out over the screens, dimmed: macOS's own picker won't
record video; Esc puts it away), from a shortcut Settings
> Screenshots records, the menu bar item or Quick Open; with the
microphone's sound and clicks shown, if Settings say. While it records, ●
and how long sit beside the menu bar item, whose menu stops it (as does
Quick Open's Stop Recording, or the shortcut again). It's saved as MP4
(macOS's own recording, made an MP4 as it is) where screenshots are
saved, the Desktop else, and shown in the corner: drag it into an app,
click it to play it, or show it in its folder.

The last 50 are kept: Screenshot History (Quick Open, or the menu bar
item) lists them newest first beside a big preview; Return copies one (or
pastes it, as Settings say), ⌘E edits it, ⌘S saves it, and ⌘K also pins
it or deletes it.

**Windows**, as Rectangle and Raycast place them: the window in front to
a half (again: ⅔, then ⅓), a quarter, a third or two, the whole screen
(or almost, or its height), the middle, a size smaller or larger, the
next or previous display, back where it was, or full screen. Quick Open
finds them ("left half", "next display"), and Settings > Windows sets
shortcuts in any app: Rectangle's (⌃⌥←…) or Spectacle's (⌥⌘←…) at a
click, or any recorded, and a gap round windows. It takes
Accessibility.

**Send Feedback** (Help menu, or Quick Open) goes straight to F2's maker.
If F2 quits unexpectedly, it asks at the next start before sending a
report (or always, or never: Settings > Advanced), with nothing in it that
says whose Mac it was. Both go to getf2.com only.

**The welcome**, at the first start (and from Quick Open's Welcome to
F2, or Settings > Permissions), in the website's look (a painting a step,
in Geist): what it does; the permissions it asks macOS for (Full Disk
Access, Accessibility, Screen Recording, the clipboard), each with why
and a button, turning green as they're allowed; the calendar, Reminders
and the Do Not Disturb shortcut, for Quick Open, if you'd like them; the
shortcuts (Quick Open's, those in the way: Spotlight's ⌘Space, macOS's
screenshot keys) and a set of window shortcuts; and things to try, with
Open at Login.

**Keyboard shortcuts**, all in one place: Settings > Keyboard (or
"keyboard shortcuts" in Quick Open) lists every shortcut, to search and
record. In any app: Quick Open's, the clipboard's, screenshots', windows',
and any command (Lock Screen, Toggle Dark Mode, Translate, Paste as Plain
Text…), app or folder (an app's, pressed again, hides it), quicklink or
snippet (pasted); a command that asks first (Empty Trash) asks in Quick
Open; one that wants words typed (Translate) opens Quick Open with them.
⌘K > Set Shortcut… on a result in Quick Open goes there. In F2's
windows: every menu item's key, Finder's to start with, changed by
recording another (Return to open, say), ↺ to put Finder's back. One
shortcut does one thing: recorded for another, it moves there.

Like Finder it's always running: ⌘Q closes its windows and steps aside,
and its item in the menu bar (an F2 tile) opens a window, a favorite
folder or the search again, turns on Open at Login, and quits it.

**Views.** Icons (any size), a list (choose, resize and reorder its
columns: Date Modified, Created, Added, Last Opened, Size, Kind, Tags;
folders expand in place, ⌥→ expands all) and columns (⌘3). Group By kind,
date, size or tags; sort by any of them; each folder remembers its view
(⌘J for View Options). In icon view, Sort By None puts icons where you
drag them, as Finder does: a folder you arranged in Finder opens arranged
so (its .DS_Store read, never written), what you move in F2 is kept in F2's
own file, the rest fill the grid's free places; Clean Up puts each in the
nearest place of the grid, Clean Up By lines them up by name, kind, date or
size, and the arrow keys go to the nearest icon that way. Finder's icons,
Quick Look thumbnails, kind names, date and size formats, tag dots, iCloud
badges, hidden extensions.
Calculate all sizes (View Options) and Get Info add folders up from the
index in milliseconds, and keep them current as files change; folders it
doesn't hold (other volumes, or guarded ones without Full Disk Access) are
walked as Finder would.

**Files.** Native right-click menus: Open, Open With, Move to Trash,
Delete Immediately, Get Info, Rename (several at once: replace text, add
text, or number them, seeing the new names as you type), Compress, Duplicate, Make Alias,
Quick Look, Copy, Share, Tags, Quick Actions (Make Smaller: a photo to
1600 pixels as JPEG, a video to 720p MP4, for email; convert images to
JPEG, PNG or HEIC; rotate; make or combine a PDF from images and PDFs;
Remove Background, a photo's subject kept on nothing (macOS 14 and later);
Copy Text, from a photo, a screenshot, a PDF or a scan; Resize…, to a
width, a share, a box or a height you choose, as copies named by their new
size, as they are or as JPEG, PNG or HEIC; Compress PDF;
Convert to MP4; and your Shortcuts; also in Quick Open's
⌘K for a file found), Download Now / Remove Download for iCloud,
New Folder with Selection, Show Original, Eject, Set Desktop Picture (an
image), Customize Folder (as Finder 26's: a color, which is a color tag,
and a symbol or an emoji on it, seen as it'll look while you pick); and,
on a folder or a
window's empty space, Import from iPhone or iPad (Continuity Camera's
Take Photo, Scan Documents, Add Sketch), saved into that folder. Copies and moves ask
before replacing (Replace, Keep Both, Merge folders, Stop, Apply to All),
show progress, and can be stopped; copies are APFS clones where they can
be. Tagged folders take their tag's color, as on macOS 26 (unless Finder's
Settings > Tags turns that off), an emoji on one kept as it is.
Drag and drop within the app, to and from other apps (files promised
by Mail or Photos too), spring-loaded folders, in every view. AirDrop
without Finder: drop files on it in the sidebar, or in its view. Undo and
redo for all of it.
The Trash: Put Back, Empty Trash, Delete Immediately.

**Get Info** is a window of its own, as Finder's: tags, More Info
(dimensions, duration, pages, where from, last opened), Locked, name and
extension, Hide extension, comments, Open with and Change All, a custom
icon, permissions. ⌥⌘I is the Inspector, following the selection.

**Quick Look** is the system's own panel: video, PDF pages, Markup, full
screen; arrows move through the files.

**Search.** As you type (or type to select by name, as Finder does: see
Settings), scoped to the folder or the whole Mac, by name with fsearch
(typo-tolerant, `ext:` `type:` `size:` `mtime:` `in:`, globs, `grep:`) or
inside files with Spotlight (PDFs, documents). Results sort by any column.
The ? in the search field (or Help > Search Tips) is a cheat sheet of the
query language, each example a click from trying. Or, with nothing to
learn, the bar under the toolbar (there as soon as the field's clicked)
has filters to pick, as Google Drive's chips: Kind (Folders, Images,
Videos, Music, Documents, PDFs, Presentations, Spreadsheets, Archives,
Apps, Code, Fonts), Date (Today, Past 7 Days, Past 30 Days, Past Year,
Over a Year Ago), Size (Under 1 MB to Over 1 GB) and Where (Desktop,
Documents, Downloads, iCloud Drive, Pictures, Movies, Music, Applications,
the home folder: whatever folder you're in). Picked, a chip says what
("PDFs ✕"), and Clear drops them all; they search by themselves ("PDFs,
Past 7 Days, in Downloads") or with what's typed, by name or inside files,
and a saved search keeps them.
Save a search as a Smart Folder; Smart Folders made in Finder open here.

**Servers over SSH**: your `~/.ssh/config`'s hosts, under Servers in the
sidebar (once there's one), browsed as folders with the Mac's own ssh, so
your keys, agent and known hosts work as in Terminal. A file opened is
copied over and saved back each time; copy both ways in two panes, and
Open in Terminal opens an SSH session there.

**Elsewhere.** Go to Folder (⇧⌘G) with completion, Connect to Server (⌘K)
and the network's file servers, the Go menu's places, folders sent from
other apps or the Dock, and (Settings > General) taking over other apps'
"Show in Finder". macOS doesn't let Finder or its Dock icon be removed,
and keeps opening other apps' folders in Finder (Launch Services refuses
another app for folders); an experimental setting closes the windows
Finder opens (its Dock icon's, those folders') and shows them here,
leaving Finder's AirDrop window be.

## Keys

| | |
|---|---|
| ↩ | rename (several selected: all at once) |
| ⌘O, ⌘↓ / double-click | open |
| type a name | select it (or search, as Settings say) |
| ↑ ↓, Home End, Page Up Page Down | move the selection |
| ⇧↑ ⇧↓, ⌘-click, ⇧-click, ⌘A | select several |
| → ← (⌥ for all) | expand, collapse; across in icons and columns |
| space, ⌘Y | Quick Look |
| ⌘1 ⌘2 ⌘3 ⌘4 | icons, list, columns, gallery |
| ⌘N ⌘T ⌘W, ⌃⇥ | new window, new tab, close, next tab |
| ⌥⌘S, ⇥, F5 F6, ⌥⌘U ⌥⌘O | dual pane, the other pane, copy or move to it, swap, open there |
| ⌘Q | close all windows (F2 keeps running; quit from its menu bar item) |
| ⌘Space (anywhere), ⌘P | Quick Open |
| ⇧⌘4 ⇧⌘3 ⇧⌘O (anywhere, as Settings set them) | capture an area, the screen, text |
| ↩, ⌘↩ (a clip or an emoji) | paste it into the app in front, copy it |
| ⌃⌥← ⌃⌥→ ⌃⌥↑ ⌃⌥↓ ⌃⌥↩ ⌃⌥C ⌃⌥⌫ ⌃⌥⌘← ⌃⌥⌘→ (with Rectangle's) | halves, maximize, center, restore, displays |
| V A L R O D H T 1 B C (editor) | select, arrow, line, rectangle, ellipse, pen, highlight, text, step, blur, crop |
| ⌘C ⌘S ⌘↩ ⌘Z ⇧⌘Z ⌫ (editor) | copy, save, done, undo, redo, delete the mark picked |
| ⌥⌘T ⌥⌘E | open in Terminal, in Editor |
| ⌘⌫, ⌥⌘⌫, ⇧⌘⌫ | move to Trash, delete immediately, empty Trash |
| ⌘D ⌘I ⌥⌘I ⇧⌘N ⌃⌘N | duplicate, Get Info, Inspector, new folder, with selection |
| ⌘C ⌥⌘C ⌘V ⌥⌘V | copy, copy as pathname, paste, move here |
| ⌘Z ⇧⌘Z | undo, redo |
| ⌃1–⌃7, ⌃0 | tag Red…Gray, no tags |
| ⌘E | eject |
| ⌘↑ ⌘[ ⌘] | enclosing folder, back, forward |
| ⇧⌘G ⌘K | Go to Folder, Connect to Server |
| ⇧⌘H ⇧⌘D ⇧⌘O ⌥⌘L ⇧⌘A ⇧⌘U ⇧⌘L ⇧⌘I ⇧⌘C ⇧⌘K ⇧⌘F | Home, Desktop, Documents, Downloads, Applications, Utilities, Library, iCloud, Computer, Network, Recents |
| ⌘F, esc | search, clear it |
| ⇧⌘. ⌥⌘P ⌘/ ⇧⌘P ⌃⌘S ⌘J | hidden files, path bar, status bar, preview, sidebar, view options |

# F2: what's built

Everything F2 does so far, as of 2026-10-26 (version 0.1.10). What 0.1.3
to 0.1.10 added is listed near the end. The README explains each part in
more detail.

## A Finder replacement

### Windows and getting around
- Windows and tabs (⌘N, ⌘T, ⌘-double-click a folder). Each one remembers where it was.
- A sidebar like Finder's:
  - Favorites: drag to reorder, drop folders to add, ⌃⌘T.
  - Recents, AirDrop, iCloud Drive.
  - Disks with eject buttons.
  - Cloud drives (Dropbox, Google Drive, OneDrive), the network, and Tags.
- A toolbar you can customize, with an Up button to the enclosing folder (Finder has none). When it's too narrow, it leaves out the least needed tools first.
- A path bar, a status bar showing free space, and a preview pane (⇧⌘P).
- A menu bar like Finder's, the Go menu's places, and all of Finder's shortcuts, each one changeable (Settings › Keyboard › In F2's Windows).
- Go to Folder (⇧⌘G) with completion. Connect to Server (⌘K) and the network's file servers.
- New windows open in Home, Desktop, Documents, Downloads, Macintosh HD, or any folder you pick (Settings › General).
- Like Finder, it's always running. ⌘Q closes the windows, and the menu bar item quits it.

### Views
- Four views:
  - **Icons**, at any size.
  - **List**, with columns you can choose, resize and reorder (Date Modified, Created, Added, Last Opened, Size, Kind, Tags). Folders expand in place, and ⌥→ expands all of them.
  - **Columns** (⌘3).
  - **Gallery** (⌘4): a big preview with a strip of thumbnails along the bottom.
- Group by kind, date, size or tags, and sort by any column. Each folder remembers its own view (⌘J for View Options).
- Finder's own icons, Quick Look thumbnails, kind names, date and size formats, tag dots, iCloud badges and hidden extensions.
- Folder sizes appear in milliseconds (worked out from the search index) and stay current as files change.
- When a list is too narrow, the last columns are left out.

### Files
- Native right-click menus:
  - Open, Open With, Move to Trash, Delete Immediately.
  - Get Info, Rename, Compress, Duplicate, Make Alias.
  - Quick Look, Cut, Copy, Share, Tags.
  - iCloud's Download Now and Remove Download.
  - New Folder with Selection, Show Original, Eject, Set Desktop Picture.
  - Import from iPhone or iPad (Continuity Camera): Take Photo, Scan Documents or Add Sketch, saved straight into the folder.
- Rename several files at once: replace text, add text, or number them, with the new names shown as you type.
- Quick Actions:
  - Make Smaller: a photo to 1600 px JPEG, or a video to 720p MP4, for email.
  - Resize…: images to a size you choose (a width, a share of it, a box, a height; typed, or one of those offered), copies beside them named by their new size ("IMG_1234 (1920×1440).jpg"), upright and with their metadata, as they are or as JPEG, PNG or HEIC. Also "Resize Images" in Quick Open, on what's selected.
  - Remove Background: keeps a photo's subject on a clear background (macOS 14 and later).
  - Copy Text: from a photo, a screenshot, a PDF or a scanned PDF.
  - Convert images to JPEG, PNG or HEIC, and rotate them.
  - Make or combine PDFs, Compress PDF, Convert to MP4.
  - Your own Shortcuts.
- Copying and moving:
  - Cut and paste: ⌘X, then ⌘V moves the files (Finder only copies, or moves with ⌥⌘V). Cut files are dimmed until they're pasted; Escape lets them be.
  - New File (⌥⌘N): an empty "untitled.txt", ready to name.
  - It asks before replacing (Replace, Keep Both, Merge, Stop, Apply to All).
  - It shows progress, how fast it's going and how long's left, and can be stopped.
  - A file that can't be copied (no permission, a name the disk won't take) is left out and the rest copied on, with a list of what was left out and why at the end; what would fail every file (the disk full, the drive or server gone) stops it. A move never deletes the original if any of it couldn't be copied.
  - Stopped, what's copied stays: copied again, Continue copies what's missing and leaves what's there.
  - Copies are APFS clones where possible.
- Drag and drop:
  - Within F2, and to and from other apps (including files that Mail and Photos only promise).
  - Out to other apps as from Finder: onto a window in front of F2's too, and from a window in the background, which stays there (and the app you were in stays in front). The app you drop on picks: Finder moves within a disk and copies to another, the Dock's Trash trashes (with Undo), Mail attaches. The drag shows the items' own icons.
  - You see it land: the folder you drop on pulses, what moves out fades away and what comes in fades in. Dropped in a folder that isn't shown, the status bar says where it went.
  - And hear it: a soft pop (Settings › Advanced turns it off).
  - The drag shows how many items it carries, and a + while ⌥ makes it a copy.
  - Spring-loaded folders, which blink just before they open, as Finder's do.
- AirDrop without Finder.
- Undo and redo for everything.
- The Trash: Put Back, Empty Trash, Delete Immediately. Moving to it and emptying it make Finder's sounds (unless they're off in System Settings › Sound).
- **Get Info** window, like Finder's:
  - Tags and More Info (dimensions, duration, pages, where from, last opened).
  - Locked, Hide extension, comments.
  - Open With and Change All, a custom icon, permissions.
  - The Inspector (⌥⌘I) follows the selection.
- **Preview (Space)**: F2's own, at once, over the files, following the selection as it moves (the next and previous read ahead): images (HEIC and camera raw too, fitted), text and code (wrapped, the first 256 KB), PDFs (their first 40 pages), folders (what's in them), anything else as its icon. Open and Quick Look buttons; Space, Esc or ⌘W put it away.
- **Quick Look**, the system's own panel, for the rest (video, audio, Office and iWork documents), and from the preview: video, PDF pages, Markup, full screen.

### Search
- Search as you type, in the folder or across the whole Mac. It runs on fsearch's whole-disk index, where results come back in under a millisecond.
- Forgives typos. Supports `ext:`, `type:`, `size:`, `mtime:`, `in:`, globs and `grep:`, or searching inside files with Spotlight.
- Plain-words search: "photos from last week", "big videos", "pdfs from this month", "screenshots in downloads".
- A cheat sheet of search tips (the ? in the search field).
- Smart Folders: save a search as one, and Smart Folders made in Finder open here too.
- Settings › Search shows how the index is doing: what it holds, what it leaves out and why, and Rebuild Index.
- Searching in a USB drive, a disk image or a network share works too: the index doesn't hold other disks, so the folder is looked through there and then (and kept while you type). The same goes for Desktop, Documents and Downloads without Full Disk Access.

### Archives
- .zip, .tar (.gz, .bz2, .xz), .7z and .rar open like folders.
- Extract unpacks them the way Archive Utility does, and Extract to Other Pane works with two panes. Compress makes a zip.

### Dual pane (⌥⌘S)
- Two folders side by side, like Total Commander or ForkLift. Each pane has its own tabs.
- Tab switches panes. ⌃⌘C copies to the other pane and ⌃⌘M moves there (or F5 and F6).
- Right-click a folder or a server in the sidebar: Open in Other Pane (two panes, if there weren't).
- ⌥⌘U swaps the panes. ⌥⌘O opens a folder in the other pane.

### Servers over SSH
- Your `~/.ssh/config` hosts under Servers in the sidebar, and in Quick Open ("Open pi", "SSH to pi"). Connect to Server (⌘K) takes `ssh://user@host:port/folder` too.
- Servers shows in the sidebar only once there's one (in `~/.ssh/config`, or added with ⌘K or "Add Server" in Quick Open), so it isn't there if you don't use SSH. Settings › Sidebar hides it.
- Uses the Mac's own ssh, so your config, keys, agent (1Password's too) and known hosts work as they do in Terminal. A password, a key's passphrase or a new server's key is asked for in a dialog.
- Browse a server's folders in a window: list, columns or icons, folders opened inline, the path bar from the server's top.
- Open a file: it's copied to the Mac and opened in its app, and each time you save it, it goes back to the server. Space shows it.
- New Folder, New File, Rename, Delete (asked first: a server has no Trash), Copy Path (`host:/path`).
- Copy both ways with ⌃⌘C in two panes or by pasting, with progress; names already taken there are left out, not replaced.
- Open in Terminal opens an SSH session in that folder, in your terminal.

### For code
- Git status next to file names (M, A, U, D, !), plus a dot on folders with changes inside.
- Open in Terminal (⌥⌘T) and Open in Editor (⌥⌘E):
  - Terminals: Ghostty, cmux, iTerm, Warp…
  - Editors: Zed, Cursor, VS Code…

### Standing in for Finder
- Other apps' "Show in Finder" opens F2 instead.
- **Disk images** (.dmg, .iso, .sparseimage…) open in F2 itself, so they still open with Finder quit (macOS's own mounter leaves their window to Finder):
  - Shown as their makers laid them out for Finder: the window's size, no toolbar or sidebar, their picture (or color) behind, each icon where they put it, the labels dark or light to read over it.
  - A locked one asks for its password. A downloaded one is checked through first; one with a license to agree to goes to macOS's mounter, which shows it.
  - With an app alone in it: Install copies it to Applications, then Open, Eject, or Eject and Move to Trash.
  - Settings › General can make F2 what opens .dmg files from other apps too (macOS asks you to confirm).
- Any folder given a picture or color in Finder's View Options shows it behind its icons.
- Experimental: windows Finder opens (from its Dock icon, or folders other apps open) close and open in F2 instead.

## Quick Open (the launcher)

### Opening it
- ⌘Space from any app, or ⌘P in a window, or the menu bar item.
- Settings › Quick Open:
  - The shortcut: one of the presets, or record your own.
  - Which screen it opens on: the one with the pointer, the active window's, or the main one.
  - The font and the text size.
- With nothing typed, it lists Recent, Favorites, the commands, every app and System Settings' panes.

### Finding things
- **Apps**:
  - By name, initials ("vsc") or letters in order, with the apps you open most listed first.
  - Also by their names in Japanese, Korean or Chinese ("メモ", "めも", "메모" find Notes).
- **Files and folders** anywhere on the Mac, by a few letters, typos forgiven:
  - What you've worked on lately comes first.
  - Return opens it; ⌘Return shows it in its folder.
- A path ("~/Do…") goes to that folder. A web address opens, and anything else can be searched on the web.
- ⌘K on a result shows what else you can do with it:
  - Show in F2, open in the terminal (⌘T) or the editor (⌘E).
  - Quick Look (⌘Y), copy the path (⇧⌘C).
  - AirDrop it or email it; an image set as the desktop picture.
  - Trash it, quit the app, Uninstall, Quick Actions.
- Each command shows its shortcut or keyword.
- **Keyboard shortcuts** (Settings › Keyboard), in one list you can search:
  - Any command (Lock Screen, Toggle Dark Mode, Translate, Paste as Plain Text…), app, folder, quicklink or snippet, from any app. An app's shortcut pressed again hides it; a snippet's pastes it; one that asks first (Empty Trash) asks in Quick Open.
  - Quick Open's own, the clipboard's, screenshots' and windows' shortcuts are there too.
  - ⌘K › Set Shortcut… on a command, app or folder in Quick Open goes straight to it.
  - One shortcut does one thing: recorded for another, it moves there.

### Answers as you type
- **Calculator**: `12*7`, `80 + 15%`, `sqrt 2`, units (`5 km in mi`, `30 c to f`).
- **Colors**: `#ff8800`, `rgb(255 136 0)`, `hsl(…)`, `oklch(…)`, `color tomato`: every way it's written (HEX, RGB, HSL, HSB, OKLCH, SwiftUI, NSColor) to copy, and its nearest named color.
- **Currency**: `100 usd in eur`, `€20 to krw`, `$50` (into your own currency), at the ECB's daily rates.
- **Prices**:
  - Coins: "btc", "eth price", "0.5 sol", "price of dogecoin", in dollars with your own currency beside them, and the day's change (CoinGecko, Coinbase if it's down).
  - Stocks: "aapl stock", "samsung stock", "$tsla", in the currency they trade in, with the day's change (Yahoo Finance, may be delayed); your country's listing first.
  - By name, either: "apple price", "kaspa price".
  - In another currency: "btc in krw", "0.5 eth in eur", "aapl stock in isk".
  - A Return copies the price; the next row opens its chart.
- **World time**:
  - "time in tokyo", "time in texas" (every time zone in it), "pst", "utc+5:30".
  - "3pm in tokyo", "9:30 london in new york".
  - About 7,500 cities.
- **Dates**: "days until christmas", "100 days from today", "what day is 3 march 2027", "week number".
- **Words**:
  - Dictionary: "define serendipity", "what does … mean".
  - Spelling: "spell accomodate" gives "accommodate" (checked in English).
  - Synonyms: "synonym for happy", from macOS's thesaurus, by meaning, with opposites.
  - Word count: "word count" counts the words, characters and lines in what you copied.
- **Translate**: "translate hola", "good morning to japanese", "how do you say thank you in french".
  - It uses Apple's translation on the Mac. Languages Apple doesn't have open in Google Translate.
- **Weather**: "weather", "weather in tokyo", "東京の天気", "서울 날씨". Now and the next three days.
- **Sunrise and sunset**: "sunset", "sunrise in tokyo", "day length", worked out on the Mac, in that place's time.
- **QR codes**: "qr <link or text>" shows one to scan with your phone; "qr" alone makes one of what you copied last.
- **Holidays**: "holidays", "next holiday", "holidays in japan", "日本の祝日": the next public holidays, here or in a country.
- **Air quality**: "air quality", "fine dust", "미세먼지", "uv", "air quality in tokyo". Fine dust with what to do about it (graded as Korea grades it, in Korea), and the UV index now and today's high.
- **Bill**: "tip 18% on 84", "tip on 84" (15, 18 and 20%), "split 120 four ways", "split 84 between 4 with 20% tip".
- **Kitchen**: "1 cup flour in grams", "200 g butter in cups", "2 tbsp sugar". Each ingredient weighs its own.
- **Sizes**: shoes ("us 10 shoe in eu", "270 shoes in us", "women's 8 shoe") and clothes ("us 8 dress in uk", "korean 66 dress in us", "medium shirt in korean").
- **Chance**: "flip a coin", "roll a die", "roll 2d6", "random number 1 to 100", "pick sushi, pizza or tacos". Again gives another.
- **Maps**: "directions to incheon airport", "walk to the park", "coffee near me", "map of busan". Apple Maps or Google Maps; Naver Map and Kakao Map in Korea.
- **Flights**: type a flight number ("KE 17", "UA837") to open it on FlightAware, Flightradar24 or Google.
- **Package tracking**: paste a tracking number (UPS, USPS, FedEx, DHL, Amazon, Japan Post, Yamato, Sagawa…) and it opens the carrier's page, with 17TRACK for anything else.

### Your day
- **Reminders**: "remind me to call mom tomorrow at 5pm", "remind me in 20 minutes to check the oven".
- **Calendar**:
  - Adds events as you say them: "lunch with Sara friday at 1", "standup tomorrow 10-10:15am", "workshop monday 2pm for 3 hours".
  - "next meeting", "my day", "agenda" list what's on. Return joins a Zoom, Meet or Teams call.
- **To-do**: "todo", "my reminders", "what do i need to do" list what's due today and overdue, then reminders with no date. Return checks one off.
- **Timers**: "timer 10 min", "timer 25m tea", "pomodoro". A countdown shows in the menu bar, and a sound plays when it's done.
- **Stopwatch**: "stopwatch" counts up in the menu bar; pause, resume or stop it from Quick Open or the menu.
- **Breaks**: "eye break every 20 min", "stand up every hour", "drink water every 30 minutes". A soft sound and a note each time, till you stop them from the menu bar item or "breaks".
- **Alarms**: "alarm 7am", "wake me at 6:30", "alarm friday 9am gym". ⏰ and the time show in the menu bar.
- **Sleep timers**: "sleep in 30 min", "sleep at 11pm" put the Mac to sleep; "stop music in 45 minutes" pauses Music and Spotify (or mutes, for a video in a browser).
- **People**: "call mom" (through your iPhone), "facetime sara", "message dad", "email jun", found in Contacts; or a number ("call 090 1234 5678", or "call" with one copied).
- **Notes**: "note buy milk" adds it to Apple Notes.

### Your Mac
- **About this Mac**: "about this mac" shows the model, chip, memory, macOS and serial number, to copy.
- **Disk space**: "storage" shows how much is free, with Free Up Space and Disk Usage a Return away.
- **My IP**: "my ip" shows the Mac's address on this network and to the internet.
- **Restart an app**: "restart safari" quits it and opens it again.
- **Sound and screen**:
  - "volume 30", "volume up", "mute", "brightness 50", plus Play/Pause, Next and Previous Track.
  - Sound output: "speakers", "airpods" or "sound output" lists where sound can go; Return switches it.
- **Bluetooth**: "connect airpods", "disconnect …" for headphones, speakers, mice and keyboards.
- **Battery**: "battery" shows the Mac's charge and time left, plus the AirPods' (left, right, case), mouse's and keyboard's.
- **Internet**:
  - Speed test: "speed test" measures download, upload and responsiveness with macOS's own test.
  - Wi-Fi password: "wifi password" shows a network's password (macOS asks for yours) and a QR code guests' phones can scan to join.
- **Toggles**:
  - Wi-Fi and Bluetooth on and off.
  - Do Not Disturb, through the bundled "F2 Do Not Disturb" shortcut.
  - Keep awake: "keep awake", "for 2 hours", "until 5pm", "until downloads finish".
  - Night Shift on and off.
  - "hide desktop icons" (before sharing your screen) and back; Show Desktop.
  - Whole phrases work: "turn wifi off", "night shift on", "dnd off".
- **System**:
  - Lock Screen, Sleep, Restart, Shut Down, Log Out.
  - Empty Trash, Toggle Dark Mode, Eject All Disks.
  - Any System Settings pane by name.
  - "quit safari", "force quit …", "quit all".
- **Windows**: "left half", "next display", and so on (see Window management).
- **Your Shortcuts**: run any of them by name.
- **Latest download**: "latest download" lists the newest files in Downloads.
- **Passwords**: Generate Password (found by typing "password") makes a strong one and a random one; "generate password", "password 32" and "generate pin" go straight to them. Return copies it out of clipboard history, and it's cleared from the clipboard after 90 seconds.

### Typing help
- **Ghost text** (Tab accepts it):
  - Examples ("time" → "in tokyo") and what a conversion goes into ("5 km" → "in mi").
  - What a reminder or translation still needs.
  - The rest of a city, language, unit or currency name.
  - The rest of the top result ("saf" → "ari").
- **Input methods**: Korean, Japanese and Chinese compose properly. Ghost text hides while you're composing.
- Full-width characters count as normal ones ("１００ｕｓｄ"), and the time can be asked in Japanese, Chinese or Korean ("東京の時間", "东京时间", "도쿄 시간").

### Clipboard, emoji, quicklinks, snippets
- **Clipboard History**:
  - Text, links, images and files, newest first, with a preview. Images are found by the words in them, too.
  - Skips what password managers mark as secret.
  - Keeps 50, 200 or 1000 items.
- **Emoji & Symbols**: all of Unicode's emoji (to 17.0) plus arrows, keys, math and currency symbols, with recently used first. Type ":" and a word to search.
- Return pastes a clip or emoji into the app you were in; ⌘Return only copies it.
- **Paste as Plain Text**: pastes what you copied without its formatting, then puts the clipboard back as it was.
- **Text tools**: "uppercase", "lowercase", "title case", "sentence case", "clean up spaces", "remove line breaks" (or "text tools" for all) change what you copied; on selected text they replace it where it is.
- **Fix Spelling**: fixes the spelling of the text selected in any app, where it is, with macOS's own spell checker (no AI, nothing leaves your Mac), in the text's language. Names and code are left alone, and a note says what changed. Give it a shortcut in Settings › Keyboard.
- **Pinned clips**: ⌘K pins a clip (an address, a bank number) so it's listed first and never pruned, even by Clear Clipboard History.
- **Selected text, anywhere**: select text in any app, then press its shortcut (Settings › Quick Open) or choose Use Selected Text. Quick Open lists what to do with it: translate it (if it's in another language), fix its spelling, define it, open its link, track it, work it out, search it, count it, make a QR code of it, or copy it as plain text.
- **Quicklinks**: open a site or folder by keyword, with `{query}` ("gh rust" searches GitHub).
- **Snippets**:
  - Text to paste, found by name or typed as a keyword in any app (";date").
  - Fills in `{clipboard}`, `{date}`, `{time}`, `{uuid}` and `{cursor}`.

### For developers
- "kill port 3000" and "ports" list what's listening; Return stops it.
- Repos by name:
  - Open on GitHub, in the editor or in the terminal.
  - For the repo you're in: Open Repo on GitHub, Open Pull Requests, Copy Branch Name.
- Format or minify JSON, encode and decode Base64 and URLs.
- Generate a UUID, copy the Unix time, and read a typed Unix timestamp as a date.

### Extensions
- **Extension Store** (Quick Open, or Settings › Extensions › Browse the Store…): what each does, its commands, the websites it may reach and what it asks for; install, update and uninstall. From github.com/getf2/extensions, its list signed and each file checked before it's installed.
- At launch: Wikipedia (articles read in Quick Open), Hacker News (its comments too), MDN, Can I Use, Tailwind CSS, Homebrew, crates.io, PyPI, npm, Lorem Ipsum and Kaomoji, with no account; GitHub, Linear, Jira, Notion, Todoist, Vercel and Cloudflare with a token of yours (in the keychain).
- Each runs in F2's sandbox (macOS's JavaScriptCore, shut in): no files, no programs, only the websites its manifest names.
- **New Extension…**: one of your own, in JavaScript, ready to run; saved, its open page runs again, and errors say where. Programs in any language with Developer Mode on (Settings › Extensions). EXTENSIONS.md says all of it.

## Screenshots
- From a shortcut in any app, the menu bar item or Quick Open:
  - An area (Space switches to a window), a window, or the screen the pointer's on.
  - **Scrolling Capture**: drag out an area, and F2 scrolls what's in it and takes all of it, put together where the steps overlap into one tall screenshot (a header or toolbar that stays put, kept once). It stops at the bottom, at 30,000 pixels, or at Esc. It scrolls for you, with Accessibility.
  - The text in an area (copied).
  - A color off the screen (Pick Color), copied as Settings › Screenshots says: HEX, RGB, HSL, HSB, OKLCH, SwiftUI or NSColor. **Colors** in Quick Open keeps those picked, each with a swatch, every way it's written, and its contrast on white and black (WCAG).
  - A QR code read off the screen.
- Each shot is copied and shown in the corner. Drag it into an app, or use Edit, Copy, Save or Pin.
- Pin floats a screenshot over everything.
- **Editor**:
  - Arrows, lines, rectangles, ellipses, a pen, a highlighter, text, numbered steps, blur and crop, in eight colors and three widths.
  - Undo, and marks you can pick, move and delete.
  - Gradient backgrounds with padding, rounded corners and a shadow.
- **Screenshot History**: the last 50, with a big preview. Copy, paste, edit, save, pin or delete each one.
- **Screen recording**: Record Screen or Record Area (drag out the area; Esc cancels), from a shortcut, the menu bar item or Quick Open. The microphone and clicks if you'd like. ● and the time show in the menu bar while it records; stop it there, in Quick Open, or with the shortcut again. Saved as MP4 and shown in the corner to drag, play or show in its folder, or save as a GIF (up to 800 px wide and 12 frames a second, for where a video won't play).
- Optional: keep the Desktop tidy by moving screenshots into a Screenshots folder.

## Window management
- Like Rectangle and Raycast:
  - Halves (again for ⅔, then ⅓), quarters, thirds, maximize, almost maximize, maximize height.
  - Center, smaller and larger, next and previous display, restore, full screen.
- Rectangle's or Spectacle's shortcuts set up in one click, or record your own. Optional gap around windows.
- **Drag a window with keys held** (⌃⌘, or ⌥⌘, ⌃⌥, or off, in Settings › Windows): drag from anywhere in a window to move it, or from near an edge or a corner to size it from there. The app underneath never sees the click.
- **Switch Window**: every window open, not just apps as ⌘Tab has them, front to back (minimized, hidden and those on other desktops too, marked; picking one there goes to its desktop), with a picture of the one picked beside the list (with Screen Recording allowed; a minimized one, its app's icon). Type to find one by its title or app. Give it a shortcut (⌥Tab, say) in Settings › Keyboard: hold its modifier, press again to go on, let go to switch, as ⌘Tab does.

## Dictation
- **Hold a key, speak, let go** (Settings › Dictation; macOS 26): what you said is typed where you're typing, in any app. Apple's own speech recognition does it, on the Mac: nothing is sent anywhere and no recording is kept.
- The shortcut is one key held alone (Right ⌥, Right ⌘, Fn, a function key) or two together (⌃⌥, ⌥Space): pick one, or hold the keys you'd like. Holding it with another key (⌥E for é) is the app's, not dictation.
- **Hands-free**: press the shortcut twice to keep listening, then once more to finish. Or set it to **Press to start, again to stop** instead of holding. Escape stops, pasting nothing.
- A pill at the bottom of the screen moves with your voice while it listens.
- **Microphone**: Automatic (Sound settings' input, but not Bluetooth headphones': AirPods would switch to call quality, so the Mac's own stands in, unless its lid's closed), Same as Sound Settings, or one you pick.
- In 30 languages and dialects (English, French, German, Spanish, Italian, Portuguese, Chinese, Cantonese, Japanese, Korean), this Mac's own unless you pick one; macOS downloads each language's model the first time.
- **Paste Last Dictation**, in Quick Open, pastes the last one again.
- After [Hex](https://github.com/anomalyco/hex)'s dictation, by Kit Langton.

## Disk space
- **Disk Usage**, like DaisyDisk:
  - A ring chart of what takes up a disk. Click a slice to go in; right-click to show it in F2 or trash it.
  - The startup disk is read from the index in about 35 ms.
  - Big files count what they really take up on the disk, so a sparse disk image like Docker's isn't counted at its full size.
  - A file with more than one name (a hard link, as in Rust's and pnpm's builds) counts once, a share under each name.
- **Free Up Space**: cards for what's safe to let go of. Review opens each list; Tick All ticks the whole list.
  - Installers in Downloads.
  - Old Downloads: in Downloads for over six months and not opened since.
  - iPhone and iPad backups, apps' caches, the Trash.
  - Large files and duplicates.
  - **Build Folders**: your projects' builds and packages (Rust's target, node_modules, Next.js's .next, Python environments, Swift, Gradle, CocoaPods, Flutter and more), each with its project and when it was last built. Only one beside what makes it (a target beside its Cargo.toml) and in a git repository, so not an app's, an editor plugin's or a tool's own.
- **Clean Up**: Large Files (100 MB and up) and Duplicates (byte-for-byte identical files, keeping the oldest). Shows each file's last-opened date, and Put Back works.
- **System Data**, taken apart (what macOS's Storage settings lump together):
  - To review and move to the Trash: Xcode's build data (DerivedData, device support, simulator caches), developer caches (npm, pnpm, Bun, Cargo, Gradle, Maven, Go, ~/.cache), apps' caches, logs, and iPhone updates or macOS installers kept.
  - Simulator runtimes, each with when it was last used, to delete (two clicks); simulators whose runtime is gone.
  - Said how they go, with what they take: Time Machine's local snapshots, the sleep image and swap, temporary files, the simulator runtimes' caches, Docker's disk.
- **Uninstall App**: an app plus what it leaves in your Library (support files, caches, preferences, containers, logs, login agents). Each item shows its size, and it all goes to the Trash at once. An app an administrator installed goes too: Finder asks for the password or Touch ID first.
- **Tidy Up**: sorts a folder's loose files into Screenshots, Images, Videos, Audio, Documents, Installers and Archives. ⌘Z puts it all back.
- **Folder Rules** (Settings › Folder Rules; "Folder Rules…" in a folder's right-click menu; Quick Open), like Hazel: a folder kept in order on its own.
  - A rule is a folder, what in it it's for (kinds, a word in the name, larger than so many MB, added more than so many days ago, not opened in so many days), and what's done with it: sorted into folders by kind beside it, moved to a folder (made if it isn't there), or put in the Trash.
  - Starters: Downloads sorted after a day, installers to the Trash after a week, invoices into Documents, old screenshots off the Desktop.
  - Its form says what it would do right now ("Right now it would move “Zoom.dmg”, “Slack.dmg” and 2 more to the Trash").
  - Safe: only files loose in the folder, once they've finished arriving (not downloads still coming in); nothing replaced (a name taken is numbered); nothing deleted, and the Trash only after some days. Off rules do nothing.
  - Lately lists what the rules did, each with Undo (from the Trash too); what's put back is left alone after.
  - A folder is looked at when it changes and once a minute, within about 5 s of something arriving.

## The app itself
- Runs on any Mac with macOS 13 Ventura or later, Apple silicon or Intel: one app for both.
- **Menu bar item** (an F2 tile):
  - Opens a window, a favorite folder, the search or Disk Usage.
  - Shows timers, alarms and keep-awake, and cancels them.
  - Turns on Open at Login, and quits F2.
- **Welcome**, in the website's look (a painting per step, set in Geist). It explains what F2 does and asks for each permission:
  - Full Disk Access, Accessibility, Screen Recording, the clipboard.
  - Each with a reason and a button.
  - Optional: Calendar, Reminders and the Do Not Disturb shortcut, for Quick Open.
  - It also lets you pick Quick Open's shortcut, points out shortcuts that clash, offers window shortcuts, and suggests things to try.
- **Settings › Permissions** lists all of them, plus Calendar, Reminders, Contacts and the microphone. macOS also asks, the first time each is used, to let F2 control Notes, use Bluetooth, and (only to tell which Wi-Fi network you're on) know your location.
- **Settings**: General, Quick Open, Quicklinks, Snippets, Clipboard, Dictation, Windows, Screenshots, Sidebar, Advanced, Permissions. ⌘, only ever opens one Settings window.
- **Updates**: Sparkle, from download.getf2.com, with a notarized DMG laid out for dragging F2 into Applications.
- **Other**:
  - About and Acknowledgements (every dependency's license).
  - A log file (Help › Show Log File).
  - **Send Feedback** (Help menu, or Quick Open): straight to F2's maker, with your email if you'd like an answer.
  - **Crash reports**: if F2 quits unexpectedly, it asks at the next start before sending a report (or always sends, or never: Settings › Advanced). A report says where in F2 it stopped, F2's and macOS's versions and the Mac's model; the Mac's ids and your name in paths are taken out, and Show Report shows exactly what goes. They go to getf2.com only.
  - Runs on macOS 13 and later.
  - About half the memory of early builds.

## New in 0.1.3
- Archives as folders. Dual pane tabs, Swap Panes, Open in Other Pane.
- Screenshot History, Gallery view, a toolbar that fits narrow windows.
- Quick Open:
  - Commands show their shortcuts, and a back arrow appears in Quick Open's own lists.
- Disk space:
  - Disk Usage counts big files by what they take on disk.
  - Clean Up (Large Files, Duplicates), Free Up Space, Old Downloads, Tick All, last-opened dates.
- Quick Actions conversions, Tidy Up, Uninstall App, plain-words search.
- Quick Open answers:
  - World time, dictionary, translate, Wi-Fi, Bluetooth and Do Not Disturb, reminders.
  - Timers, keep awake, volume, music and brightness, calendar, weather, QR codes, dates.
- Ghost-text hints. Korean, Japanese and Chinese input, and finding apps by their names in those languages.
- Fixes: one Settings window; crash fixes (keyboard languages, Open in Other Pane).
- New-window folder, a recorded Quick Open shortcut, and which screen Quick Open opens on.
- Fixes for which screen opens on a second monitor, also for the screenshot thumbnail in the corner, pins, the editor and Capture Screen.

## New in 0.1.4
- **Welcome**: redesigned in the website's look, with a step for Calendar, Reminders and Do Not Disturb, and Quick Open's shortcut picked there.
- **Quick Actions**: Remove Background and Copy Text.
- **Quick Open**:
  - People (call, message, FaceTime, email), notes, alarms.
  - Battery, sound output, Bluetooth connect, speed test, Wi-Fi password and join code.
  - Spelling, synonyms, word count, package tracking.
  - Generate Password, Paste as Plain Text, latest download, your Shortcuts by name.
- **Settings › Permissions**: Calendar, Reminders and Contacts.
- Geist bundled, so Quick Open can always offer it as a font.
- Sunrise and sunset, Night Shift, desktop icons, Show Desktop, whole-phrase commands ("turn wifi off").
- Selected text, anywhere: a shortcut, or Use Selected Text.
- Import from iPhone or iPad in the right-click menus.
- Notes redesigned as macOS's notifications look.

## New in 0.1.10
- Folder Rules (Settings › Folder Rules; "Folder Rules…" in a folder's right-click menu; Quick Open), like Hazel: a folder kept in order on its own, each run listed with Undo.
- Dragging out to other apps as from Finder: onto a window in front of F2's, from a window in the background (which stays there), with Finder's choices (move, copy, link, the Dock's Trash) and the items' own icons.
- Free Up Space's Build Folders: your projects' builds and packages, each with its project and when it was last built.
- A file with more than one name (hard links) counts once in Disk Usage, Get Info and search's sizes.
- Quick Open comes up whole, at once: its list and icons in place in the first frame shown.
- New windows open all on the screen: the last frame saved, moved back in (and made smaller if it must be), on the main screen if its own is gone.

## New in 0.1.9
- Drag a window from anywhere in it with ⌃⌘ held, or size it from near an edge or a corner (Settings › Windows; "Window Settings" in Quick Open).
- Settings › Search: how search's index is doing, what it leaves out and why (folders macOS guards without Full Disk Access, others it refused, other disks), what text search covers, and Rebuild Index, which lists the whole disk again while searches keep working ("Search Index" in Quick Open).
- Two panes: ⌃⌘C copies to the other pane and ⌃⌘M moves there (F5 and F6 need fn on a Mac), and Open in Other Pane on a sidebar folder or server.
- Servers over SSH: your ~/.ssh/config hosts in the sidebar and Quick Open, browsed in F2's windows over SFTP with the Mac's own ssh. Open a file and its saves go back; copy both ways, rename, delete, new folders; Open in Terminal logs in there.
- Search in a USB drive, a disk image or a network share, which found nothing before (and in Desktop, Documents or Downloads without Full Disk Access).
- Drag and drop you can see: the folder you drop on pulses, what moves out fades away and what arrives fades in, a folder blinks before it springs open, and a drop in a folder that isn't shown says where it went. The drag shows how many items (and + for a copy). A soft pop on a drop (Settings › Advanced turns it off). Sheets ease in. The Trash makes Finder's sounds.
- Showing Finder's windows in F2 (Settings › General, experimental) is quicker: F2 notices Finder's window as it opens, asks Finder itself rather than through a script process, and puts its window up over Finder's before closing it, so there's no gap with neither.
- Send Feedback (Help menu, Quick Open), and crash reports, asked about before they're sent, with nothing in them that says whose Mac it was. Both go to getf2.com only.
- Safer copying, tested the awkward ways (another disk, a full one, one stopped part-way, Replace, Merge, Undo, a USB stick's FAT32, names in every script): copying a folder that has both "README" and "readme" onto a disk that doesn't tell them apart keeps the one copied (it was removed) and says why the other couldn't be; a locked file that can't be moved says it's locked.
- Lighter in the background: a folder of git repos reads only the repos on screen (more as you scroll), not all of them at every start; and search's index is rewritten at most every 20 minutes on a busy disk (it was every few minutes on a developer's Mac, 2 s and ~280 MB each time), its upkeep at a lower-energy priority.
- The list stays where it is when a folder in it opens (it scrolled to the selection, which could be far below) and when something in the folder changes; it scrolls only to show what you just made, pasted or renamed.

## New in 0.1.8
- Switch Window shows a picture of the window picked, beside the list, and the windows on your other desktops too (picked, it goes there).
- Scrolling Capture: an area scrolled and taken all the way down, in one tall screenshot.
- F2's own preview on Space, at once: images, text and code, PDFs, folders (Quick Look for video, audio and documents, and made at launch so it's quicker the first time).
- ⌘W closes Quick Look first (as in Finder), not the window; the sidebar says nothing about the search index unless it's getting ready; the list's stripes don't show through the path bar.
- Copies you can trust: speed and time left; what can't be copied left out and listed, the rest copied on (where a whole folder used to fail and be thrown away); stopped, what's copied stays, and Continue picks up from there.
- A screen recording saves as a GIF too, from its corner preview: written a frame at a time, so a long one doesn't take gigabytes.
- Clipboard History finds images by the words in them (read on this Mac, as each is copied), and lists each by its first line.
- What isn't yours to move (an app an administrator installed, say) goes to the Trash through Finder, which asks for an administrator's password or Touch ID once, as it does itself: Uninstall App, Move to Trash, Clean Up's lists. F2 waits for it without stopping.

## New in 0.1.7
- Keyboard shortcuts, all in one place (Settings › Keyboard, or "keyboard shortcuts" in Quick Open): give any command, app, folder, quicklink or snippet a shortcut that works in any app, or change F2's own keys in its windows (Return to open, say); ⌘K › Set Shortcut… on a result in Quick Open.
- The welcome says what F2 is for in everyday words, as the website does.
- Runs on Intel Macs too: releases are one app for Apple silicon and Intel.
- Icons placed freely in icon view (Sort By: None): Finder's arrangement read from its .DS_Store, F2's moves kept in its own file; Clean Up and Clean Up By.
- A Where filter in the window's search bar (Downloads, Desktop, iCloud Drive, Applications…), and Clear for all the filters.
- Customize Folder: a folder's color and a symbol or an emoji on it, as Finder 26 does; tagged folders take their tag's color.
- Fix Spelling: selected text in any app, its spelling fixed in place (macOS's spell checker, in the text's language); give it a shortcut in Settings › Keyboard.
- The window looks more like Finder's: the files meet the sidebar, the stripes go on to the bottom, and dark mode has Finder's colors.
- What Finder never had: cut and paste to move files (⌘X, ⌘V), New File (⌥⌘N), and an Up button in the toolbar.
- Switch Window: every window open, to find by name or switch to with a shortcut held, as ⌘Tab does for apps.
- System Data taken apart in Disk Usage: what each part is, what it takes, and how it's let go of (Xcode's builds, developer caches, simulator runtimes, snapshots, swap, temporary files). Move to Trash in a Clean Up list now moves only what's ticked in that list.

## New in 0.1.6
- About this Mac, disk space, my IP, holidays, calling a number, restarting an app, Set Desktop Picture.
- Stock and coin prices.
- Air quality and UV, tips and splitting the bill, kitchen conversions, shoe and clothing sizes, coin flips and dice, maps and directions, flight status.
- To-do (check off reminders), a stopwatch, and break reminders.
- What's slowing my Mac, is the internet down, clean keyboard, now playing, and play music.
- Search filters to pick in the window's search bar (Kind, Date, Size), no query language needed.
- Half a question ("18% of", "tip", "price of") shows what it's going to be; answers from the internet show a loading row, and a light along the field, until they're here.

## New in 0.1.5
- Screen recording: the screen or an area, saved as MP4.
- Sleep timers, AirDrop and email from ⌘K, pinned clips, text tools.

## Not yet
- Finder Sync extension badges (Dropbox's). These only run in Finder.
- A full VoiceOver pass.

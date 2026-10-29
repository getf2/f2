# Where things are

F2 is a [GPUI](https://gpui.rs) app in Rust, with what macOS gives only
to Objective-C or Swift in `src/macos/` (`.m` files, and
`translate.swift` and `speech.swift`), called through `src/macos.rs`.
Search is [fsearch](https://github.com/noahdunnagan/fsearch), linked in
(vendored in `vendor/fsearch`).

## The window, as Finder's

- `src/main.rs` start-up and files sent to the app; `src/menus.rs` the menu
  bar; `src/browser.rs` windows, their tabs and two panes.
- `src/finder.rs` a tab's state and moves, and in `src/finder/`:
  - `actions` and `keys` (keys), `chrome` (the sidebar, toolbar and bars),
    `items` (list and icons), `columns`, `gallery`, `preview`, `peek`;
  - `sheets` (Go to Folder, Connect to Server, View Options, questions),
    `menu` (right-click menus and their commands), `rename_items`
    (renaming several), `customize` (Customize Folder), `arrange` (icons
    placed freely; `src/ds_store.rs` reads Finder's .DS_Store);
  - `places` (the sidebar, Recents, tags, Smart Folders, the network, the
    Trash), `filters` and `search_tips` (the search's filters), `servers`
    (servers over SSH), `airdrop`;
  - `transfer` (copy and move), `selection`, `undo`, `dnd` and `motion`
    (drag and drop).
- `src/listing.rs` the item model; `src/fileops.rs` file operations;
  `src/icons.rs` icons and thumbnails; `src/info.rs` Get Info;
  `src/style.rs` Finder's look; `src/archives.rs` archives as folders;
  `src/tools.rs` the terminal and editor.
- `src/search.rs` fsearch (and folder sizes from its index);
  `src/natural.rs` searching in plain words.
- `src/sftp.rs` and `src/remote.rs` servers over SSH, through the Mac's own
  `ssh` (`src/macos/askpass.m` asks for passwords).
- `src/quick_actions.rs` Quick Actions; `src/resize.rs` Resize Images;
  `src/rules.rs` Folder Rules; `src/tidy.rs` Tidy Up; `src/takeover.rs`
  standing in for Finder.

## Quick Open

- `src/quick_panel.rs` the panel, its results and their actions;
  `src/quick_panel/pages.rs` pages in it (extensions', and F2's own:
  Colors, Resize Images, the Extension Store).
- `src/commands.rs` its commands, `src/apps.rs` the apps it opens,
  `src/unfinished.rs` half a question, `src/recent.rs` what was chosen.
- Answers as you type: `src/calc.rs`, `src/currency.rs`, `src/prices.rs`
  (coins and stocks), `src/world_time.rs` (`assets/cities.tsv`, made by
  `scripts/cities.py`), `src/dates.rs`, `src/sun.rs`, `src/dictionary.rs`,
  `src/words.rs` (spelling, synonyms, word counts), `src/translate.rs`
  (`src/macos/translate.swift`), `src/weather.rs`, `src/air.rs`,
  `src/holidays.rs`, `src/flights.rs`, `src/maps.rs`, `src/kitchen.rs`,
  `src/sizes.rs`, `src/bill.rs`, `src/chance.rs`, `src/colors.rs`.
- Your day and your Mac: `src/reminders.rs`, `src/calendar.rs`,
  `src/timers.rs`, `src/awake.rs`, `src/media.rs` (sound and brightness),
  `src/music.rs`, `src/toggles.rs` (Wi-Fi, Bluetooth, Do Not Disturb),
  `src/devices.rs` (battery, sound outputs, Bluetooth devices),
  `src/people.rs`, `src/notes.rs`, `src/passwords.rs`, `src/net.rs` (speed
  test, Wi-Fi passwords), `src/busy.rs` (what's slowing the Mac),
  `src/online.rs`, `src/this_mac.rs`, `src/downloads.rs`,
  `src/tracking.rs`, `src/qr.rs`, `src/dev.rs` (for developers),
  `src/continuity.rs` (importing from an iPhone).
- `src/clips.rs` the clipboard's history; `src/emoji.rs` emoji and symbols
  (`assets/emoji.tsv`, from Unicode's emoji-test.txt and CLDR's
  annotations: © Unicode, Inc., Unicode License v3); `src/snippets.rs`.
- `src/hotkeys.rs` shortcuts that work in any app; `src/keyboard.rs` those
  you give commands, apps and snippets; `src/keymap.rs` F2's own keys.

## Extensions

- `src/extensions.rs` manifests, settings and passwords, and running
  them: a program, or a script in F2's JavaScript host (`src/js_host.js`,
  the API scripts get; `src/macos/jshost.m`, the sandboxed process;
  `src/macos/http.m`, their `fetch`). `EXTENSIONS.md` says all of it.
- `src/store.rs` the Extension Store (its signed list, installing and
  updating). The store's extensions are in
  [getf2/extensions](https://github.com/getf2/extensions);
  `scripts/sync-store.sh` copies what it shares with F2 (the types, the
  schema, the docs, the host) there.
- `extensions/`: the template New Extension makes, F2's API's types and the
  manifest's schema (`extensions/js/`), `run.js` (an extension run without
  F2, as F2 runs it), and two programs (`github`, `cloudflare`).

## Disk space, screenshots, windows

- `src/disk_usage.rs` and `src/usage.rs` Disk Usage; `src/cleanup.rs`,
  `src/disk_usage/clean.rs` and `src/disk_usage/safety.rs` Clean Up and
  Free Up Space; `src/uninstall.rs` Uninstall App.
- `src/shots/` screenshots: `mod` (taking them), `marks` (what the editor
  draws), `editor`, `preview` (the corner), `pin`, `record` (screen
  recording), `scrolling` (Scrolling Capture).
- `src/windows.rs` window management; `src/screens.rs` which screen.

## Dictation

- `src/dictation.rs` its shortcut (one key or two), hold to talk (press
  twice for hands-free) or press to start and stop, and the `Engine` that
  turns speech into text (Apple's now; another, as Hex's Parakeet, would
  be one more);
  `src/settings_window/dictation.rs` its settings.
- `src/macos/dictation.m` the keys (an event tap), the microphone (16 kHz
  mono) and the pill; `src/macos/speech.swift` Apple's SpeechAnalyzer
  (adapted from [Hex](https://github.com/anomalyco/hex)'s, MIT).

## The app

- `src/settings.rs` settings, `src/settings_window.rs` (and
  `src/settings_window/`: Extensions, Folder Rules) their window;
  `src/onboarding.rs` the welcome; `src/permissions.rs` what F2 asks macOS
  for; `src/status_item.rs` the menu bar item.
- `src/about.rs` About, Acknowledgements and updates (Sparkle, loaded by
  `src/macos/updates.m`); `src/reports.rs` Send Feedback and crash reports;
  `src/log.rs` the log.

## Around it

- `scripts/bundle-macos.sh` builds `F2.app`; `scripts/release-macos.sh` a
  notarized DMG and Sparkle's appcast (`scripts/dmg-settings.py`,
  `scripts/render-dmg-background.swift`); `scripts/licenses.py` the
  acknowledgements; `scripts/render-icon.swift` the icon.
- `assets/`: the icon, fonts, the welcome's pictures, cities and emoji.
- `site/`: [getf2.com](https://getf2.com) (its own README).
- `video/`: the feature video (Remotion).

## Memory

As fsearch's daemon does, F2 uses its allocator (blocks of a megabyte and
up go straight back to the system), and starts in malloc's space-efficient
mode (Info.plist's LSEnvironment), which hands freed small blocks back.
Without it, what the engine frees after loading its index and after each
search stayed F2's: about twice the heap.

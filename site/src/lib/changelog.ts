import type { Art } from '#/components/Plate'

// Every release, newest first: the changelog page, and the site's VERSION
// (the newest one with a date). When releasing, give the top entry its date
// (or add one), and features.ts's `since` badges follow.
//
// An entry without a date is built but not out yet; the page shows it as
// coming in the next update.
export type Release = {
  version: string
  date?: string // YYYY-MM-DD
  title: string
  summary: string
  art: Art
  added?: string[]
  improved?: string[]
  fixed?: string[]
}

export const RELEASES: Release[] = [
  {
    version: '0.2.3',
    date: '2026-10-31',
    title: 'A clearer welcome',
    summary: 'The welcome says why F2 asks for each permission, and comes back to where you were when F2 has to reopen for one.',
    art: { paint: 'meadow', y: '45%' },
    added: [
      'The welcome offers the notch, with a note to show it working.',
      'Reopen F2, right in the welcome, once Full Disk Access or Screen Recording is turned on: macOS lets both work only after F2 reopens.',
    ],
    improved: ['The welcome says plainly what each permission is for, and which ones need F2 to reopen.'],
    fixed: ['The welcome no longer disappears when macOS reopens F2 for a permission: it comes back to the step you were on.'],
  },
  {
    version: '0.2.2',
    date: '2026-10-30',
    title: 'The notch, put to work',
    summary: 'The notch shows what’s going on, and F2’s notes come down from it. Disk images open without Finder, and Quick Open goes where you put it.',
    art: { paint: 'night-aurora', y: '50%', glass: 'notch' },
    added: [
      'The notch, as Alcove has it: a timer counting down, the stopwatch, a screen recording, dictation listening or the song playing show beside it. Point at it to see all of that, stop it or skip, and your last notes. Turn it on in Settings › Notch.',
      'F2’s notes come down from the notch instead of the corner: a timer done, text copied. Screens without a notch get one while there’s something to show.',
      'Disk images open in F2 as their makers laid them out, their picture behind the app and the arrow, even with Finder quit. Install puts the app in Applications, then Eject tidies up.',
      'Drag Quick Open anywhere on the screen: it snaps to the middle, and opens there from then on.',
      'Simple mode, for just the search field until you type. And files can wait till you ask for them, with f and a space.',
    ],
    fixed: [
      '“now playing” works with Spotify again, with its artwork.',
      'Quick Open finds Safari, which newer macOS keeps outside Applications.',
    ],
  },
  {
    version: '0.2.1',
    date: '2026-10-29',
    title: 'Say it, and it’s typed',
    summary: 'Hold a key in any app, say what you want to write, and let go: F2 types it for you. It’s all worked out on your Mac, and your AirPods keep playing music as they should.',
    art: { paint: 'study-window', y: '50%', glass: 'dictation' },
    added: [
      'Dictation, in any app: hold Right ⌥ (or a key of your own, or two together), speak, and let go. What you said is typed where you were, a moment later. Turn it on in Settings › Dictation, on Macs with macOS 26 or later.',
      'Press the key twice to keep talking hands-free, then once more when you’re done. Or set it to start with one press and stop with the next. Esc stops without typing anything.',
      'It listens in 30 languages, with Apple’s own speech recognition, on your Mac: nothing you say is sent anywhere, and no recording is kept. A small pill at the bottom of the screen moves with your voice while it listens.',
      'With AirPods in, F2 listens through your Mac’s own microphone, so your music doesn’t drop to call quality. Or pick a microphone yourself.',
      'Paste Last Dictation, in Quick Open, pastes the last thing you said again.',
    ],
  },
  {
    version: '0.2.0',
    date: '2026-10-27',
    title: 'Extensions, and a store for them',
    summary: 'Quick Open gets extensions: Wikipedia, Hacker News, GitHub, Linear, Notion and more, from F2’s own store, free. Plus a color picker that remembers, and images resized to any size.',
    art: { paint: 'golden-hour', y: '45%', queries: ['wiki alan turing', 'hn', 'brew ffmpeg', 'linear'] },
    added: [
      'The Extension Store, in Quick Open: 18 extensions to start, each installed with Return. Read Wikipedia articles and Hacker News threads right in Quick Open, look things up on MDN or Can I Use, search Homebrew, npm, crates.io and PyPI, and see your GitHub pull requests, Linear and Jira issues, Notion pages, Todoist tasks, and Vercel and Cloudflare deployments.',
      'Extensions are shut in: one can’t open your files or run anything on your Mac, and it reaches only the websites it names, which the store shows you before you install it. Tokens are kept in your Mac’s keychain, and every extension is read by a person before it’s in the store.',
      'Make your own: New Extension, in Quick Open or Settings › Extensions, makes one ready to run, in plain JavaScript. Save a change and it runs again. Then share it in the store, at github.com/getf2/extensions.',
      'Colors: Pick Color keeps every color it picks, and copies it as you like it: HEX, RGB, HSL, OKLCH, or as SwiftUI or NSColor code. Colors in Quick Open shows them all, each with how well it reads on white and on black, and a color you type there shows every way it’s written.',
      'Resize images to the size you want, from Quick Actions or Quick Open: 1920 pixels wide, half the size, or to fit a box. The copies go beside the originals, named by their new size, and phone photos come out the right way up.',
    ],
  },
  {
    version: '0.1.10',
    date: '2026-10-26',
    title: 'Folders that tidy themselves',
    summary: 'Give a folder a few rules and F2 keeps it in order for you. Plus files you can drag out to any app, as from Finder, and space to win back from your coding projects.',
    art: {
      paint: 'meadow',
      y: '40%',
      shots: [
        {
          src: 'tidy-after',
          w: 2800,
          h: 594,
          alt: 'Downloads sorted into a folder for each kind of file, as a rule leaves it',
          at: { width: '76%', left: '12%', top: '31%' },
        },
      ],
    },
    added: [
      'Folder Rules: tell a folder what to do with what lands in it, and it stays tidy on its own. Sort Downloads after a day, send old installers to the Trash after a week, file invoices in Documents. Each rule shows what it would do before you turn it on, and everything it does can be undone. In Settings › Folder Rules, or right-click a folder.',
      'Drag files out of F2 into any app, as from Finder: into Mail to attach them, onto a Finder window to move or copy them, or onto the Trash in the Dock. Even from an F2 window in the background, which stays where it is.',
      'Free Up Space finds what your coding projects leave behind: their builds and downloaded packages, each with its project and when it was last built. Only in your own projects, never an app’s.',
    ],
    improved: [
      'Quick Open comes up all at once, its list and icons in place, rather than in steps.',
      'Disk Usage no longer counts some files twice, which could make folders of coding projects look twice their real size.',
    ],
    fixed: ['New windows always open fully on the screen, even when the last one was left partly off it or on a display that’s since been unplugged.'],
  },
  {
    version: '0.1.9',
    date: '2026-10-24',
    title: 'Every drop lands',
    summary: 'See and hear your files arrive where you drop them, open your servers like any folder, and search your USB sticks too. Plus windows you can drag from anywhere, and a way to tell us what’s wrong.',
    art: { paint: 'dusk-sea', y: '50%', glass: 'snap' },
    added: [
      'Your servers, as folders: if you use SSH, they’re in the sidebar to browse like anything else. Open a file and it saves back, copy both ways, and open a terminal right there.',
      'Search finds things on USB sticks, disk images and network shares too, where it found nothing before.',
      'Drag a window from anywhere in it with ⌃⌘ held, or size it from near an edge or a corner (Settings › Windows).',
      'Send Feedback, in the Help menu: it goes straight to F2’s maker. And if F2 ever quits unexpectedly, it asks before sending a report, which holds nothing of your files.',
      'Settings › Search shows how search is doing and what it leaves out, with a way to rebuild it if something seems missing.',
    ],
    improved: [
      'Every drop shows: the folder you drop on pulses, what moves out fades away and what arrives fades in, with a soft pop (Settings › Advanced turns it off). Folders blink before they spring open, and the Trash makes Finder’s sounds.',
      'Two panes without the fn key: ⌃⌘C copies to the other pane and ⌃⌘M moves there. Right-click a folder in the sidebar to open it in the other pane.',
      'Lighter on your battery: F2 does much less in the background, especially in a folder full of projects.',
      'Showing Finder’s windows in F2 (Settings › General) is quicker, with no blank moment in between.',
    ],
    fixed: [
      'Opening a folder in the list no longer jumps you to the bottom of it.',
      'Copying a folder that has both “README” and “readme” onto your Mac keeps the one copied, and says why the other couldn’t be. A locked file that can’t be moved now says it’s locked.',
    ],
  },
  {
    version: '0.1.8',
    date: '2026-10-23',
    title: 'A look before you open',
    summary: 'Press Space for a preview that opens at once, find every window on every desktop, and copy without one stubborn file stopping the rest. Plus long screenshots, GIFs, and images you can find by the words in them.',
    art: { paint: 'mist-forest', y: '50%', glass: 'menu' },
    added: [
      'A preview of its own on Space that opens at once: photos (HEIC and raw too), text and code, PDFs and folders, following along as you move through your files. Quick Look still plays your videos and shows documents.',
      'Scrolling Capture: drag over a long page or list, and F2 scrolls it for you and takes all of it in one tall screenshot.',
      'Clipboard History finds a copied image by the words in it: a screenshot of an invoice by its number, say. They’re read on your Mac.',
      'Save a screen recording as a GIF, from its preview in the corner, for the places a video won’t play.',
    ],
    improved: [
      'Apps an administrator installed can be uninstalled, and files like them moved to the Trash: F2 asks for your password or Touch ID, as Finder does, instead of saying it can’t.',
      'Switch Window shows what each window looks like as you go through them, and the windows on your other desktops too: pick one and you’re there.',
      'Copies you can trust: how fast and how long, and a file that can’t be copied no longer stops the rest. You get a list of what was left out, and a stopped copy picks up where it left off.',
    ],
    fixed: [
      '⌘W closes Quick Look first, as in Finder, instead of the window behind it.',
      'The stripes in a short list no longer show through the path bar, and the sidebar no longer counts the files search knows about.',
    ],
  },
  {
    version: '0.1.7',
    date: '2026-10-23',
    title: 'What Finder never had',
    summary: 'Cut and paste for files, a switcher for every window, a shortcut for anything in any app, and a look inside the mysterious “System Data”. Folders get colors and emoji, and F2 now runs on Intel Macs too.',
    art: { paint: 'still-lake', y: '50%', glass: 'menu' },
    added: [
      'Cut and paste to move files, New File for an empty text file, and an Up button that goes to the enclosing folder.',
      'Keyboard shortcuts in one place (Settings › Keyboard): give anything Quick Open does a shortcut that works in any app, from Lock Screen to an app, a folder or a snippet, and change F2’s own keys, like Return to open.',
      'Switch Window: every window you have open, not just every app, to find by its name or flip through as you do with apps.',
      'System Data, taken apart: Disk Usage shows what the mysterious “System Data” in your Mac’s settings is made of, and how to free each part.',
      'Customize Folder, in the right-click menu: a color, and a symbol or an emoji on the folder, shown as it’ll look while you pick.',
      'Put icons wherever you like in icon view (Sort By: None). Folders you arranged in Finder open just as you left them, and Clean Up tidies them into the grid.',
      'Fix Spelling: select text in any app and fix its typos right where they are, with your Mac’s own spell checker, in any language it knows. Names are left alone, and nothing leaves your Mac.',
    ],
    improved: [
      'Tagged folders take their tag’s color, as Finder shows them on macOS Tahoe.',
      'Search filters can pick where to look too (Downloads, Desktop, iCloud Drive, Applications…), and Clear takes them all off.',
      'The welcome says what F2 is for in everyday words.',
      'F2 runs on Intel Macs too, from the same download.',
      'The window looks even more like Finder’s: the files reach the sidebar, the stripes run to the bottom, and dark mode has Finder’s colors.',
    ],
  },
  {
    version: '0.1.6',
    date: '2026-10-21',
    title: 'Prices, the bill, and the rest of your day',
    summary: 'Quick Open answers more of the everyday questions, from prices and the bill to your music and what’s slowing your Mac. And searches you can narrow down without typing a thing.',
    art: { paint: 'golden-hour', y: '45%', queries: ['btc price', 'tip 18% on 84', '1 cup flour in grams', 'flip a coin'] },
    added: [
      'Stock and coin prices, with the day’s change: “btc price”, “aapl stock”, “btc in krw”.',
      'Air quality and the UV index: “air quality”, “uv”.',
      'Tips and splitting the bill: “tip 18% on 84”, “split 120 four ways”.',
      'Kitchen conversions that know what each ingredient weighs: “1 cup flour in grams”.',
      'Shoe and clothing sizes from one country to another: “us 10 shoe in eu”.',
      'Coin flips, dice, and picking for you: “flip a coin”, “pick sushi, pizza or tacos”.',
      'Maps and directions: “coffee near me”, “walk to the park”.',
      'Where a flight is, from its number: “UA837”.',
      'A to-do list of what’s due today, checked off with Return. A stopwatch, and reminders to take a break.',
      'About this Mac, free space, your IP address, public holidays, calling a number, and restarting an app.',
      'Set Desktop Picture in the right-click menu.',
      'What’s slowing your Mac: how busy it is, and the apps using the most, to quit: “what’s slowing my mac”.',
      'Is the internet down? Wi-Fi, your router and the internet checked one at a time, and what’s wrong said plainly.',
      'Now playing in Music or Spotify, with its artwork, to pause, skip or love. “play taylor swift” plays it from your library.',
      'Cleaning mode: the keys and clicks off for 30 seconds while you wipe the keyboard: “clean keyboard”.',
      'Search filters to pick under the search field: Kind, Date and Size. No need to type them.',
    ],
    improved: [
      'Half a question (“18% of”, “tip”) already shows what the answer will be, and answers from the internet show they’re on their way.',
      '“12*7=” works out, as you’d type it on a calculator.',
    ],
    fixed: ['“time in” and “weather in” no longer show India’s while you’re still typing the place.'],
  },
  {
    version: '0.1.5',
    date: '2026-10-21',
    title: 'Screen recording',
    summary: 'Record your screen, put your Mac to sleep on a timer, and send files straight from Quick Open.',
    art: { paint: 'meadow', y: '40%', glass: 'recording' },
    added: [
      'Screen recording: the whole screen or an area you drag out, with your microphone and clicks if you like, saved as MP4.',
      'Sleep timers: “sleep in 30 min” puts the Mac to sleep, and “stop music in 45 minutes” pauses Music or Spotify.',
      'AirDrop or email a file from Quick Open.',
      'Pinned clips: an address or a bank number stays at the top of Clipboard History.',
      'Text tools: make what you copied uppercase, lowercase or title case, clean up its spaces, or take out its line breaks.',
    ],
  },
  {
    version: '0.1.4',
    date: '2026-10-21',
    title: 'A new welcome, and your Mac at your fingertips',
    summary: 'A friendlier first start, and Quick Open learns your contacts, your battery, your Wi-Fi and more.',
    art: {
      paint: 'dusk-sea',
      y: '35%',
      shots: [
        {
          src: 'start-welcome',
          w: 920,
          h: 640,
          alt: "F2's welcome: a few permissions, each with what it's for and a button to allow it",
          at: { width: '44%', left: '28%', top: '15%' },
        },
      ],
    },
    added: [
      'A new welcome, in this site’s look. It explains each permission F2 asks for, and helps you pick Quick Open’s shortcut.',
      'Remove Background for photos, and Copy Text from photos, screenshots and PDFs.',
      'Call, message, FaceTime or email anyone in Contacts (“call mom”), add notes (“note buy milk”) and set alarms (“alarm 7am”).',
      'Battery for your Mac, AirPods, mouse and keyboard; where the sound plays; connecting Bluetooth headphones; a speed test; and Wi-Fi passwords, with a QR code guests can scan to join.',
      'Spelling, synonyms, word counts and package tracking.',
      'Generate a password, paste as plain text, find your latest download, and run your Shortcuts by name.',
      'Sunrise and sunset, Night Shift, hiding the desktop icons, and whole phrases like “turn wifi off”.',
      'Select text in any app, then press a shortcut to translate it, define it, look it up and more.',
      'Import from iPhone or iPad: take a photo or scan a document straight into a folder.',
    ],
    improved: [
      'F2’s notices look like macOS’s own notifications.',
      'Settings › Permissions covers Calendar, Reminders and Contacts too.',
    ],
  },
  {
    version: '0.1.3',
    date: '2026-10-21',
    title: 'Free Up Space, and answers as you type',
    summary:
      'The biggest update yet: help with a full disk, a tidier Downloads, search in plain words, and a Quick Open that answers questions.',
    art: {
      paint: 'terraces',
      y: '60%',
      shots: [
        {
          src: 'freeupspace',
          w: 1080,
          h: 752,
          alt: "Free Up Space: cards for old installers, old Downloads, backups, caches and the Trash, each with what it'd free",
          at: { width: '46%', left: '27%', top: '13%' },
        },
      ],
    },
    added: [
      'Free Up Space: a card for each kind of thing that’s safe to let go of, including Downloads you haven’t opened in six months.',
      'Large Files and Duplicates, Uninstall App (with everything an app leaves behind), and Tidy Up for messy folders.',
      'Search in plain words: “photos from last week”, “big videos”.',
      'Quick Open tells the time anywhere, what words mean, translations, the weather and dates, and makes QR codes.',
      'Reminders, calendar events and timers, typed the way you’d say them.',
      'Wi-Fi, Bluetooth, Do Not Disturb, keep awake, volume, brightness and music, from Quick Open.',
      'Hints that finish what you’re typing. Korean, Japanese and Chinese typing, and apps found by their names in those languages.',
      'Zip files and other archives open like folders.',
      'Gallery view and Screenshot History. Two folders side by side get a tab bar each, and swap places with ⌥⌘U.',
      'Quick Actions: Make Smaller, Create PDF, Combine into PDF, Compress PDF, and images to JPEG, PNG or HEIC.',
      'New windows open in the folder you choose, and Quick Open on the screen you choose, with any shortcut you like.',
    ],
    improved: [
      'Disk Usage counts big files by the space they really take.',
      'The toolbar fits narrow windows, and Quick Open shows each command’s shortcut.',
    ],
    fixed: [
      'Only one Settings window opens at a time.',
      'Two crashes: one when switching keyboard languages, one with Open in Other Pane.',
      'With a second display, windows, screenshots and pins appear on the right screen.',
    ],
  },
  {
    version: '0.1.2',
    date: '2026-10-20',
    title: 'Two folders side by side',
    summary: 'Show two folders at once, convert currencies, and use half the memory.',
    art: {
      paint: 'hero-valley',
      y: '30%',
      shots: [
        {
          src: 'hero-dual',
          w: 2800,
          h: 1680,
          alt: 'F2 with two folders side by side: Documents as a list, and a folder of photos as icons',
          at: { width: '58%', left: '12%', top: '15%' },
        },
        {
          src: 'quickopen-currency',
          w: 1360,
          h: 358,
          alt: "Quick Open converting 100 USD to 89.09 EUR at the ECB's rate",
          at: { width: '38%', right: '7%', top: '9%' },
        },
      ],
    },
    added: [
      'Two folders side by side (⌥⌘S): Tab moves between them, F5 copies across, F6 moves.',
      'Currency in Quick Open: “100 usd in eur”, or “$50” into your own, at the European Central Bank’s daily rates.',
    ],
    improved: ['F2 uses about half the memory it did.', 'The disk image’s window has room for everything in it.'],
    fixed: ['A crash when Quick Open was opened from a tab that was still updating.'],
  },
  {
    version: '0.1.1',
    date: '2026-10-20',
    title: 'Ventura, and a proper disk image',
    summary: 'F2 runs on more Macs, and installs the usual way.',
    art: {
      paint: 'still-lake',
      y: '45%',
      shots: [
        {
          src: 'start-dmg',
          w: 1280,
          h: 856,
          alt: "F2's disk image: F2 on the left, an arrow, and the Applications folder on the right",
          at: { width: '44%', left: '28%', top: '13%' },
        },
      ],
    },
    improved: [
      'Runs on macOS 13 Ventura and later.',
      'The disk image shows F2 and your Applications folder with an arrow between them: drag one onto the other.',
      'Folders stay on top only when sorting by name, as in Finder.',
    ],
  },
  {
    version: '0.1.0',
    date: '2026-10-20',
    title: 'The first release',
    summary: 'F2’s first public version.',
    art: {
      paint: 'mist-forest',
      y: '60%',
      shots: [
        {
          src: 'quickopen-empty',
          w: 1360,
          h: 1096,
          alt: 'Quick Open with nothing typed: favorite folders, then commands such as Lock Screen and Sleep',
          at: { width: '34%', left: '15%', top: '10%' },
        },
        {
          src: 'quickopen-calc',
          w: 1360,
          h: 496,
          alt: 'Quick Open working out 12*7 + 15% as 96.6',
          at: { width: '40%', right: '11%', bottom: '14%' },
        },
      ],
    },
    added: [
      'Finder’s windows, tabs, sidebar and views, with its icons, right-click menus, Get Info, Quick Look and Trash.',
      'Search across your whole Mac as you type.',
      'Quick Open on ⌘ Space: apps, files, a calculator, unit conversions, System Settings, and commands like Lock Screen.',
      'Clipboard History, Emoji & Symbols, snippets and quicklinks.',
      'Screenshots with an editor: arrows, text, blur, numbered steps and backgrounds.',
      'Windows snapped to halves, thirds and quarters, from a shortcut or Quick Open.',
      'Disk Usage: a ring chart of what fills your disk.',
      'For developers: Git’s status beside files, Open in Terminal and Open in Editor, and stopping what’s on a port.',
    ],
  },
]

// The newest release that's out: the first with a date.
export const LATEST = RELEASES.find((r) => r.date)!

// Compares "0.1.5"-style versions.
export function compareVersions(a: string, b: string) {
  const x = a.split('.').map(Number)
  const y = b.split('.').map(Number)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0)
    if (d) return d
  }
  return 0
}

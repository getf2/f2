import type { Art } from '#/components/Plate'

// Everything F2 does, for the features page: grouped by what you'd use it
// for, everyday things first, in plain words. FEATURES.md at the repo's root
// is the full list; when something's added there, add it here too.
//
// `examples` are what to type (shown as chips), `keys` a shortcut, and
// `since` the version it arrived in: the page marks the current version's as
// New, and ones not out yet (newer than the latest dated release in
// changelog.ts) as coming next. Older features don't need it.
export type Feature = {
  name: string
  text: string
  examples?: string[]
  keys?: string[]
  since?: string
}

export type Category = {
  id: string
  name: string
  lede: string
  art: Art
  features: Feature[]
}

export const CATEGORIES: Category[] = [
  {
    id: 'space',
    name: 'Free up space',
    lede: 'When your Mac says its storage is almost full: see what’s using it, and clear what’s safe to let go of. It all goes to the Trash first.',
    art: {
      paint: 'terraces',
      y: '60%',
      shots: [
        {
          src: 'diskusage',
          w: 2160,
          h: 1504,
          alt: "Disk Usage: a ring chart of what's filling the disk, beside a list of the biggest folders",
          at: { width: '70%', left: '5%', top: '7%' },
        },
        {
          src: 'freeupspace',
          w: 1080,
          h: 752,
          alt: "Free Up Space: cards for old installers, old Downloads, backups, caches and the Trash, each with what it'd free",
          at: { width: '66%', right: '5%', bottom: '7%' },
        },
      ],
    },
    features: [
      {
        name: 'Free Up Space',
        text: 'A card for each kind of thing that’s safe to let go of, with how much it frees. Review a list, or tick it all.',
      },
      { name: 'Old installers', text: 'Disk images and installers left in Downloads.' },
      { name: 'Old Downloads', text: 'Files that have sat in Downloads for over six months without being opened.' },
      { name: 'Backups, caches and the Trash', text: 'Old iPhone and iPad backups, apps’ caches, and what’s waiting in the Trash.' },
      { name: 'Large files', text: 'Everything of 100 MB and up, with when you last opened it.' },
      { name: 'Duplicates', text: 'Files that are exact copies of each other. The oldest one is kept.' },
      {
        name: 'Build folders',
        text: 'What your coding projects leave behind, their builds and downloaded packages, each with its project and when it was last built.',
        since: '0.1.10',
      },
      {
        name: 'Disk Usage',
        text: 'A ring chart of what’s filling a disk. Click a slice to look inside, or right-click to show it or move it to the Trash. Your whole startup disk appears in half a blink.',
      },
      {
        name: 'Uninstall apps',
        text: 'Removes an app and what it leaves in your Library: support files, caches, settings and logs, each with its size. Even one an administrator installed, with your password or Touch ID.',
        examples: ['uninstall zoom'],
      },
      {
        name: 'Tidy Up',
        text: 'Sorts a folder’s loose files into Screenshots, Images, Videos, Documents, Installers and more. One undo puts it all back.',
      },
      {
        name: 'Folder Rules',
        text: 'Tell a folder what to do with what lands in it, and it stays tidy on its own: Downloads sorted after a day, installers to the Trash after a week, invoices into Documents. Each rule shows what it would do first, and everything it does can be undone.',
        since: '0.1.10',
      },
      { name: 'Put Back', text: 'Whatever F2 clears goes to the Trash, and Put Back returns it.' },
      {
        name: 'Free space at a glance',
        text: 'See how much space is free, with Free Up Space and Disk Usage a Return away.',
        examples: ['storage'],
        since: '0.1.6',
      },
      {
        name: 'System Data, taken apart',
        text: 'What the “System Data” in your Mac’s settings is made of, from app caches and logs to developer leftovers, and how to free each part.',
        since: '0.1.7',
      },
    ],
  },
  {
    id: 'search',
    name: 'Find anything',
    lede: 'Type a few letters, or say what you’re after. Results come back before you’ve finished typing.',
    art: {
      paint: 'study-window',
      y: '42%',
      shots: [
        {
          src: 'search-photos',
          w: 2800,
          h: 1680,
          alt: 'F2 searching for "photos from last week": the week\'s photos, from several folders, as thumbnails',
          at: { width: '108%', left: '7%', top: '13%' },
        },
      ],
    },
    features: [
      { name: 'Search as you type', text: 'In the folder you’re in, or across your whole Mac.' },
      {
        name: 'In plain words',
        text: 'Say what you’re looking for, as you’d ask a person.',
        examples: ['photos from last week', 'big videos', 'pdfs from this month', 'screenshots in downloads'],
      },
      { name: 'Typos forgiven', text: '“recipies” still finds your recipes.' },
      {
        name: 'Filters to pick',
        text: 'Kind, date, size and where, from menus under the search field: PDFs from the past week, or videos over 1 GB in Downloads. Nothing to learn.',
        since: '0.1.6',
      },
      { name: 'Inside documents', text: 'Look for a word inside your files too, with Spotlight’s help.' },
      { name: 'Search tips', text: 'The ? in the search field lists every way to narrow a search down.' },
      { name: 'Other disks too', text: 'Search a USB stick, a disk image or a network share, as you would your Mac.', since: '0.1.9' },
      {
        name: 'How search is doing',
        text: 'Settings › Search shows what it covers and what it leaves out, and why, with a way to rebuild it.',
        since: '0.1.9',
      },
      { name: 'Smart Folders', text: 'Save a search to come back to. The Smart Folders you made in Finder open here too.' },
      {
        name: 'From any app',
        text: 'Press ⌘ Space and type a file’s name. What you worked on lately comes first.',
        keys: ['⌘', 'Space'],
      },
    ],
  },
  {
    id: 'open',
    name: 'Open anything',
    lede: 'Press ⌘ Space in any app, type a few letters, press Return. Quick Open opens apps, files, folders and websites, and does a lot more besides.',
    art: {
      paint: 'meadow',
      y: '30%',
      shots: [
        {
          src: 'quickopen-empty',
          w: 1360,
          h: 1096,
          alt: 'Quick Open with nothing typed: favorite folders, then commands such as Lock Screen and Sleep',
          at: { width: '64%', left: '6%', top: '7%' },
        },
        {
          src: 'quickopen-calc',
          w: 1360,
          h: 496,
          alt: 'Quick Open working out 12*7 + 15% as 96.6',
          at: { width: '62%', right: '5%', bottom: '8%' },
        },
      ],
    },
    features: [
      {
        name: 'From any app',
        text: 'Or from the F2 in the menu bar. Choose another shortcut, the screen it opens on, and its font and size in Settings.',
        keys: ['⌘', 'Space'],
      },
      {
        name: 'Apps',
        text: 'By name, initials, or a few letters in order. The ones you use most come first.',
        examples: ['vsc'],
      },
      { name: 'Apps in your language', text: 'Find apps by their Korean, Japanese or Chinese names: both of these find Notes.', examples: ['메모', 'めも'] },
      {
        name: 'Files and folders',
        text: 'Anywhere on your Mac, by a few letters, typos forgiven. Return opens it, and ⌘ Return shows it in its folder.',
      },
      { name: 'Websites and folders', text: 'Type a web address or a folder’s path to go there, or anything else to search the web.', examples: ['~/Downloads'] },
      {
        name: 'More with ⌘K',
        text: 'Show a result in its folder, Quick Look it, copy its path, make an image your desktop picture, move it to the Trash, or quit an app.',
      },
      { name: 'AirDrop and email', text: 'Send a file from Quick Open, without opening a window.', since: '0.1.5' },
      { name: 'Recent and favorites', text: 'With nothing typed, it lists what you opened lately, your favorite folders and every app.' },
      { name: 'Hints as you type', text: 'Gray text finishes what you’re typing: “time” suggests “in tokyo”. Tab takes it.' },
      { name: 'Korean, Japanese and Chinese', text: 'Typing in them works as it should, and you can ask in them too.', examples: ['東京の時間', '도쿄 시간'] },
      {
        name: 'Selected text, anywhere',
        text: 'Select text in any app and press a shortcut: translate it, define it, open its link, work it out, count it, or make a QR code of it.',
      },
      { name: 'Your Shortcuts', text: 'Run any of your Shortcuts by name.' },
    ],
  },
  {
    id: 'answers',
    name: 'Quick answers',
    lede: 'Ask in plain words in Quick Open and the answer appears as you type. Return copies it.',
    art: {
      paint: 'dusk-sea',
      y: '40%',
      shots: [
        {
          src: 'quickopen-currency',
          w: 1360,
          h: 358,
          alt: "Quick Open converting 100 USD to 89.09 EUR at the ECB's rate",
          at: { width: '72%', left: '6%', top: '9%' },
        },
        {
          src: 'quickopen-define',
          w: 680,
          h: 225,
          alt: 'Quick Open defining a word, from the Mac’s dictionary',
          at: { width: '64%', right: '5%', top: '38%' },
        },
        {
          src: 'quickopen-temp',
          w: 680,
          h: 179,
          alt: 'Quick Open converting 30 °C to 86 °F',
          at: { width: '60%', left: '9%', bottom: '9%' },
        },
      ],
    },
    features: [
      { name: 'Calculator', text: 'Sums, percentages and more, worked out as you type.', examples: ['12*7', '80 + 15%', 'sqrt 2'] },
      { name: 'Units', text: 'Distances, weights, temperatures and the rest.', examples: ['5 km in mi', '30 c to f'] },
      {
        name: 'Colors',
        text: 'Type a color and see it every way it’s written, to copy: HEX, RGB, HSL, OKLCH, or as SwiftUI code.',
        examples: ['#ff8800', 'rgb(255 136 0)', 'color tomato'],
        since: '0.2.0',
      },
      {
        name: 'Currency',
        text: 'At the European Central Bank’s daily rates. A plain “$50” goes into your own currency.',
        examples: ['100 usd in eur', '€20 to krw', '$50'],
      },
      {
        name: 'Stock and coin prices',
        text: 'With the day’s change. Return copies the price, and the next row opens its chart.',
        examples: ['btc price', 'aapl stock', 'btc in krw'],
        since: '0.1.6',
      },
      { name: 'Time around the world', text: 'About 7,500 cities, and every time zone.', examples: ['time in tokyo', '3pm in london', 'pst'] },
      { name: 'Dates', text: 'Count the days, or find out what day something falls on.', examples: ['days until christmas', '100 days from today', 'week number'] },
      { name: 'Dictionary', text: 'What a word means, from your Mac’s dictionary.', examples: ['define serendipity'] },
      { name: 'Spelling', text: 'The right spelling of a word you’re not sure of.', examples: ['spell accomodate'] },
      { name: 'Synonyms', text: 'Other words for it, from macOS’s thesaurus, with opposites.', examples: ['synonym for happy'] },
      { name: 'Word count', text: 'Counts the words, characters and lines in what you copied.', examples: ['word count'] },
      {
        name: 'Translate',
        text: 'With Apple’s translation, on your Mac. Languages Apple doesn’t have open in Google Translate.',
        examples: ['translate hola', 'good morning to japanese'],
      },
      { name: 'Weather', text: 'Now and the next three days, here or anywhere.', examples: ['weather', 'weather in tokyo'] },
      { name: 'Sunrise and sunset', text: 'Here or anywhere, in that place’s time.', examples: ['sunset', 'day length'] },
      {
        name: 'Air quality and UV',
        text: 'Fine dust, with what to do about it, and the UV index now and at today’s peak.',
        examples: ['air quality', 'uv'],
        since: '0.1.6',
      },
      { name: 'Holidays', text: 'The next public holidays, here or in another country.', examples: ['next holiday', 'holidays in japan'], since: '0.1.6' },
      { name: 'QR codes', text: 'Make one of a link or any text, to scan with your phone. “qr” alone makes one of what you copied.', examples: ['qr'] },
      { name: 'Tips and the bill', text: 'Work out the tip, and split it between friends.', examples: ['tip 18% on 84', 'split 120 four ways'], since: '0.1.6' },
      { name: 'Kitchen', text: 'Cups, spoons and grams, with each ingredient weighing what it should.', examples: ['1 cup flour in grams', '200 g butter in cups'], since: '0.1.6' },
      { name: 'Shoe and clothing sizes', text: 'From one country’s sizes to another’s.', examples: ['us 10 shoe in eu', 'medium shirt in korean'], since: '0.1.6' },
      {
        name: 'Flip a coin',
        text: 'Coins, dice, a random number, or a pick between a few things.',
        examples: ['flip a coin', 'roll 2d6', 'pick sushi, pizza or tacos'],
        since: '0.1.6',
      },
      {
        name: 'Maps and directions',
        text: 'In Apple Maps or Google Maps, and Naver Map or Kakao Map in Korea.',
        examples: ['coffee near me', 'walk to the park'],
        since: '0.1.6',
      },
      { name: 'Flights', text: 'Type a flight number to see where it is.', examples: ['UA837'], since: '0.1.6' },
      { name: 'Parcels', text: 'Paste a tracking number to open the carrier’s page: UPS, FedEx, DHL, Japan Post and more.' },
    ],
  },
  {
    id: 'day',
    name: 'Your day',
    lede: 'Reminders, events, timers and calls, typed the way you’d say them.',
    art: {
      paint: 'golden-hour',
      y: '40%',
      shots: [
        {
          src: 'quickopen-remind',
          w: 680,
          h: 179,
          alt: 'Quick Open adding a reminder to call mom tomorrow at 17:00',
          at: { width: '66%', left: '6%', top: '12%' },
        },
        {
          src: 'quickopen-time-australia',
          w: 680,
          h: 338,
          alt: "Quick Open showing the time in Australia's cities",
          at: { width: '66%', right: '6%', bottom: '10%' },
        },
      ],
    },
    features: [
      { name: 'Reminders', text: 'Added to Reminders, with the time worked out.', examples: ['remind me to call mom tomorrow at 5pm', 'remind me in 20 minutes to check the oven'] },
      { name: 'Calendar events', text: 'Added to your calendar as you say them.', examples: ['lunch with Sara friday at 1', 'workshop monday 2pm for 3 hours'] },
      { name: 'What’s next', text: 'Your next meeting, or the whole day. Return joins a Zoom, Meet or Teams call.', examples: ['next meeting', 'my day'] },
      { name: 'To-do', text: 'What’s due today, and overdue. Return checks one off.', examples: ['todo'], since: '0.1.6' },
      { name: 'Timers', text: 'The countdown sits in the menu bar, and a sound plays when it’s done.', examples: ['timer 10 min', 'pomodoro'] },
      { name: 'Stopwatch', text: 'Counts up in the menu bar. Pause, resume or stop it from there.', examples: ['stopwatch'], since: '0.1.6' },
      { name: 'Alarms', text: 'The alarm’s time shows in the menu bar until it goes off.', examples: ['alarm 7am', 'wake me at 6:30'] },
      {
        name: 'Breaks',
        text: 'A soft sound and a note to rest your eyes, stand up or drink some water, until you stop them.',
        examples: ['eye break every 20 min', 'drink water every 30 minutes'],
        since: '0.1.6',
      },
      {
        name: 'Sleep timers',
        text: 'Put the Mac to sleep, or pause Music and Spotify, a while from now.',
        examples: ['sleep in 30 min', 'stop music in 45 minutes'],
        since: '0.1.5',
      },
      { name: 'Calls and messages', text: 'Anyone in Contacts. Calls go through your iPhone.', examples: ['call mom', 'facetime sara', 'message dad'] },
      { name: 'Notes', text: 'Straight into Apple Notes.', examples: ['note buy milk'] },
      { name: 'Passwords', text: 'A strong one, copied, and cleared from the clipboard after 90 seconds.', examples: ['generate password', 'generate pin'] },
    ],
  },
  {
    id: 'mac',
    name: 'Your Mac',
    lede: 'The settings you reach for most, and what’s going on with your Mac, without opening System Settings.',
    art: {
      paint: 'night-aurora',
      y: '30%',
      shots: [
        {
          src: 'quickopen-wifi',
          w: 680,
          h: 218,
          alt: 'Quick Open turning Wi-Fi off and on',
          at: { width: '66%', left: '6%', top: '12%' },
        },
        {
          src: 'quickopen-darkmode',
          w: 680,
          h: 179,
          alt: 'Quick Open switching Dark Mode',
          at: { width: '62%', right: '6%', bottom: '14%' },
        },
      ],
    },
    features: [
      { name: 'Battery', text: 'Your Mac’s charge and time left, and your AirPods’, mouse’s and keyboard’s.', examples: ['battery'] },
      { name: 'Bluetooth', text: 'Connect or disconnect headphones, speakers, mice and keyboards.', examples: ['connect airpods'] },
      { name: 'Sound', text: 'Change the volume, or where the sound plays.', examples: ['volume 30', 'mute', 'speakers'] },
      { name: 'Brightness and music', text: 'Set the screen’s brightness. Play, pause and skip tracks.', examples: ['brightness 50'] },
      { name: 'Now playing', text: 'The song in Music or Spotify, with its artwork. Pause, skip, love it or find its lyrics.', examples: ['now playing'], since: '0.1.6' },
      { name: 'Play music', text: 'The best match from your Music library, or a search in Apple Music or Spotify.', examples: ['play taylor swift', 'play chill playlist'], since: '0.1.6' },
      { name: 'Wi-Fi password', text: 'Shows a network’s password, with a QR code guests can scan to join.', examples: ['wifi password'] },
      { name: 'Speed test', text: 'Download, upload and responsiveness, with macOS’s own test.', examples: ['speed test'] },
      { name: 'Is the internet down?', text: 'Checks Wi-Fi, your router and the internet one at a time, and says what’s wrong in plain words.', examples: ['is the internet down'], since: '0.1.6' },
      { name: 'Keep awake', text: 'For two hours, until 5pm, or until your downloads finish.', examples: ['keep awake'] },
      { name: 'Switch things on and off', text: 'Wi-Fi, Bluetooth, Do Not Disturb and Night Shift.', examples: ['turn wifi off', 'night shift on', 'dnd off'] },
      { name: 'Dark mode', text: 'Switch between light and dark.', examples: ['dark mode'] },
      { name: 'Hide desktop icons', text: 'Before you share your screen. Show Desktop too.', examples: ['hide desktop icons'] },
      { name: 'About this Mac', text: 'Model, chip, memory, macOS and serial number, ready to copy.', examples: ['about this mac'], since: '0.1.6' },
      { name: 'What’s slowing your Mac', text: 'How busy it is, and the apps using the most. Return quits one.', examples: ["what's slowing my mac"], since: '0.1.6' },
      { name: 'Clean your keyboard', text: 'Keys and clicks off for 30 seconds while you wipe it.', examples: ['clean keyboard'], since: '0.1.6' },
      { name: 'Your IP address', text: 'On this network, and to the internet.', examples: ['my ip'], since: '0.1.6' },
      { name: 'Restart an app', text: 'Quits it and opens it again.', examples: ['restart safari'], since: '0.1.6' },
      { name: 'Quit apps', text: 'One, all of them, or force quit one that’s stuck.', examples: ['quit safari', 'quit all'] },
      { name: 'Lock, sleep and restart', text: 'Lock Screen, Sleep, Restart, Shut Down and Log Out. Empty Trash and Eject All Disks too.' },
      { name: 'System Settings', text: 'Any pane, by name.', examples: ['displays', 'sound'] },
      { name: 'Latest download', text: 'The newest files in Downloads.', examples: ['latest download'] },
    ],
  },
  {
    id: 'finder',
    name: 'Everything Finder does',
    lede: 'The same windows, sidebar and Trash, so there’s nothing new to learn, and Finder stays on your Mac.',
    art: {
      paint: 'hero-valley',
      y: '35%',
      shots: [
        {
          src: 'dual-photos',
          w: 2800,
          h: 1680,
          alt: 'Two folders side by side: holiday photos on the left, a USB stick on the right',
          at: { width: '90%', left: '5%', top: '14%' },
        },
      ],
    },
    features: [
      { name: 'Windows and tabs', text: 'Each one remembers where it was.', keys: ['⌘N', '⌘T'] },
      { name: 'The sidebar', text: 'Favorites, Recents, AirDrop, iCloud Drive, your disks, Dropbox, Google Drive, OneDrive, the network and Tags.' },
      { name: 'Four views', text: 'Icons at any size, a list, columns, or a gallery with a big preview.' },
      { name: 'A list your way', text: 'Choose, resize and reorder its columns, and open folders in place.' },
      { name: 'Sort and group', text: 'By kind, date, size or tags. Each folder remembers its own view.' },
      { name: 'Folder sizes', text: 'Shown right away, and kept up to date as files change.' },
      { name: 'Looks like Finder', text: 'Finder’s own icons, thumbnails, tag colors and iCloud badges.' },
      { name: 'Right-click menus', text: 'Open With, Get Info, Rename, Compress, Duplicate, Share, Tags and the rest, as in Finder.' },
      {
        name: 'Icons where you put them',
        text: 'Drag icons anywhere in icon view, as in Finder. Folders you arranged in Finder open just as you left them; Clean Up tidies them into rows.',
        since: '0.1.7',
      },
      {
        name: 'Folder colors and emoji',
        text: 'Give a folder a color, and a symbol or an emoji on it, as on macOS Tahoe. Tagged folders take their tag’s color.',
        since: '0.1.7',
      },
      {
        name: 'Cut and paste',
        text: 'Cut files and paste them somewhere else to move them, which Finder never could. And New File, and an Up button to the enclosing folder.',
        keys: ['⌘X', '⌘V'],
        since: '0.1.7',
      },
      { name: 'Import from iPhone', text: 'Take a photo, scan a document or add a sketch on your iPhone or iPad, saved straight into the folder.' },
      { name: 'Rename many at once', text: 'Replace or add text, or number them, and see the new names as you type.' },
      { name: 'Copy and move safely', text: 'It asks before replacing anything, shows progress, and can be stopped.' },
      {
        name: 'Copies that keep going',
        text: 'How fast it’s going and how long is left. A file that can’t be copied is set aside and listed, not the end of it all, and a stopped copy picks up where it left off.',
        since: '0.1.8',
      },
      { name: 'Drag and drop', text: 'Between windows, and to and from any app, even Mail and Photos. Folders spring open as you drag over them, and the one you drop on pulses, so you see it land.' },
      {
        name: 'Drag out to any app',
        text: 'Into Mail to attach, onto a Finder window to move or copy, or onto the Trash in the Dock, as from Finder. Even from a window in the background, which stays where it is.',
        since: '0.1.10',
      },
      { name: 'AirDrop', text: 'Send files to nearby devices without opening Finder.' },
      { name: 'Undo', text: 'Undo and redo for everything you do.', keys: ['⌘Z'] },
      { name: 'The Trash', text: 'Put Back, Empty Trash, and Delete Immediately.' },
      { name: 'Get Info', text: 'Tags, comments, permissions, a custom icon, and which app opens it.', keys: ['⌘I'] },
      {
        name: 'A preview at once',
        text: 'Space shows a photo, a text file, a PDF or what’s in a folder straight away, and follows along as you move through your files.',
        keys: ['Space'],
        since: '0.1.8',
      },
      { name: 'Quick Look', text: 'Videos, Office documents and the rest, in macOS’s own preview, one click from F2’s.' },
      { name: 'Zip files as folders', text: 'Open .zip, .rar, .7z and .tar files like folders. Extract unpacks them.' },
      { name: 'Two folders side by side', text: 'Drag from one to the other, each with its own tabs.', keys: ['⌥⌘S'] },
      { name: 'Go to Folder', text: 'Type where to go, with completion. Connect to a file server too.', keys: ['⇧⌘G'] },
      { name: 'Preview pane', text: 'And a path bar, and a status bar with the free space.', keys: ['⇧⌘P'] },
      { name: 'Start where you like', text: 'New windows open in Home, Downloads, or any folder you pick.' },
      { name: 'Show in Finder opens F2', text: 'When an app offers to show a file in Finder, it opens in F2.' },
      { name: 'Set Desktop Picture', text: 'Right-click an image to make it your desktop picture.', since: '0.1.6' },
    ],
  },
  {
    id: 'fix',
    name: 'Quick fixes for files',
    lede: 'Right-click a file for the jobs you’d otherwise need another app for.',
    art: { paint: 'still-lake', y: '45%', glass: 'menu' },
    features: [
      { name: 'Make Smaller', text: 'A photo to a 1600-pixel JPEG, or a video to 720p, small enough to email.' },
      {
        name: 'Resize images',
        text: 'To the size you want: 1920 pixels wide, half the size, or to fit a box. The copies go beside the originals, named by their new size, the right way up.',
        examples: ['resize images'],
        since: '0.2.0',
      },
      { name: 'Remove Background', text: 'Keeps a photo’s subject on a clear background. On macOS 14 and later.' },
      { name: 'Copy Text', text: 'Copies the words in a photo, a screenshot or a scanned PDF.' },
      { name: 'Convert images', text: 'To JPEG, PNG or HEIC, and rotate them.' },
      { name: 'PDFs', text: 'Make a PDF, combine several into one, or make one smaller.' },
      { name: 'Videos to MP4', text: 'Convert a video to MP4, for whatever won’t play it as it is.' },
      { name: 'Your Shortcuts', text: 'Your own Shortcuts, right in the menu.' },
    ],
  },
  {
    id: 'clipboard',
    name: 'Copy, paste, type less',
    lede: 'Everything you copy, kept, plus emoji and shortcuts for what you type again and again.',
    art: {
      paint: 'mist-forest',
      y: '55%',
      shots: [
        {
          src: 'clipboard',
          w: 680,
          h: 548,
          alt: 'Clipboard history: text, links, an image and files, newest first, with a preview of the one picked',
          at: { width: '74%', left: '13%', top: '11%' },
        },
      ],
    },
    features: [
      { name: 'Clipboard History', text: 'Text, links, images and files, newest first, with a preview. Keeps 50, 200 or 1,000.' },
      {
        name: 'Find images by their words',
        text: 'A screenshot of an invoice turns up when you type its number. The words are read on your Mac.',
        since: '0.1.8',
      },
      { name: 'Passwords stay out', text: 'What password managers mark as secret isn’t kept.' },
      { name: 'Pinned clips', text: 'Pin an address or a bank number so it stays at the top, and is never cleared.', since: '0.1.5' },
      { name: 'Paste as Plain Text', text: 'Pastes without the formatting, then puts your clipboard back as it was.' },
      {
        name: 'Fix Spelling',
        text: 'Select text in any app and fix its typos right where they are, with your Mac’s own spell checker. Names are left alone.',
        examples: ['fix spelling'],
        since: '0.1.7',
      },
      {
        name: 'Text tools',
        text: 'Change what you copied, or the text you’ve selected, right where it is.',
        examples: ['uppercase', 'title case', 'remove line breaks'],
        since: '0.1.5',
      },
      { name: 'Emoji & Symbols', text: 'Every emoji, plus arrows, keys, math and currency symbols, found by name.', examples: [':check', ':+1'] },
      { name: 'Paste where you were', text: 'Return pastes a clip or an emoji into the app you were in. ⌘ Return just copies it.' },
      {
        name: 'Snippets',
        text: 'Type a short keyword in any app and it becomes your text, with today’s date or what you copied filled in.',
        examples: [';date'],
      },
      { name: 'Quicklinks', text: 'Open a site or a folder by keyword: “gh rust” searches GitHub.', examples: ['gh rust'] },
    ],
  },
  {
    id: 'screenshots',
    name: 'Screenshots and recordings',
    lede: 'Take one, mark it up and send it, from a shortcut in any app.',
    art: { paint: 'meadow', y: '35%', glass: 'marquee' },
    features: [
      { name: 'Area, window or screen', text: 'Drag out an area (Space switches to a window), or take the whole screen.' },
      { name: 'Copy text off the screen', text: 'Drag over any text, even in a picture, and it’s copied.' },
      { name: 'Pick a color', text: 'Copies any color on the screen, as you like it: HEX, RGB, HSL, OKLCH, or as code for your app.' },
      {
        name: 'Colors you’ve picked',
        text: 'Every color you pick is kept, in Colors in Quick Open: each with a big swatch, every way it’s written, and how well it reads on white and on black.',
        examples: ['colors'],
        since: '0.2.0',
      },
      { name: 'Read a QR code', text: 'Straight off the screen, without your phone.' },
      { name: 'Ready to share', text: 'Each shot is copied and shown in the corner: drag it into an app, or edit, save or pin it.' },
      { name: 'Mark it up', text: 'Arrows, shapes, a pen, a highlighter, text, numbered steps, blur and crop, in eight colors.' },
      { name: 'Backgrounds', text: 'Set a shot on a gradient, with padding, rounded corners and a shadow.' },
      { name: 'Pin', text: 'Float a screenshot over everything while you work.' },
      { name: 'Screenshot History', text: 'Your last 50, to copy, edit, save or pin again.' },
      { name: 'Screen recording', text: 'The whole screen or an area, with your microphone and clicks if you like, saved as MP4.', since: '0.1.5' },
      {
        name: 'Scrolling Capture',
        text: 'Drag over a long page or list: F2 scrolls it for you and takes all of it in one tall screenshot.',
        examples: ['scrolling screenshot'],
        since: '0.1.8',
      },
      { name: 'GIFs', text: 'Save a recording as a GIF, for the places a video won’t play.', since: '0.1.8' },
      { name: 'A tidy Desktop', text: 'Screenshots can go into a Screenshots folder instead of piling up.' },
    ],
  },
  {
    id: 'windows',
    name: 'Tidy windows',
    lede: 'Snap the window in front to half the screen, a quarter or a third, from a shortcut or Quick Open.',
    art: { paint: 'terraces', y: '40%', glass: 'snap' },
    features: [
      { name: 'Halves, thirds and quarters', text: 'Press again for two thirds, then one third.', examples: ['left half'] },
      { name: 'Fill the screen', text: 'All of it, or almost. Or just its height.' },
      { name: 'Center and resize', text: 'Center a window, or make it a size smaller or larger.' },
      { name: 'Other displays', text: 'Move a window to the next screen, or back.', examples: ['next display'] },
      { name: 'Back where it was', text: 'Restore a window’s size and place.' },
      {
        name: 'Switch Window',
        text: 'Every window you have open, not just every app. Find one by its name, or give it a shortcut and flip through them.',
        examples: ['switch window'],
        since: '0.1.7',
      },
      {
        name: 'Every desktop, at a glance',
        text: 'Switch Window shows what each window looks like, and the ones on your other desktops too: pick one and you’re there.',
        since: '0.1.8',
      },
      { name: 'Shortcuts', text: 'Familiar ones set up in one click, or record your own. A gap around windows if you like.' },
      {
        name: 'Drag from anywhere',
        text: 'Hold ⌃⌘ and drag anywhere in a window to move it, or near an edge or a corner to size it.',
        keys: ['⌃⌘'],
        since: '0.1.9',
      },
    ],
  },
  {
    id: 'extensions',
    name: 'Extensions',
    lede: 'Commands for the websites and services you use, from F2’s own store. Free, read by a person before they’re in, and shut in so they can only do what they say.',
    art: { paint: 'golden-hour', y: '45%', queries: ['wiki alan turing', 'hn', 'brew ffmpeg', 'linear'] },
    features: [
      {
        name: 'The Extension Store',
        text: 'In Quick Open: what each one does, the websites it may reach and what it asks for, before you install it with Return. Updates come from there too.',
        examples: ['extension store'],
        since: '0.2.0',
      },
      {
        name: 'No account needed',
        text: 'Wikipedia, with articles to read right in Quick Open. Hacker News, its comments too. MDN, Can I Use and Tailwind CSS’s colors. Homebrew, npm, crates.io and PyPI. Lorem Ipsum and Kaomoji.',
        examples: ['wiki alan turing', 'hn', 'brew ffmpeg'],
        since: '0.2.0',
      },
      {
        name: 'With your accounts',
        text: 'Your GitHub pull requests and Actions runs, Linear and Jira issues, Notion pages, Todoist tasks, and Vercel and Cloudflare deployments. Their tokens are kept in your Mac’s keychain.',
        since: '0.2.0',
      },
      {
        name: 'Safe by design',
        text: 'An extension can’t open your files or run anything on your Mac, and it reaches only the websites it names. F2 checks each one it downloads against the store’s signed list.',
        since: '0.2.0',
      },
      {
        name: 'Make your own',
        text: 'New Extension makes one, ready to run, in plain JavaScript. Save a change and it runs again. Then share it in the store.',
        examples: ['new extension'],
        since: '0.2.0',
      },
    ],
  },
  {
    id: 'power',
    name: 'For power users',
    lede: 'Out of the way until you want them.',
    art: {
      paint: 'dusk-sea',
      y: '35%',
      shots: [
        {
          src: 'hero-dual',
          w: 2800,
          h: 1680,
          alt: 'F2 with two folders side by side: Documents as a list, and a folder of photos as icons',
          at: { width: '108%', left: '7%', top: '13%' },
        },
      ],
    },
    features: [
      {
        name: 'Your servers, as folders',
        text: 'The servers you reach over SSH, in the sidebar: browse them, open and save files, copy both ways, and open a terminal there. With your Mac’s own SSH keys and settings.',
        since: '0.1.9',
      },
      { name: 'Two panes by keyboard', text: 'Tab between them, ⌃⌘C copies across, ⌃⌘M moves (F5 and F6 too), and ⌥⌘U swaps them. Or right-click a sidebar folder: Open in Other Pane.', keys: ['⌃⌘C', '⌃⌘M'] },
      { name: 'Git’s status', text: 'Modified, added and untracked files marked beside their names, and a dot on folders with changes.' },
      { name: 'Terminal and editor', text: 'Open the folder or selection in Ghostty, iTerm, Warp, Zed, Cursor or VS Code.', keys: ['⌥⌘T', '⌥⌘E'] },
      { name: 'Ports', text: 'See what’s listening, and stop it.', examples: ['kill port 3000', 'ports'] },
      { name: 'Repos', text: 'Find your repos by name, open them on GitHub, see their pull requests, or copy the branch name.' },
      { name: 'JSON, Base64 and URLs', text: 'Format or minify the JSON you copied, or encode and decode it.' },
      { name: 'UUIDs and timestamps', text: 'Generate a UUID, copy the Unix time, or read a timestamp as a date.' },
      { name: 'Search filters', text: 'By extension, kind, size, date or folder, with globs, or through files’ contents.', examples: ['ext:pdf'] },
      { name: 'Snippet fill-ins', text: '{clipboard}, {date}, {time}, {uuid} and {cursor} in your snippets.' },
    ],
  },
  {
    id: 'app',
    name: 'The app itself',
    lede: 'Small, quick, and out of the way.',
    art: {
      paint: 'night-aurora',
      y: '35%',
      shots: [
        {
          src: 'start-welcome',
          w: 920,
          h: 640,
          alt: "F2's welcome: a few permissions, each with what it's for and a button to allow it",
          at: { width: '78%', left: '11%', top: '12%' },
        },
      ],
    },
    features: [
      { name: 'In the menu bar', text: 'Opens a window, a favorite folder, the search or Disk Usage, and shows your timers and alarms.' },
      { name: 'A friendly welcome', text: 'Explains what F2 does, and asks for each permission with the reason it needs it.' },
      { name: 'Settings', text: 'General, Quick Open, Clipboard, Windows, Screenshots, Sidebar, Permissions and more.' },
      {
        name: 'Keyboard shortcuts',
        text: 'Give anything Quick Open does a shortcut that works in any app, and change F2’s own keys, all in one place.',
        since: '0.1.7',
      },
      { name: 'Always there', text: 'Like Finder, it keeps running: ⌘Q closes its windows, and the menu bar item quits it.' },
      { name: 'Updates', text: 'F2 checks for new versions for you, and the changelog says what each one brings.' },
      {
        name: 'Tell us what’s wrong',
        text: 'Send feedback from the Help menu. If F2 ever quits unexpectedly, it asks before sending a report, and the report holds nothing of your files.',
        since: '0.1.9',
      },
      { name: 'Private', text: 'Your files and the search index stay on your Mac.' },
      { name: 'Runs on', text: 'Any Mac on macOS 13 Ventura or later, Apple silicon or Intel.' },
    ],
  },
]

export const FEATURE_COUNT = CATEGORIES.reduce((n, c) => n + c.features.length, 0)

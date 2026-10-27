import { LATEST } from './changelog'

// The current release: the newest dated entry in changelog.ts. Keep that in
// step with Cargo.toml's version and the appcast at https://download.getf2.com.
export const VERSION = LATEST.version
export const MIN_MACOS = '13'

// The site's canonical address. Canonical tags, og:url, the sitemap and the
// JSON-LD all point here. www and *.workers.dev 301 here via src/server.ts.
export const SITE_URL = 'https://getf2.com'

// The latest DMG: release-macos.sh uploads each release as F2-<version>.dmg
// and as F2.dmg.
export const DOWNLOAD_URL = 'https://download.getf2.com/F2.dmg'

// Who it's for, under every Download button. (Release builds run on both:
// see scripts/bundle-macos.sh.)
export const FINE_PRINT = `Free and open source · Apple silicon and Intel · macOS ${MIN_MACOS} or later`

// Read off F2's own status bar (entries indexed, a folder listed in 0.2 ms)
// and Disk Usage's footer (the whole disk from the index in 45 ms), in
// blinks: a blink takes about 100 ms.
export const NUMBERS = [
  { value: '4.4 million', label: 'files and folders, found as you type' },
  { value: '500×', label: 'quicker than a blink, to open a folder' },
  { value: 'Half a blink', label: "to see what's filling your whole disk" },
]

// The feature video (video/, rendered with Remotion) on the f2-updates bucket
// beside the DMGs: too big for the site's own files. It's the render made
// smaller (ffmpeg -c:v libx264 -preset slow -crf 22 -c:a aac -b:a 128k
// -movflags +faststart), named by its contents (shasum's first 8) so it can
// be cached for good: a new render goes up under a new name. Its still is
// public/img/video/f2-feature.webp, the frame 3 s in.
export const VIDEO_URL = 'https://download.getf2.com/video/f2-feature-4ced64ff.mp4'
export const VIDEO_SECONDS = 55

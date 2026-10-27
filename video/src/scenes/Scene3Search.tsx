// 3. Plain words: an F2 window, ⌘F, "photos from last week", and the week's
// photos fill in. While it's typed, the list follows each key, as fsearch
// does: names with every word in them.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, random, useCurrentFrame } from 'remotion'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { FinderWindow, type ListRow } from '../components/FinderWindow'
import { Doc, Folder, Thumb } from '../components/icons'
import { Keycaps } from '../components/Keycaps'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { DOCUMENTS, photoThumb } from '../samples'
import { sec } from '../theme'
import { compile, fieldAt, mark, type Script } from '../timeline'

export const SCENE3_LENGTH = sec(6.6)

const QUERY = 'photos from last week'

const script: Script = {
	pointer: [1180, 700],
	steps: [
		{ at: 0.55, keys: ['⌘', 'F'], id: 'find' },
		{ at: 1.05, type: QUERY, field: 'search', seed: 'week', id: 'typed' },
	],
}

const row = (icon: React.ReactNode, name: string, kind: string, location: string, date: string, size: string): ListRow => ({ icon, name, kind, location, date, size })
const pdf = <Doc size={16} label="PDF" color="#e5483f" />
const txt = <Doc size={16} label="TXT" color="#8e8e93" />

// What a few letters find on the Mac (the names, not yet the meaning).
const POOL: ListRow[] = [
	row(<Folder size={16} />, 'Photos', 'Folder', '~', 'Oct 19, 2026 at 18:02', '4.62 GB'),
	row(<Doc size={16} label="ZIP" color="#8e8e93" />, 'Photos from the trip.zip', 'ZIP archive', '~/Downloads', 'Oct 3, 2026 at 20:41', '48.2 MB'),
	row(pdf, 'Photos to print.pdf', 'PDF document', '~/Documents', 'Sep 28, 2026 at 10:15', '2.4 MB'),
	row(<Folder size={16} />, 'Old phone photos', 'Folder', '~/Photos', 'Jun 2, 2026 at 21:30', '1.18 GB'),
	row(pdf, 'Photography course.pdf', 'PDF document', '~/Documents', 'Apr 9, 2026 at 14:22', '3.1 MB'),
	row(txt, 'Photo ideas.txt', 'Plain Text Document', '~/Documents', 'Aug 14, 2026 at 08:47', '2 KB'),
	row(pdf, 'Phone bill March.pdf', 'PDF document', '~/Documents', 'Mar 30, 2026 at 09:12', '88 KB'),
	row(pdf, 'Passport scan.pdf', 'PDF document', '~/Documents', 'Jan 12, 2026 at 16:40', '1.2 MB'),
	row(<Thumb size={16} paint="terraces" />, 'Paint swatches.png', 'PNG image', '~/Desktop', 'Sep 21, 2026 at 11:03', '640 KB'),
	row(txt, 'Packing list.txt', 'Plain Text Document', '~/Documents', 'Jul 1, 2026 at 19:20', '1 KB'),
	row(txt, 'Podcast notes.txt', 'Plain Text Document', '~/Documents', 'May 18, 2026 at 12:05', '3 KB'),
]
// How many the whole Mac has, for so few letters.
const COUNTS: Record<string, number> = { p: 48210, ph: 9316, pho: 2108, phot: 1402, photo: 1377, photos: 241 }

// The week's photos, newest first (today being Tuesday, October 20).
const WEEK: [string, string, string][] = [
	['Reading nook.jpg', 'Yesterday at 13:05', '598 KB'],
	['Northern lights.jpg', 'Yesterday at 10:52', '341 KB'],
	['Misty forest.jpg', 'Oct 18, 2026 at 18:39', '465 KB'],
	['Wildflowers.jpg', 'Oct 18, 2026 at 17:10', '417 KB'],
	['Golden hour.jpg', 'Oct 17, 2026 at 15:26', '419 KB'],
	['Summer valley.jpg', 'Oct 17, 2026 at 14:57', '300 KB'],
	['River bend.jpg', 'Oct 16, 2026 at 19:31', '329 KB'],
	['Still lake.jpg', 'Oct 16, 2026 at 11:44', '417 KB'],
	['Dusk at sea.jpg', 'Oct 15, 2026 at 12:13', '369 KB'],
	['Rice terraces.jpg', 'Oct 14, 2026 at 16:18', '558 KB'],
]

// As listing::count says it ("1 item", "2108 items").
const count = (n: number, what: string) => `${n} ${what}${n === 1 ? '' : 's'}`

function found(q: string, frame: number, done: number): { rows: ListRow[]; n: number } {
	if (q === QUERY) {
		const rows = WEEK.map(([name, date, size], i) => ({
			...row(photoThumb(name), name, 'JPEG image', '~/Photos', date, size),
			selected: i === 0,
			// They fill in, one after another.
			opacity: interpolate(frame - done - i * 2, [0, 8], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' }),
		}))
		return { rows, n: WEEK.length }
	}
	const words = q.toLowerCase().split(' ').filter(Boolean)
	const rows = POOL.filter((r) => words.every((w) => r.name.toLowerCase().includes(w)))
	return { rows, n: COUNTS[q.trim()] ?? rows.length }
}

const ease = Easing.bezier(0.23, 1, 0.32, 1)

export function Scene3Search() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-3'), [])
	const q = fieldAt(t, 'search', frame)
	const find = mark(t, 'find')
	const done = mark(t, 'typed')
	const open = interpolate(frame, [find, find + 9], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease })
	const searching = open >= 1
	const { rows, n } = searching ? found(q.text, frame, done) : { rows: DOCUMENTS, n: DOCUMENTS.length }
	// fsearch's and the list's times, as the window says them.
	const r = random(`time-${q.text}`)
	const timing =
		q.text === QUERY
			? '10 results · fsearch 0.38 ms · keystroke to list 3.22 ms'
			: q.text
				? `${count(n, 'result')} · fsearch ${(0.2 + r * 1.2).toFixed(2)} ms · keystroke to list ${(2 + r * 3).toFixed(2)} ms`
				: ''
	const since = frame - Math.max(q.frame, find)
	const caret = since < sec(0.5) || Math.floor((since - sec(0.5)) / sec(0.5)) % 2 === 1
	// A slow push in on the list as it fills.
	const zoom = interpolate(frame, [done, done + sec(3)], [1, 1.06], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: Easing.inOut(Easing.quad) })

	return (
		<AbsoluteFill>
			<Desktop wallpaper="meadow" at="center 60%" timeline={t} camera={{ zoom, x: 700, y: 320 }}>
				<FinderWindow
					x={118}
					y={150}
					width={1300}
					height={560}
					sidebar="Documents"
					panes={[
						{
							title: 'Documents',
							view: 'list',
							rows: searching && q.text ? rows : DOCUMENTS,
							columns: searching ? ['date', 'size', 'kind', 'location'] : ['date', 'size'],
							status: searching && q.text ? count(n, 'item') : '11 items, 88.53 GB available · listed in 0.3 ms',
							search: { open, query: q.text, caret, scope: 'mac', timing },
							active: !searching,
						},
					]}
				/>
			</Desktop>
			<Caption title="Find anything, in plain words." />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

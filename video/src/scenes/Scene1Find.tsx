// 1. ⌘ Space: Quick Open over the desktop. "invocie", a typo, and Invoice
// March.pdf is the top result anyway; clicked, it opens.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from 'remotion'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { Doc, Folder, Thumb, Tile } from '../components/icons'
import { Keycaps } from '../components/Keycaps'
import { QuickOpen, type Row, type Section } from '../components/QuickOpen'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { sec } from '../theme'
import { compile, fieldAt, mark, type Script } from '../timeline'

export const SCENE1_LENGTH = sec(6.4)

const script: Script = {
	pointer: [1236, 652],
	steps: [
		{ at: 0.55, keys: ['⌘', 'Space'], id: 'open' },
		{ at: 0.62, sound: 'whoosh', volume: 0.4 },
		{ at: 1.35, type: 'invocie', field: 'q', id: 'typed' },
		{ at: 3.55, move: [598, 309], dur: 0.8 },
		{ at: 4.8, click: true, id: 'chosen' },
	],
}

// What Quick Open lists with nothing typed (as quickopen-empty.webp): the
// sidebar's Favorites, then the commands.
export const EMPTY: Section[] = [
	{
		heading: 'Favorites',
		rows: [
			{ icon: <Folder glyph="desktop" />, title: 'Desktop', subtitle: '~', kind: 'Folder' },
			{ icon: <Folder glyph="documents" />, title: 'Documents', subtitle: '~', kind: 'Folder' },
			{ icon: <Folder glyph="downloads" />, title: 'Downloads', subtitle: '~', kind: 'Folder' },
			{ icon: <Folder />, title: 'Photos', subtitle: '~', kind: 'Folder' },
			{ icon: <Folder />, title: 'Projects', subtitle: '~', kind: 'Folder' },
			{ icon: <Folder glyph="apps" />, title: 'Applications', subtitle: '/', kind: 'Folder' },
		],
	},
	{
		heading: 'Commands',
		rows: [
			{ icon: <Tile color="#ff453a" symbol="lock" />, title: 'Lock Screen', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="sleep" />, title: 'Sleep', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="restart" />, title: 'Restart', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="power" />, title: 'Shut Down', subtitle: 'System', kind: 'Command' },
		],
	},
]

const pdf = <Doc label="PDF" color="#e5483f" />
const docx = <Doc label="DOCX" color="#2f6fd6" />
const FILES: Record<string, Row> = {
	march: { icon: pdf, title: 'Invoice March.pdf', subtitle: '~/Documents/Invoices', kind: 'File' },
	february: { icon: pdf, title: 'Invoice February.pdf', subtitle: '~/Documents/Invoices', kind: 'File' },
	invoices: { icon: <Folder />, title: 'Invoices', subtitle: '~/Documents', kind: 'Folder' },
	template: { icon: docx, title: 'Invoice template.docx', subtitle: '~/Documents/Work', kind: 'File' },
	img: { icon: <Thumb paint="meadow" />, title: 'IMG_2041.HEIC', subtitle: '~/Photos', kind: 'File' },
	interview: { icon: docx, title: 'Interview notes.docx', subtitle: '~/Documents/Work', kind: 'File' },
	insurance: { icon: pdf, title: 'Insurance 2026.pdf', subtitle: '~/Documents', kind: 'File' },
}

/** What's listed for what's typed: files, what you worked on lately first, then the web. */
function results(q: string): { sections: Section[]; primary: string } {
	if (!q) return { sections: EMPTY, primary: 'Open Folder' }
	const f = FILES
	const files = q.length === 1 ? [f.img, f.march, f.invoices, f.interview] : q.length === 2 ? [f.march, f.invoices, f.interview, f.insurance] : [f.march, f.february, f.invoices, f.template]
	const web: Row = { icon: <Tile color="#30b0c7" symbol="globe" />, title: `Search the web for “${q}”`, subtitle: 'Google', kind: 'Web Search' }
	return {
		sections: [
			{ heading: 'Files and Folders', rows: files },
			{ heading: 'Web', rows: [web] },
		],
		primary: 'Open File',
	}
}

const ease = Easing.bezier(0.23, 1, 0.32, 1)

export function Scene1Find() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-1'), [])
	const q = fieldAt(t, 'q', frame)
	const open = mark(t, 'open')
	const chosen = mark(t, 'chosen')
	const appear =
		interpolate(frame, [open, open + 9], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease }) *
		interpolate(frame, [chosen + 3, chosen + 11], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })
	const { sections, primary } = results(q.text)

	return (
		<AbsoluteFill>
			<Desktop timeline={t}>
				<QuickOpen query={q.text} sections={sections} primary={primary} appear={appear} typedAt={Math.max(q.frame, open)} />
			</Desktop>
			<Caption title="Find any file in a few letters." sub="Typos forgiven." subFrom={mark(t, 'typed') + sec(0.35)} />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

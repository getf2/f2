// 2. Answers: Quick Open again, and in quick succession a sum, a currency
// (its ghost text taken with Tab), the time in London (the same), and a
// translation.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from 'remotion'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { Doc, Tile } from '../components/icons'
import { Keycaps } from '../components/Keycaps'
import { QuickOpen, type Row, type Section } from '../components/QuickOpen'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { sec } from '../theme'
import { compile, fieldAt, mark, type Script } from '../timeline'
import { EMPTY } from './Scene1Find'

export const SCENE2_LENGTH = sec(9.6)

const script: Script = {
	pointer: [1236, 652],
	steps: [
		{ at: 0.4, keys: ['⌘', 'Space'], id: 'open' },
		{ at: 0.47, sound: 'whoosh', volume: 0.4 },
		{ at: 1.0, type: '80 + 15%', field: 'q', seed: 'sum' },
		{ at: 2.35, keys: ['⌘', 'A'], field: 'q', edit: 'selectAll' },
		{ at: 2.6, type: '100 usd', field: 'q', seed: 'usd' },
		{ at: 3.4, keys: ['Tab'], field: 'q', edit: { append: ' in eur' } },
		{ at: 4.3, keys: ['⌘', 'A'], field: 'q', edit: 'selectAll' },
		{ at: 4.55, type: '3pm', field: 'q', seed: 'pm' },
		{ at: 5.05, keys: ['Tab'], field: 'q', edit: { append: ' in london' } },
		{ at: 5.95, keys: ['⌘', 'A'], field: 'q', edit: 'selectAll' },
		{ at: 6.2, type: 'good morning to japanese', field: 'q', seed: 'jp', id: 'asked' },
	],
}

// With nothing typed: what was opened here last (scene 1's invoice), then
// as before.
const OPENED: Section[] = [
	{ heading: 'Recent', rows: [{ icon: <Doc label="PDF" color="#e5483f" />, title: 'Invoice March.pdf', subtitle: '~/Documents/Invoices', kind: 'File' }] },
	...EMPTY,
]

const sum = (title: string, q: string): Section => ({
	heading: 'Calculator',
	rows: [{ icon: <Tile color="#ff9f0a" symbol="equal" />, title, subtitle: q, kind: 'Calculator' }],
})
const SUMS: Record<string, string> = { '80 + 1': '81', '80 + 15': '95', '80 + 15%': '92' }
const clock = <Tile color="#34aadc" symbol="clock" />

/** What Quick Open lists for what's typed, what Return does, and the ghost text. */
function answer(q: string, translated: boolean): { sections: Section[]; primary: string; ghost?: string } {
	if (!q) return { sections: OPENED, primary: 'Open File' }
	const web: Section = { heading: 'Web', rows: [{ icon: <Tile color="#30b0c7" symbol="globe" />, title: `Search the web for “${q}”`, subtitle: 'Google', kind: 'Web Search' }] }
	if (SUMS[q]) return { sections: [sum(SUMS[q], q), web], primary: 'Copy Answer' }
	if (q === '100 usd') return { sections: [web], primary: 'Search the Web', ghost: ' in eur' }
	if (q === '100 usd in eur') {
		const row: Row = { icon: <Tile color="#ff9f0a" symbol="equal" />, title: '89.09 EUR', subtitle: '100 USD in EUR · ECB rate of 19 Oct', kind: 'Calculator' }
		return { sections: [{ heading: 'Currency', rows: [row] }, web], primary: 'Copy Answer' }
	}
	if (q === '3pm') return { sections: [web], primary: 'Search the Web', ghost: ' in london' }
	if (q === '3pm in london') {
		const rows: Row[] = [
			{ icon: clock, title: '20:00', subtitle: "London, United Kingdom · Tuesday, October 20 · when it's 15:00 here", kind: 'Time' },
			{ icon: clock, title: '15:00', subtitle: "London, Canada · Tuesday, October 20 · when it's 15:00 here", kind: 'Time' },
		]
		return { sections: [{ heading: 'Time', rows }, web], primary: 'Copy Answer' }
	}
	if (q === 'good morning to japanese') {
		const card: Row = {
			icon: <Tile color="#007aff" symbol="translate" />,
			over: 'English → Japanese',
			title: translated ? 'おはようございます' : 'Translating…',
			pending: !translated,
			kind: 'Translation',
		}
		return { sections: [{ heading: 'Translation', rows: [card] }, web], primary: translated ? 'Copy Translation' : 'Open in Google Translate' }
	}
	return { sections: [web], primary: 'Search the Web' }
}

const ease = Easing.bezier(0.23, 1, 0.32, 1)

export function Scene2Ask() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-2'), [])
	const q = fieldAt(t, 'q', frame)
	const open = mark(t, 'open')
	const appear = interpolate(frame, [open, open + 9], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease })
	// Apple's translation, a moment after it's asked.
	const translated = frame >= mark(t, 'asked') + sec(0.3)
	const { sections, primary, ghost } = answer(q.text, translated)

	return (
		<AbsoluteFill>
			<Desktop timeline={t}>
				<QuickOpen
					query={q.text}
					sections={sections}
					primary={primary}
					ghost={ghost}
					appear={appear}
					selectedText={q.selected}
					typedAt={Math.max(q.frame, open)}
				/>
			</Desktop>
			<Caption title="Press ⌘ Space. Ask your Mac." />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

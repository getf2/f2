// 7. Small tools, in three quick cuts: an arrow drawn on a screenshot in
// the editor; clipboard history (⌘Space, "clip", Return; it has no
// shortcut of its own); a window put in the bottom half of the screen
// ("bottom", Return: window shortcuts are only there once they're set).
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from 'remotion'
import { Caption } from '../components/Caption'
import { ClipboardPanel } from '../components/ClipboardPanel'
import { Desktop } from '../components/Desktop'
import { FinderWindow } from '../components/FinderWindow'
import { Tile } from '../components/icons'
import { Keycaps } from '../components/Keycaps'
import { QuickOpen, type Row, type Section } from '../components/QuickOpen'
import { Arrow, EDITOR_TITLE, EDITOR_TOOLBAR, ScreenshotEditor } from '../components/ScreenshotEditor'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { DOCUMENTS } from '../samples'
import { SCREEN, sec } from '../theme'
import { compile, fieldAt, mark, pointerAt, type Script } from '../timeline'
import { HOLIDAY } from './Scene6Panes'

export const SCENE7_LENGTH = sec(7.6)
const CUT_B = sec(2.45)
const CUT_C = sec(4.95)

const EDITOR = { x: 238, y: 168, width: 1060, height: 600 }
const SHOT = { x: EDITOR.x + 80, y: EDITOR.y + EDITOR_TITLE + EDITOR_TOOLBAR + 42, width: 900, height: 440 }
const FROM: [number, number] = [SHOT.x + 720, SHOT.y + 382]
const TO: [number, number] = [SHOT.x + 503, SHOT.y + 286]
const WIN = { x: 268, y: 200, width: 1000, height: 520 }
const MENU_BAR = 25

const script: Script = {
	pointer: [FROM[0] + 40, FROM[1] + 60],
	steps: [
		{ at: 0.12, sound: 'whoosh', volume: 0.35 },
		{ at: 0.35, move: FROM, dur: 0.3 },
		{ at: 0.75, press: 'down', id: 'down' },
		{ at: 0.8, move: TO, dur: 0.55 },
		{ at: 1.45, press: 'up', id: 'drawn' },
		{ at: 2.6, keys: ['⌘', 'Space'], id: 'qo1' },
		{ at: 2.67, sound: 'whoosh', volume: 0.35 },
		{ at: 3.05, type: 'clip', field: 'clip', seed: 'clip' },
		{ at: 3.55, keys: ['↩'], id: 'history' },
		{ at: 4.0, keys: ['↓'], id: 'd1' },
		{ at: 4.35, keys: ['↓'], id: 'd2' },
		{ at: 5.1, keys: ['⌘', 'Space'], id: 'qo2' },
		{ at: 5.17, sound: 'whoosh', volume: 0.35 },
		{ at: 5.55, type: 'bottom', field: 'snap', seed: 'snap' },
		{ at: 6.2, keys: ['↩'], id: 'snap' },
	],
}

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const
const ease = Easing.bezier(0.23, 1, 0.32, 1)
const web = (q: string): Section => ({ heading: 'Web', rows: [{ icon: <Tile color="#30b0c7" symbol="globe" />, title: `Search the web for “${q}”`, subtitle: 'Google', kind: 'Web Search' }] })
const rest = (whole: string, typed: string) => (typed.length >= 2 && whole.toLowerCase().startsWith(typed.toLowerCase()) ? whole.slice(typed.length) : undefined)

const CLIPBOARD: Row = { icon: <Tile color="#30d158" symbol="clipboard" />, title: 'Clipboard History', subtitle: 'F2', kind: 'Command' }
const BOTTOM: Row[] = [
	{ icon: <Tile color="#5e5ce6" symbol="bottomHalf" />, title: 'Bottom Half', subtitle: 'Window', kind: 'Command' },
	{ icon: <Tile color="#5e5ce6" symbol="bottomLeft" />, title: 'Bottom Left Quarter', subtitle: 'Window', kind: 'Command' },
	{ icon: <Tile color="#5e5ce6" symbol="bottomRight" />, title: 'Bottom Right Quarter', subtitle: 'Window', kind: 'Command' },
]

export function Scene7Tools() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-7'), [])
	const m = (id: string) => mark(t, id)
	const p = pointerAt(t, frame)!
	// Each cut comes in, and the one before it goes.
	const cutIn = (from: number) => interpolate(frame, [from, from + 8], [0, 1], { ...clamp, easing: ease })
	const cutOut = (to: number) => interpolate(frame, [to - 7, to], [1, 0], clamp)

	// (a) The editor, the arrow following the pointer while it's held.
	const editor = frame < CUT_B ? cutIn(sec(0.12)) * cutOut(CUT_B) : 0
	const arrowTo: [number, number] = frame >= m('drawn') ? TO : [p.x, p.y]
	const drawing = frame >= m('down')

	// (b) Quick Open, "clip", then the clipboard's history.
	const clip = fieldAt(t, 'clip', frame)
	const qo1 = frame >= m('qo1') && frame < m('history') ? cutIn(m('qo1')) : 0
	const history = frame >= m('history') && frame < CUT_C ? cutOut(CUT_C) : 0
	const selected = frame >= m('d2') ? 2 : frame >= m('d1') ? 1 : 0

	// (c) A window, Quick Open, "bottom", and it's in the bottom half.
	const snap = fieldAt(t, 'snap', frame)
	const win = frame >= CUT_C ? cutIn(CUT_C) : 0
	const snapped = frame >= m('snap')
	const qo2 = frame >= m('qo2') && frame < m('snap') + 4 ? cutIn(m('qo2')) * interpolate(frame, [m('snap'), m('snap') + 4], [1, 0], clamp) : 0
	const half = (SCREEN.height - MENU_BAR) / 2

	return (
		<AbsoluteFill>
			<Desktop wallpaper="dusk-sea" at="center 35%" timeline={t} pointer={frame < CUT_B}>
				{editor > 0 && (
					<ScreenshotEditor {...EDITOR} title="Screenshot 2026-10-20 at 09.41.12" appear={editor} drawn={frame >= m('drawn')}>
						<div style={{ position: 'absolute', left: SHOT.x - EDITOR.x, top: SHOT.y - EDITOR.y - EDITOR_TITLE - EDITOR_TOOLBAR }}>
							<FinderWindow
								x={0}
								y={0}
								width={SHOT.width}
								height={SHOT.height}
								sidebar="Photos"
								panes={[{ title: 'Holiday 2026', view: 'icons', icons: HOLIDAY.map((h) => ({ icon: h.icon, name: h.name })), status: '6 items, 88.53 GB available · listed in 0.2 ms' }]}
							/>
							{drawing && <Arrow from={[FROM[0] - SHOT.x, FROM[1] - SHOT.y]} to={[arrowTo[0] - SHOT.x, arrowTo[1] - SHOT.y]} />}
						</div>
					</ScreenshotEditor>
				)}
				{qo1 > 0 && (
					<QuickOpen
						query={clip.text}
						sections={clip.text.length >= 2 ? [{ heading: 'Commands', rows: [CLIPBOARD] }, web(clip.text)] : clip.text ? [web(clip.text)] : []}
						primary={clip.text.length >= 2 ? 'Run Command' : 'Search the Web'}
						ghost={rest(CLIPBOARD.title, clip.text)}
						appear={qo1}
						typedAt={Math.max(clip.frame, m('qo1'))}
					/>
				)}
				{history > 0 && <ClipboardPanel selected={selected} appear={history} />}
				{win > 0 && (
					<FinderWindow
						{...(snapped ? { x: 0, y: MENU_BAR + half, width: SCREEN.width, height: half } : WIN)}
						sidebar="Documents"
						appear={win}
						panes={[{ title: 'Documents', view: 'list', rows: DOCUMENTS, columns: ['date', 'size'], status: '11 items, 88.53 GB available · listed in 0.3 ms' }]}
					/>
				)}
				{qo2 > 0 && (
					<QuickOpen
						query={snap.text}
						sections={snap.text.length >= 2 ? [{ heading: 'Commands', rows: BOTTOM }, web(snap.text)] : snap.text ? [web(snap.text)] : []}
						primary={snap.text.length >= 2 ? 'Run Command' : 'Search the Web'}
						ghost={rest(BOTTOM[0].title, snap.text)}
						appear={qo2}
						typedAt={Math.max(snap.frame, m('qo2'))}
					/>
				)}
			</Desktop>
			<Caption title="The little apps you'd pay for, built in." />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

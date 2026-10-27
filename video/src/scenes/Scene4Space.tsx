// 4. Storage: Window › Disk Usage (it has no shortcut), the ring chart drawn
// in, then Free Up Space, a card's Review, and a few ticks: what's to free
// adds up.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from 'remotion'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { cardButton, DiskUsage, FREE_ROW, tickBox, type DuPage } from '../components/DiskUsage'
import { Menu, menuItemY, type MenuItem } from '../components/Menu'
import { WINDOW_MENU } from '../components/MenuBar'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { sec } from '../theme'
import { compile, mark, pointerAt, type Script } from '../timeline'

export const SCENE4_LENGTH = sec(8.8)

const WIN = { x: 228, y: 150, width: 1080, height: 700 }
const MENU_TOP = 27
// F2's Window menu (src/menus.rs).
const WINDOW_ITEMS: MenuItem[] = [{ title: 'Minimize', keys: '⌘M' }, { title: 'Zoom' }, '-', { title: 'Disk Usage' }, '-', { title: 'Show Previous Tab' }, { title: 'Show Next Tab' }]
const at = (p: { x: number; y: number }): [number, number] => [WIN.x + p.x, WIN.y + p.y]

const script: Script = {
	pointer: [430, 190],
	steps: [
		{ at: 0.25, move: [WINDOW_MENU.x, 12], dur: 0.5 },
		{ at: 0.82, click: true, id: 'menu' },
		{ at: 0.98, move: [WINDOW_MENU.left + 62, MENU_TOP + menuItemY(WINDOW_ITEMS, 3)], dur: 0.4 },
		{ at: 1.5, click: true, id: 'open' },
		{ at: 1.52, sound: 'whoosh', volume: 0.4 },
		// Out of the caption's way, into the window.
		{ at: 1.75, move: [WIN.x + 120, WIN.y + 330], dur: 0.7 },
		{ at: 3.3, move: at(FREE_ROW), dur: 0.65 },
		{ at: 4.1, click: true, id: 'free' },
		{ at: 4.65, move: at(cardButton(1, WIN.width)), dur: 0.55 },
		{ at: 5.35, click: true, id: 'review' },
		{ at: 5.75, move: at(tickBox(0)), dur: 0.45 },
		{ at: 6.3, click: true, id: 'tick0' },
		{ at: 6.5, move: at(tickBox(1)), dur: 0.25 },
		{ at: 6.85, click: true, id: 'tick1' },
		{ at: 7.05, move: at(tickBox(2)), dur: 0.25 },
		{ at: 7.4, click: true, id: 'tick2' },
	],
}

const ease = Easing.bezier(0.23, 1, 0.32, 1)
const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const

export function Scene4Space() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-4'), [])
	const m = (id: string) => mark(t, id)
	const menuOpen = frame >= m('menu') && frame < m('open')
	// The item under the pointer, while the menu's open.
	const p = pointerAt(t, frame)!
	const hovered = WINDOW_ITEMS.findIndex((item, i) => item !== '-' && Math.abs(p.y - (MENU_TOP + menuItemY(WINDOW_ITEMS, i))) < 12 && p.x > WINDOW_MENU.left && p.x < WINDOW_MENU.left + 230)
	const appear = interpolate(frame, [m('open') + 2, m('open') + 11], [0, 1], { ...clamp, easing: ease })
	const grow = interpolate(frame, [m('open') + 6, m('open') + sec(1.25)], [0, 1], clamp)
	const page: DuPage = frame >= m('review') + 2 ? 'review' : frame >= m('free') + 2 ? 'free' : 'chart'
	const since = frame - (page === 'review' ? m('review') : m('free')) - 2
	const shown = interpolate(since, [0, 9], [0, 1], clamp)
	const ticked = [frame >= m('tick0'), frame >= m('tick1'), frame >= m('tick2'), false, false]

	return (
		<AbsoluteFill>
			<Desktop wallpaper="terraces" at="center 35%" timeline={t} menu={menuOpen ? 'Window' : undefined}>
				<DiskUsage {...WIN} page={page} grow={grow} free={shown} ticked={ticked} appear={appear} />
				{menuOpen && <Menu x={WINDOW_MENU.left} y={MENU_TOP} items={WINDOW_ITEMS} highlighted={hovered >= 0 ? hovered : undefined} />}
			</Desktop>
			{/* (In once the menu at the top's done with.) */}
			<Caption title="Storage almost full? Clear it in a few clicks." from={mark(t, 'open')} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

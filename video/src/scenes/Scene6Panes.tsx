// 6. Drag and drop: ⌥⌘S opens a second pane (at the same folder), Tab goes
// over to it and ⌘↑ up to Photos. Three photos dragged across, F2's red
// count on them, onto a folder there: it pulses, the pop plays, and they
// fade out of the first pane; the second says where they went.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame } from 'remotion'
import { type Big } from '../components/bigIcons'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { CELL, FinderWindow, SIDEBAR, type IconItem } from '../components/FinderWindow'
import { Keycaps } from '../components/Keycaps'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { photoIcon } from '../samples'
import { FONT, sec } from '../theme'
import { compile, mark, pointerAt, type Script } from '../timeline'

export const SCENE6_LENGTH = sec(6.6)

const WIN = { x: 68, y: 158, width: 1400, height: 590 }
const HALF = (WIN.width - SIDEBAR) / 2
const TOOLBAR = 52

// Holiday 2026, as dual-photos.webp has it.
export const HOLIDAY: { name: string; icon: Big }[] = [
	{ name: 'Big cloud.jpg', icon: { kind: 'photo', paint: 'hero-valley', at: '88% 18%' } },
	{ name: 'Cabin.jpg', icon: { kind: 'photo', paint: 'night-aurora', at: 'center 60%' } },
	{ name: 'Hill walk.jpg', icon: { kind: 'photo', paint: 'terraces', at: 'center 70%' } },
	{ name: 'Jetty.jpg', icon: { kind: 'photo', paint: 'still-lake', at: '20% 85%' } },
	{ name: 'Picnic.jpg', icon: { kind: 'photo', paint: 'meadow', at: 'center 85%' } },
	{ name: 'Sunset.jpg', icon: { kind: 'photo', paint: 'golden-hour', at: 'center 45%' } },
]
const MOVED = 3
// Photos, folders first.
const PHOTOS_FOLDER: IconItem[] = [
	{ icon: { kind: 'folder' }, name: 'Best of 2026' },
	{ icon: { kind: 'folder' }, name: 'Family' },
	{ icon: { kind: 'folder' }, name: 'Holiday 2026' },
	{ icon: photoIcon('Dusk at sea.jpg'), name: 'Dusk at sea.jpg' },
	{ icon: photoIcon('Golden hour.jpg'), name: 'Golden hour.jpg' },
	{ icon: photoIcon('Misty forest.jpg'), name: 'Misty forest.jpg' },
]

/** Where icon `i` of a pane is on the screen. */
const cell = (pane: 0 | 1, i: number): [number, number] => [
	WIN.x + SIDEBAR + pane * HALF + CELL.first + (i % 3) * CELL.x,
	WIN.y + TOOLBAR + CELL.top + Math.floor(i / 3) * CELL.y,
]

const script: Script = {
	pointer: [760, 660],
	steps: [
		{ at: 0.45, keys: ['⌥', '⌘', 'S'], id: 'split' },
		{ at: 1.3, keys: ['Tab'], id: 'tab' },
		{ at: 1.75, keys: ['⌘', '↑'], id: 'up' },
		{ at: 2.2, move: cell(0, 1), dur: 0.5 },
		{ at: 2.85, press: 'down', id: 'grab' },
		{ at: 3.0, move: cell(1, 0), dur: 1.0 },
		{ at: 4.15, press: 'up', id: 'drop' },
		{ at: 4.15, sound: 'pop', volume: 0.5 },
	],
}

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const

/** The drag's image (finder/dnd.rs): the grabbed one's icon and name on a
 * pill, lifted, with how many there are in red at its corner. */
function Dragged({ x, y, lift }: { x: number; y: number; lift: number }) {
	return (
		<div
			style={{
				position: 'absolute',
				left: x + 10,
				top: y + 8,
				height: 24,
				padding: '0 9px 0 6px',
				display: 'flex',
				alignItems: 'center',
				gap: 6,
				borderRadius: 6,
				background: '#0a64d8',
				color: '#fff',
				fontFamily: FONT.ui,
				fontSize: 12,
				boxShadow: '0 4px 10px rgba(0,0,0,0.2), 0 1px 3px rgba(0,0,0,0.15)',
				opacity: 0.6 + 0.35 * lift,
				zIndex: 900,
			}}
		>
			<Img src={staticFile('paint/night-aurora.webp')} style={{ width: 16, height: 12, objectFit: 'cover', borderRadius: 2 }} />
			Cabin.jpg
			<div
				style={{
					position: 'absolute',
					top: -7,
					right: -8,
					height: 17,
					minWidth: 17,
					padding: '0 4px',
					borderRadius: 9,
					background: '#ff3b30',
					fontSize: 10,
					fontWeight: 700,
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'center',
				}}
			>
				{MOVED}
			</div>
		</div>
	)
}

export function Scene6Panes() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-6'), [])
	const m = (id: string) => mark(t, id)
	const split = interpolate(frame, [m('split'), m('split') + 8], [0, 1], { ...clamp, easing: Easing.out(Easing.cubic) })
	const drop = m('drop')
	const dropped = frame >= drop
	// The grabbed photos fade (160 ms) before they go; the rest close up.
	const leave = interpolate(frame, [drop, drop + sec(0.16)], [1, 0], clamp)
	const gone = frame >= drop + sec(0.16)
	const left: IconItem[] = HOLIDAY.flatMap((p, i) => {
		const moving = i < MOVED
		if (moving && gone) return []
		return [{ icon: p.icon, name: p.name, selected: moving, opacity: moving ? leave : 1, at: gone ? i - MOVED : i }]
	})
	// The folder dropped on: pointed at while the drag's over it, then its
	// pulse (650 ms, up in the first 18%, then falling off).
	const p = pointerAt(t, frame)!
	const dragging = frame >= m('grab') && frame < drop
	const [fx, fy] = cell(1, 0)
	const over = dragging && Math.abs(p.x - fx) < 60 && Math.abs(p.y - fy) < 60
	const k = (frame - drop) / sec(0.65)
	const pulse = k < 0 || k > 1 ? 0 : k < 0.18 ? k / 0.18 : Math.pow(1 - (k - 0.18) / 0.82, 2)
	const up = frame >= m('up')
	const right: IconItem[] = up
		? PHOTOS_FOLDER.map((item, i) => ({ ...item, selected: i === 0 && over, pulse: i === 0 ? pulse : 0 }))
		: HOLIDAY.map((h, i) => ({ icon: h.icon, name: h.name, selected: i < MOVED }))
	// The keyboard's pane: the right after Tab, the left again once it's clicked in.
	const rightActive = frame >= m('tab') && frame < m('grab')
	const lift = interpolate(frame, [m('grab') + 4, m('grab') + 4 + sec(0.14)], [0, 1], clamp)

	return (
		<AbsoluteFill>
			<Desktop wallpaper="still-lake" at="center 40%" timeline={t}>
				<FinderWindow
					{...WIN}
					sidebar="Photos"
					split={split}
					panes={[
						{
							title: 'Holiday 2026',
							view: 'icons',
							icons: left,
							status: gone ? '3 items, 88.53 GB available · listed in 0.2 ms' : '3 of 6 selected, 88.53 GB available · listed in 0.2 ms',
							active: !rightActive,
						},
						{
							title: up ? 'Photos' : 'Holiday 2026',
							view: 'icons',
							icons: right,
							status: up ? '6 items, 88.53 GB available · listed in 0.3 ms' : '3 of 6 selected, 88.53 GB available · listed in 0.2 ms',
							notice: dropped ? 'Moved 3 items to “Best of 2026”' : undefined,
							active: rightActive,
						},
					]}
				/>
				{dragging && frame >= m('grab') + 4 && <Dragged x={p.x} y={p.y} lift={lift} />}
			</Desktop>
			<Caption title="Everything Finder does. Just quicker." />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

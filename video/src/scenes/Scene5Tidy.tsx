// 5. Tidy: a messy Downloads (tidy-before.webp), right-click › Tidy Up (it
// has no shortcut), and the files fly into folders of their kinds
// (tidy-after.webp). Then ⌘Z, and they're all back.
import { useMemo } from 'react'
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from 'remotion'
import { type Big } from '../components/bigIcons'
import { Caption } from '../components/Caption'
import { Desktop } from '../components/Desktop'
import { CELL, FinderWindow, SIDEBAR, type IconItem } from '../components/FinderWindow'
import { Keycaps } from '../components/Keycaps'
import { Menu, MENU_ROW, menuItemY, type MenuItem } from '../components/Menu'
import { Sounds } from '../components/Sounds'
import { useFonts } from '../fonts'
import { SCREEN, sec } from '../theme'
import { compile, mark, pointerAt, type Script } from '../timeline'

export const SCENE5_LENGTH = sec(7)

const WIN = { x: 68, y: 158, width: 1400, height: 590 }

// Downloads, as tidy-before.webp has it, and where Tidy Up puts each
// (tidy.rs: by its name, then its extension).
const FILES: { name: string; icon: Big; into: string }[] = [
	{ name: 'Beach day.mov', icon: { kind: 'doc', glyph: 'video', label: 'MOV' }, into: 'Videos' },
	{ name: 'Boarding pass LHR.pdf', icon: { kind: 'page', band: '#1f9cc4', title: 'Boarding pass' }, into: 'Documents' },
	{ name: 'Concert tickets.pdf', icon: { kind: 'page', band: '#c8501a', title: 'Concert tickets' }, into: 'Documents' },
	{ name: 'Firefox 131.0.dmg', icon: { kind: 'doc', glyph: 'download' }, into: 'Installers' },
	{ name: 'Fonts.zip', icon: { kind: 'doc', glyph: 'zip', label: 'ZIP' }, into: 'Archives' },
	{ name: 'GoogleChrome.dmg', icon: { kind: 'doc', glyph: 'download' }, into: 'Installers' },
	{ name: 'IMG_4182.jpg', icon: { kind: 'photo', paint: 'still-lake' }, into: 'Images' },
	{ name: 'IMG_4190.heic', icon: { kind: 'photo', paint: 'meadow', at: 'center 70%' }, into: 'Images' },
	{ name: 'Invoice 0423.pdf', icon: { kind: 'page', band: '#7b61d6', title: 'Invoice 0423' }, into: 'Documents' },
	{ name: 'Meeting notes.docx', icon: { kind: 'page', title: 'Meeting notes' }, into: 'Documents' },
	{ name: 'Photos from the trip.zip', icon: { kind: 'doc', glyph: 'zip', label: 'ZIP' }, into: 'Archives' },
	{ name: 'Road trip mix.mp3', icon: { kind: 'audio' }, into: 'Audio' },
	{ name: 'Screenshot 2026-10-12 at 14.02.11.png', icon: { kind: 'photo', paint: 'golden-hour' }, into: 'Screenshots' },
	{ name: 'Screenshot 2026-10-14 at 09.31.47.png', icon: { kind: 'photo', paint: 'mist-forest' }, into: 'Screenshots' },
	{ name: 'Screenshot 2026-10-18 at 21.15.03.png', icon: { kind: 'photo', paint: 'dusk-sea' }, into: 'Screenshots' },
	{ name: 'Spotify.dmg', icon: { kind: 'doc', glyph: 'download' }, into: 'Installers' },
	{ name: 'Voice memo.m4a', icon: { kind: 'audio' }, into: 'Audio' },
	{ name: 'Zoom.pkg', icon: { kind: 'package' }, into: 'Installers' },
]
const FOLDERS = ['Archives', 'Audio', 'Documents', 'Images', 'Installers', 'Screenshots', 'Videos']

// The window's background menu (finder/menu.rs: background_menu).
const BACKGROUND: MenuItem[] = [
	{ title: 'New Folder', keys: '⇧⌘N' },
	{ title: 'New File', keys: '⌥⌘N' },
	'-',
	{ title: 'Open in Terminal', keys: '⌥⌘T' },
	'-',
	{ title: 'Get Info', keys: '⌘I' },
	{ title: 'Tidy Up' },
	{ title: 'Import from iPhone or iPad', submenu: true },
	'-',
	{ title: 'Paste Item', keys: '⌘V', disabled: true },
	'-',
	{ title: 'View as', submenu: true },
	{ title: 'Group By', submenu: true },
	{ title: 'Sort By', submenu: true },
	{ title: 'Show View Options', keys: '⌘J' },
	{ title: 'Show Hidden Files', keys: '⇧⌘.' },
	'-',
	{ title: 'Show in Finder' },
]
const MENU_HEIGHT = menuItemY(BACKGROUND, BACKGROUND.length) - MENU_ROW / 2 + 5
const TIDY = BACKGROUND.findIndex((i) => i !== '-' && i.title === 'Tidy Up')
// Right-clicked on the empty end of the third row; the menu fits on the
// screen, so it's moved up over the pointer.
const CLICK: [number, number] = [WIN.x + SIDEBAR + 820, WIN.y + 410]
const MENU = { x: CLICK[0], y: Math.min(CLICK[1], SCREEN.height - 8 - MENU_HEIGHT) }

const script: Script = {
	pointer: [1060, 700],
	steps: [
		{ at: 0.35, move: CLICK, dur: 0.5 },
		{ at: 1.0, click: true, id: 'menu' },
		{ at: 1.2, move: [MENU.x + 70, MENU.y + menuItemY(BACKGROUND, TIDY)], dur: 0.4 },
		{ at: 1.8, click: true, id: 'tidy' },
		{ at: 3.75, keys: ['⌘', 'Z'], id: 'undo' },
	],
}

const pos = (i: number) => ({ x: CELL.first + (i % 7) * CELL.x, y: CELL.top + Math.floor(i / 7) * CELL.y })
const fly = Easing.bezier(0.5, 0, 0.25, 1)
const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const

export function Scene5Tidy() {
	useFonts()
	const frame = useCurrentFrame()
	const t = useMemo(() => compile(script, 'scene-5'), [])
	const m = (id: string) => mark(t, id)
	const menuOpen = frame >= m('menu') && frame < m('tidy')
	const p = pointerAt(t, frame)!
	const hovered = BACKGROUND.findIndex((item, i) => item !== '-' && Math.abs(p.y - (MENU.y + menuItemY(BACKGROUND, i))) < MENU_ROW / 2 && p.x > MENU.x && p.x < MENU.x + 250)

	// Into the folders (0 → 1), and back out with ⌘Z (1 → 0).
	const tidy = m('tidy') + 4
	const undo = m('undo') + 2
	const progress = (i: number) => {
		const lag = i * 1.2
		const into = interpolate(frame, [tidy + lag, tidy + lag + 26], [0, 1], { ...clamp, easing: fly })
		const back = interpolate(frame, [undo + lag * 0.6, undo + lag * 0.6 + 24], [0, 1], { ...clamp, easing: fly })
		return into * (1 - back)
	}
	const folders = interpolate(frame, [tidy, tidy + 10], [0, 1], clamp) * interpolate(frame, [undo + 24, undo + 34], [1, 0], clamp)

	const icons: IconItem[] = []
	if (folders > 0)
		FOLDERS.forEach((name, i) => icons.push({ icon: { kind: 'folder' }, name, at: i, opacity: folders, scale: 0.85 + folders * 0.15 }))
	FILES.forEach((f, i) => {
		const k = progress(i)
		if (k >= 0.999) return
		const from = pos(i)
		const to = pos(FOLDERS.indexOf(f.into))
		icons.push({ icon: f.icon, name: f.name, at: i, dx: (to.x - from.x) * k, dy: (to.y - from.y) * k, scale: 1 - 0.7 * k, opacity: 1 - Math.max(0, k - 0.55) / 0.45 })
	})
	const tidied = frame >= tidy

	return (
		<AbsoluteFill>
			<Desktop wallpaper="mist-forest" at="center 45%" timeline={t}>
				<FinderWindow
					{...WIN}
					sidebar="Downloads"
					panes={[
						{
							title: 'Downloads',
							view: 'icons',
							icons,
							status: frame >= undo + 30 || !tidied ? '18 items, 86.37 GB available · listed in 1.2 ms' : '7 items, 86.37 GB available · listed in 0.4 ms',
							// Said for six seconds (finder/menu.rs).
							notice: tidied ? 'Tidied 18 items into 7 folders. ⌘Z puts them back.' : undefined,
						},
					]}
				/>
				{menuOpen && <Menu x={MENU.x} y={MENU.y} width={250} items={BACKGROUND} highlighted={hovered >= 0 ? hovered : undefined} />}
			</Desktop>
			<Caption title="Downloads, tidied in one click." sub="⌘Z puts it all back." subFrom={m('undo') + sec(0.2)} />
			<Keycaps timeline={t} />
			<Sounds timeline={t} />
		</AbsoluteFill>
	)
}

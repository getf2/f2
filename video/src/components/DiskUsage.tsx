// Disk Usage, as diskusage.webp and freeupspace.webp show it and
// src/disk_usage.rs and disk_usage/clean.rs draw it: the sidebar (Disks,
// Clean Up), and either the ring chart with its list, the Free Up Space
// page of cards, or a card's list to tick.
import { random } from 'remotion'
import { FONT } from '../theme'
import { Sym, type SYMBOL } from './symbols'
import { Window } from './Window'

const SECONDARY = 'rgba(0, 0, 0, 0.5)'
const SEPARATOR = 'rgba(0, 0, 0, 0.09)'
const ACCENT = '#0a64d8'
const SIDE = 209
const TITLE = 31

// What's on the disk (the top of it), biggest first, as the list says it.
export const TOP: { name: string; size: number; shown: string; folder: boolean }[] = [
	{ name: 'Users', size: 248.12, shown: '248.12 GB', folder: true },
	{ name: 'Applications', size: 98.4, shown: '98.40 GB', folder: true },
	{ name: 'System', size: 32.71, shown: '32.71 GB', folder: true },
	{ name: 'Library', size: 14.03, shown: '14.03 GB', folder: true },
	{ name: 'private', size: 9.86, shown: '9.86 GB', folder: true },
	{ name: 'opt', size: 4.12, shown: '4.12 GB', folder: true },
	{ name: 'usr', size: 2.51, shown: '2.51 GB', folder: true },
	{ name: 'bin', size: 0.0118, shown: '11.8 MB', folder: true },
	{ name: 'sbin', size: 0.0092, shown: '9.2 MB', folder: true },
]
// The innermost ring's hues (disk_usage.rs: HUES), the rest gray.
const HUES = [0.58, 0.76, 0.92, 0.07, 0.13, 0.33, 0.5, 0.67, 0.0, 0.42]

const color = (hue: number | null, depth: number) => {
	if (hue === null) return `hsl(0 0% ${74 + depth * 3}%)`
	const s = Math.max(0.25, 0.72 - depth * 0.07)
	const l = 0.55 + depth * 0.05
	return `hsl(${hue * 360} ${s * 100}% ${l * 100}%)`
}

type Slice = { a0: number; a1: number; depth: number; fill: string }

/** The rings: each folder's share of its parent's arc, six deep, seeded. */
function slices(): Slice[] {
	const out: Slice[] = []
	const total = TOP.reduce((s, t) => s + t.size, 0)
	const split = (a0: number, a1: number, depth: number, hue: number | null, seed: string) => {
		if (depth >= 6 || a1 - a0 < 0.6) return
		const n = 2 + Math.floor(random(`${seed}-n`) * 5)
		const weights = Array.from({ length: n }, (_, i) => Math.pow(random(`${seed}-w${i}`), 2.4) + 0.02)
		const sum = weights.reduce((a, b) => a + b, 0)
		// Not all of a folder is in its folders: the rest is files.
		const covered = 0.72 + random(`${seed}-c`) * 0.26
		let a = a0
		weights.forEach((w, i) => {
			const span = ((a1 - a0) * covered * w) / sum
			const file = depth > 1 && random(`${seed}-f${i}`) < 0.18
			out.push({ a0: a, a1: a + span, depth, fill: color(file ? null : hue, depth) })
			if (!file) split(a, a + span, depth + 1, hue, `${seed}-${i}`)
			a += span
		})
	}
	let a = 0
	TOP.forEach((t, i) => {
		const span = (t.size / total) * 360
		const hue = i < 7 ? HUES[i] : null
		out.push({ a0: a, a1: a + span, depth: 0, fill: color(hue, 0) })
		split(a, a + span, 1, hue, `du-${t.name}`)
		a += span
	})
	return out
}
const SLICES = slices()

const polar = (cx: number, cy: number, r: number, deg: number) => {
	const rad = ((deg - 90) * Math.PI) / 180
	return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)]
}
function arc(cx: number, cy: number, r0: number, r1: number, a0: number, a1: number) {
	const large = a1 - a0 > 180 ? 1 : 0
	const [x0, y0] = polar(cx, cy, r1, a0)
	const [x1, y1] = polar(cx, cy, r1, a1)
	const [x2, y2] = polar(cx, cy, r0, a1)
	const [x3, y3] = polar(cx, cy, r0, a0)
	return `M${x0} ${y0}A${r1} ${r1} 0 ${large} 1 ${x1} ${y1}L${x2} ${y2}A${r0} ${r0} 0 ${large} 0 ${x3} ${y3}Z`
}

/** The ring chart, drawn in as `grow` goes 0 → 1: the inner rings first. */
export function Ring({ r, grow }: { r: number; grow: number }) {
	const hole = r * 0.3
	const band = (r - hole) / 6
	const ease = (t: number) => 1 - Math.pow(1 - Math.min(1, Math.max(0, t)), 3)
	return (
		<svg width={r * 2 + 4} height={r * 2 + 4} viewBox={`${-r - 2} ${-r - 2} ${r * 2 + 4} ${r * 2 + 4}`} style={{ overflow: 'visible' }}>
			{SLICES.map((s, i) => {
				const sweep = 360 * ease((grow - s.depth * 0.07) / 0.62)
				if (sweep <= s.a0) return null
				const a1 = Math.min(s.a1, sweep)
				const r0 = hole + band * s.depth + (s.depth ? 0.6 : 0)
				return <path key={i} d={arc(0, 0, r0, r0 + band - 0.6, s.a0, Math.min(a1, s.a0 + 359.99))} fill={s.fill} stroke="#fff" strokeWidth={0.7} />
			})}
			<circle r={hole - 2} fill="#f5f5f7" stroke="rgba(0,0,0,0.06)" strokeWidth={0.5} opacity={Math.min(1, grow * 3)} />
		</svg>
	)
}

function TitleBar() {
	return (
		<div style={{ position: 'absolute', left: 0, right: 0, top: 0, height: TITLE, borderBottom: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', background: '#fafafa' }}>
			<div style={{ display: 'flex', gap: 9, marginLeft: 11 }}>
				{['#ff5f57', '#febc2e', '#28c840'].map((c) => (
					<div key={c} style={{ width: 12, height: 12, borderRadius: 6, background: c, boxShadow: 'inset 0 0 0 0.5px rgba(0,0,0,0.12)' }} />
				))}
			</div>
			<div style={{ marginLeft: 17, fontSize: 13, fontWeight: 650 }}>Disk Usage</div>
		</div>
	)
}

export type DuPage = 'chart' | 'free' | 'review'

function Sidebar({ page, height }: { page: DuPage; height: number }) {
	const heading = (text: string, top: number) => <div style={{ position: 'absolute', left: 16, top, fontSize: 11, fontWeight: 600, color: 'rgba(0,0,0,0.42)' }}>{text}</div>
	const clean: [keyof typeof SYMBOL, string, string][] = [
		['sparkles', 'Free Up Space', 'Up to 12.30 GB to free'],
		['largeFile', 'Large Files', '100 MB and more'],
		['duplicates', 'Duplicates', 'In your home folder'],
		['systemData', 'System Data', "What it's made of"],
	]
	return (
		<div style={{ position: 'absolute', left: 0, top: TITLE, width: SIDE, height: height - TITLE, background: '#efeff1', borderRight: `1px solid ${SEPARATOR}` }}>
			{heading('Disks', 14)}
			<div style={{ position: 'absolute', left: 8, right: 8, top: 36, height: 66, borderRadius: 8, background: page === 'chart' ? 'rgba(0,0,0,0.07)' : 'transparent', padding: '10px 8px' }}>
				<div style={{ fontSize: 13 }}>Macintosh HD</div>
				<div style={{ height: 4, borderRadius: 2, background: 'rgba(0,0,0,0.1)', margin: '7px 0 6px' }}>
					<div style={{ width: '84%', height: '100%', borderRadius: 2, background: ACCENT }} />
				</div>
				<div style={{ fontSize: 11, color: SECONDARY }}>79.05 GB free of 494.38 GB</div>
			</div>
			{heading('Clean Up', 118)}
			{clean.map(([sym, title, sub], i) => {
				const on = (page === 'free' || page === 'review') && i === 0
				return (
					<div
						key={title}
						style={{ position: 'absolute', left: 8, right: 8, top: 138 + i * 50, height: 46, borderRadius: 8, background: on ? 'rgba(0,0,0,0.07)' : 'transparent', display: 'flex', gap: 9, padding: '7px 8px' }}
					>
						<Sym name={sym} size={16} color={ACCENT} style={{ marginTop: 1 }} />
						<div>
							<div style={{ fontSize: 12.5 }}>{title}</div>
							<div style={{ fontSize: 11, color: SECONDARY, marginTop: 2 }}>{sub}</div>
						</div>
					</div>
				)
			})}
		</div>
	)
}

/** Where the sidebar's Free Up Space row is, in the window. */
export const FREE_ROW = { x: 70, y: TITLE + 138 + 23 }

function Chart({ width, height, grow }: { width: number; height: number; grow: number }) {
	const left = SIDE
	const top = TITLE
	const listWidth = 300
	const r = Math.min(232, (height - top - 74 - 40) / 2)
	const cx = left + (width - left - listWidth - 24) / 2
	const cy = top + 44 + (height - top - 44 - 26) / 2
	const fade = (i: number) => Math.min(1, Math.max(0, (grow - 0.25 - i * 0.04) * 4))
	return (
		<>
			<div style={{ position: 'absolute', left, right: 0, top, height: 44, borderBottom: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', padding: '0 12px' }}>
				<div style={{ width: 26, height: 26, borderRadius: 13, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'rgba(0,0,0,0.3)' }}>
					<Sym name="back" size={16} />
				</div>
				<div style={{ marginLeft: 12, fontSize: 15, fontWeight: 650 }}>Macintosh HD</div>
				<div style={{ flex: 1 }} />
				<div style={{ width: 30, height: 30, borderRadius: 15, background: '#fff', boxShadow: '0 0 0 0.5px rgba(0,0,0,0.12), 0 1px 3px rgba(0,0,0,0.08)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
					<Sym name="refresh" size={16} />
				</div>
			</div>
			<div style={{ position: 'absolute', left: cx - r - 2, top: cy - r - 2 }}>
				<Ring r={r} grow={grow} />
			</div>
			<div style={{ position: 'absolute', left: cx - 80, width: 160, top: cy - 30, textAlign: 'center', opacity: fade(3) }}>
				<div style={{ fontSize: 12.5, fontWeight: 650 }}>Macintosh HD</div>
				<div style={{ fontSize: 20, fontWeight: 650, margin: '3px 0 4px' }}>409.77 GB</div>
				<div style={{ fontSize: 11, color: SECONDARY }}>3,912,447 items</div>
			</div>
			<div style={{ position: 'absolute', right: 16, width: listWidth, top: top + 62 }}>
				{TOP.map((t, i) => (
					<div key={t.name} style={{ height: 35.5, opacity: fade(i) }}>
						<div style={{ display: 'flex', alignItems: 'center', fontSize: 13 }}>
							<div style={{ width: 9, height: 9, borderRadius: 5, marginRight: 9, background: i < 7 ? color(HUES[i], 0) : '#c4c4c8' }} />
							<span style={{ flex: 1 }}>{t.name}</span>
							<Sym name="chevronRight" size={10} color="rgba(0,0,0,0.35)" style={{ marginRight: 10 }} />
							<span style={{ color: SECONDARY }}>{t.shown}</span>
						</div>
						<div style={{ height: 3, borderRadius: 2, background: 'rgba(0,0,0,0.07)', margin: '7px 0 0 18px' }}>
							<div style={{ width: `${Math.max(0.6, (t.size / TOP[0].size) * 52)}%`, height: '100%', borderRadius: 2, background: i < 7 ? color(HUES[i], 0) : '#c4c4c8' }} />
						</div>
					</div>
				))}
				<div style={{ display: 'flex', fontSize: 13, color: SECONDARY, opacity: fade(TOP.length), paddingLeft: 18 }}>
					<span style={{ flex: 1 }}>7 smaller items</span>
					<span>2 KB</span>
				</div>
			</div>
			<StatusLine text="From the index in 41 ms" />
		</>
	)
}

function StatusLine({ text }: { text: string }) {
	return (
		<div style={{ position: 'absolute', left: SIDE, right: 0, bottom: 0, height: 26, borderTop: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', padding: '0 13px', fontSize: 11, color: SECONDARY }}>
			{text}
		</div>
	)
}

function Button({ children, primary, dim }: { children: string; primary?: boolean; dim?: boolean }) {
	return (
		<div
			style={{
				height: 26,
				padding: '0 13px',
				borderRadius: 13,
				display: 'flex',
				alignItems: 'center',
				fontSize: 12,
				fontWeight: primary ? 600 : 500,
				background: primary ? ACCENT : '#fff',
				color: primary ? '#fff' : 'rgba(0,0,0,0.86)',
				boxShadow: primary ? 'none' : '0 0 0 0.5px rgba(0,0,0,0.14), 0 1px 2px rgba(0,0,0,0.06)',
				opacity: dim ? 0.45 : 1,
				flexShrink: 0,
			}}
		>
			{children}
		</div>
	)
}

// Free Up Space's cards (cleanup.rs, clean.rs), as freeupspace.webp has them.
const CARDS: { sym: keyof typeof SYMBOL; title: string; about: string; found: string; button?: string }[] = [
	{ sym: 'box', title: 'Installers', about: 'Disk images and packages in Downloads, once their apps are installed', found: 'None' },
	{ sym: 'downloadCircle', title: 'Old Downloads', about: 'In Downloads for over six months, and not opened in that time', found: '5 items · 1.32 GB', button: 'Review' },
	{ sym: 'iphone', title: 'iPhone & iPad Backups', about: "Device backups on this Mac (keep the one you'd restore from)", found: '2 items · 8.70 GB', button: 'Review' },
	{ sym: 'archive', title: 'App Caches', about: 'Apps rebuild these when they need them', found: '4 items · 2.19 GB', button: 'Review' },
	{ sym: 'hammer', title: 'Build Folders', about: "Your projects' builds and packages (Rust's target, node_modules…): made again by a build or an install", found: 'None' },
	{ sym: 'trash', title: 'Trash', about: "Emptied, it's gone for good", found: '83.9 MB', button: 'Empty Trash' },
	{ sym: 'largeFile', title: 'Large Files', about: '100 MB and more: yours to judge', found: '', button: 'Review' },
	{ sym: 'duplicates', title: 'Duplicates', about: 'Identical files in your home folder', found: '', button: 'Look for Copies' },
]
/** Where a card's button is, in the window (the right edge's middle). */
export const cardButton = (i: number, width: number) => ({ x: width - 63, y: TITLE + 86 + i * 70 + 31 })

function FreeUpSpace({ shown }: { shown: number }) {
	return (
		<>
			<div style={{ position: 'absolute', left: SIDE + 17, right: 17, top: TITLE + 22, opacity: shown }}>
				<div style={{ fontSize: 20, fontWeight: 700, height: 26 }}>Up to 12.30 GB can be freed</div>
				<div style={{ fontSize: 12, color: SECONDARY, marginTop: 6, height: 16 }}>What's on this Mac that's safe to let go of, each kind on its own</div>
				<div style={{ marginTop: 16, display: 'flex', flexDirection: 'column', gap: 8 }}>
					{CARDS.map((c, i) => (
						<div
							key={c.title}
							style={{
								height: 62,
								borderRadius: 10,
								background: '#f6f6f8',
								boxShadow: `inset 0 0 0 1px ${SEPARATOR}`,
								display: 'flex',
								alignItems: 'center',
								padding: '0 13px',
								gap: 12,
								opacity: Math.min(1, Math.max(0, shown * 3 - i * 0.2)),
							}}
						>
							<div style={{ width: 34, height: 34, borderRadius: 8, background: 'rgba(10,100,216,0.12)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
								<Sym name={c.sym} size={18} color={ACCENT} />
							</div>
							<div style={{ flex: 1, minWidth: 0 }}>
								<div style={{ fontSize: 13, fontWeight: 650 }}>{c.title}</div>
								<div style={{ fontSize: 11, color: SECONDARY, marginTop: 3, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{c.about}</div>
							</div>
							<div style={{ fontSize: 13, fontWeight: 500, marginRight: c.button ? 2 : 16 }}>{c.found}</div>
							{c.button && <Button>{c.button}</Button>}
						</div>
					))}
				</div>
			</div>
			<StatusLine text="Nothing's deleted until you say so: what you move to the Trash can be put back" />
		</>
	)
}

// Old Downloads' list: 1.32 GB in all.
export const OLD: { name: string; opened: string; mb: number }[] = [
	{ name: 'Holiday video.mov', opened: 'Never opened', mb: 640 },
	{ name: 'Tax forms 2024.zip', opened: 'Opened Jan 14, 2026', mb: 412 },
	{ name: 'Conference slides.pdf', opened: 'Opened Feb 3, 2026', mb: 186 },
	{ name: 'Old wallpapers.zip', opened: 'Never opened', mb: 58 },
	{ name: 'Brochure.pdf', opened: 'Opened Mar 22, 2026', mb: 24 },
]
/** As human_size says it: one decimal for MB, two for GB. */
export const size = (mb: number) => (mb >= 999.5 ? `${(mb / 1000).toFixed(2)} GB` : `${mb.toFixed(1)} MB`)
const REVIEW_TOP = TITLE + 64
/** Where row `i`'s tick box is, in the window. */
export const tickBox = (i: number) => ({ x: SIDE + 26, y: REVIEW_TOP + 10 + i * 34 + 17 })

function Review({ ticked, shown }: { ticked: boolean[]; shown: number }) {
	const n = ticked.filter(Boolean).length
	const mb = OLD.reduce((s, o, i) => s + (ticked[i] ? o.mb : 0), 0)
	return (
		<div style={{ opacity: shown }}>
			<div style={{ position: 'absolute', left: SIDE, right: 0, top: TITLE, height: 64, borderBottom: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', padding: '0 14px', gap: 12 }}>
				<div style={{ width: 26, height: 26, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
					<Sym name="back" size={16} />
				</div>
				<div style={{ flex: 1 }}>
					<div style={{ fontSize: 15, fontWeight: 650 }}>Old Downloads</div>
					<div style={{ fontSize: 11, color: SECONDARY, marginTop: 3 }}>In Downloads for over six months, and not opened in that time</div>
				</div>
				<div style={{ width: 30, height: 30, borderRadius: 15, background: '#fff', boxShadow: '0 0 0 0.5px rgba(0,0,0,0.12), 0 1px 3px rgba(0,0,0,0.08)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
					<Sym name="refresh" size={16} />
				</div>
			</div>
			<div style={{ position: 'absolute', left: SIDE + 10, right: 10, top: REVIEW_TOP + 10 }}>
				{OLD.map((o, i) => (
					<div key={o.name} style={{ height: 34, display: 'flex', alignItems: 'center', gap: 10, padding: '0 8px', fontSize: 13, borderRadius: 7, background: i % 2 === 0 ? 'rgba(0,0,0,0.03)' : 'transparent' }}>
						<div
							style={{
								width: 16,
								height: 16,
								borderRadius: 4,
								background: ticked[i] ? ACCENT : '#fff',
								boxShadow: ticked[i] ? 'none' : 'inset 0 0 0 1.2px rgba(0,0,0,0.28)',
								display: 'flex',
								alignItems: 'center',
								justifyContent: 'center',
							}}
						>
							{ticked[i] && <Sym name="check" size={13} />}
						</div>
						<span style={{ width: 230 }}>{o.name}</span>
						<span style={{ flex: 1, color: SECONDARY }}>~/Downloads</span>
						<span style={{ width: 170, color: SECONDARY }}>{o.opened}</span>
						<span style={{ width: 80, textAlign: 'right', color: SECONDARY }}>{size(o.mb)}</span>
					</div>
				))}
			</div>
			<div style={{ position: 'absolute', left: SIDE, right: 0, bottom: 26, height: 50, borderTop: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', padding: '0 16px', gap: 10, fontSize: 13 }}>
				<div style={{ flex: 1, fontWeight: n ? 600 : 400, color: n ? undefined : SECONDARY }}>{n ? `${n} ticked · ${size(mb)} to free` : 'Tick what to move to the Trash'}</div>
				<Button>{n === OLD.length ? 'Untick All' : 'Tick All'}</Button>
				<Button primary dim={!n}>
					Move to Trash
				</Button>
			</div>
			<StatusLine text="5 items · 1.32 GB · found in 9 ms" />
		</div>
	)
}

export function DiskUsage({
	x,
	y,
	width = 1080,
	height = 700,
	page,
	grow = 1,
	free = 1,
	ticked = [],
	appear = 1,
}: {
	x: number
	y: number
	width?: number
	height?: number
	page: DuPage
	grow?: number
	/** The page coming in (0–1). */
	free?: number
	ticked?: boolean[]
	appear?: number
}) {
	return (
		<Window x={x} y={y} width={width} height={height} radius={14} material="#fff" appear={appear} style={{ backdropFilter: 'none', fontFamily: FONT.ui }}>
			<TitleBar />
			<Sidebar page={page} height={height} />
			{page === 'chart' && <Chart width={width} height={height} grow={grow} />}
			{page === 'free' && <FreeUpSpace shown={free} />}
			{page === 'review' && <Review ticked={ticked} shown={free} />}
		</Window>
	)
}

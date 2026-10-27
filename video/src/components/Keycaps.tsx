// The keys pressed, as keycaps at the bottom centre: each appears as it's
// pressed, goes down, and fades about 0.6 s later. A chord's keys sit
// together; a row of typing builds up left to right, laid out in place from
// the start so nothing shifts as it fills in.
import { interpolate, useCurrentFrame } from 'remotion'
import { FONT, FPS, HEIGHT, INK, PAPER, WIDTH } from '../theme'
import { type Cap, type Timeline } from '../timeline'

const SIZE = 66
const FONT_SIZE = 27
const GAP = 8
// Between one chord or word and the next.
const GROUP_GAP = 16
const HOLD = 0.6 * FPS
const FADE = 0.22 * FPS
// A row cut short by the next one fades this quickly.
const SWAP = 0.12 * FPS
const BOTTOM = 64
const MAX_ROW = 1100

const MODIFIERS = new Set(['⌘', '⇧', '⌥', '⌃'])
const NAMES: Record<string, string> = { ' ': '' }

function capWidth(cap: Cap) {
	if (cap.space) return SIZE * 1.9
	const label = NAMES[cap.label] ?? cap.label
	if ([...label].length <= 1) return SIZE
	return Math.max(SIZE, 34 + label.length * FONT_SIZE * 0.6)
}

type Placed = { cap: Cap; x: number; width: number; end: number }

/** Every cap in a row, placed, and when it goes. */
function layout(t: Timeline) {
	const rows = new Map<number, Placed[]>()
	t.groups.forEach((g, i) => {
		const row = rows.get(g.burst) ?? []
		rows.set(g.burst, row)
		let x = row.length ? row[row.length - 1].x + row[row.length - 1].width : 0
		// Typed letters run together; a chord or a word stands apart.
		const prev = t.groups[i - 1]
		const apart = row.length && (g.chord || prev?.chord || g.caps[0].space || prev?.caps[0].space)
		if (row.length) x += apart ? GROUP_GAP : GAP
		g.caps.forEach((cap, j) => {
			const width = capWidth(cap)
			// A chord's keys all go when its last one does.
			row.push({ cap, x: x, width, end: g.last + HOLD })
			x += width + (j < g.caps.length - 1 ? GAP : 0)
		})
	})
	return [...rows.values()]
}

export function Keycaps({ timeline, from = 0 }: { timeline: Timeline; from?: number }) {
	const frame = useCurrentFrame() + from
	const rows = layout(timeline)

	return (
		<div style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
			{rows.map((row, r) => {
				const width = row[row.length - 1].x + row[row.length - 1].width
				// The next row's first key, which hurries this one away.
				const next = rows[r + 1]?.[0].cap.frame ?? Infinity
				// A long row drifts along with the typing, what's showing of it
				// kept in the middle; a short one is centered whole.
				const left = width > MAX_ROW ? WIDTH / 2 - shownMiddle(row, frame) : (WIDTH - width) / 2
				return row.map((p, i) => <Key key={`${r}-${i}`} placed={p} left={left} frame={frame} next={next} />)
			})}
		</div>
	)
}

/** The middle of what's showing of a row, eased from key to key. */
function shownMiddle(row: Placed[], frame: number) {
	// The right edge: the newest key's, coming in over its first frames.
	let right = row[0].x + row[0].width
	for (let i = 0; i < row.length; i++) {
		const p = row[i]
		if (p.cap.frame > frame) break
		const prev = i > 0 ? row[i - 1].x + row[i - 1].width : p.x + p.width
		right = interpolate(frame - p.cap.frame, [0, 6], [prev, p.x + p.width], { extrapolateRight: 'clamp' })
	}
	// The left edge: the oldest key still showing's, leaving as it fades.
	let left = row[0].x
	for (let i = 0; i < row.length; i++) {
		const p = row[i]
		const next = row[i + 1]
		if (frame < p.end || !next) {
			left = p.x
			break
		}
		if (frame < p.end + FADE) {
			left = interpolate(frame, [p.end, p.end + FADE], [p.x, next.x])
			break
		}
	}
	return (left + right) / 2
}

function Key({ placed, left, frame, next }: { placed: Placed; left: number; frame: number; next: number }) {
	const { cap, x, width } = placed
	const age = frame - cap.frame
	if (age < 0) return null
	const end = Math.min(placed.end, next)
	const fade = placed.end <= next ? FADE : SWAP
	if (frame > end + fade) return null

	const appear = interpolate(age, [0, 5], [0, 1], { extrapolateRight: 'clamp' })
	const out = interpolate(frame, [end, end + fade], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })
	// Down as it's pressed, back up a few frames later.
	const down = interpolate(age, [0, 2, 7, 12], [0, 1, 1, 0], { extrapolateRight: 'clamp' })
	const label = NAMES[cap.label] ?? cap.label
	const modifier = MODIFIERS.has(label)

	return (
		<div
			style={{
				position: 'absolute',
				left: left + x,
				top: HEIGHT - BOTTOM - SIZE,
				width,
				height: SIZE,
				opacity: appear * out,
				transform: `translateY(${down * 4 + (1 - appear) * 6}px) scale(${0.94 + appear * 0.06})`,
			}}
		>
			<div
				style={{
					width: '100%',
					height: '100%',
					borderRadius: 14,
					background: PAPER,
					color: INK,
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'center',
					fontFamily: FONT.mono,
					fontSize: modifier ? FONT_SIZE + 3 : FONT_SIZE,
					fontWeight: 500,
					letterSpacing: '-0.01em',
					// The key's side, shorter while it's down, and its shadow.
					boxShadow: [
						`0 ${5 - down * 4}px 0 #d8d1c1`,
						`0 ${6 - down * 4}px 0 rgba(13, 23, 38, 0.18)`,
						'0 18px 40px rgba(13, 23, 38, 0.32)',
						'inset 0 1px 0 rgba(255, 255, 255, 0.9)',
					].join(', '),
				}}
			>
				{label}
			</div>
		</div>
	)
}

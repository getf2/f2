// A scene's script: what's pressed, typed, moved and clicked, and when. The
// keycaps, the text in a field, the pointer and the sounds are all read off
// the compiled script, so they can't drift apart.
import { Easing, random } from 'remotion'
import { FPS } from './theme'

type At = { at: number; id?: string }

export type Step =
	// Keys pressed together (modifiers first): ['⌘', 'Space']. `edit` changes
	// a field when the last key goes down (⌘A selects all, ⌫ clears, Tab
	// takes the ghost text in).
	| (At & { keys: string[]; field?: string; edit?: 'selectAll' | 'clear' | { append: string } })
	// Typed into a field, a key at a time, in a human rhythm.
	| (At & { type: string; field: string; seed?: string })
	// The pointer, to a point on the screen (in points).
	| (At & { move: [number, number]; dur?: number })
	// A click where the pointer is.
	| (At & { click: true })
	// The button held down (a drag), or let go.
	| (At & { press: 'down' | 'up' })
	// A sound on its own: a window appearing, files dropped.
	| (At & { sound: 'whoosh' | 'pop'; volume?: number })

export type Script = { pointer?: [number, number]; steps: Step[] }

export type Cap = { frame: number; label: string; group: number; space?: boolean }
export type Group = { first: number; last: number; caps: Cap[]; chord: boolean; burst: number }
export type FieldState = { text: string; selected: boolean; frame: number }
type Move = { start: number; end: number; from: [number, number]; to: [number, number] }
export type Cue = { frame: number; src: string; volume: number }

export type Timeline = {
	groups: Group[]
	fields: Record<string, FieldState[]>
	moves: Move[]
	clicks: number[]
	/** Held down from one frame to the other (drags). */
	holds: [number, number][]
	pointer?: [number, number]
	cues: Cue[]
	marks: Record<string, number>
}

const KEY_SOUNDS = ['sfx/key-1.wav', 'sfx/key-2.wav', 'sfx/key-3.wav', 'sfx/key-4.wav', 'sfx/key-5.wav']
// The bigger keys sound lower.
const BIG_KEYS = new Set(['Space', '↩', '⌫', 'esc', 'Tab', ' '])
// A chord's keys go down this far apart, modifiers first.
const CHORD_GAP = 0.07
// Keycaps in a row closer than this belong together; further, a new row.
export const BURST_GAP = 0.45

const frame = (s: number) => Math.round(s * FPS)

export function compile(script: Script, seed = 'f2'): Timeline {
	const groups: Group[] = []
	const fields: Record<string, FieldState[]> = {}
	const moves: Move[] = []
	const clicks: number[] = []
	const holds: [number, number][] = []
	const cues: Cue[] = []
	const marks: Record<string, number> = {}
	let pointer = script.pointer
	let n = 0

	const keySound = (at: number, label: string) => {
		const r = random(`${seed}-key-${n++}`)
		const big = BIG_KEYS.has(label)
		cues.push({
			frame: frame(at),
			src: big ? 'sfx/key-space.wav' : KEY_SOUNDS[Math.floor(r * KEY_SOUNDS.length)],
			volume: 0.55 + random(`${seed}-vol-${n}`) * 0.25,
		})
	}
	const field = (name: string) => (fields[name] ??= [{ text: '', selected: false, frame: 0 }])
	const current = (name: string) => field(name)[field(name).length - 1]

	const steps = [...script.steps].sort((a, b) => a.at - b.at)
	for (const step of steps) {
		if ('keys' in step) {
			const caps = step.keys.map((label, i) => {
				const at = step.at + i * CHORD_GAP
				keySound(at, label)
				return { frame: frame(at), label, group: groups.length }
			})
			const last = caps[caps.length - 1].frame
			groups.push({ first: caps[0].frame, last, caps, chord: caps.length > 1, burst: 0 })
			if (step.id) marks[step.id] = last
			if (step.field && step.edit) {
				const now = current(step.field)
				const edit = step.edit
				field(step.field).push(
					edit === 'selectAll'
						? { text: now.text, selected: true, frame: last }
						: edit === 'clear'
							? { text: '', selected: false, frame: last }
							: { text: now.text + edit.append, selected: false, frame: last },
				)
			}
		} else if ('type' in step) {
			// 60–110 ms between keys, from a seeded random: the same every render.
			let at = step.at
			const chars = [...step.type]
			chars.forEach((ch, i) => {
				if (i > 0) at += 0.06 + random(`${step.seed ?? seed}-${step.type}-${i}`) * 0.05
				keySound(at, ch)
				const f = frame(at)
				groups.push({ first: f, last: f, caps: [{ frame: f, label: ch, group: groups.length, space: ch === ' ' }], chord: false, burst: 0 })
				const now = current(step.field)
				const text = (now.selected ? '' : now.text) + ch
				field(step.field).push({ text, selected: false, frame: f })
			})
			if (step.id) marks[step.id] = frame(at)
		} else if ('move' in step) {
			const from = moves.length ? moves[moves.length - 1].to : (pointer ?? step.move)
			const distance = Math.hypot(step.move[0] - from[0], step.move[1] - from[1])
			const dur = step.dur ?? 0.35 + distance / 1400
			moves.push({ start: frame(step.at), end: frame(step.at + dur), from, to: step.move })
			pointer ??= from
			if (step.id) marks[step.id] = frame(step.at + dur)
		} else if ('click' in step) {
			clicks.push(frame(step.at))
			cues.push({ frame: frame(step.at), src: 'sfx/click.wav', volume: 0.9 })
			if (step.id) marks[step.id] = frame(step.at)
		} else if ('press' in step) {
			const f = frame(step.at)
			if (step.press === 'down') holds.push([f, Infinity])
			else if (holds.length) holds[holds.length - 1][1] = f
			cues.push({ frame: f, src: step.press === 'down' ? 'sfx/press.wav' : 'sfx/release.wav', volume: 0.9 })
			if (step.id) marks[step.id] = f
		} else if ('sound' in step) {
			cues.push({ frame: frame(step.at), src: `sfx/${step.sound}.wav`, volume: step.volume ?? 0.6 })
			if (step.id) marks[step.id] = frame(step.at)
		}
	}

	// Rows of keycaps: a new one when there's been a pause.
	groups.sort((a, b) => a.first - b.first)
	let burst = 0
	groups.forEach((g, i) => {
		if (i > 0 && g.first - groups[i - 1].last > BURST_GAP * FPS) burst++
		g.burst = burst
		g.caps.forEach((c) => (c.group = i))
	})

	return { groups, fields, moves, clicks, holds, pointer, cues, marks }
}

/** A field's text at a frame, whether it's all selected, and when it last changed. */
export function fieldAt(t: Timeline, name: string, f: number): FieldState {
	const states = t.fields[name] ?? [{ text: '', selected: false, frame: 0 }]
	let state = states[0]
	for (const s of states) if (s.frame <= f) state = s
	return state
}

/** Where a mark falls (a key's action, a move's end, a click). */
export function mark(t: Timeline, id: string): number {
	const f = t.marks[id]
	if (f === undefined) throw new Error(`No mark "${id}"`)
	return f
}

const glide = Easing.bezier(0.45, 0, 0.2, 1)

/** The pointer at a frame: where it is, and how far it's pressed (0–1). */
export function pointerAt(t: Timeline, f: number): { x: number; y: number; press: number } | null {
	if (!t.pointer) return null
	let [x, y] = t.pointer
	for (const m of t.moves) {
		if (f >= m.end) [x, y] = m.to
		else if (f > m.start) {
			const p = glide((f - m.start) / (m.end - m.start))
			// A hand's move bows a little.
			const [dx, dy] = [m.to[0] - m.from[0], m.to[1] - m.from[1]]
			const bow = Math.sin(Math.PI * p) * 0.08
			x = m.from[0] + dx * p - dy * bow
			y = m.from[1] + dy * p + dx * bow
			break
		}
	}
	let press = 0
	for (const c of t.clicks) {
		const d = f - c
		if (d >= -2 && d < 0) press = Math.max(press, (d + 2) / 2)
		else if (d >= 0 && d < 5) press = 1
		else if (d >= 5 && d < 10) press = Math.max(press, 1 - (d - 5) / 5)
	}
	for (const [down, up] of t.holds) {
		if (f >= down - 2 && f < up) press = Math.max(press, Math.min(1, (f - down + 2) / 2))
		else if (f >= up && f < up + 5) press = Math.max(press, 1 - (f - up) / 5)
	}
	return { x, y, press }
}

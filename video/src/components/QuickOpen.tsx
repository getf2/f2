// Quick Open, as src/quick_panel.rs draws it: 680 pt wide, the search field
// (22 pt text), the results under headings (40 pt rows, the picked one
// gray), and the footer with what Return does and ⌘K's Actions. Its list
// stops at 440 pt; more is cut off, as it scrolls.
import { type ReactNode } from 'react'
import { useCurrentFrame } from 'remotion'
import { FPS, SCREEN } from '../theme'
import { AppIcon } from './icons'
import { Window } from './Window'

export const QO_WIDTH = 680
export const QO_TOP = 196
export const PLACEHOLDER = 'Search for apps, commands and files…'

const SECONDARY = 'rgba(0, 0, 0, 0.52)'
const SEPARATOR = 'rgba(0, 0, 0, 0.1)'

export type Row = {
	icon: ReactNode
	title: string
	subtitle?: string
	kind: string
	/** Two lines: this small, over the title (a translation's languages). */
	over?: string
	/** Still being worked out ("Translating…"): the title in gray. */
	pending?: boolean
}
export type Section = { heading: string; rows: Row[] }

export function QuickOpen({
	query,
	sections,
	selected = 0,
	primary,
	appear = 1,
	typedAt = 0,
	selectedText = false,
	ghost,
	top = QO_TOP,
}: {
	query: string
	sections: Section[]
	/** The picked row, counting from the first. */
	selected?: number
	/** What Return does: "Open File", "Copy Answer"… */
	primary: string
	appear?: number
	/** When the field last changed (frames): the caret holds still while typing. */
	typedAt?: number
	/** The field's text all selected (⌘A). */
	selectedText?: boolean
	/** The rest of what's being typed, in gray, for Tab to take. */
	ghost?: string
	top?: number
}) {
	const frame = useCurrentFrame()
	// macOS's caret: steady while typing, then blinking.
	const since = frame - typedAt
	const caret = since < 0.5 * FPS || Math.floor((since - 0.5 * FPS) / (0.5 * FPS)) % 2 === 1
	let index = 0

	return (
		<Window x={(SCREEN.width - QO_WIDTH) / 2} y={top} width={QO_WIDTH} appear={appear}>
			{/* The field. */}
			<div style={{ height: 56, display: 'flex', alignItems: 'center', padding: '0 18px', fontSize: 22 }}>
				{query ? (
					<span style={{ background: selectedText ? 'rgba(0, 122, 255, 0.24)' : 'none', borderRadius: 2, whiteSpace: 'pre' }}>{query}</span>
				) : null}
				<span style={{ width: 2, height: 26, marginLeft: 1, background: caret && !selectedText ? 'rgba(0,0,0,0.85)' : 'transparent', borderRadius: 1 }} />
				{!query && <span style={{ marginLeft: -3, color: 'rgba(0, 0, 0, 0.36)' }}>{PLACEHOLDER}</span>}
				{query && ghost && <span style={{ marginLeft: -3, color: 'rgba(0, 0, 0, 0.36)', whiteSpace: 'pre' }}>{ghost}</span>}
			</div>
			<div style={{ height: 1, background: SEPARATOR }} />
			{/* The results. */}
			<div style={{ maxHeight: 440 + 8, overflow: 'hidden', padding: '4px 0' }}>
				{sections.map((s) => (
					<div key={s.heading}>
						<div style={{ height: 30, padding: '0 18px 8px', display: 'flex', alignItems: 'flex-end', fontSize: 12, fontWeight: 500, color: SECONDARY }}>
							{s.heading}
						</div>
						{s.rows.map((row) => {
							const on = index++ === selected
							return <ResultRow key={index} row={row} on={on} />
						})}
					</div>
				))}
			</div>
			<Footer primary={primary} />
		</Window>
	)
}

function ResultRow({ row, on }: { row: Row; on: boolean }) {
	const tall = row.over !== undefined
	return (
		<div style={{ padding: '0 8px' }}>
			<div
				style={{
					height: tall ? 60 : 40,
					padding: '0 11.5px 0 10px',
					borderRadius: 8,
					display: 'flex',
					alignItems: 'center',
					gap: 10,
					background: on ? 'rgba(0, 0, 0, 0.087)' : 'transparent',
				}}
			>
				<div style={{ width: 20, height: 20, flexShrink: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>{row.icon}</div>
				{tall ? (
					<div style={{ flex: 1, minWidth: 0 }}>
						<div style={{ fontSize: 12, color: SECONDARY }}>{row.over}</div>
						<div style={{ fontSize: 15, marginTop: 2, color: row.pending ? SECONDARY : undefined }}>{row.title}</div>
					</div>
				) : (
					<div style={{ flex: 1, minWidth: 0, display: 'flex', alignItems: 'baseline', gap: 8, whiteSpace: 'nowrap' }}>
						<span style={{ fontSize: 14, letterSpacing: '0.01em', flexShrink: 0, maxWidth: 380, overflow: 'hidden', textOverflow: 'ellipsis' }}>{row.title}</span>
						<span style={{ fontSize: 13, color: SECONDARY, overflow: 'hidden', textOverflow: 'ellipsis' }}>{row.subtitle}</span>
					</div>
				)}
				<div style={{ fontSize: 13, color: SECONDARY, flexShrink: 0, alignSelf: tall ? 'flex-start' : 'center', marginTop: tall ? 9 : 0 }}>
					{row.kind}
				</div>
			</div>
		</div>
	)
}

function Caps({ keys }: { keys: string[] }) {
	return (
		<div style={{ display: 'flex', gap: 3 }}>
			{keys.map((k) => (
				<div
					key={k}
					style={{
						height: 20,
						minWidth: 20,
						padding: '0 5px',
						borderRadius: 5,
						background: 'rgba(0, 0, 0, 0.087)',
						display: 'flex',
						alignItems: 'center',
						justifyContent: 'center',
						fontSize: 11,
						color: SECONDARY,
					}}
				>
					{k}
				</div>
			))}
		</div>
	)
}

function Footer({ primary }: { primary: string }) {
	return (
		<div
			style={{
				height: 40,
				padding: '2px 11px 0 18px',
				display: 'flex',
				alignItems: 'center',
				gap: 8,
				borderTop: `1px solid ${SEPARATOR}`,
				fontSize: 12,
			}}
		>
			<div style={{ opacity: 0.85, display: 'flex' }}>
				<AppIcon size={16} />
			</div>
			<div style={{ flex: 1 }} />
			<div style={{ fontWeight: 500 }}>{primary}</div>
			<Caps keys={['↩']} />
			<div style={{ margin: '0 4px', width: 1, height: 14, background: SEPARATOR }} />
			<div style={{ color: SECONDARY }}>Actions</div>
			<Caps keys={['⌘', 'K']} />
		</div>
	)
}

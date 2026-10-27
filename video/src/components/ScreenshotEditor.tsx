// The screenshot editor (src/shots/editor.rs): its toolbar (the tools, the
// eight colors, three widths, Undo and Redo; Background, Copy, Save and
// Done), and the shot on the canvas with what's drawn on it.
import { type ReactNode } from 'react'
import { FONT } from '../theme'
import { Window } from './Window'

const SEPARATOR = 'rgba(0, 0, 0, 0.1)'
const ACCENT = '#0a64d8'
export const EDITOR_TITLE = 28
export const EDITOR_TOOLBAR = 48
export const EDITOR_MARGIN = 32
const COLORS = ['#ff3b30', '#ff9500', '#ffcc00', '#34c759', '#007aff', '#af52de', '#000000', '#ffffff']

const line = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.6, strokeLinecap: 'round', strokeLinejoin: 'round' } as const
// After the SF Symbols each tool is drawn with (marks.rs: Tool::about).
const TOOLS: ReactNode[] = [
	<path key="select" d="M7 4v14l3.6-3.4 2.4 5.4 2-.9-2.4-5.3H17z" {...line} />,
	<path key="arrow" d="M6 18 18 6M10 6h8v8" {...line} />,
	<path key="line" d="M6 18 18 6" {...line} />,
	<rect key="rect" x="4.5" y="6.5" width="15" height="11" rx="1.5" {...line} />,
	<circle key="ellipse" cx="12" cy="12" r="7.5" {...line} />,
	<path key="pen" d="M4 15c2-4 4-8 6-6s-3 7 0 7 4-8 6-7 1 5 4 5" {...line} />,
	<path key="highlight" d="M8 16l-3 3h5l1-1M8 16l7-9 3 3-8 7zM14 8l3 3" {...line} />,
	<path key="text" d="M5 7V5h9v2M9.5 5v14M8 19h3M15 11v-1h5v1M17.5 10v9M16.5 19h2" {...line} />,
	<g key="step">
		<circle cx="12" cy="12" r="8" {...line} />
		<path d="M10.6 9.4 12.4 8v8" {...line} />
	</g>,
	<g key="blur" fill="currentColor">
		{[0, 1, 2].flatMap((r) => [0, 1, 2].map((c) => <rect key={`${r}${c}`} x={5 + c * 5} y={5 + r * 5} width="4" height="4" rx="0.8" opacity={(r + c) % 2 ? 0.5 : 1} />))}
	</g>,
	<path key="crop" d="M7 3v14h14M3 7h14v14" {...line} />,
]

function IconButton({ on, children, dim }: { on?: boolean; children: ReactNode; dim?: boolean }) {
	return (
		<div
			style={{
				width: 28,
				height: 28,
				borderRadius: 7,
				display: 'flex',
				alignItems: 'center',
				justifyContent: 'center',
				background: on ? ACCENT : 'transparent',
				color: on ? '#fff' : 'rgba(0,0,0,0.82)',
				opacity: dim ? 0.4 : 1,
				flexShrink: 0,
			}}
		>
			<svg width="16" height="16" viewBox="0 0 24 24">
				{children}
			</svg>
		</div>
	)
}

const divider = <div style={{ width: 1, height: 20, margin: '0 4px', background: SEPARATOR, flexShrink: 0 }} />

export function ScreenshotEditor({
	x,
	y,
	width,
	height,
	title,
	appear = 1,
	drawn,
	children,
}: {
	x: number
	y: number
	width: number
	height: number
	title: string
	appear?: number
	/** Undo has something to undo. */
	drawn?: boolean
	/** The shot. */
	children: ReactNode
}) {
	return (
		<Window x={x} y={y} width={width} height={height} radius={14} material="#ececee" appear={appear} style={{ backdropFilter: 'none', fontFamily: FONT.ui }}>
			<div style={{ height: EDITOR_TITLE, display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative', background: '#fafafa', fontSize: 13, fontWeight: 650 }}>
				<div style={{ position: 'absolute', left: 10, display: 'flex', gap: 8 }}>
					{['#ff5f57', '#febc2e', '#28c840'].map((c) => (
						<div key={c} style={{ width: 12, height: 12, borderRadius: 6, background: c, boxShadow: 'inset 0 0 0 0.5px rgba(0,0,0,0.12)' }} />
					))}
				</div>
				{title}
			</div>
			<div style={{ height: EDITOR_TOOLBAR, display: 'flex', alignItems: 'center', gap: 2, padding: '0 12px', background: '#fafafa', borderBottom: `1px solid ${SEPARATOR}` }}>
				{TOOLS.map((t, i) => (
					<IconButton key={i} on={i === 1}>
						{t}
					</IconButton>
				))}
				{divider}
				{COLORS.map((c, i) => (
					<div key={c} style={{ width: 22, height: 22, borderRadius: 11, display: 'flex', alignItems: 'center', justifyContent: 'center', boxShadow: i === 0 ? `inset 0 0 0 2px ${ACCENT}` : 'none', flexShrink: 0 }}>
						<div style={{ width: 16, height: 16, borderRadius: 8, background: c, boxShadow: 'inset 0 0 0 1px rgba(0,0,0,0.2)' }} />
					</div>
				))}
				{divider}
				{[3, 5, 8].map((w) => (
					<div key={w} style={{ width: 26, height: 26, borderRadius: 6, display: 'flex', alignItems: 'center', justifyContent: 'center', background: w === 5 ? 'rgba(0,0,0,0.07)' : 'transparent', flexShrink: 0 }}>
						<div style={{ width: 4 + w, height: 4 + w, borderRadius: '50%', background: 'rgba(0,0,0,0.85)' }} />
					</div>
				))}
				{divider}
				<IconButton dim={!drawn}>
					<path d="M9 7 5 11l4 4M5 11h9a5 5 0 0 1 0 10h-2" {...line} />
				</IconButton>
				<IconButton dim>
					<path d="M15 7l4 4-4 4M19 11h-9a5 5 0 0 0 0 10h2" {...line} />
				</IconButton>
				<div style={{ flex: 1 }} />
				<IconButton>
					<rect x="6" y="7" width="13" height="10" rx="1.5" {...line} transform="rotate(-8 12 12)" />
					<rect x="4" y="9" width="13" height="10" rx="1.5" {...line} fill="#fafafa" />
				</IconButton>
				<IconButton>
					<rect x="8" y="8" width="11" height="12" rx="2" {...line} />
					<path d="M5 15V6a2 2 0 0 1 2-2h8" {...line} />
				</IconButton>
				<IconButton>
					<path d="M12 4v11M8 11l4 4 4-4M5 14v4a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4" {...line} />
				</IconButton>
				<div style={{ marginLeft: 4, height: 28, padding: '0 12px', borderRadius: 7, background: ACCENT, color: '#fff', fontSize: 13, fontWeight: 500, display: 'flex', alignItems: 'center' }}>Done</div>
			</div>
			<div style={{ position: 'absolute', left: 0, right: 0, top: EDITOR_TITLE + EDITOR_TOOLBAR, bottom: 0, overflow: 'hidden' }}>{children}</div>
		</Window>
	)
}

/** An arrow as marks.rs draws one: a round-capped shaft, a filled head. */
export function Arrow({ from, to, width = 5, color = '#ff3b30' }: { from: [number, number]; to: [number, number]; width?: number; color?: string }) {
	const [dx, dy] = [to[0] - from[0], to[1] - from[1]]
	const length = Math.hypot(dx, dy)
	if (length < 2) return null
	const head = Math.min(width * 3.6 + 8, length * 0.6)
	const [ux, uy] = [dx / length, dy / length]
	const base: [number, number] = [to[0] - ux * head, to[1] - uy * head]
	const half = head * 0.55
	const shaftEnd: [number, number] = [base[0] + ux * head * 0.25, base[1] + uy * head * 0.25]
	return (
		<svg style={{ position: 'absolute', inset: 0, overflow: 'visible', width: 1, height: 1 }}>
			<line x1={from[0]} y1={from[1]} x2={shaftEnd[0]} y2={shaftEnd[1]} stroke={color} strokeWidth={width} strokeLinecap="round" />
			<path d={`M${to[0]} ${to[1]}L${base[0] - uy * half} ${base[1] + ux * half}L${base[0] + uy * half} ${base[1] - ux * half}Z`} fill={color} />
		</svg>
	)
}

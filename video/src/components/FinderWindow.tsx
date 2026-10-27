// An F2 window, as hero-dual.webp and search-photos.webp show it (sizes in
// points, read off those 2× shots): the floating sidebar, a pane's toolbar
// (back and forward, the title, the views, Group, Share, Tags, More and
// Search), the search bar while searching, the list (20 pt rows, striped)
// or icons (156 pt apart), and the status bar. One pane, or two.
import { type ReactNode } from 'react'
import { BigIcon, type Big } from './bigIcons'
import { Sym, type SYMBOL } from './symbols'
import { Window } from './Window'

const SECONDARY = 'rgba(0, 0, 0, 0.5)'
const TERTIARY = 'rgba(0, 0, 0, 0.32)'
const SEPARATOR = 'rgba(0, 0, 0, 0.09)'
const ACCENT = '#0a64d8'
export const SIDEBAR = 196
const TOOLBAR = 52
const SCOPE = 32
const HEADER = 27.5
export const ROW = 20
const STATUS = 22.5
export const CELL = { x: 155.7, y: 160.3, first: 86, top: 77.5, label: 76.3 }

export type ListRow = {
	icon: ReactNode
	name: string
	date?: string
	size?: string
	kind?: string
	location?: string
	selected?: boolean
	folder?: boolean
	open?: boolean
	indent?: number
	opacity?: number
	/** The accent laid over it (0–1): something dropped on it. */
	pulse?: number
}
export type IconItem = {
	icon: Big
	name: string
	selected?: boolean
	opacity?: number
	/** Moved from its place (points), and sized: flying into a folder, say. */
	dx?: number
	dy?: number
	scale?: number
	pulse?: number
	/** Its place in the grid, if not its turn in the list. */
	at?: number
}
export type Pane = {
	title: string
	view: 'list' | 'icons'
	rows?: ListRow[]
	icons?: IconItem[]
	/** List columns after Name; a narrow pane leaves out the last ones. */
	columns?: ('date' | 'size' | 'kind' | 'location')[]
	status: string
	/** Said in the status bar for a while, over what's there. */
	notice?: string
	/** The search field: open (0–1), what's typed, and the bar under it. */
	search?: { open: number; query: string; caret: boolean; scope?: string; timing?: string }
	/** This pane has the keyboard: its selection in the accent color. */
	active?: boolean
}

const COLUMNS = {
	date: { title: 'Date Modified', width: 170 },
	size: { title: 'Size', width: 80 },
	kind: { title: 'Kind', width: 150 },
	location: { title: 'Location', width: 219 },
}

export function FinderWindow({
	x,
	y,
	width,
	height,
	sidebar = 'Documents',
	panes,
	split = panes.length > 1 ? 1 : 0,
	appear = 1,
}: {
	x: number
	y: number
	width: number
	height: number
	sidebar?: string
	panes: Pane[]
	/** How far the second pane's come in (0–1). */
	split?: number
	appear?: number
}) {
	const content = width - SIDEBAR
	const leftWidth = split > 0 ? content - (content / 2) * split : content
	return (
		<Window x={x} y={y} width={width} height={height} radius={22} material="#fff" appear={appear} style={{ backdropFilter: 'none' }}>
			<Sidebar selected={sidebar} height={height} />
			<PaneView pane={panes[0]} x={SIDEBAR} width={leftWidth} height={height} />
			{split > 0 && panes[1] && (
				<>
					<div style={{ position: 'absolute', left: SIDEBAR + leftWidth, top: 0, bottom: 0, width: 1, background: SEPARATOR }} />
					<div style={{ opacity: Math.min(1, split * 1.5) }}>
						<PaneView pane={panes[1]} x={SIDEBAR + leftWidth} width={width - SIDEBAR - leftWidth} height={height} />
					</div>
				</>
			)}
		</Window>
	)
}

const FAVORITES: [string, keyof typeof SYMBOL][] = [
	['Recents', 'recents'],
	['AirDrop', 'airdrop'],
	['Desktop', 'desktop'],
	['Documents', 'documents'],
	['Downloads', 'downloads'],
	['Photos', 'folder'],
	['Applications', 'applications'],
	['Trash', 'trash'],
]
const TAGS: [string, string][] = [
	['Red', '#ff3b30'],
	['Orange', '#ff9500'],
	['Yellow', '#ffcc00'],
	['Green', '#34c759'],
	['Blue', '#007aff'],
	['Purple', '#af52de'],
	['Gray', '#8e8e93'],
]

function Sidebar({ selected, height }: { selected: string; height: number }) {
	const heading = (text: string, top: number) => (
		<div style={{ position: 'absolute', left: 16.5, top: top - 7, fontSize: 11, fontWeight: 600, color: 'rgba(0,0,0,0.42)' }}>{text}</div>
	)
	const item = (label: string, top: number, icon: ReactNode) => (
		<div
			key={label}
			style={{
				position: 'absolute',
				left: 8.3,
				right: 8.3,
				top: top - 13.6,
				height: 27.3,
				borderRadius: 8,
				background: label === selected ? 'rgba(0, 0, 0, 0.075)' : 'transparent',
				display: 'flex',
				alignItems: 'center',
				gap: 9,
				paddingLeft: 8.5,
				fontSize: 13,
			}}
		>
			{icon}
			{label}
		</div>
	)
	return (
		<div
			style={{
				position: 'absolute',
				left: 8.5,
				top: 8.5,
				width: 179,
				height: height - 17,
				borderRadius: 18,
				background: 'rgba(243, 243, 245, 0.96)',
				boxShadow: '0 0 0 0.5px rgba(0,0,0,0.1), 0 1px 3px rgba(0,0,0,0.05)',
				overflow: 'hidden',
			}}
		>
			<TrafficLights />
			{heading('Favorites', 67)}
			{FAVORITES.map(([label, sym], i) => item(label, 94.6 + i * 28, <Sym name={sym} size={17} color="rgba(0,0,0,0.62)" />))}
			{heading('Locations', 325.5)}
			{item('Macintosh HD', 352.6, <Sym name="disk" size={17} color="rgba(0,0,0,0.62)" />)}
			{heading('Tags', 387)}
			{TAGS.map(([label, color], i) =>
				item(label, 414.6 + i * 28, <span style={{ width: 10, height: 10, margin: '0 3.5px', borderRadius: 5, background: color }} />),
			)}
		</div>
	)
}

function TrafficLights() {
	return (
		<div style={{ position: 'absolute', left: 13.2, top: 11, display: 'flex', gap: 9.1 }}>
			{['#ff5f57', '#febc2e', '#28c840'].map((c) => (
				<div key={c} style={{ width: 14, height: 14, borderRadius: 7, background: c, boxShadow: 'inset 0 0 0 0.5px rgba(0,0,0,0.12)' }} />
			))}
		</div>
	)
}

/** A toolbar control: a white capsule of glass. */
function Glass({ width, children, style }: { width: number; children: ReactNode; style?: React.CSSProperties }) {
	return (
		<div
			style={{
				width,
				height: 33,
				borderRadius: 16.5,
				background: '#fff',
				boxShadow: '0 0 0 0.5px rgba(0,0,0,0.11), 0 1px 4px rgba(0,0,0,0.07)',
				display: 'flex',
				alignItems: 'center',
				justifyContent: 'center',
				flexShrink: 0,
				color: 'rgba(0,0,0,0.78)',
				...style,
			}}
		>
			{children}
		</div>
	)
}

function Toolbar({ pane, width }: { pane: Pane; width: number }) {
	const searching = pane.search && pane.search.query.length > 0
	const title = searching ? `Searching “${pane.search!.query}”` : pane.title
	const open = pane.search?.open ?? 0
	const fieldWidth = 36 + (218 - 36) * open
	// What's typed scrolls to its end in the field.
	const typed = pane.search?.query ?? ''
	const room = fieldWidth - 96
	const over = Math.max(0, typed.length * 6.85 - room)
	return (
		<div style={{ position: 'absolute', left: 0, top: 0, width, height: TOOLBAR, display: 'flex', alignItems: 'center', padding: '0 12.5px 0 5px', gap: 10.5 }}>
			<Glass width={77} style={{ justifyContent: 'space-around', padding: '0 4px' }}>
				<Sym name="back" size={19} />
				<div style={{ width: 1, height: 18, background: SEPARATOR }} />
				<Sym name="forward" size={19} color="rgba(0,0,0,0.3)" />
			</Glass>
			<div style={{ flex: 1, minWidth: 0, fontSize: 15.6, fontWeight: 650, letterSpacing: '-0.005em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', marginLeft: 6 }}>{title}</div>
			<Glass width={148} style={{ justifyContent: 'space-between', padding: '0 4px' }}>
				{(['icons', 'list', 'columns', 'gallery'] as const).map((v) => (
					<div
						key={v}
						style={{
							width: 34,
							height: 25,
							borderRadius: 12.5,
							display: 'flex',
							alignItems: 'center',
							justifyContent: 'center',
							background: (v === 'icons' ? pane.view === 'icons' : v === 'list' && pane.view === 'list') ? 'rgba(0,0,0,0.075)' : 'transparent',
						}}
					>
						<Sym name={v} size={18} />
					</div>
				))}
			</Glass>
			<Glass width={36}>
				<Sym name="group" size={18} />
			</Glass>
			<Glass width={36}>
				<Sym name="share" size={18} />
			</Glass>
			<Glass width={36}>
				<Sym name="tag" size={18} />
			</Glass>
			<Glass width={36}>
				<Sym name="more" size={18} />
			</Glass>
			<Glass width={fieldWidth} style={{ justifyContent: open > 0.5 ? 'flex-start' : 'center', padding: open > 0.5 ? '0 10px 0 13px' : 0, overflow: 'hidden' }}>
				<Sym name="search" size={17} />
				{open > 0.5 && (
					<>
						<div style={{ flex: 1, minWidth: 0, overflow: 'hidden', margin: '0 6px 0 9px', fontSize: 13.5, display: 'flex', alignItems: 'center', opacity: (open - 0.5) * 2 }}>
							<span style={{ whiteSpace: 'pre', transform: `translateX(${-over}px)` }}>{typed}</span>
							<span style={{ width: 1.5, height: 17, marginLeft: 1, flexShrink: 0, background: pane.search?.caret ? '#000' : 'transparent', transform: `translateX(${-over}px)` }} />
						</div>
						{typed && <Sym name="xmark" size={13} color="rgba(0,0,0,0.6)" style={{ marginRight: 12 }} />}
						<Sym name="help" size={17} color="rgba(0,0,0,0.45)" />
					</>
				)}
			</Glass>
		</div>
	)
}

function ScopeBar({ pane }: { pane: Pane }) {
	const s = pane.search!
	const pill = (label: string, on: boolean) => (
		<div style={{ height: 22, padding: '0 8px', borderRadius: 6, display: 'flex', alignItems: 'center', background: on ? 'rgba(0,0,0,0.065)' : 'transparent', color: on ? '#000' : SECONDARY }}>
			{label}
		</div>
	)
	const chip = (label: string) => (
		<div style={{ height: 22, padding: '0 6px 0 8px', borderRadius: 11, display: 'flex', alignItems: 'center', gap: 4, boxShadow: `inset 0 0 0 1px ${SEPARATOR}`, color: SECONDARY }}>
			{label}
			<Sym name="chevronDown" size={9} />
		</div>
	)
	const divider = <div style={{ width: 1, height: 14, margin: '0 8px', background: SEPARATOR }} />
	return (
		<div
			style={{
				position: 'absolute',
				left: 0,
				right: 0,
				top: TOOLBAR - 1,
				height: SCOPE,
				display: 'flex',
				alignItems: 'center',
				gap: 4,
				padding: '0 12px 0 16px',
				fontSize: 12,
				borderBottom: `1px solid ${SEPARATOR}`,
			}}
		>
			<div style={{ marginRight: 4, color: SECONDARY }}>Search:</div>
			{pill('This Mac', s.scope === 'mac')}
			{pill(`“${pane.title}”`, s.scope !== 'mac')}
			{divider}
			{pill('Name', true)}
			{pill('Contents', false)}
			{divider}
			<div style={{ display: 'flex', gap: 6 }}>
				{chip('Kind')}
				{chip('Date')}
				{chip('Size')}
				{chip('Where')}
			</div>
			<div style={{ flex: 1 }} />
			<div style={{ fontSize: 11, color: TERTIARY, whiteSpace: 'nowrap' }}>{s.timing}</div>
			<Glass width={52} style={{ height: 22, borderRadius: 11, marginLeft: 8, fontSize: 11 }}>
				Save
			</Glass>
		</div>
	)
}

function PaneView({ pane, x, width, height }: { pane: Pane; x: number; width: number; height: number }) {
	const searching = !!pane.search && pane.search.open >= 1
	const top = TOOLBAR + (searching ? SCOPE : 0)
	return (
		<div style={{ position: 'absolute', left: x, top: 0, width, height, overflow: 'hidden' }}>
			<Toolbar pane={pane} width={width} />
			{searching && <ScopeBar pane={pane} />}
			{pane.view === 'list' ? <List pane={pane} top={top} width={width} /> : <Icons pane={pane} top={top} width={width} />}
			<div
				style={{
					position: 'absolute',
					left: 0,
					right: 0,
					bottom: 0,
					height: STATUS,
					borderTop: `1px solid ${SEPARATOR}`,
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'center',
					fontSize: 11,
					color: pane.notice ? 'rgba(0,0,0,0.85)' : SECONDARY,
				}}
			>
				{pane.notice ?? pane.status}
			</div>
		</div>
	)
}

function List({ pane, top, width }: { pane: Pane; top: number; width: number }) {
	// The columns that fit, Name taking the rest.
	const wanted = pane.columns ?? ['date', 'size', 'kind', 'location']
	const cols: (keyof typeof COLUMNS)[] = []
	let used = 0
	for (const c of wanted) {
		if (width - 16 - used - COLUMNS[c].width < 300) break
		cols.push(c)
		used += COLUMNS[c].width
	}
	// The last column takes what's left over.
	const nameWidth = width - 12 - used
	const header = (
		<div style={{ position: 'absolute', left: 0, right: 0, top, height: HEADER, borderBottom: `1px solid ${SEPARATOR}`, display: 'flex', fontSize: 12, color: SECONDARY }}>
			<div style={{ width: nameWidth, paddingLeft: 50, display: 'flex', alignItems: 'center' }}>Name</div>
			{cols.map((c) => (
				<div key={c} style={{ width: COLUMNS[c].width, display: 'flex', alignItems: 'center', paddingLeft: 8.5, borderLeft: `1px solid ${SEPARATOR}`, margin: '5px 0' }}>
					{COLUMNS[c].title}
				</div>
			))}
		</div>
	)
	const selColor = pane.active === false ? 'rgba(0,0,0,0.1)' : ACCENT
	return (
		<>
			{header}
			<div style={{ position: 'absolute', left: 8, right: 8, top: top + HEADER + 1 }}>
				{(pane.rows ?? []).map((r, i) => {
					const on = r.selected
					const white = on && pane.active !== false
					return (
						<div
							key={`${r.name}-${i}`}
							style={{
								position: 'relative',
								height: ROW,
								borderRadius: 6,
								display: 'flex',
								alignItems: 'center',
								fontSize: 13,
								background: on ? selColor : i % 2 === 0 ? 'rgba(0,0,0,0.035)' : 'transparent',
								color: white ? '#fff' : 'rgba(0,0,0,0.86)',
								opacity: r.opacity ?? 1,
							}}
						>
							{r.pulse ? <div style={{ position: 'absolute', inset: 0, borderRadius: 6, background: ACCENT, opacity: 0.45 * r.pulse }} /> : null}
							<div style={{ width: nameWidth - 8, display: 'flex', alignItems: 'center', paddingLeft: 4 + (r.indent ?? 0) * 18, whiteSpace: 'nowrap' }}>
								<div style={{ width: 17, display: 'flex', justifyContent: 'center', color: white ? '#fff' : 'rgba(0,0,0,0.45)' }}>
									{r.folder && <Sym name={r.open ? 'chevronDown' : 'chevronRight'} size={11} />}
								</div>
								<div style={{ width: 16, height: 16, margin: '0 6.4px 0 4.6px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>{r.icon}</div>
								{r.name}
							</div>
							{cols.map((c) => (
								<div
									key={c}
									style={{
										width: COLUMNS[c].width,
										padding: '0 8.5px',
										textAlign: c === 'size' ? 'right' : 'left',
										color: white ? '#fff' : 'rgba(0,0,0,0.5)',
										whiteSpace: 'nowrap',
										overflow: 'hidden',
									}}
								>
									{r[c]}
								</div>
							))}
						</div>
					)
				})}
			</div>
		</>
	)
}

function Icons({ pane, top, width }: { pane: Pane; top: number; width: number }) {
	const perRow = Math.max(1, Math.floor((width - 20) / CELL.x))
	return (
		<div style={{ position: 'absolute', left: 0, top, width }}>
			{(pane.icons ?? []).map((item, i) => {
				const at = item.at ?? i
				const cx = CELL.first + (at % perRow) * CELL.x + (item.dx ?? 0)
				const cy = CELL.top + Math.floor(at / perRow) * CELL.y + (item.dy ?? 0)
				const active = pane.active !== false
				return (
					<div
						key={`${item.name}-${i}`}
						style={{
							position: 'absolute',
							left: cx - 70,
							top: cy - 62,
							width: 140,
							opacity: item.opacity ?? 1,
							transform: `scale(${item.scale ?? 1})`,
							transformOrigin: '70px 62px',
							display: 'flex',
							flexDirection: 'column',
							alignItems: 'center',
						}}
					>
						<div style={{ position: 'relative', width: 123, height: 122, borderRadius: 10, background: item.selected ? 'rgba(0,0,0,0.065)' : 'transparent', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
							<BigIcon icon={item.icon} />
							{item.pulse ? <div style={{ position: 'absolute', inset: 0, borderRadius: 10, background: ACCENT, opacity: 0.45 * item.pulse }} /> : null}
						</div>
						<div
							style={{
								position: 'absolute',
								left: 0,
								right: 0,
								top: 62 + CELL.label,
								transform: 'translateY(-50%)',
								display: 'flex',
								justifyContent: 'center',
							}}
						>
							<span
								style={{
									fontSize: 13,
									lineHeight: '19.5px',
									textAlign: 'center',
									padding: '0 5px',
									borderRadius: 4,
									maxWidth: 140,
									background: item.selected ? (active ? ACCENT : 'rgba(0,0,0,0.1)') : 'transparent',
									color: item.selected && active ? '#fff' : 'rgba(0,0,0,0.86)',
								}}
							>
								{item.name}
							</span>
						</div>
					</div>
				)
			})}
		</div>
	)
}

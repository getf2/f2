// A macOS (Tahoe) menu: a menu bar title's, or a right-click's. Translucent,
// round-cornered, 13 pt items, the one under the pointer in the accent.
import { FONT } from '../theme'

export type MenuItem = { title: string; keys?: string; disabled?: boolean; submenu?: boolean } | '-'

export const MENU_ROW = 24
const SEP = 11
const PAD = 5

/** Where item `i`'s middle is, from the menu's top. */
export function menuItemY(items: MenuItem[], i: number) {
	let y = PAD
	for (let k = 0; k < i; k++) y += items[k] === '-' ? SEP : MENU_ROW
	return y + MENU_ROW / 2
}

export function Menu({ x, y, width = 230, items, highlighted, appear = 1 }: { x: number; y: number; width?: number; items: MenuItem[]; highlighted?: number; appear?: number }) {
	if (appear <= 0) return null
	return (
		<div
			style={{
				position: 'absolute',
				left: x,
				top: y,
				width,
				padding: PAD,
				borderRadius: 12,
				fontFamily: FONT.ui,
				fontSize: 13,
				color: 'rgba(0,0,0,0.86)',
				background: 'rgba(242, 242, 245, 0.86)',
				backdropFilter: 'blur(30px) saturate(1.6)',
				boxShadow: '0 0 0 0.5px rgba(0,0,0,0.2), inset 0 0 0 0.5px rgba(255,255,255,0.6), 0 12px 36px rgba(0,0,0,0.22), 0 2px 6px rgba(0,0,0,0.1)',
				opacity: appear,
				zIndex: 600,
			}}
		>
			{items.map((item, i) =>
				item === '-' ? (
					<div key={i} style={{ height: SEP, display: 'flex', alignItems: 'center', padding: '0 10px' }}>
						<div style={{ flex: 1, height: 1, background: 'rgba(0,0,0,0.1)' }} />
					</div>
				) : (
					<div
						key={i}
						style={{
							height: MENU_ROW,
							borderRadius: 7,
							display: 'flex',
							alignItems: 'center',
							padding: '0 10px 0 22px',
							background: highlighted === i ? '#0a64d8' : 'transparent',
							color: highlighted === i ? '#fff' : item.disabled ? 'rgba(0,0,0,0.26)' : undefined,
						}}
					>
						<span style={{ flex: 1 }}>{item.title}</span>
						{item.keys && <span style={{ color: highlighted === i ? 'rgba(255,255,255,0.85)' : 'rgba(0,0,0,0.42)', letterSpacing: '0.04em' }}>{item.keys}</span>}
						{item.submenu && (
							<svg width="7" height="11" viewBox="0 0 7 11" style={{ opacity: highlighted === i ? 0.9 : 0.5 }}>
								<path d="M1 1l4.5 4.5L1 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
							</svg>
						)}
					</div>
				),
			)}
		</div>
	)
}

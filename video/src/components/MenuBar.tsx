// macOS Tahoe's menu bar: clear over the wallpaper, white text. F2's menus
// (src/menus.rs) on the left; on the right F2's own item (an F2 tile, as
// src/macos/status.m draws it), Wi-Fi, the battery, Control Center and the
// clock.
import { FONT } from '../theme'

export const MENU_BAR_HEIGHT = 25

// Where the Window menu's title is (points), for the pointer and its menu.
export const WINDOW_MENU = { x: 270, left: 237.5 }

export function MenuBar({ app = 'F2', menus = ['File', 'Edit', 'View', 'Go', 'Window', 'Help'], clock = 'Tue Oct 20  9:41', open }: { app?: string; menus?: string[]; clock?: string; open?: string }) {
	return (
		<div
			style={{
				position: 'absolute',
				top: 0,
				left: 0,
				right: 0,
				height: MENU_BAR_HEIGHT,
				display: 'flex',
				alignItems: 'center',
				padding: '0 10px 0 12px',
				fontFamily: FONT.ui,
				fontSize: 13,
				color: '#fff',
				textShadow: '0 0 6px rgba(0, 0, 0, 0.18), 0 0.5px 1px rgba(0, 0, 0, 0.2)',
				// Tahoe's clear bar keeps a faint shade so it reads on any wallpaper.
				background: 'linear-gradient(rgba(10, 25, 70, 0.16), rgba(10, 25, 70, 0))',
				zIndex: 500,
			}}
		>
			<span style={{ fontSize: 15, padding: '0 10px 0 6px', marginTop: -1 }}>{''}</span>
			<span style={{ fontWeight: 700, padding: '0 9px' }}>{app}</span>
			{menus.map((m) => (
				<span key={m} style={{ padding: '2px 9px', fontWeight: 500, borderRadius: 6, background: m === open ? 'rgba(255,255,255,0.24)' : 'transparent' }}>
					{m}
				</span>
			))}
			<div style={{ flex: 1 }} />
			<div style={{ display: 'flex', alignItems: 'center', gap: 16, filter: 'drop-shadow(0 0 4px rgba(0,0,0,0.15))' }}>
				<StatusF2 />
				<Wifi />
				<Battery />
				<ControlCenter />
				<span style={{ fontWeight: 500, whiteSpace: 'pre', marginLeft: 2 }}>{clock}</span>
			</div>
		</div>
	)
}

function StatusF2() {
	return (
		<svg width="20" height="16" viewBox="0 0 20 16">
			<defs>
				<mask id="f2-cut">
					<rect x="1.5" y="1" width="17" height="14" rx="3.5" fill="#fff" />
					<text x="10" y="11.6" textAnchor="middle" fontSize="9.5" fontWeight="900" letterSpacing="-0.2" fill="#000" fontFamily="-apple-system, system-ui">
						F2
					</text>
				</mask>
			</defs>
			<rect x="0" y="0" width="20" height="16" fill="#fff" mask="url(#f2-cut)" />
		</svg>
	)
}

function Wifi() {
	return (
		<svg width="17" height="13" viewBox="0 0 17 13" fill="#fff">
			<path d="M8.5 1.2c2.9 0 5.6 1.1 7.6 3l-1.3 1.4A9.2 9.2 0 0 0 8.5 3a9.2 9.2 0 0 0-6.3 2.6L.9 4.2c2-1.9 4.7-3 7.6-3z" />
			<path d="M8.5 4.9c1.9 0 3.6.7 4.9 1.9l-1.3 1.4a5.2 5.2 0 0 0-3.6-1.4c-1.4 0-2.7.5-3.6 1.4L3.6 6.8c1.3-1.2 3-1.9 4.9-1.9z" />
			<path d="M8.5 8.6c.9 0 1.7.3 2.3.9L8.5 12 6.2 9.5c.6-.6 1.4-.9 2.3-.9z" />
		</svg>
	)
}

function Battery() {
	return (
		<svg width="27" height="13" viewBox="0 0 27 13">
			<rect x="0.6" y="0.6" width="22.8" height="11.8" rx="3.4" fill="none" stroke="rgba(255,255,255,0.55)" strokeWidth="1.1" />
			<rect x="2.3" y="2.3" width="15.6" height="8.4" rx="1.9" fill="#fff" />
			<path d="M24.8 4.4c.9.3 1.4 1.1 1.4 2.1s-.5 1.8-1.4 2.1z" fill="rgba(255,255,255,0.55)" />
		</svg>
	)
}

function ControlCenter() {
	return (
		<svg width="16" height="14" viewBox="0 0 16 14">
			<rect x="0.7" y="0.7" width="14.6" height="5.4" rx="2.7" fill="none" stroke="#fff" strokeWidth="1.3" />
			<circle cx="12.6" cy="3.4" r="1.6" fill="#fff" />
			<rect x="0.7" y="7.9" width="14.6" height="5.4" rx="2.7" fill="none" stroke="#fff" strokeWidth="1.3" />
			<circle cx="3.4" cy="10.6" r="1.6" fill="#fff" />
		</svg>
	)
}

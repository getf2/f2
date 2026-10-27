// Icons as F2 shows them: macOS's folders and documents, photos as
// thumbnails, and Quick Open's colored tiles with a white symbol (lit a
// little from above, as System Settings draws its panes').
import { type ReactNode } from 'react'
import { Img, staticFile } from 'remotion'

// The glyphs macOS engraves on its own folders.
const GLYPHS = {
	apps: 'M8.4 14.6l2.6-5 M11.6 14.6l-2.6-5 M7.6 12.9h4.8',
	documents: 'M8.4 8.8h2.4l1.2 1.2v4.6H8.4z M9.4 11.4h1.6 M9.4 12.8h1.6',
	downloads: 'M10 8.6v5 M8 11.8l2 2 2-2 M7.6 14.8h4.8',
	desktop: 'M7.4 9h5.2v3.8H7.4z M9 14.6h2 M10 12.8v1.8',
}

export function Folder({ size = 20, glyph }: { size?: number; glyph?: keyof typeof GLYPHS }) {
	return (
		<svg width={size} height={size} viewBox="0 0 20 20">
			<defs>
				<linearGradient id="folder-front" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0" stopColor="#86d0f8" />
					<stop offset="1" stopColor="#5db6f1" />
				</linearGradient>
			</defs>
			<path d="M1.2 4.4c0-.8.6-1.4 1.4-1.4h4.3c.4 0 .7.1 1 .4l1.1 1.1h8.4c.8 0 1.4.6 1.4 1.4v9.8H1.2z" fill="#4ea6e8" />
			<rect x="1.2" y="5.6" width="17.6" height="11.6" rx="1.8" fill="url(#folder-front)" />
			<rect x="1.2" y="5.6" width="17.6" height="0.6" rx="0.3" fill="#b5e3fb" opacity="0.8" />
			{glyph && (
				<g fill="none" stroke="#3f8fd0" strokeWidth="0.75" strokeLinecap="round" strokeLinejoin="round" opacity="0.75">
					<path d={GLYPHS[glyph]} />
				</g>
			)}
		</svg>
	)
}

/** A document: a page with its corner turned, and a colored label. */
export function Doc({ size = 20, label, color }: { size?: number; label: string; color: string }) {
	return (
		<svg width={size} height={size} viewBox="0 0 20 20">
			<path d="M4 1.4h8.2l4 4v13.2H4z" fill="#fff" stroke="rgba(0,0,0,0.18)" strokeWidth="0.5" />
			<path d="M12.2 1.4v4h4" fill="#eceef1" stroke="rgba(0,0,0,0.18)" strokeWidth="0.5" />
			<g stroke="#c9ccd2" strokeWidth="0.6">
				<path d="M6 7.6h6M6 9.2h8M6 10.8h7.2M6 12.4h8" />
			</g>
			<rect x="5.4" y="14.2" width="9.2" height="3.3" rx="0.8" fill={color} />
			<text x="10" y="16.6" textAnchor="middle" fontSize="2.6" fontWeight="700" fill="#fff" fontFamily="-apple-system, system-ui">
				{label}
			</text>
		</svg>
	)
}

/** A photo's thumbnail: a crop of one of the site's paintings. */
export function Thumb({ size = 20, paint, at = 'center' }: { size?: number; paint: string; at?: string }) {
	return (
		<div style={{ width: size, height: size, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
			<Img
				src={staticFile(`paint/${paint}.webp`)}
				style={{
					width: size,
					height: size * 0.75,
					objectFit: 'cover',
					objectPosition: at,
					borderRadius: size * 0.08,
					boxShadow: '0 0 0 0.5px rgba(0,0,0,0.25)',
				}}
			/>
		</div>
	)
}

export function AppIcon({ size = 16 }: { size?: number }) {
	return <Img src={staticFile('icon.png')} style={{ width: size, height: size }} />
}

// Drawn on the panel, F2's tiles come out softer than their colors.
const muted = (hex: string) => {
	const n = parseInt(hex.slice(1), 16)
	const mix = (c: number, g: number) => Math.round(c * 0.78 + g * 0.22)
	return `#${[mix(n >> 16, 196), mix((n >> 8) & 255, 196), mix(n & 255, 200)].map((c) => c.toString(16).padStart(2, '0')).join('')}`
}

const lit = (hex: string) => {
	const n = parseInt(hex.slice(1), 16)
	const mix = (c: number) => Math.round(c + (255 - c) * 0.16)
	return `rgb(${mix(n >> 16)}, ${mix((n >> 8) & 255)}, ${mix(n & 255)})`
}

/** A white symbol on a tile of `color`. */
export function Tile({ size = 20, color, symbol }: { size?: number; color: string; symbol: keyof typeof SYMBOLS }) {
	return (
		<div
			style={{
				width: size,
				height: size,
				borderRadius: size * 0.25,
				background: `linear-gradient(${lit(muted(color))}, ${muted(color)})`,
				boxShadow: 'inset 0 0 0 0.5px rgba(0,0,0,0.12)',
				display: 'flex',
				alignItems: 'center',
				justifyContent: 'center',
				flexShrink: 0,
			}}
		>
			<svg width={size * 0.66} height={size * 0.66} viewBox="0 0 24 24">
				{SYMBOLS[symbol]}
			</svg>
		</div>
	)
}

const stroke = { fill: 'none', stroke: '#fff', strokeWidth: 2.4, strokeLinecap: 'round', strokeLinejoin: 'round' } as const

// Drawn after SF Symbols' shapes: lock.fill, moon.zzz.fill, arrow.clockwise,
// power, globe, equal, clock.fill, translate.
export const SYMBOLS = {
	lock: (
		<>
			<path d="M7.5 10.5V7.8a4.5 4.5 0 0 1 9 0v2.7" {...stroke} />
			<rect x="5" y="10" width="14" height="10.5" rx="2.2" fill="#fff" />
		</>
	),
	sleep: (
		<>
			<path d="M15.5 17.6A7.6 7.6 0 0 1 8.2 6.2a7.6 7.6 0 1 0 9.6 10.4 7.6 7.6 0 0 1-2.3 1z" fill="#fff" />
			<path d="M13.6 3.4h3.2l-3.2 3.8h3.2 M18 7.8h2.4L18 10.6h2.4" {...stroke} strokeWidth={1.4} />
		</>
	),
	restart: (
		<>
			<path d="M18.2 13.2a6.4 6.4 0 1 1-2.1-6" {...stroke} />
			<path d="M17.6 3.6v4.4h-4.4" {...stroke} />
		</>
	),
	power: (
		<>
			<path d="M8 6.6a7 7 0 1 0 8 0" {...stroke} />
			<path d="M12 3.6v7.6" {...stroke} />
		</>
	),
	globe: (
		<>
			<circle cx="12" cy="12" r="8.4" {...stroke} strokeWidth={1.8} />
			<ellipse cx="12" cy="12" rx="3.6" ry="8.4" {...stroke} strokeWidth={1.6} />
			<path d="M3.8 12h16.4M5.4 7.4h13.2M5.4 16.6h13.2" {...stroke} strokeWidth={1.4} />
		</>
	),
	equal: (
		<>
			<path d="M6 9.2h12M6 14.8h12" {...stroke} strokeWidth={2.8} />
		</>
	),
	clock: (
		<>
			<circle cx="12" cy="12" r="8.6" fill="#fff" />
			<path d="M12 7v5.4l3.4 2" fill="none" stroke="#34aadc" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
		</>
	),
	translate: (
		<>
			<rect x="2.6" y="3.4" width="11" height="11" rx="2.4" fill="#fff" />
			<text x="8.1" y="12.2" textAnchor="middle" fontSize="8.4" fontWeight="700" fill="#007aff" fontFamily="-apple-system, system-ui">
				文
			</text>
			<rect x="10.4" y="9.6" width="11" height="11" rx="2.4" fill="#fff" stroke="#007aff" strokeWidth="1" />
			<text x="15.9" y="18.3" textAnchor="middle" fontSize="8.4" fontWeight="700" fill="#007aff" fontFamily="-apple-system, system-ui">
				A
			</text>
		</>
	),
	clipboard: (
		<>
			<rect x="7" y="6.5" width="11" height="14" rx="2" {...stroke} strokeWidth={2} />
			<path d="M10 4.5h5v3h-5z" fill="#fff" />
			<path d="M10 12h5M10 15.5h5" {...stroke} strokeWidth={1.6} />
		</>
	),
	bottomHalf: (
		<>
			<rect x="4" y="5" width="16" height="14" rx="2.4" {...stroke} strokeWidth={1.8} />
			<rect x="4" y="12" width="16" height="7" rx="2" fill="#fff" />
		</>
	),
	bottomLeft: (
		<>
			<rect x="4" y="5" width="16" height="14" rx="2.4" {...stroke} strokeWidth={1.8} />
			<rect x="6" y="12.5" width="6" height="4.5" rx="1" fill="#fff" />
		</>
	),
	bottomRight: (
		<>
			<rect x="4" y="5" width="16" height="14" rx="2.4" {...stroke} strokeWidth={1.8} />
			<rect x="12" y="12.5" width="6" height="4.5" rx="1" fill="#fff" />
		</>
	),
} satisfies Record<string, ReactNode>

// Icon view's icons, at 104 pt: macOS's folder, Quick Look's thumbnails of
// photos (crops of the site's paintings) and documents, and the generic
// icons of disk images, zips, videos and sound.
import { Img, staticFile } from 'remotion'

export type Big =
	| { kind: 'folder' }
	| { kind: 'photo'; paint: string; at?: string; tall?: boolean }
	| { kind: 'page'; band?: string; title?: string }
	| { kind: 'doc'; glyph: 'download' | 'zip' | 'video'; label?: string }
	| { kind: 'audio' }
	| { kind: 'package' }

const SIZE = 104

export function BigIcon({ icon, size = SIZE }: { icon: Big; size?: number }) {
	const s = size / SIZE
	const box = { width: size, height: size, display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative' } as const
	switch (icon.kind) {
		case 'folder':
			return (
				<div style={box}>
					<svg width={size} height={size} viewBox="0 0 104 104" style={{ filter: 'drop-shadow(0 1.5px 2px rgba(0,0,0,0.18))' }}>
						<defs>
							<linearGradient id="big-folder" x1="0" y1="0" x2="0" y2="1">
								<stop offset="0" stopColor="#7fcbf6" />
								<stop offset="1" stopColor="#4fb0ef" />
							</linearGradient>
						</defs>
						<path d="M2 24c0-3.3 2.7-6 6-6h24c1.6 0 3.1.6 4.2 1.8L41 24h55c3.3 0 6 2.7 6 6v56H2z" fill="#4aa4e6" />
						<rect x="2" y="31" width="100" height="58" rx="6" fill="url(#big-folder)" />
						<rect x="2" y="31" width="100" height="2" rx="1" fill="#b3e2fb" />
					</svg>
				</div>
			)
		case 'photo':
			return (
				<div style={box}>
					<Img
						src={staticFile(`paint/${icon.paint}.webp`)}
						style={{
							width: icon.tall ? size * 0.62 : size * 1.08,
							height: icon.tall ? size * 0.82 : size * 0.68,
							objectFit: 'cover',
							objectPosition: icon.at ?? 'center',
							boxShadow: '0 0 0 0.5px rgba(0,0,0,0.2), 0 1px 2px rgba(0,0,0,0.15)',
						}}
					/>
				</div>
			)
		case 'page':
			return (
				<div style={box}>
					<div
						style={{
							width: size * 0.62,
							height: size * 0.86,
							background: '#fff',
							boxShadow: '0 0 0 0.5px rgba(0,0,0,0.2), 0 1px 2px rgba(0,0,0,0.12)',
							padding: `${5 * s}px ${6 * s}px`,
							display: 'flex',
							flexDirection: 'column',
							gap: 3.2 * s,
						}}
					>
						{icon.band && (
							<div style={{ height: 15 * s, margin: `${-5 * s}px ${-6 * s}px ${3 * s}px`, background: icon.band, color: '#fff', fontSize: 4.2 * s, fontWeight: 700, padding: `${4 * s}px ${4 * s}px` }}>
								{icon.title}
							</div>
						)}
						{!icon.band && icon.title && <div style={{ fontSize: 4.4 * s, fontWeight: 700, color: '#1f3b8f' }}>{icon.title}</div>}
						{Array.from({ length: icon.band ? 12 : 16 }, (_, i) => (
							<div key={i} style={{ height: 1.1 * s, background: '#c9ccd2', width: `${[96, 88, 92, 70, 94, 84, 90, 60, 92, 86, 78, 94, 66, 88, 92, 72][i]}%` }} />
						))}
					</div>
				</div>
			)
		case 'doc':
			return (
				<div style={box}>
					<svg width={size} height={size} viewBox="0 0 104 104" style={{ filter: 'drop-shadow(0 1.5px 2px rgba(0,0,0,0.2))' }}>
						<path d="M22 8h42l20 20v68H22z" fill="#f7f7f8" stroke="#d7d8dc" strokeWidth="1" />
						<path d="M64 8v20h20" fill="#e6e7ea" stroke="#d7d8dc" strokeWidth="1" />
						{icon.glyph === 'download' && (
							<g>
								<rect x="35" y="34" width="34" height="34" rx="4" fill="#e8e9ec" stroke="#b9bcc3" strokeWidth="1.4" />
								<path d="M52 41v17M45 51l7 7 7-7" fill="none" stroke="#8c9099" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
								<path d="M38 64h28" stroke="#b9bcc3" strokeWidth="1.4" />
							</g>
						)}
						{icon.glyph === 'zip' && (
							<g fill="#9a9ea6">
								<rect x="49" y="26" width="6" height="12" rx="3" />
								{[42, 48, 54, 60].map((y) => (
									<rect key={y} x={y % 12 === 0 ? 48 : 52} y={y} width="4" height="3" rx="1" />
								))}
							</g>
						)}
						{icon.glyph === 'video' && (
							<g fill="none" stroke="#a5a8b0" strokeWidth="3">
								<rect x="35" y="44" width="24" height="18" rx="4" />
								<path d="M59 50l10-6v18l-10-6" strokeLinejoin="round" />
							</g>
						)}
						{icon.label && (
							<text x="53" y="88" textAnchor="middle" fontSize="11" fill="#a0a3aa" fontFamily="-apple-system, system-ui" fontWeight="500">
								{icon.label}
							</text>
						)}
					</svg>
				</div>
			)
		case 'audio':
			return (
				<div style={box}>
					<div style={{ width: size * 0.84, height: size * 0.84, background: '#ececee', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
						<svg width={size * 0.5} height={size * 0.5} viewBox="0 0 24 24">
							<path d="M9 17.5V6.2l10-2.2v11.2" fill="none" stroke="#c4c5ca" strokeWidth="2.2" />
							<ellipse cx="6.6" cy="17.6" rx="2.8" ry="2.2" fill="#c4c5ca" />
							<ellipse cx="16.6" cy="15.4" rx="2.8" ry="2.2" fill="#c4c5ca" />
						</svg>
					</div>
				</div>
			)
		case 'package':
			return (
				<div style={box}>
					<svg width={size} height={size} viewBox="0 0 104 104" style={{ filter: 'drop-shadow(0 2px 2px rgba(0,0,0,0.2))' }}>
						<path d="M16 34 52 22l36 12-36 12z" fill="#f2d04a" />
						<path d="M16 34v38l36 14V46z" fill="#c98a45" />
						<path d="M88 34v38L52 86V46z" fill="#a8692f" />
						<path d="M16 34l-8 14 36 13 8-15zM88 34l8 14-36 13-8-15z" fill="#d9a05a" />
						<path d="M30 29l36 12" stroke="#e7b93a" strokeWidth="3" />
					</svg>
				</div>
			)
	}
}

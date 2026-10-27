// Quick Open's clipboard history, as clipboard.webp shows it: the list,
// Copied Lately, beside the whole of the one picked, and what it is.
import { Img, staticFile } from 'remotion'
import { SCREEN } from '../theme'
import { AppIcon, Thumb, Tile } from './icons'
import { QO_TOP } from './QuickOpen'
import { Window } from './Window'

const SECONDARY = 'rgba(0, 0, 0, 0.52)'
const SEPARATOR = 'rgba(0, 0, 0, 0.1)'
const WIDTH = 680
const LIST = 302

type Clip = { kind: 'text' | 'link' | 'image' | 'files'; title: string; ago: string; full?: string; info: [string, string][] }

// The clipboard.webp's own, newest first.
export const CLIPS: Clip[] = [
	{
		kind: 'text',
		title: 'Ferry leaves at 18:40 from pier 2. Bring a jumper!',
		ago: '10 min ago',
		full: 'Ferry leaves at 18:40 from pier 2. Bring a jumper!',
		info: [['From', 'Messages'], ['Type', 'Text'], ['Copied', '10 min ago'], ['Characters', '50'], ['Words', '10']],
	},
	{ kind: 'link', title: 'https://www.bbcgoodfood.com/recipes/pancakes', ago: '12 min ago', full: 'https://www.bbcgoodfood.com/recipes/pancakes', info: [['From', 'Safari'], ['Type', 'Link'], ['Copied', '12 min ago']] },
	{ kind: 'image', title: 'Image (900×502)', ago: '16 min ago', info: [['From', 'Safari'], ['Type', 'Image'], ['Copied', '16 min ago'], ['Size', '900 × 502']] },
	{ kind: 'text', title: 'Flat 3B, 14 Harbour Street', ago: '24 min ago', info: [] },
	{ kind: 'files', title: 'Wildflowers.jpg', ago: '34 min ago', info: [] },
	{ kind: 'text', title: 'Dentist on Thursday at 10:30', ago: '49 min ago', info: [] },
	{ kind: 'text', title: 'Happy birthday, Gran! Lots of love', ago: '1 h ago', info: [] },
	{ kind: 'link', title: 'https://maps.apple.com/?q=harbour', ago: '1 h ago', info: [] },
	{ kind: 'files', title: '3 Files', ago: '2 h ago', info: [] },
	{ kind: 'text', title: 'Gate code 4417', ago: 'Yesterday', info: [] },
]

const textIcon = (
	<svg width="20" height="20" viewBox="0 0 20 20">
		<rect width="20" height="20" rx="5" fill="#5fd27a" />
		<path d="M5 6.5h10M5 9.5h10M5 12.5h10M5 15.5h6" stroke="#fff" strokeWidth="1.4" strokeLinecap="round" />
	</svg>
)

function icon(c: Clip) {
	if (c.kind === 'text') return textIcon
	if (c.kind === 'link') return <Tile color="#0a84ff" symbol="globe" />
	if (c.kind === 'image') return <Thumb paint="hero-valley" at="center 30%" />
	return <Thumb paint={c.title === '3 Files' ? 'golden-hour' : 'meadow'} at="center 80%" />
}

export function ClipboardPanel({ selected, appear = 1 }: { selected: number; appear?: number }) {
	const c = CLIPS[selected]
	return (
		<Window x={(SCREEN.width - WIDTH) / 2} y={QO_TOP} width={WIDTH} height={548} appear={appear}>
			<div style={{ height: 56, display: 'flex', alignItems: 'center', padding: '0 12px 0 18px', gap: 12, borderBottom: `1px solid ${SEPARATOR}` }}>
				<div style={{ width: 28, height: 28, borderRadius: 7, background: 'rgba(0,0,0,0.08)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
					<svg width="14" height="14" viewBox="0 0 24 24">
						<path d="M20 12H5M11 5l-7 7 7 7" fill="none" stroke="rgba(0,0,0,0.75)" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
					</svg>
				</div>
				<span style={{ width: 2, height: 24, background: 'rgba(0,0,0,0.85)', marginRight: -10 }} />
				<div style={{ flex: 1, fontSize: 20, color: 'rgba(0,0,0,0.36)' }}>Search what's been copied…</div>
				<div style={{ fontSize: 12, color: SECONDARY }}>Clipboard History</div>
				<div style={{ height: 20, padding: '0 5px', borderRadius: 5, background: 'rgba(0,0,0,0.087)', fontSize: 11, color: SECONDARY, display: 'flex', alignItems: 'center' }}>esc</div>
			</div>
			<div style={{ display: 'flex', height: 548 - 56 - 41 }}>
				<div style={{ width: LIST, padding: '4px 0', borderRight: `1px solid ${SEPARATOR}` }}>
					<div style={{ height: 30, padding: '0 18px 8px', display: 'flex', alignItems: 'flex-end', fontSize: 12, fontWeight: 500, color: SECONDARY }}>Copied Lately</div>
					{CLIPS.map((clip, i) => (
						<div key={i} style={{ padding: '0 8px' }}>
							<div style={{ height: 40, borderRadius: 8, padding: '0 10px', display: 'flex', alignItems: 'center', gap: 10, background: i === selected ? 'rgba(0,0,0,0.087)' : 'transparent' }}>
								<div style={{ width: 20, height: 20, flexShrink: 0, display: 'flex' }}>{icon(clip)}</div>
								<div style={{ flex: 1, minWidth: 0, fontSize: 14, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{clip.title}</div>
								<div style={{ fontSize: 13, color: SECONDARY, flexShrink: 0 }}>{clip.ago}</div>
							</div>
						</div>
					))}
				</div>
				<div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
					<div style={{ flex: 1, padding: '14px 13px', overflow: 'hidden' }}>
						{c.kind === 'text' && <div style={{ fontSize: 13 }}>{c.full}</div>}
						{c.kind === 'link' && <div style={{ fontSize: 14, color: '#0a64d8', wordBreak: 'break-all' }}>{c.full}</div>}
						{c.kind === 'image' && (
							<Img src={staticFile('paint/hero-valley.webp')} style={{ width: '100%', height: 200, objectFit: 'cover', objectPosition: 'center 30%', borderRadius: 4 }} />
						)}
					</div>
					<div style={{ borderTop: `1px solid ${SEPARATOR}`, padding: '8px 13px 6px', fontSize: 12 }}>
						{c.info.map(([k, v]) => (
							<div key={k} style={{ height: 23, display: 'flex', alignItems: 'center' }}>
								<span style={{ flex: 1, color: SECONDARY }}>{k}</span>
								<span>{v}</span>
							</div>
						))}
					</div>
				</div>
			</div>
			<div style={{ height: 41, borderTop: `1px solid ${SEPARATOR}`, display: 'flex', alignItems: 'center', gap: 8, padding: '2px 11px 0 18px', fontSize: 12 }}>
				<div style={{ opacity: 0.85, display: 'flex' }}>
					<AppIcon size={16} />
				</div>
				<div style={{ flex: 1 }} />
				<div style={{ fontWeight: 500 }}>Copy to Clipboard</div>
				<Key k="↩" />
				<div style={{ margin: '0 4px', width: 1, height: 14, background: SEPARATOR }} />
				<div style={{ color: SECONDARY }}>Actions</div>
				<div style={{ display: 'flex', gap: 3 }}>
					<Key k="⌘" />
					<Key k="K" />
				</div>
			</div>
		</Window>
	)
}

function Key({ k }: { k: string }) {
	return (
		<div style={{ height: 20, minWidth: 20, padding: '0 5px', borderRadius: 5, background: 'rgba(0,0,0,0.087)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 11, color: SECONDARY }}>
			{k}
		</div>
	)
}

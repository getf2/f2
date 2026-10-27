// For matching the screenshots: a component drawn at 2×, as they were
// taken, with the screenshot over it (half see-through, or as a difference,
// where what lines up goes dark). Only for Remotion Studio.
import { type ReactNode } from 'react'
import { AbsoluteFill, Img, staticFile } from 'remotion'
import { FinderWindow } from './components/FinderWindow'
import { Folder, Tile } from './components/icons'
import { DOCUMENTS, PHOTOS, photoIcon } from './samples'
import { QO_TOP, QO_WIDTH, QuickOpen, type Section } from './components/QuickOpen'
import { useFonts } from './fonts'
import { SCREEN } from './theme'

export type CheckProps = { mode: 'mine' | 'overlay' | 'difference' }

function Compare({ shot, mode, children }: { shot: string; mode: CheckProps['mode']; children: ReactNode }) {
	useFonts()
	return (
		<AbsoluteFill style={{ background: '#fff' }}>
			{children}
			{mode !== 'mine' && (
				<Img
					src={staticFile(`ref/${shot}.webp`)}
					style={{ position: 'absolute', inset: 0, opacity: mode === 'overlay' ? 0.5 : 1, mixBlendMode: mode === 'difference' ? 'difference' : 'normal' }}
				/>
			)}
		</AbsoluteFill>
	)
}

const studio = (title: string, glyph?: 'apps') => ({
	icon: <Folder glyph={glyph} />,
	title,
	subtitle: title === 'Applications' ? '/' : '/Volumes/Studio',
	kind: 'Folder',
})

// quickopen-empty.webp's own list.
const SHOT: Section[] = [
	{ heading: 'Favorites', rows: [studio('Projects'), studio('Design'), studio('Photos'), studio('Documents'), studio('Downloads'), studio('Applications', 'apps')] },
	{
		heading: 'Commands',
		rows: [
			{ icon: <Tile color="#ff453a" symbol="lock" />, title: 'Lock Screen', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="sleep" />, title: 'Sleep', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="restart" />, title: 'Restart', subtitle: 'System', kind: 'Command' },
			{ icon: <Tile color="#ff453a" symbol="power" />, title: 'Shut Down', subtitle: 'System', kind: 'Command' },
		],
	},
]

export function CheckQuickOpen({ mode }: CheckProps) {
	return (
		<Compare shot="quickopen-empty" mode={mode}>
			<div style={{ position: 'absolute', transformOrigin: '0 0', transform: `scale(2) translate(${-(SCREEN.width - QO_WIDTH) / 2}px, ${-QO_TOP}px)` }}>
				<QuickOpen query="" sections={SHOT} primary="Open Folder" typedAt={-1000} />
			</div>
		</Compare>
	)
}

export function CheckFinder({ mode }: CheckProps) {
	const rows = DOCUMENTS.map((r) => ({ ...r, selected: r.name === 'Travel' }))
	return (
		<Compare shot="hero-dual" mode={mode}>
			<div style={{ position: 'absolute', transformOrigin: '0 0', transform: 'scale(2)' }}>
				<FinderWindow
					x={0}
					y={0}
					width={1400}
					height={840}
					panes={[
						{ title: 'Documents', view: 'list', rows, columns: ['date', 'size'], status: '11 items, 88.53 GB available · listed in 0.3 ms' },
						{
							title: 'Photos',
							view: 'icons',
							icons: [{ icon: { kind: 'folder' }, name: 'Holiday 2026' }, ...PHOTOS.map((p) => ({ icon: photoIcon(p.name), name: p.name }))],
							status: '11 items, 88.53 GB available · listed in 0.4 ms',
						},
					]}
				/>
			</div>
		</Compare>
	)
}

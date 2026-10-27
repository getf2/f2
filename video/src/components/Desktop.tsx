// The Mac's screen: a painting for the wallpaper (alive, and with its
// sound), the menu bar, whatever's open, and the pointer on top. Laid out
// in points on a 1536×864 screen and drawn 1.25× (see theme). `camera`
// eases in on a point for a closer look.
import { type ReactNode } from 'react'
import { AbsoluteFill } from 'remotion'
import { Ambience } from '../living/Ambience'
import { LivingPainting } from '../living/LivingPainting'
import { SCALE, SCREEN } from '../theme'
import { type Timeline } from '../timeline'
import { MenuBar } from './MenuBar'
import { Pointer } from './Pointer'

export function Desktop({
	wallpaper = 'hero-valley',
	at = 'center 30%',
	app,
	menus,
	timeline,
	camera,
	menu,
	pointer = true,
	children,
}: {
	wallpaper?: string
	/** Which part of the painting shows. */
	at?: string
	app?: string
	menus?: string[]
	timeline?: Timeline
	/** A zoom on a point of the screen (in points): 1 is the whole screen. */
	camera?: { zoom: number; x: number; y: number }
	/** The menu bar's title whose menu is open. */
	menu?: string
	/** The pointer shows (off while only keys are used). */
	pointer?: boolean
	children?: ReactNode
}) {
	const zoom = camera?.zoom ?? 1
	const origin = camera ? `${camera.x}px ${camera.y}px` : 'center'
	return (
		<AbsoluteFill style={{ overflow: 'hidden', background: '#1a47a6' }}>
			<div
				style={{
					position: 'absolute',
					left: 0,
					top: 0,
					width: SCREEN.width,
					height: SCREEN.height,
					transformOrigin: '0 0',
					transform: `scale(${SCALE})`,
				}}
			>
				<div style={{ position: 'absolute', inset: 0, transformOrigin: origin, transform: `scale(${zoom})` }}>
					<LivingPainting name={wallpaper} width={SCREEN.width} height={SCREEN.height} ratio={SCALE} at={position(at)} />
					<MenuBar app={app} menus={menus} open={menu} />
					{children}
					{timeline && pointer && <Pointer timeline={timeline} />}
				</div>
			</div>
			<Ambience name={wallpaper} />
		</AbsoluteFill>
	)
}

/** A CSS position ("center 30%") as fractions. */
function position(at: string): [number, number] {
	const word: Record<string, number> = { left: 0, top: 0, center: 0.5, right: 1, bottom: 1 }
	const [x, y] = at.split(' ').map((p) => (p.endsWith('%') ? parseFloat(p) / 100 : (word[p] ?? 0.5)))
	return [x, y ?? 0.5]
}

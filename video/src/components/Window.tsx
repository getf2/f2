// A window or panel on the desktop: rounded, translucent over what's behind
// (blurred, as macOS's materials are), with a soft shadow. `appear` (0–1)
// brings it in as macOS does: a quick fade and a slight grow.
import { type CSSProperties, type ReactNode } from 'react'
import { FONT } from '../theme'

export function Window({
	x,
	y,
	width,
	height,
	radius = 16,
	material = 'rgba(231, 231, 234, 0.9)',
	appear = 1,
	style,
	children,
}: {
	x: number
	y: number
	width: number
	height?: number
	radius?: number
	/** What's laid over the blurred backdrop. */
	material?: string
	appear?: number
	style?: CSSProperties
	children?: ReactNode
}) {
	if (appear <= 0) return null
	return (
		<div
			style={{
				position: 'absolute',
				left: x,
				top: y,
				width,
				height,
				borderRadius: radius,
				overflow: 'hidden',
				fontFamily: FONT.ui,
				color: 'rgba(0, 0, 0, 0.86)',
				background: material,
				backdropFilter: 'blur(40px) saturate(1.5)',
				WebkitFontSmoothing: 'antialiased',
				boxShadow: [
					'0 0 0 0.5px rgba(0, 0, 0, 0.22)',
					'inset 0 0 0 0.5px rgba(255, 255, 255, 0.55)',
					'0 2px 6px rgba(0, 0, 0, 0.08)',
					'0 22px 60px rgba(8, 24, 70, 0.34)',
				].join(', '),
				opacity: appear,
				transform: `scale(${0.97 + appear * 0.03})`,
				transformOrigin: 'center top',
				...style,
			}}
		>
			{children}
		</div>
	)
}

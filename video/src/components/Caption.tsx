// A scene's headline, the site's own words, in Fraunces at the top, and
// sometimes a smaller line under it that comes in later.
import { Easing, interpolate, useCurrentFrame, useVideoConfig } from 'remotion'
import { EASE_OUT, FONT, sec } from '../theme'

const ease = Easing.bezier(...EASE_OUT)

export function Caption({
	title,
	sub,
	subFrom,
	from = sec(0.15),
	top = 74,
	color = '#fff',
}: {
	title: string
	sub?: string
	/** When the smaller line comes in (frames); with the title if not given. */
	subFrom?: number
	/** When the title comes in (frames): after something at the top's done. */
	from?: number
	top?: number
	color?: string
}) {
	const frame = useCurrentFrame()
	const { durationInFrames } = useVideoConfig()
	// In after the scene's cross-fade, out before the next one's.
	const out = interpolate(frame, [durationInFrames - sec(0.55), durationInFrames - sec(0.2)], [1, 0], {
		extrapolateLeft: 'clamp',
		extrapolateRight: 'clamp',
	})
	const rise = (from: number) => {
		const p = interpolate(frame, [from, from + sec(0.7)], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease })
		return { opacity: p * out, transform: `translateY(${(1 - p) * 14}px)` }
	}
	const shadow = color === '#fff' ? '0 2px 30px rgba(10, 30, 90, 0.45), 0 1px 3px rgba(10, 30, 90, 0.3)' : 'none'

	return (
		<div style={{ position: 'absolute', left: 0, right: 0, top, textAlign: 'center', color, pointerEvents: 'none' }}>
			<div
				style={{
					fontFamily: FONT.display,
					fontVariationSettings: "'SOFT' 100, 'WONK' 0",
					fontWeight: 600,
					fontSize: 66,
					lineHeight: 1.04,
					letterSpacing: '-0.03em',
					textShadow: shadow,
					...rise(from),
				}}
			>
				{title}
			</div>
			{sub && (
				<div
					style={{
						marginTop: 14,
						fontFamily: FONT.sans,
						fontSize: 30,
						fontWeight: 450,
						lineHeight: 1.3,
						color: color === '#fff' ? 'rgba(255, 255, 255, 0.94)' : color,
						textShadow: shadow,
						...rise(subFrom ?? sec(0.35)),
					}}
				>
					{sub}
				</div>
			)}
		</div>
	)
}

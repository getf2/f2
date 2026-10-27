// 8. The end: golden-hour, as the site's closing section has it, the icon,
// the site's headline and its fine print.
import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame } from 'remotion'
import { Ambience } from '../living/Ambience'
import { LivingPainting } from '../living/LivingPainting'
import { useFonts } from '../fonts'
import { EASE_OUT, FONT, HEIGHT, sec, WIDTH } from '../theme'

export const SCENE8_LENGTH = sec(5.4)

const ease = Easing.bezier(...EASE_OUT)

export function Scene8End() {
	useFonts()
	const frame = useCurrentFrame()
	const rise = (from: number, by = 18) => {
		const p = interpolate(frame, [from, from + sec(0.8)], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease })
		return { opacity: p, transform: `translateY(${(1 - p) * by}px)` }
	}
	// The painting settles, slowly.
	const zoom = interpolate(frame, [0, sec(5.4)], [1.07, 1], { easing: Easing.out(Easing.quad) })

	return (
		<AbsoluteFill style={{ background: '#b9814d', overflow: 'hidden' }}>
			<div style={{ position: 'absolute', inset: 0, transform: `scale(${zoom})` }}>
				<LivingPainting name="golden-hour" width={WIDTH} height={HEIGHT} at={[0.5, 0.4]} />
			</div>
			<Ambience name="golden-hour" fadeOut={1.4} />
			{/* The closing section's shade (styles.css: .close::before). */}
			<AbsoluteFill
				style={{
					background:
						'radial-gradient(ellipse 42% 36% at 50% 46%, rgba(64, 28, 6, 0.5), rgba(64, 28, 6, 0) 100%), linear-gradient(180deg, rgba(60, 30, 10, 0.2), rgba(60, 30, 10, 0.05) 50%, rgba(40, 20, 10, 0.3))',
				}}
			/>
			<AbsoluteFill style={{ alignItems: 'center', justifyContent: 'center', color: '#fff', textAlign: 'center' }}>
				<Img src={staticFile('icon.png')} style={{ width: 150, height: 150, filter: 'drop-shadow(0 14px 30px rgba(60, 25, 0, 0.35))', ...rise(sec(0.25), 12) }} />
				<div
					style={{
						marginTop: 34,
						maxWidth: 1400,
						fontFamily: FONT.display,
						fontVariationSettings: "'SOFT' 100, 'WONK' 0",
						fontWeight: 600,
						fontSize: 96,
						lineHeight: 1.0,
						letterSpacing: '-0.035em',
						textWrap: 'balance',
						textShadow: '0 2px 30px rgba(80, 30, 0, 0.35)',
						...rise(sec(0.5)),
					}}
				>
					The Finder your Mac should have come with.
				</div>
				<div
					style={{
						marginTop: 34,
						fontFamily: FONT.mono,
						fontSize: 22,
						fontWeight: 500,
						letterSpacing: '0.12em',
						textTransform: 'uppercase',
						textShadow: '0 1px 10px rgba(80, 30, 0, 0.7), 0 0 2px rgba(80, 30, 0, 0.5)',
						...rise(sec(0.95), 10),
					}}
				>
					Free · macOS 13 or later · getf2.com
				</div>
			</AbsoluteFill>
		</AbsoluteFill>
	)
}

// What passes over a painting: birds (inked, as the paintings' lines are,
// flapping and gliding; mirrored in still water), motes floating in the
// light, and stars twinkling.
import { random } from 'remotion'
import { type Place } from './LivingPainting'
import { type Flight, type Life } from './paintings'

const INK = '#1e2a40'

/** A bird: wings out from its body, beating for a while, then gliding. */
function Bird({ size, t, seed }: { size: number; t: number; seed: number }) {
	const cycle = 2.4 + seed * 0.8
	const local = (t + seed * 5) % cycle
	const a = local < 1.1 ? Math.sin(local * Math.PI * 2 * 3.1) : 0.3
	const s = size
	const tip = -a * s * 0.28 + s * 0.04
	const wrist = -s * 0.16 - a * s * 0.06
	const body = s * 0.05
	return (
		<path
			d={`M ${-s / 2} ${tip} Q ${-s * 0.22} ${wrist} 0 ${body} Q ${s * 0.22} ${wrist} ${s / 2} ${tip}`}
			fill="none"
			stroke={INK}
			strokeWidth={Math.max(1.3, s * 0.085)}
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
	)
}

function Flock({ flight, since, place, scale, n }: { flight: Flight; since: number; place: Place; scale: number; n: number }) {
	const p = (since - flight.at) / flight.dur
	if (p < 0 || p > 1) return null
	const birds = Array.from({ length: flight.count }, (_, i) => {
		const r = (k: string) => random(`flock-${n}-${i}-${k}`)
		// Loosely together, each a little behind or ahead.
		const x = flight.from[0] + (flight.to[0] - flight.from[0]) * p + (i - (flight.count - 1) / 2) * 0.024 + (r('x') - 0.5) * 0.01
		const bob = Math.sin((since + r('b') * 4) * 1.6) * 0.004
		const y = flight.from[1] + (flight.to[1] - flight.from[1]) * p + (r('y') - 0.5) * 0.03 + bob
		return { x, y, size: flight.size * (0.85 + r('s') * 0.3) * scale, seed: r('seed') }
	})
	// Heading left or right: the wings' sweep follows.
	const left = flight.to[0] < flight.from[0]
	return (
		<>
			{birds.map((b, i) => {
				const [x, y] = place([b.x, b.y])
				return (
					<g key={i} transform={`translate(${x} ${y}) scale(${left ? -1 : 1} 1)`} opacity={0.85}>
						<Bird size={b.size} t={since} seed={b.seed} />
					</g>
				)
			})}
			{flight.reflect !== undefined &&
				birds.map((b, i) => {
					const [x, y] = place([b.x, 2 * flight.reflect! - b.y])
					return (
						<g key={`r${i}`} transform={`translate(${x + Math.sin(since * 3 + i) * 1.5} ${y}) scale(${left ? -1 : 1} -1)`} opacity={0.28}>
							<Bird size={b.size} t={since} seed={b.seed} />
						</g>
					)
				})}
		</>
	)
}

function Motes({ motes, t, place, scale }: { motes: NonNullable<Life['motes']>; t: number; place: Place; scale: number }) {
	const [x0, y0] = place([motes.box[0], motes.box[1]])
	const [x1, y1] = place([motes.box[2], motes.box[3]])
	const w = x1 - x0
	const h = y1 - y0
	return (
		<>
			{Array.from({ length: motes.count }, (_, i) => {
				const r = (k: string) => random(`mote-${i}-${k}`)
				const rise = motes.rise * (0.6 + r('rise') * 0.8) * scale
				const x = x0 + ((((r('x') * w + t * 6 * scale + Math.sin(t * 0.5 + r('p') * 6) * 14 * scale) % w) + w) % w)
				const y = y0 + ((((r('y') * h - t * rise) % h) + h) % h)
				// Fading in and out at the box's edges, and twinkling.
				const edge = Math.min(1, (y - y0) / (h * 0.15), (y1 - y) / (h * 0.15))
				const glint = 0.35 + 0.65 * (0.5 + 0.5 * Math.sin(t * (1.2 + r('f') * 1.6) + r('p') * 9))
				const size = motes.size * (0.5 + r('s') * 0.7) * scale
				return (
					<div
						key={i}
						style={{
							position: 'absolute',
							left: x - size / 2,
							top: y - size / 2,
							width: size,
							height: size,
							borderRadius: '50%',
							background: `rgba(${motes.color}, ${0.85 * edge * glint})`,
							boxShadow: `0 0 ${size * 2}px rgba(${motes.color}, ${0.5 * edge * glint})`,
							mixBlendMode: 'screen',
						}}
					/>
				)
			})}
		</>
	)
}

function Stars({ stars, t, place, drift, scale }: { stars: [number, number][]; t: number; place: Place; drift: number; scale: number }) {
	return (
		<svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', overflow: 'visible' }}>
			{stars.map(([sx, sy], i) => {
				const r = (k: string) => random(`star-${i}-${k}`)
				const [x, y] = place([sx + drift, sy])
				// A slow shimmer, and now and then a twinkle.
				const twinkle = Math.pow(0.5 + 0.5 * Math.sin(t * (0.8 + r('f') * 1.4) + r('p') * 20), 8)
				const glow = 0.25 + 0.2 * Math.sin(t * 0.7 + r('q') * 9) + 0.55 * twinkle
				const s = (2 + r('s') * 2.5) * scale * (0.7 + twinkle * 0.6)
				return (
					<g key={i} transform={`translate(${x} ${y})`} opacity={Math.max(0, glow)}>
						<path d={`M0 ${-s} L${s * 0.22} ${-s * 0.22} L${s} 0 L${s * 0.22} ${s * 0.22} L0 ${s} L${-s * 0.22} ${s * 0.22} L${-s} 0 L${-s * 0.22} ${-s * 0.22} Z`} fill="#fffbea" />
					</g>
				)
			})}
		</svg>
	)
}

export function Overlays({ life, place, t, since, drift, stars, scale }: { life: Life; place: Place; t: number; since: number; drift: number; stars: [number, number][]; scale: number }) {
	return (
		<>
			{life.stars && <Stars stars={stars} t={t} place={place} drift={drift} scale={scale} />}
			{life.motes && <Motes motes={life.motes} t={t} place={place} scale={scale} />}
			{life.birds && (
				<svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', overflow: 'visible' }}>
					{life.birds.map((flight, n) => (
						<Flock key={n} flight={flight} since={since} place={place} scale={scale} n={n} />
					))}
				</svg>
			)}
		</>
	)
}

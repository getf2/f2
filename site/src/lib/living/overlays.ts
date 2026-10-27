// What passes over a painting, drawn on a canvas over it: birds (inked, as
// the paintings' lines are, flapping and gliding; mirrored in still water),
// motes floating in the light, and stars twinkling.
import { rand } from './masks'
import { type Flight, type Life, REPEAT } from './paintings'

const INK = '#1e2a40'

/** Where the painting's point `uv` (0–1) is shown, in the sky's CSS px. */
export type Place = (uv: [number, number]) => [number, number]

/** A bird: wings out from its body, beating for a while, then gliding. */
function bird(ctx: CanvasRenderingContext2D, s: number, t: number, seed: number) {
  const cycle = 2.4 + seed * 0.8
  const local = (t + seed * 5) % cycle
  const a = local < 1.1 ? Math.sin(local * Math.PI * 2 * 3.1) : 0.3
  const tip = -a * s * 0.28 + s * 0.04
  const wrist = -s * 0.16 - a * s * 0.06
  ctx.beginPath()
  ctx.moveTo(-s / 2, tip)
  ctx.quadraticCurveTo(-s * 0.22, wrist, 0, s * 0.05)
  ctx.quadraticCurveTo(s * 0.22, wrist, s / 2, tip)
  ctx.lineWidth = Math.max(1.3, s * 0.085)
  ctx.stroke()
}

function flock(ctx: CanvasRenderingContext2D, flight: Flight, n: number, t: number, place: Place, scale: number) {
  // (Again every REPEAT s.)
  const since = t - flight.at
  if (since < 0) return
  const p = (since % REPEAT) / flight.dur
  if (p > 1) return
  const left = flight.to[0] < flight.from[0]
  ctx.strokeStyle = INK
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'
  for (let i = 0; i < flight.count; i++) {
    const r = (k: string) => rand(`flock-${n}-${i}-${k}`)
    // Loosely together, each a little behind or ahead.
    const x = flight.from[0] + (flight.to[0] - flight.from[0]) * p + (i - (flight.count - 1) / 2) * 0.024 + (r('x') - 0.5) * 0.01
    const bob = Math.sin((t + r('b') * 4) * 1.6) * 0.004
    const y = flight.from[1] + (flight.to[1] - flight.from[1]) * p + (r('y') - 0.5) * 0.03 + bob
    const size = flight.size * (0.85 + r('s') * 0.3) * scale
    const seed = r('seed')
    const at = place([x, y])
    ctx.save()
    ctx.globalAlpha = 0.85
    ctx.translate(at[0], at[1])
    ctx.scale(left ? -1 : 1, 1)
    bird(ctx, size, t, seed)
    ctx.restore()
    if (flight.reflect !== undefined) {
      const mirror = place([x, 2 * flight.reflect - y])
      ctx.save()
      ctx.globalAlpha = 0.28
      ctx.translate(mirror[0] + Math.sin(t * 3 + i) * 1.5, mirror[1])
      ctx.scale(left ? -1 : 1, -1)
      bird(ctx, size, t, seed)
      ctx.restore()
    }
  }
}

function motes(ctx: CanvasRenderingContext2D, m: NonNullable<Life['motes']>, t: number, place: Place, scale: number) {
  const [x0, y0] = place([m.box[0], m.box[1]])
  const [x1, y1] = place([m.box[2], m.box[3]])
  const w = x1 - x0
  const h = y1 - y0
  for (let i = 0; i < m.count; i++) {
    const r = (k: string) => rand(`mote-${i}-${k}`)
    const rise = m.rise * (0.6 + r('rise') * 0.8) * scale
    const x = x0 + ((((r('x') * w + t * 6 * scale + Math.sin(t * 0.5 + r('p') * 6) * 14 * scale) % w) + w) % w)
    const y = y0 + ((((r('y') * h - t * rise) % h) + h) % h)
    // Fading in and out at the box's edges, and twinkling.
    const edge = Math.max(0, Math.min(1, (y - y0) / (h * 0.15), (y1 - y) / (h * 0.15)))
    const glint = 0.35 + 0.65 * (0.5 + 0.5 * Math.sin(t * (1.2 + r('f') * 1.6) + r('p') * 9))
    const size = m.size * (0.5 + r('s') * 0.7) * scale
    ctx.fillStyle = `rgba(${m.color}, ${0.22 * edge * glint})`
    ctx.beginPath()
    ctx.arc(x, y, size * 1.4, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = `rgba(${m.color}, ${0.85 * edge * glint})`
    ctx.beginPath()
    ctx.arc(x, y, size / 2, 0, Math.PI * 2)
    ctx.fill()
  }
}

function stars(ctx: CanvasRenderingContext2D, list: [number, number][], t: number, place: Place, drift: number, scale: number) {
  ctx.fillStyle = '#fffbea'
  list.forEach(([sx, sy], i) => {
    const r = (k: string) => rand(`star-${i}-${k}`)
    const [x, y] = place([sx + drift, sy])
    // A slow shimmer, and now and then a twinkle.
    const twinkle = Math.pow(0.5 + 0.5 * Math.sin(t * (0.8 + r('f') * 1.4) + r('p') * 20), 8)
    const glow = 0.25 + 0.2 * Math.sin(t * 0.7 + r('q') * 9) + 0.55 * twinkle
    if (glow <= 0) return
    const s = (2 + r('s') * 2.5) * scale * (0.7 + twinkle * 0.6)
    const k = s * 0.22
    ctx.globalAlpha = glow
    ctx.beginPath()
    ctx.moveTo(x, y - s)
    ctx.lineTo(x + k, y - k)
    ctx.lineTo(x + s, y)
    ctx.lineTo(x + k, y + k)
    ctx.lineTo(x, y + s)
    ctx.lineTo(x - k, y + k)
    ctx.lineTo(x - s, y)
    ctx.lineTo(x - k, y - k)
    ctx.closePath()
    ctx.fill()
  })
  ctx.globalAlpha = 1
}

/** Whether a painting has anything to draw over it. */
export const hasOverlays = (life: Life) => Boolean(life.birds || life.motes || life.stars)

/** All of it, at `t` s; `scale`: the painting's width shown over 1920. */
export function overlays(ctx: CanvasRenderingContext2D, life: Life, t: number, place: Place, drift: number, list: [number, number][], scale: number) {
  if (life.stars) stars(ctx, list, t, place, drift, scale)
  if (life.motes) motes(ctx, life.motes, t, place, scale)
  life.birds?.forEach((f, n) => flock(ctx, f, n, t, place, scale))
}

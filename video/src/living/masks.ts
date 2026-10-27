// What moves where in a painting, worked out from its colors and its Life:
// an RGBA map at a small size (the GPU smooths it up). R: the sky's drift
// (0–1), G: the wind's sway, B: water, A: unused.
import { type Life } from './paintings'

export const MASK_WIDTH = 640

const hsv = (r: number, g: number, b: number) => {
	const max = Math.max(r, g, b)
	const min = Math.min(r, g, b)
	const d = max - min
	let h = 0
	if (d > 0) {
		if (max === r) h = ((g - b) / d) % 6
		else if (max === g) h = (b - r) / d + 2
		else h = (r - g) / d + 4
		h *= 60
		if (h < 0) h += 360
	}
	return [h, max === 0 ? 0 : d / max, max / 255]
}

/** The skyline's y at x (0–1), between its points. */
function skyline(points: [number, number][], x: number) {
	for (let i = 1; i < points.length; i++) {
		const [x0, y0] = points[i - 1]
		const [x1, y1] = points[i]
		if (x <= x1) return y0 + ((y1 - y0) * (x - x0)) / Math.max(1e-6, x1 - x0)
	}
	return points[points.length - 1][1]
}

const inBox = (box: [number, number, number, number], x: number, y: number) => x >= box[0] && x <= box[2] && y >= box[1] && y <= box[3]

/** A box blur, `r` cells each way, of one channel. */
function blur(src: Float32Array, w: number, h: number, r: number) {
	const tmp = new Float32Array(src.length)
	const out = new Float32Array(src.length)
	for (let y = 0; y < h; y++)
		for (let x = 0; x < w; x++) {
			let s = 0
			let n = 0
			for (let k = -r; k <= r; k++) {
				const xx = x + k
				if (xx >= 0 && xx < w) {
					s += src[y * w + xx]
					n++
				}
			}
			tmp[y * w + x] = s / n
		}
	for (let y = 0; y < h; y++)
		for (let x = 0; x < w; x++) {
			let s = 0
			let n = 0
			for (let k = -r; k <= r; k++) {
				const yy = y + k
				if (yy >= 0 && yy < h) {
					s += tmp[yy * w + x]
					n++
				}
			}
			out[y * w + x] = s / n
		}
	return out
}

export function makeMask(image: HTMLImageElement, life: Life): { data: Uint8Array; width: number; height: number } {
	const w = MASK_WIDTH
	const h = Math.round((image.height / image.width) * w)
	const canvas = document.createElement('canvas')
	canvas.width = w
	canvas.height = h
	const ctx = canvas.getContext('2d', { willReadFrequently: true })!
	ctx.drawImage(image, 0, 0, w, h)
	const px = ctx.getImageData(0, 0, w, h).data

	// Sky or cloud by color: not plants, not dark ink or shade. And in
	// water, what reflects the sky (blue, or a cloud's cream), or a river's
	// pale gray.
	const skyish = new Uint8Array(w * h)
	const mirrors = new Uint8Array(w * h)
	const river = new Uint8Array(w * h)
	for (let i = 0; i < w * h; i++) {
		const [hue, s, v] = hsv(px[i * 4], px[i * 4 + 1], px[i * 4 + 2])
		// (Misty pines are teal.)
		const plant = (hue > 55 && hue < 175 && s > 0.22) || (hue >= 175 && hue < 192 && s > 0.2 && v < 0.82)
		skyish[i] = !plant && v > 0.42 ? 1 : 0
		mirrors[i] = (hue > 185 && hue < 235 && s > 0.3 && v > 0.45) || (s < 0.3 && v > 0.7) ? 1 : 0
		river[i] = s < 0.2 && v > 0.62 ? 1 : 0
	}
	const line = Array.from({ length: w }, (_, x) => Math.round(skyline(life.horizon, x / w) * h))

	// Land reaches up past the skyline where it isn't sky-colored (a tree, a
	// peak): followed up from the skyline. Everything else above it is sky,
	// clouds' inked edges and all.
	const land = new Uint8Array(w * h)
	const stack: number[] = []
	for (let x = 0; x < w; x++)
		for (let y = Math.max(0, line[x]); y < h; y++) {
			land[y * w + x] = 1
			if (y === line[x] && y > 0) stack.push((y - 1) * w + x)
		}
	while (stack.length) {
		const i = stack.pop()!
		if (land[i] || skyish[i]) continue
		land[i] = 1
		const x = i % w
		const y = (i - x) / w
		if (y > 0) stack.push(i - w)
		if (x > 0) stack.push(i - 1)
		if (x < w - 1) stack.push(i + 1)
		if (y < h - 1 && !land[i + w]) stack.push(i + w)
	}
	let sky = new Float32Array(w * h)
	for (let i = 0; i < w * h; i++) sky[i] = land[i] ? 0 : 1

	// Water that mirrors the sky drifts with it: the sky's mask, mirrored at
	// the waterline. What stands in the water (a jetty, reeds) doesn't: it's
	// what isn't the sky's colors and reaches the bottom edge; the clouds'
	// inked edges in the reflection are left to drift with them.
	const water = new Float32Array(w * h)
	const standing = new Uint8Array(w * h)
	if (life.water) {
		const { box, kind } = life.water
		const [x0, y0, x1, y1] = box
		if (kind === 'mirror') {
			const fill: number[] = []
			for (let x = 0; x < w; x++) fill.push((h - 1) * w + x)
			while (fill.length) {
				const i = fill.pop()!
				if (standing[i] || mirrors[i]) continue
				const x = i % w
				const y = (i - x) / w
				if (y / h < y0) continue
				standing[i] = 1
				if (y > 0) fill.push(i - w)
				if (x > 0) fill.push(i - 1)
				if (x < w - 1) fill.push(i + 1)
				if (y < h - 1) fill.push(i + w)
			}
		}
		for (let y = 0; y < h; y++)
			for (let x = 0; x < w; x++) {
				const i = y * w + x
				if (!inBox(box, x / w, y / h)) continue
				// (Softly in from the box's edges.)
				const edge = Math.min((x / w - x0) / 0.03, (x1 - x / w) / 0.03, (y / h - y0) / 0.02, (y1 - y / h) / 0.02, 1)
				const wet = kind === 'river' ? river[i] : kind === 'mirror' ? 1 - standing[i] : skyish[i]
				water[i] = Math.max(0, edge) * (wet ? 1 : kind === 'waves' ? 0.15 : 0)
				if (kind === 'mirror' && !standing[i]) {
					const my = Math.round(2 * y0 * h - y)
					if (my >= 0 && my < h) sky[i] = Math.max(sky[i], sky[my * w + x])
				}
			}
	}
	// Held still, softly (the sky stretches a little there, it doesn't tear).
	for (const box of life.still ?? []) {
		const feather = 0.06
		for (let y = 0; y < h; y++)
			for (let x = 0; x < w; x++) {
				const fx = x / w
				const fy = y / h
				const dx = Math.max(box[0] - fx, fx - box[2], 0)
				const dy = Math.max(box[1] - fy, fy - box[3], 0)
				const k = Math.min(1, Math.hypot(dx, dy) / feather)
				sky[y * w + x] *= k
			}
	}
	sky = blur(sky, w, h, 1)
	let wet = blur(water, w, h, 2)
	// (Not a ripple near what stands in the water: posts and reeds stay straight.)
	if (life.water?.kind === 'mirror') {
		const near = new Float32Array(w * h)
		for (let i = 0; i < w * h; i++) near[i] = standing[i]
		const around = blur(near, w, h, 3)
		wet = wet.map((v, i) => v * Math.max(0, 1 - around[i] * 4))
	}

	// The wind: on the land, more toward the front.
	const wind = new Float32Array(w * h)
	if (life.wind) {
		const { from, region, strong } = life.wind
		for (let y = 0; y < h; y++)
			for (let x = 0; x < w; x++) {
				const i = y * w + x
				const fy = y / h
				if (region ? !inBox(region, x / w, fy) : fy < from) continue
				let k = Math.min(1, Math.max(0, (fy - from) / Math.max(0.05, 1 - from)))
				if (strong?.some((b) => inBox(b, x / w, fy))) k = 1
				wind[i] = (1 - sky[i]) * (0.25 + 0.75 * Math.pow(k, 1.2)) * (1 - water[i])
			}
	}
	const sway = blur(wind, w, h, 3)

	const data = new Uint8Array(w * h * 4)
	for (let i = 0; i < w * h; i++) {
		data[i * 4] = Math.round(sky[i] * 255)
		data[i * 4 + 1] = Math.round(sway[i] * 255)
		data[i * 4 + 2] = Math.round(wet[i] * 255)
		data[i * 4 + 3] = 255
	}
	return { data, width: w, height: h }
}

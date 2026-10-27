// A painting that's alive: drawn by WebGL each frame, its clouds drifting
// behind the hills, its grass and reeds swaying in gusts, its water
// rippling (a mirror's reflections drifting with the sky), a sun breathing.
// Time is the video's, so a painting shown in two scenes carries on.
import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { continueRender, delayRender, random, staticFile, useCurrentFrame, useVideoConfig } from 'remotion'
import { makeMask } from './masks'
import { Overlays } from './Overlays'
import { LIVES, type Life } from './paintings'

/** Where a scene starts in the whole video (s), where its painting was
 * first shown (in the scene before it, maybe) and when the sky's drift is
 * centered for it: so a painting carries on across scenes. */
export const SceneTime = createContext({ start: 0, spanStart: 0, center: 0 })

// Drawn a little bigger than the screen, for the drift's room at the edges.
const ZOOM = 1.06

const VERTEX = `
attribute vec2 p;
varying vec2 v;
void main() { v = vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5); gl_Position = vec4(p, 0.0, 1.0); }
`

const FRAGMENT = `
precision highp float;
uniform sampler2D img;
uniform sampler2D map;
uniform vec2 scale;
uniform vec2 offset;
uniform vec2 res;
uniform float t;
uniform float drift;
uniform vec4 wind;      // amp (px at 1920), speed, on
uniform vec4 water;     // amp, kind (0 mirror, 1 waves, 2 river), waterline y
uniform vec4 sun;       // x, y, r, on
uniform float show;     // 1: the map, to see what moves where
uniform float shade;    // cloud shadows' darkness
uniform vec4 rays;      // sunbeams' box
uniform float raysK;
uniform vec4 mist;      // mist's box
uniform float mistK;
varying vec2 v;

// Value noise, smooth enough for wind.
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

void main() {
  vec2 uv = v * scale + offset;
  vec4 m = texture2D(map, uv);
  float px = 1.0 / res.x;

  // Wind: gusts rolling across from the left, and a flutter within them.
  vec2 d = vec2(0.0);
  if (wind.z > 0.5 && m.g > 0.001) {
    float gust = 0.55 + 0.45 * sin((uv.x * 3.2 - t * wind.y) * 6.2831 + noise(vec2(uv.x * 2.0, t * 0.2)) * 3.0);
    float flutter = noise(vec2(uv.x * 140.0 + t * 1.7, uv.y * 30.0)) - 0.5;
    float sway = (gust * 0.8 + flutter * 0.9) * wind.x * m.g;
    // (Things bend from their feet: the sway's sideways, a little down.)
    d += vec2(sway, abs(sway) * 0.25) * px * (1920.0 / res.x) * (res.x / 1920.0);
  }

  // Water: a mirror ripples sideways; waves roll toward the shore; a river glints.
  float wet = m.b;
  if (wet > 0.001) {
    if (water.y < 0.5) {
      // (Long, slow wobbles, as a calm lake's reflections have.)
      float r = sin(uv.y * 240.0 + t * 1.4 + noise(vec2(uv.x * 6.0, uv.y * 20.0 + t * 0.25)) * 3.0);
      d.x += r * water.x * wet * px;
    } else if (water.y < 1.5) {
      float r = sin(uv.y * 260.0 - t * 1.6 + uv.x * 18.0 + noise(uv * vec2(8.0, 30.0) + t * 0.2) * 3.0);
      d.y += r * water.x * wet * px;
      d.x += r * water.x * 0.5 * wet * px;
    } else {
      float r = sin(uv.x * 500.0 + t * 2.0) * sin(uv.y * 300.0 - t * 1.3);
      d.x += r * water.x * wet * px;
    }
  }

  vec4 col = texture2D(img, uv + d);

  // The sky drifts: where it's sky here, and was sky where it comes from.
  if (m.r > 0.001) {
    vec2 from = uv - vec2(drift, 0.0);
    float back = texture2D(map, from).r;
    vec4 sky = texture2D(img, from + vec2(d.x * wet, 0.0));
    col = mix(col, sky, m.r * smoothstep(0.2, 0.9, back));
  }

  // A sun breathing.
  if (sun.w > 0.5) {
    // (Where the drifting sky has taken it.)
    vec2 q = (uv - sun.xy - vec2(drift, 0.0)) * vec2(1.0, res.y / res.x);
    float glow = exp(-dot(q, q) / (sun.z * sun.z));
    float breathe = 0.5 + 0.5 * sin(t * 1.1);
    col.rgb += vec3(1.0, 0.82, 0.5) * glow * (0.07 + 0.06 * breathe);
  }

  // Cloud shadows sliding over the land, with the clouds.
  if (shade > 0.0) {
    vec2 q = uv * vec2(1.7, 3.4) - vec2(t * 0.011, 0.0);
    float c = noise(q * 2.0) * 0.65 + noise(q * 5.0 + 3.1) * 0.35;
    col.rgb *= 1.0 - shade * 0.2 * smoothstep(0.5, 0.75, c) * (1.0 - m.r);
  }

  // Sunbeams brightening and fading, a swell running down them.
  if (raysK > 0.0) {
    float inside = step(rays.x, uv.x) * step(uv.x, rays.z) * step(rays.y, uv.y) * step(uv.y, rays.w);
    float hi = max(col.r, max(col.g, col.b)), lo = min(col.r, min(col.g, col.b));
    float pale = smoothstep(0.55, 0.85, hi) * (1.0 - smoothstep(0.12, 0.35, (hi - lo) / max(hi, 0.001)));
    float swell = 0.5 + 0.5 * sin(t * 0.9 - (uv.x * 2.0 + uv.y * 3.0));
    col.rgb += raysK * inside * pale * swell * vec3(1.0, 0.98, 0.9);
  }

  // Mist drifting along the ground.
  if (mistK > 0.0) {
    float edge = smoothstep(mist.y, mist.y + 0.12, uv.y) * smoothstep(mist.x - 0.01, mist.x + 0.1, uv.x) * (1.0 - smoothstep(mist.z - 0.1, mist.z + 0.01, uv.x));
    vec2 q = uv * vec2(2.4, 6.0) + vec2(t * 0.035, sin(t * 0.2) * 0.05);
    float fog = smoothstep(0.35, 0.8, noise(q) * 0.6 + noise(q * 2.3 + 7.0) * 0.4);
    col.rgb = mix(col.rgb, vec3(0.86, 0.94, 0.95), mistK * fog * edge);
  }

  if (show > 0.5) col.rgb = mix(col.rgb * 0.35, m.rgb, 0.75);
  gl_FragColor = vec4(col.rgb, 1.0);
}
`

type Gl = {
	gl: WebGLRenderingContext
	program: WebGLProgram
	image: HTMLImageElement
	u: (name: string) => WebGLUniformLocation | null
}

function compile(gl: WebGLRenderingContext, type: number, source: string) {
	const shader = gl.createShader(type)!
	gl.shaderSource(shader, source)
	gl.compileShader(shader)
	if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? 'shader')
	return shader
}

function texture(gl: WebGLRenderingContext, unit: number, set: () => void) {
	const tex = gl.createTexture()
	gl.activeTexture(gl.TEXTURE0 + unit)
	gl.bindTexture(gl.TEXTURE_2D, tex)
	set()
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR)
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
}

function setup(canvas: HTMLCanvasElement, image: HTMLImageElement, life: Life): Gl {
	const gl = canvas.getContext('webgl', { preserveDrawingBuffer: true, antialias: false, premultipliedAlpha: false })!
	const program = gl.createProgram()!
	gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX))
	gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT))
	gl.linkProgram(program)
	gl.useProgram(program)
	const buffer = gl.createBuffer()
	gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
	gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW)
	const p = gl.getAttribLocation(program, 'p')
	gl.enableVertexAttribArray(p)
	gl.vertexAttribPointer(p, 2, gl.FLOAT, false, 0, 0)
	texture(gl, 0, () => gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, image))
	const mask = makeMask(image, life)
	texture(gl, 1, () => gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, mask.width, mask.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, mask.data))
	const u = (name: string) => gl.getUniformLocation(program, name)
	gl.uniform1i(u('img'), 0)
	gl.uniform1i(u('map'), 1)
	return { gl, program, image, u }
}

/** Where the painting's point `uv` (0–1) is shown, in the area's px. */
export type Place = (uv: [number, number]) => [number, number]

/** The painting, covering `width` × `height` (CSS px, drawn at `ratio`×),
 * its `at` part showing, alive; birds, motes and stars over it. */
export function LivingPainting({
	name,
	width,
	height,
	ratio = 1,
	at = [0.5, 0.3],
	show = false,
}: {
	name: string
	width: number
	height: number
	ratio?: number
	at?: [number, number]
	show?: boolean
}) {
	const frame = useCurrentFrame()
	const { fps } = useVideoConfig()
	const time = useContext(SceneTime)
	const life = LIVES[name]
	const canvas = useRef<HTMLCanvasElement>(null)
	const state = useRef<Gl | null>(null)
	const [handle] = useState(() => delayRender(`Painting ${name}`))
	const [ready, setReady] = useState<{ w: number; h: number; stars: [number, number][] } | null>(null)
	const drawn = useRef(false)
	const [pw, ph] = [Math.round(width * ratio), Math.round(height * ratio)]

	useEffect(() => {
		const image = new Image()
		image.crossOrigin = 'anonymous'
		image.onload = () => {
			state.current = setup(canvas.current!, image, life)
			setReady({ w: image.width, h: image.height, stars: life.stars ? findStars(image, life.stars) : [] })
		}
		image.src = staticFile(`paint/${name}.webp`)
	}, [name, life])

	// Cover, a little bigger, the `at` part showing.
	const cover = ready ? Math.max(width / ready.w, height / ready.h) * ZOOM : 1
	const shown: [number, number] = ready ? [width / (ready.w * cover), height / (ready.h * cover)] : [1, 1]
	const offset: [number, number] = [(1 - shown[0]) * at[0], (1 - shown[1]) * at[1]]
	const t = time.start + frame / fps
	const drift = ((t - time.center) * life.drift) / 1920 / ZOOM

	useLayoutEffect(() => {
		const s = state.current
		if (!ready || !s) return
		const { gl, u } = s
		gl.viewport(0, 0, pw, ph)
		gl.uniform2f(u('scale'), shown[0], shown[1])
		gl.uniform2f(u('offset'), offset[0], offset[1])
		gl.uniform2f(u('res'), pw, ph)
		gl.uniform1f(u('t'), t)
		gl.uniform1f(u('drift'), drift)
		gl.uniform4f(u('wind'), life.wind?.amp ?? 0, life.wind?.speed ?? 0, life.wind ? 1 : 0, 0)
		gl.uniform4f(u('water'), life.water?.amp ?? 0, life.water ? { mirror: 0, waves: 1, river: 2 }[life.water.kind] : 0, life.water?.box[1] ?? 0, 0)
		gl.uniform4f(u('sun'), life.sun?.at[0] ?? 0, life.sun?.at[1] ?? 0, life.sun?.r ?? 0, life.sun ? 1 : 0)
		gl.uniform1f(u('show'), show ? 1 : 0)
		gl.uniform1f(u('shade'), life.shadows ?? 0)
		gl.uniform4f(u('rays'), ...(life.rays?.box ?? [0, 0, 0, 0]))
		gl.uniform1f(u('raysK'), life.rays?.strength ?? 0)
		gl.uniform4f(u('mist'), ...(life.mist?.box ?? [0, 0, 0, 0]))
		gl.uniform1f(u('mistK'), life.mist?.strength ?? 0)
		gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
		if (!drawn.current) {
			drawn.current = true
			continueRender(handle)
		}
	})

	const place: Place = ([x, y]) => [((x - offset[0]) / shown[0]) * width, ((y - offset[1]) / shown[1]) * height]

	return (
		<div style={{ position: 'absolute', inset: 0, width, height, overflow: 'hidden' }}>
			<canvas ref={canvas} width={pw} height={ph} style={{ position: 'absolute', inset: 0, width, height }} />
			{ready && !show && <Overlays life={life} place={place} t={t} since={t - time.spanStart} drift={drift} stars={ready.stars} scale={width / 1920} />}
		</div>
	)
}

/** Where to put stars: the darker evening blue of the sky (not on clouds). */
function findStars(image: HTMLImageElement, { box, count }: { box: [number, number, number, number]; count: number }): [number, number][] {
	const c = document.createElement('canvas')
	c.width = 400
	c.height = Math.round((image.height / image.width) * 400)
	const ctx = c.getContext('2d', { willReadFrequently: true })!
	ctx.drawImage(image, 0, 0, c.width, c.height)
	const px = ctx.getImageData(0, 0, c.width, c.height).data
	const out: [number, number][] = []
	for (let k = 0; k < 4000 && out.length < count; k++) {
		const x = box[0] + random(`star-x-${k}`) * (box[2] - box[0])
		const y = box[1] + random(`star-y-${k}`) * (box[3] - box[1])
		const i = (Math.floor(y * c.height) * c.width + Math.floor(x * c.width)) * 4
		const [r, g, b] = [px[i], px[i + 1], px[i + 2]]
		const max = Math.max(r, g, b)
		const sat = max ? (max - Math.min(r, g, b)) / max : 0
		if (b > r && b > g && max < 175 && sat > 0.35) out.push([x, y])
	}
	return out
}

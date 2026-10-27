// The painting drawn by WebGL: its clouds drifting behind the hills, its
// grass and reeds swaying in gusts, its water rippling (a mirror's
// reflections drifting with the sky), a sun breathing, a window's light
// flickering, northern lights shimmering. (The feature video's, in video/,
// with the lights and the window added.)
import type { Mask } from './masks'
import type { Life } from './paintings'

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
uniform float aspect;   // the painting's height over its width
uniform float t;
uniform float drift;
uniform vec4 wind;      // amp (px), speed, on
uniform vec4 water;     // amp, kind (0 mirror, 1 waves, 2 river), waterline y, on
uniform vec4 sun;       // x, y, r, on
uniform vec4 lamp;      // x, y, r, on
uniform float show;     // 1: the map, to see what moves where
uniform float shade;    // cloud shadows' darkness
uniform vec4 rays;      // sunbeams' box
uniform float raysK;
uniform vec4 mist;      // mist's box
uniform float mistK;
uniform float auroraK;
varying vec2 v;

// A px, with the painting 1920 wide.
const float unit = 1.0 / 1920.0;

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

  // Wind: gusts rolling across from the left, and a flutter within them.
  vec2 d = vec2(0.0);
  if (wind.z > 0.5 && m.g > 0.001) {
    float gust = 0.55 + 0.45 * sin((uv.x * 3.2 - t * wind.y) * 6.2831 + noise(vec2(uv.x * 2.0, t * 0.2)) * 3.0);
    float flutter = noise(vec2(uv.x * 140.0 + t * 1.7, uv.y * 30.0)) - 0.5;
    float sway = (gust * 0.8 + flutter * 0.9) * wind.x * m.g;
    // (Things bend from their feet: the sway's sideways, a little down.)
    d += vec2(sway, abs(sway) * 0.25) * unit;
  }

  // Water: a mirror ripples sideways; waves roll toward the shore; a river glints.
  float wet = m.b;
  if (wet > 0.001) {
    if (water.y < 0.5) {
      // (Long, slow wobbles, as a calm lake's reflections have.)
      float r = sin(uv.y * 240.0 + t * 1.4 + noise(vec2(uv.x * 6.0, uv.y * 20.0 + t * 0.25)) * 3.0);
      d.x += r * water.x * wet * unit;
    } else if (water.y < 1.5) {
      float r = sin(uv.y * 260.0 - t * 1.6 + uv.x * 18.0 + noise(uv * vec2(8.0, 30.0) + t * 0.2) * 3.0);
      d.y += r * water.x * wet * unit;
      d.x += r * water.x * 0.5 * wet * unit;
    } else {
      float r = sin(uv.x * 500.0 + t * 2.0) * sin(uv.y * 300.0 - t * 1.3);
      d.x += r * water.x * wet * unit;
    }
  }

  vec4 col = texture2D(img, uv + d);

  // The sky drifts: where it's sky here, and was sky where it comes from.
  // Northern lights waver in it, and brighten and fade, curtain by curtain.
  if (m.r > 0.001) {
    vec2 from = uv - vec2(drift, 0.0);
    vec4 back = texture2D(map, from);
    float waver = sin(from.y * 22.0 + t * 0.5 + from.x * 5.0) * 2.5 * back.a * auroraK * unit;
    vec4 sky = texture2D(img, from + vec2(d.x * wet + waver, 0.0));
    col = mix(col, sky, m.r * smoothstep(0.2, 0.9, back.r));
    if (auroraK > 0.0 && back.a > 0.003) {
      float streak = noise(vec2(from.x * 46.0 - t * 0.22, from.y * 2.2 + t * 0.04));
      float swell = 0.5 + 0.5 * sin(from.x * 7.0 - t * 0.4 + from.y * 3.0);
      col.rgb *= 1.0 + back.a * m.r * auroraK * (streak * 0.6 + swell * 0.4 - 0.45);
    }
  }

  // A sun breathing, where the drifting sky has taken it.
  if (sun.w > 0.5) {
    vec2 q = (uv - sun.xy - vec2(drift, 0.0)) * vec2(1.0, aspect);
    float glow = exp(-dot(q, q) / (sun.z * sun.z));
    float breathe = 0.5 + 0.5 * sin(t * 1.1);
    col.rgb += vec3(1.0, 0.82, 0.5) * glow * (0.07 + 0.06 * breathe);
  }

  // A window's light flickering, and in still water, its reflection.
  if (lamp.w > 0.5) {
    float flicker = 0.7 + 0.3 * noise(vec2(t * 2.6, 1.7));
    vec2 q = (uv - lamp.xy) * vec2(1.0, aspect);
    float glow = exp(-dot(q, q) / (lamp.z * lamp.z));
    if (water.w > 0.5 && water.y < 0.5) {
      vec2 r = (uv + vec2(d.x, 0.0) - vec2(lamp.x, 2.0 * water.z - lamp.y)) * vec2(1.4, aspect * 0.6);
      glow += 0.45 * exp(-dot(r, r) / (lamp.z * lamp.z));
    }
    col.rgb += vec3(1.0, 0.72, 0.36) * glow * 0.2 * flicker;
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

  if (show > 0.5) col.rgb = mix(col.rgb * 0.35, m.rgb + vec3(0.0, m.a, m.a) * 0.6, 0.75);
  gl_FragColor = vec4(col.rgb, 1.0);
}
`

export type Gl = {
  gl: WebGLRenderingContext
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
  gl.activeTexture(gl.TEXTURE0 + unit)
  gl.bindTexture(gl.TEXTURE_2D, gl.createTexture())
  set()
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
}

/** The painting and its map on the GPU, and what doesn't change from frame
 * to frame. Null where there's no WebGL. */
export function setup(canvas: HTMLCanvasElement, image: HTMLImageElement, mask: Mask, life: Life, show: boolean): Gl | null {
  const gl = canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, powerPreference: 'low-power' })
  if (!gl) return null
  const program = gl.createProgram()!
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX))
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT))
  gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? 'program')
  gl.useProgram(program)
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer())
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW)
  const p = gl.getAttribLocation(program, 'p')
  gl.enableVertexAttribArray(p)
  gl.vertexAttribPointer(p, 2, gl.FLOAT, false, 0, 0)
  texture(gl, 0, () => gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, image))
  texture(gl, 1, () => gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, mask.width, mask.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, mask.data))
  const u = (name: string) => gl.getUniformLocation(program, name)
  gl.uniform1i(u('img'), 0)
  gl.uniform1i(u('map'), 1)
  gl.uniform1f(u('aspect'), image.naturalHeight / image.naturalWidth)
  gl.uniform4f(u('wind'), life.wind?.amp ?? 0, life.wind?.speed ?? 0, life.wind ? 1 : 0, 0)
  gl.uniform4f(u('water'), life.water?.amp ?? 0, life.water ? { mirror: 0, waves: 1, river: 2 }[life.water.kind] : 0, life.water?.box[1] ?? 0, life.water ? 1 : 0)
  gl.uniform4f(u('sun'), life.sun?.at[0] ?? 0, life.sun?.at[1] ?? 0, life.sun?.r ?? 0, life.sun ? 1 : 0)
  gl.uniform4f(u('lamp'), life.lamp?.at[0] ?? 0, life.lamp?.at[1] ?? 0, life.lamp?.r ?? 0, life.lamp ? 1 : 0)
  gl.uniform1f(u('show'), show ? 1 : 0)
  gl.uniform1f(u('shade'), life.shadows ?? 0)
  gl.uniform4f(u('rays'), ...(life.rays?.box ?? [0, 0, 0, 0]))
  gl.uniform1f(u('raysK'), life.rays?.strength ?? 0)
  gl.uniform4f(u('mist'), ...(life.mist?.box ?? [0, 0, 0, 0]))
  gl.uniform1f(u('mistK'), life.mist?.strength ?? 0)
  gl.uniform1f(u('auroraK'), life.aurora?.strength ?? 0)
  return { gl, u }
}

/** Where the painting shows: how much of it across and down (`scale`), and
 * from where (`offset`), all 0–1 of it; drawn `w` × `h` px. */
export function frame({ gl, u }: Gl, w: number, h: number, scale: [number, number], offset: [number, number]) {
  gl.viewport(0, 0, w, h)
  gl.uniform2f(u('scale'), scale[0], scale[1])
  gl.uniform2f(u('offset'), offset[0], offset[1])
}

export function draw({ gl, u }: Gl, t: number, drift: number) {
  gl.uniform1f(u('t'), t)
  gl.uniform1f(u('drift'), drift)
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
}

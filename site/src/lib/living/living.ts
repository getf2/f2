// Brings a painting behind a section to life: once the page is idle and the
// section is near, the painting's map is worked out (in a worker), then a
// canvas fades in over the painting and draws it alive, 30 times a second,
// only while it's on screen. Where anything's missing (WebGL, workers) or
// fails, the painting just stays as it is.
import type { MaskReply, MaskRequest } from './mask.worker'
import { hasOverlays, overlays, type Place } from './overlays'
import { LIVES } from './paintings'
import { draw, frame, setup } from './shader'

const FPS = 30

/** Starts `el` (a .sky, its painting its background) living, its painting
 * `name` showing at `at` as background-position's percentages (0–1).
 * Returns what stops it. */
export function living(el: HTMLElement, name: string, at: [number, number]): () => void {
  const life = LIVES[name]
  if (!life || typeof Worker === 'undefined' || typeof OffscreenCanvas === 'undefined') return () => {}
  // (?sky=map: what moves where, while working on it.)
  const show = import.meta.env.DEV && new URLSearchParams(location.search).get('sky') === 'map'
  const stops: (() => void)[] = []
  let stopped = false
  const stop = () => {
    stopped = true
    stops.splice(0).forEach((f) => f())
  }

  // Started when the page is idle and the section is within a screen of being seen.
  const near = new IntersectionObserver(
    ([entry]) => {
      if (!entry.isIntersecting) return
      near.disconnect()
      const go = () =>
        !stopped &&
        start().catch((e) => {
          if (import.meta.env.DEV) console.warn('The painting stays still:', e)
          stop()
        })
      if ('requestIdleCallback' in window) requestIdleCallback(go, { timeout: 2500 })
      else setTimeout(go, 300)
    },
    { rootMargin: '100% 0px' },
  )
  near.observe(el)
  stops.push(() => near.disconnect())

  async function start() {
    const src = new URL(`/img/paint/${name}.webp`, location.href).href
    const worker = new Worker(new URL('./mask.worker.ts', import.meta.url), { type: 'module' })
    stops.push(() => worker.terminate())
    const reply = new Promise<MaskReply>((resolve, reject) => {
      worker.onmessage = (e: MessageEvent<MaskReply>) => resolve(e.data)
      worker.onerror = reject
    })
    worker.postMessage({ src, life } satisfies MaskRequest)
    const image = new Image()
    image.src = src
    await image.decode()
    const got = await reply
    worker.terminate()
    if (stopped || 'error' in got) return stop()
    const { mask, stars } = got

    const wrap = document.createElement('div')
    wrap.className = 'living'
    const glCanvas = document.createElement('canvas')
    const fxCanvas = hasOverlays(life) && !show ? document.createElement('canvas') : null
    // (appendChild, not append: the Workers types give Element their HTMLRewriter's.)
    wrap.appendChild(glCanvas)
    if (fxCanvas) wrap.appendChild(fxCanvas)
    const gl = setup(glCanvas, image, mask, life, show)
    if (!gl) return stop()
    el.appendChild(wrap)
    stops.push(() => {
      gl.gl.getExtension('WEBGL_lose_context')?.loseContext()
      wrap.remove()
    })
    glCanvas.addEventListener('webglcontextlost', stop)
    const fx = fxCanvas?.getContext('2d') ?? null

    // Fitted as background-size: cover is, at `at`. The sky's box reaches a
    // little past its section's sides (styles.css): what's past them, and
    // what cover leaves out, is the sky's room to drift. It drifts there
    // and back, slowly, so it never runs out of painting.
    const [iw, ih] = [image.naturalWidth, image.naturalHeight]
    let size = { w: 0, h: 0, shown: [1, 1] as [number, number], offset: [0, 0] as [number, number], room: 0 }
    const resize = () => {
      const w = el.clientWidth
      const h = el.clientHeight
      if (!w || !h) return
      const cover = Math.max(w / iw, h / ih)
      const shown: [number, number] = [w / (iw * cover), h / (ih * cover)]
      const offset: [number, number] = [(1 - shown[0]) * at[0], (1 - shown[1]) * at[1]]
      const seen = Math.min(1, (el.parentElement?.clientWidth ?? w) / w)
      const past = ((1 - seen) / 2) * shown[0]
      const room = Math.max(0, Math.min(offset[0], 1 - shown[0] - offset[0]) + past - 0.004)
      // No sharper than the painting itself.
      const ratio = Math.max(0.5, Math.min(window.devicePixelRatio || 1, (iw * shown[0]) / w))
      size = { w, h, shown, offset, room }
      glCanvas.width = Math.round(w * ratio)
      glCanvas.height = Math.round(h * ratio)
      frame(gl, glCanvas.width, glCanvas.height, shown, offset)
      if (fxCanvas) {
        const r = Math.min(window.devicePixelRatio || 1, 2)
        fxCanvas.width = Math.round(w * r)
        fxCanvas.height = Math.round(h * r)
      }
      paint()
    }
    const place: Place = ([x, y]) => [((x - size.offset[0]) / size.shown[0]) * size.w, ((y - size.offset[1]) / size.shown[1]) * size.h]

    // Time goes by only while it's seen.
    let t = 0
    const speed = life.drift / 1920
    const drift = () => {
      const amp = Math.min(size.room, (speed * 120) / (2 * Math.PI))
      const period = Math.max(60, (2 * Math.PI * amp) / Math.max(speed, 1e-6))
      return amp * Math.sin((2 * Math.PI * t) / period)
    }
    function paint() {
      const d = drift()
      draw(gl!, t, d)
      if (fx && fxCanvas) {
        const r = fxCanvas.width / size.w
        fx.setTransform(r, 0, 0, r, 0, 0)
        fx.clearRect(0, 0, size.w, size.h)
        overlays(fx, life, t, place, d, stars, size.w / size.shown[0] / 1920)
      }
    }

    let raf = 0
    let last = 0
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick)
      const dt = (now - last) / 1000
      if (dt < 1 / (FPS + 1)) return
      last = now
      t += Math.min(dt, 0.1)
      paint()
    }
    let onScreen = false
    const run = () => {
      cancelAnimationFrame(raf)
      if (!onScreen || document.hidden) return
      last = performance.now()
      raf = requestAnimationFrame(tick)
    }
    const watch = new IntersectionObserver(([entry]) => {
      onScreen = entry.isIntersecting
      run()
    })
    watch.observe(el.parentElement ?? el)
    document.addEventListener('visibilitychange', run)
    const resized = new ResizeObserver(resize)
    resized.observe(el)
    stops.push(() => {
      cancelAnimationFrame(raf)
      watch.disconnect()
      resized.disconnect()
      document.removeEventListener('visibilitychange', run)
    })
    resize()
    // In once it's drawn: the same picture as the painting under it, then moving.
    requestAnimationFrame(() => wrap.classList.add('on'))
  }

  return stop
}

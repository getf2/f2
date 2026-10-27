// Works out a painting's map (masks.ts) and its stars off the main thread,
// so the page doesn't stutter while it's done.
import { findStars, makeMask, MASK_WIDTH } from './masks'
import type { Life } from './paintings'

export type MaskRequest = { src: string; life: Life }
export type MaskReply = { mask: ReturnType<typeof makeMask>; stars: [number, number][] } | { error: string }

const scope = self as unknown as {
  onmessage: ((e: MessageEvent<MaskRequest>) => void) | null
  postMessage(message: MaskReply, transfer?: Transferable[]): void
}

// The painting's pixels at `width` across.
function pixels(image: ImageBitmap, width: number) {
  const height = Math.round((image.height / image.width) * width)
  const canvas = new OffscreenCanvas(width, height)
  const ctx = canvas.getContext('2d', { willReadFrequently: true })!
  ctx.drawImage(image, 0, 0, width, height)
  return { px: ctx.getImageData(0, 0, width, height).data, width, height }
}

scope.onmessage = async ({ data: { src, life } }) => {
  try {
    const image = await createImageBitmap(await (await fetch(src)).blob())
    const m = pixels(image, MASK_WIDTH)
    const mask = makeMask(m.px, m.width, m.height, life)
    const s = life.stars ? pixels(image, 400) : null
    const stars = s && life.stars ? findStars(s.px, s.width, s.height, life.stars) : []
    scope.postMessage({ mask, stars }, [mask.data.buffer])
  } catch (e) {
    scope.postMessage({ error: String(e) })
  }
}

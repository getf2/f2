// Only the sound of a composition, as WAV (quick: no frames are drawn), to
// put back under a picture already rendered:
//   bun scripts/audio.ts F2Feature out/f2-audio.wav
import path from 'node:path'
import { bundle } from '@remotion/bundler'
import { renderMedia, selectComposition } from '@remotion/renderer'

const [id, output] = process.argv.slice(2)
const serveUrl = await bundle({ entryPoint: path.resolve('src/index.ts') })
const composition = await selectComposition({ serveUrl, id })
await renderMedia({ serveUrl, composition, codec: 'wav', outputLocation: output })
console.log(output)

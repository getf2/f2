// Stills of a composition at given frames, bundled once:
//   bun scripts/stills.ts S2-Ask 75 125 210 [--scale=0.5] [--out=/tmp/x] [--props='{"show":true}']
import path from 'node:path'
import { bundle } from '@remotion/bundler'
import { renderStill, selectComposition } from '@remotion/renderer'

const args = process.argv.slice(2)
const flags = Object.fromEntries(args.filter((a) => a.startsWith('--')).map((a) => a.slice(2).split('=')))
const [id, ...frames] = args.filter((a) => !a.startsWith('--'))
const scale = Number(flags.scale ?? 1)
const out = flags.out ?? 'out/stills'

const serveUrl = await bundle({ entryPoint: path.resolve('src/index.ts') })
const inputProps = flags.props ? JSON.parse(flags.props) : {}
const composition = await selectComposition({ serveUrl, id, inputProps })
for (const f of frames) {
	const file = path.join(out, `${id}-${f}.png`)
	await renderStill({ serveUrl, composition, frame: Number(f), output: file, scale, inputProps })
	console.log(file)
}

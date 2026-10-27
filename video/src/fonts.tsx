// The site's fonts, from @fontsource-variable, held until they've loaded so
// no frame is drawn in a fallback.
import '@fontsource-variable/fraunces/full.css'
import '@fontsource-variable/source-sans-3'
import '@fontsource-variable/geist-mono'

import { useEffect, useState } from 'react'
import { continueRender, delayRender } from 'remotion'

const FACES = [
	"600 64px 'Fraunces Variable'",
	"400 32px 'Source Sans 3 Variable'",
	"500 32px 'Geist Mono Variable'",
]

let ready: Promise<unknown> | null = null

export function useFonts() {
	const [handle] = useState(() => delayRender('Loading fonts'))
	useEffect(() => {
		ready ??= Promise.all(FACES.map((face) => document.fonts.load(face)))
		ready.then(() => continueRender(handle))
	}, [handle])
}

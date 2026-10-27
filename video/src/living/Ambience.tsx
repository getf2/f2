// A painting's sound: its beds (birdsong, wind, water, crickets), carrying
// on across scenes that show it, and a call as each flight of birds comes.
// Faded in and out with the scenes' cross-fades.
import { Audio } from '@remotion/media'
import { useContext } from 'react'
import { interpolate, Sequence, staticFile, useVideoConfig } from 'remotion'
import { SceneTime } from './LivingPainting'
import { LIVES } from './paintings'

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const

export function Ambience({ name, fadeOut = 0.4 }: { name: string; fadeOut?: number }) {
	const time = useContext(SceneTime)
	const { fps, durationInFrames } = useVideoConfig()
	const life = LIVES[name]
	// (Where this scene comes in, in the painting's sound.)
	const into = Math.round((time.start - time.spanStart) * fps)
	const fadeIn = Math.round((time.start === 0 ? 0.8 : 0.4) * fps)
	const out = Math.round(fadeOut * fps)
	const volume = (k: number) => (f: number) => k * interpolate(f, [0, fadeIn, durationInFrames - out, durationInFrames], [0, 1, 1, 0], clamp)
	return (
		<>
			{life.sound.map((s) => (
				<Audio key={s.src} src={staticFile(`ambience/${s.src}.mp3`)} trimBefore={into} volume={volume(s.volume)} />
			))}
			{(life.birds ?? []).map((flight, i) => {
				if (!flight.call) return null
				// (As they come into view.)
				const at = Math.round((time.spanStart + flight.at + 0.7 - time.start) * fps)
				if (at < 0 || at >= durationInFrames - fps) return null
				return (
					<Sequence key={i} from={at} durationInFrames={Math.round(2.5 * fps)} layout="none" name={flight.call}>
						<Audio src={staticFile(`ambience/${flight.call}.mp3`)} volume={0.45} />
					</Sequence>
				)
			})}
		</>
	)
}

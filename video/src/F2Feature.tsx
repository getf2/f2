// The whole video: the scenes one after another, each cross-fading into the
// next.
import { linearTiming, TransitionSeries } from '@remotion/transitions'
import { fade } from '@remotion/transitions/fade'
import { AbsoluteFill } from 'remotion'
import { CROSS, SCENES } from './scenes'

export const FEATURE_LENGTH = SCENES.reduce((sum, s) => sum + s.length, 0) - CROSS * (SCENES.length - 1)

export function F2Feature() {
	return (
		<AbsoluteFill style={{ background: '#000' }}>
			<TransitionSeries>
				{SCENES.flatMap(({ component: Scene, length }, i) => [
					...(i > 0 ? [<TransitionSeries.Transition key={`t${i}`} presentation={fade()} timing={linearTiming({ durationInFrames: CROSS })} />] : []),
					<TransitionSeries.Sequence key={i} durationInFrames={length}>
						<Scene />
					</TransitionSeries.Sequence>,
				])}
			</TransitionSeries>
		</AbsoluteFill>
	)
}

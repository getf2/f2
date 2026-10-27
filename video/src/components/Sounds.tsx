// Every sound a scene's script makes, each at its frame.
import { Audio } from '@remotion/media'
import { Sequence, staticFile } from 'remotion'
import { sec } from '../theme'
import { type Timeline } from '../timeline'

export function Sounds({ timeline }: { timeline: Timeline }) {
	return (
		<>
			{timeline.cues.map((cue, i) => (
				<Sequence key={i} from={cue.frame} durationInFrames={sec(cue.src.includes('whoosh') ? 1 : 0.5)} layout="none" name={cue.src}>
					<Audio src={staticFile(cue.src)} volume={cue.volume} />
				</Sequence>
			))}
		</>
	)
}

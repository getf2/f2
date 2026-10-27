// The paintings alone, alive, with their sound, to judge them by (Remotion
// Studio's "Life" folder): each for as long as the video shows it, its
// time the video's.
import { AbsoluteFill } from 'remotion'
import { SCENES } from '../scenes'
import { FPS } from '../theme'
import { Ambience } from './Ambience'
import { LivingPainting, SceneTime } from './LivingPainting'

export type LifeProps = { name: string; show: boolean }

/** Where the video first shows a painting, and for how long (frames). */
export function span(name: string) {
	const shown = SCENES.filter((s) => s.painting === name)
	const first = shown[0]?.time ?? { start: 0, spanStart: 0, center: 3 }
	const end = shown.length ? shown[shown.length - 1].time.start + shown[shown.length - 1].length / FPS : 6
	return { time: { ...first, start: first.spanStart }, frames: Math.round((end - first.spanStart) * FPS) }
}

export function LifePreview({ name, show }: LifeProps) {
	return (
		<SceneTime.Provider value={span(name).time}>
			<AbsoluteFill>
				<LivingPainting name={name} width={1920} height={1080} show={show} />
				{!show && <Ambience name={name} />}
			</AbsoluteFill>
		</SceneTime.Provider>
	)
}

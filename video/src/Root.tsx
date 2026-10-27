import { Composition, Folder } from 'remotion'
import { CheckFinder, CheckQuickOpen, type CheckProps } from './Check'
import { F2Feature, FEATURE_LENGTH } from './F2Feature'
import { LIVES } from './living/paintings'
import { LifePreview, span, type LifeProps } from './living/Preview'
import { SCENES } from './scenes'
import { FPS, HEIGHT, WIDTH } from './theme'

const frame = { fps: FPS, width: WIDTH, height: HEIGHT }

export function Root() {
	return (
		<>
			<Composition id="F2Feature" component={F2Feature} durationInFrames={FEATURE_LENGTH} {...frame} />
			<Folder name="Scenes">
				{SCENES.map((s) => (
					<Composition key={s.id} id={s.id} component={s.component} durationInFrames={s.length} {...frame} />
				))}
			</Folder>
			<Folder name="Life">
				{Object.keys(LIVES).map((name) => (
					<Composition key={name} id={`Life-${name}`} component={LifePreview} durationInFrames={span(name).frames} {...frame} defaultProps={{ name, show: false } satisfies LifeProps} />
				))}
			</Folder>
			<Folder name="Check">
				<Composition
					id="Check-QuickOpen"
					component={CheckQuickOpen}
					durationInFrames={1}
					fps={FPS}
					width={1360}
					height={1096}
					defaultProps={{ mode: 'overlay' } satisfies CheckProps}
				/>
				<Composition
					id="Check-Finder"
					component={CheckFinder}
					durationInFrames={1}
					fps={FPS}
					width={2800}
					height={1680}
					defaultProps={{ mode: 'overlay' } satisfies CheckProps}
				/>
			</Folder>
		</>
	)
}

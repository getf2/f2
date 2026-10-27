// The scenes, in order: each its own composition, and F2Feature strings
// them together.
import { type FC } from 'react'
import { SceneTime } from '../living/LivingPainting'
import { FPS } from '../theme'
import { Scene1Find, SCENE1_LENGTH } from './Scene1Find'
import { Scene2Ask, SCENE2_LENGTH } from './Scene2Ask'
import { Scene3Search, SCENE3_LENGTH } from './Scene3Search'
import { Scene4Space, SCENE4_LENGTH } from './Scene4Space'
import { Scene5Tidy, SCENE5_LENGTH } from './Scene5Tidy'
import { Scene6Panes, SCENE6_LENGTH } from './Scene6Panes'
import { Scene7Tools, SCENE7_LENGTH } from './Scene7Tools'
import { Scene8End, SCENE8_LENGTH } from './Scene8End'

type Scene = { id: string; component: FC; length: number; painting: string }

const LIST: Scene[] = [
	{ id: 'S1-Find', component: Scene1Find, length: SCENE1_LENGTH, painting: 'hero-valley' },
	{ id: 'S2-Ask', component: Scene2Ask, length: SCENE2_LENGTH, painting: 'hero-valley' },
	{ id: 'S3-Search', component: Scene3Search, length: SCENE3_LENGTH, painting: 'meadow' },
	{ id: 'S4-Space', component: Scene4Space, length: SCENE4_LENGTH, painting: 'terraces' },
	{ id: 'S5-Tidy', component: Scene5Tidy, length: SCENE5_LENGTH, painting: 'mist-forest' },
	{ id: 'S6-Panes', component: Scene6Panes, length: SCENE6_LENGTH, painting: 'still-lake' },
	{ id: 'S7-Tools', component: Scene7Tools, length: SCENE7_LENGTH, painting: 'dusk-sea' },
	{ id: 'S8-End', component: Scene8End, length: SCENE8_LENGTH, painting: 'golden-hour' },
]

/** How long scenes cross-fade into each other (frames). */
export const CROSS = 24

// Each scene's place in the whole video, and its painting's: where that was
// first shown and the middle of its showing (for the sky's drift), so a
// painting carries on across scenes, rendered alone or together.
const starts = LIST.map((_, i) => LIST.slice(0, i).reduce((sum, s) => sum + s.length - CROSS, 0) / FPS)

export const SCENES = LIST.map((scene, i) => {
	let first = i
	while (first > 0 && LIST[first - 1].painting === scene.painting) first--
	let last = i
	while (last < LIST.length - 1 && LIST[last + 1].painting === scene.painting) last++
	const spanStart = starts[first]
	const spanEnd = starts[last] + LIST[last].length / FPS
	const time = { start: starts[i], spanStart, center: (spanStart + spanEnd) / 2 }
	const Scene = scene.component
	const component: FC = () => (
		<SceneTime.Provider value={time}>
			<Scene />
		</SceneTime.Provider>
	)
	return { ...scene, component, time }
})

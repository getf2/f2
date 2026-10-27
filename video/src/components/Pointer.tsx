// The macOS arrow, a little bigger than life so it reads in the video. Its
// tip is the point it's at; a click presses it down a touch.
import { useCurrentFrame } from 'remotion'
import { pointerAt, type Timeline } from '../timeline'

const HEIGHT = 28

export function Pointer({ timeline }: { timeline: Timeline }) {
	const frame = useCurrentFrame()
	const at = pointerAt(timeline, frame)
	if (!at) return null
	return (
		<svg
			width={HEIGHT * 0.72}
			height={HEIGHT}
			viewBox="0 0 18 25"
			style={{
				position: 'absolute',
				left: at.x - 2.2,
				top: at.y - 1.6,
				overflow: 'visible',
				transformOrigin: '2.2px 1.6px',
				transform: `scale(${1 - at.press * 0.12})`,
				filter: 'drop-shadow(0 1.5px 2px rgba(0, 0, 0, 0.35))',
				zIndex: 1000,
			}}
		>
			<path
				d="M2.2 1.6 L2.2 20.4 L6.6 16.3 L9.6 23.2 L13 21.8 L10 15 L16 15 Z"
				fill="#000"
				stroke="#fff"
				strokeWidth={1.6}
				strokeLinejoin="round"
			/>
		</svg>
	)
}

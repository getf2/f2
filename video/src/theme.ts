// The video's frame, the site's colors and fonts, and the scale the Mac's
// screen is drawn at.

export const FPS = 60
export const WIDTH = 1920
export const HEIGHT = 1080

// The desktop is laid out in points, as macOS lays it out, on a 1536×864
// screen, and drawn 1.25× so F2's text reads on a phone.
export const SCALE = 1.25
export const SCREEN = { width: WIDTH / SCALE, height: HEIGHT / SCALE }

// The site's :root.
export const INK = '#0d1726'
export const INK2 = '#4a5468'
export const PAPER = '#fbf9f4'
export const LINE = '#e8e3d7'
export const SKY = '#1d4aa8'
export const SKY2 = '#123a8c'

export const FONT = {
	// Captions and titles: Fraunces, soft and not wonky, as the site's.
	display: "'Fraunces Variable', Georgia, serif",
	// The video's own text.
	sans: "'Source Sans 3 Variable', system-ui, sans-serif",
	// Keycaps: Geist Mono, with the system's font for ⌘ ⇧ ⌥ ⌃, which it lacks.
	mono: "'Geist Mono Variable', ui-monospace, -apple-system, system-ui, monospace",
	// F2's own windows are in the system's font (SF Pro), as the app is.
	ui: "-apple-system, BlinkMacSystemFont, system-ui, sans-serif",
}

// The site's motion: a strong ease-out.
export const EASE_OUT = [0.23, 1, 0.32, 1] as const

export const sec = (s: number) => Math.round(s * FPS)

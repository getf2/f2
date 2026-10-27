// How each painting comes alive: where its sky meets the land (a line
// across, in 0–1 of the painting), where the wind moves things and how
// hard, where water is and what it does, what light breathes, and what
// flies or floats past. (The same as the feature video's, in video/, less
// its sounds.)

type Box = [number, number, number, number]

/** Birds crossing: from and to (0–1 of the painting), when (s after the
 * painting's first seen, then again every REPEAT s) and for how long, how
 * big (px with the painting 1920 wide), how many; mirrored in still water
 * at `reflect` (its waterline). */
export type Flight = { at: number; dur: number; from: [number, number]; to: [number, number]; size: number; count: number; reflect?: number }

export const REPEAT = 40

export type Life = {
  /** The skyline: points [x, y] left to right. Above it, sky and clouds
   * drift; it's the most the sky can reach down (colors decide the rest). */
  horizon: [number, number][]
  /** Clouds' drift, in px a second with the painting 1920 wide. */
  drift: number
  /** Boxes the sky's drift fades out in: what reaches up into the sky in a
   * sky's colors (pampas plumes), held still. */
  still?: Box[]
  /** Wind: from this far down it starts, growing to `amp` px at the bottom
   * (or only in `region`); in `strong` boxes, all of it (tall grasses). */
  wind?: { from: number; amp: number; speed: number; region?: Box; strong?: Box[] }
  /** Water: a box; a mirror drifts with the sky, waves roll, a river glints.
   * A `fixed` mirror's reflections only ripple (where they're painted not
   * quite in line with what they reflect). */
  water?: { box: Box; kind: 'mirror' | 'waves' | 'river'; amp: number; fixed?: boolean }
  /** A sun that glows: [x, y], radius (0–1 of width). */
  sun?: { at: [number, number]; r: number }
  /** A lit window that flickers, and its reflection if there's a mirror. */
  lamp?: { at: [number, number]; r: number }
  /** Northern lights that shimmer: their colors, in a box. */
  aurora?: { box: Box; strength: number }
  /** Sunbeams that brighten and fade, in a box. */
  rays?: { box: Box; strength: number }
  /** Mist drifting through a box. */
  mist?: { box: Box; strength: number }
  /** Cloud shadows sliding over the land (0–1: how dark). */
  shadows?: number
  birds?: Flight[]
  /** Specks floating in the light: pollen, dust; color as "r, g, b". */
  motes?: { box: Box; count: number; color: string; size: number; rise: number }
  /** Stars that twinkle. */
  stars?: { box: Box; count: number }
}

export const LIVES: Record<string, Life> = {
  'hero-valley': {
    horizon: [
      [0, 0.585],
      [0.2, 0.575],
      [0.45, 0.58],
      [0.6, 0.585],
      [0.8, 0.565],
      [1, 0.575],
    ],
    drift: 7,
    wind: { from: 0.72, amp: 3.6, speed: 0.55 },
    water: { box: [0, 0.62, 1, 0.82], kind: 'river', amp: 0.8 },
    shadows: 0.5,
    birds: [
      { at: 3, dur: 7, from: [1.03, 0.33], to: [0.74, 0.12], size: 20, count: 2 },
      { at: 21, dur: 7.5, from: [-0.03, 0.3], to: [0.25, 0.14], size: 17, count: 3 },
    ],
  },
  meadow: {
    horizon: [
      [0, 0.552], [0.05, 0.576], [0.1, 0.548], [0.15, 0.596], [0.2, 0.592], [0.25, 0.588], [0.3, 0.604], [0.35, 0.6], [0.4, 0.596], [0.45, 0.616], [0.5, 0.608],
      [0.55, 0.612], [0.6, 0.56], [0.65, 0.524], [0.7, 0.552], [0.75, 0.492], [0.8, 0.548], [0.85, 0.504], [0.9, 0.512], [0.95, 0.516], [1, 0.52],
    ],
    drift: 7,
    wind: { from: 0.58, amp: 5, speed: 0.6 },
    motes: { box: [0, 0.55, 1, 1], count: 26, color: '255, 252, 235', size: 4.2, rise: 18 },
    birds: [{ at: 1.2, dur: 5.5, from: [-0.03, 0.14], to: [0.3, 0.05], size: 16, count: 3 }],
  },
  terraces: {
    horizon: [
      [0, 0.388], [0.05, 0.38], [0.1, 0.36], [0.15, 0.36], [0.2, 0.372], [0.25, 0.372], [0.3, 0.356], [0.35, 0.364], [0.4, 0.376], [0.45, 0.384], [0.5, 0.36],
      [0.55, 0.34], [0.6, 0.348], [0.65, 0.348], [0.7, 0.34], [0.75, 0.328], [0.8, 0.336], [0.85, 0.352], [0.9, 0.376], [0.95, 0.388], [1, 0.404],
    ],
    drift: 8,
    // (No wind: the terraces' rings would waver.)
    shadows: 0.75,
    birds: [{ at: 2.6, dur: 6.5, from: [1.03, 0.26], to: [0.8, 0.14], size: 22, count: 1 }],
  },
  'mist-forest': {
    // (The sky's only up to the right, between the trees.)
    horizon: [
      [0, 0],
      [0.55, 0],
      [0.565, 0.32],
      [0.6, 0.52],
      [0.75, 0.56],
      [0.88, 0.52],
      [0.9, 0],
      [1, 0],
    ],
    drift: 6,
    wind: { from: 0.86, amp: 2.2, speed: 0.45 },
    rays: { box: [0.1, 0, 0.62, 0.95], strength: 0.09 },
    mist: { box: [0, 0.55, 1, 1], strength: 0.14 },
    motes: { box: [0.15, 0.1, 0.6, 0.9], count: 30, color: '235, 255, 248', size: 3.6, rise: 10 },
  },
  'still-lake': {
    // (Traced: where the mountains, far ones too, start under the sky.)
    horizon: [
      [0, 0.344], [0.05, 0.364], [0.1, 0.4], [0.15, 0.388], [0.2, 0.376], [0.25, 0.392], [0.3, 0.424], [0.35, 0.424], [0.4, 0.428], [0.45, 0.456], [0.5, 0.472],
      [0.55, 0.452], [0.6, 0.436], [0.65, 0.436], [0.7, 0.396], [0.75, 0.356], [0.8, 0.344], [0.85, 0.352], [0.9, 0.34], [0.95, 0.324], [1, 0.3],
    ],
    drift: 7,
    wind: { from: 0.78, amp: 2.4, speed: 0.5, region: [0, 0.75, 0.22, 1] },
    water: { box: [0, 0.535, 1, 1], kind: 'mirror', amp: 1.4 },
    // (Birds over the lake, and in it.)
    birds: [{ at: 2, dur: 7, from: [0.6, 0.3], to: [1.04, 0.33], size: 21, count: 2, reflect: 0.535 }],
  },
  'dusk-sea': {
    horizon: [
      [0, 0.52],
      [0.05, 0.528],
      [0.1, 0.532],
      [0.15, 0.556],
      [0.2, 0.572],
      [0.25, 0.56],
      [0.3, 0.572],
      [0.35, 0.6],
      [0.4, 0.611],
      [1, 0.62],
    ],
    drift: 6,
    wind: { from: 0.55, amp: 3.2, speed: 0.5, region: [0, 0.5, 0.42, 1] },
    water: { box: [0.36, 0.622, 1, 1], kind: 'waves', amp: 1.6 },
    stars: { box: [0, 0, 1, 0.3], count: 26 },
    birds: [{ at: 2.6, dur: 7, from: [1.03, 0.4], to: [0.8, 0.3], size: 20, count: 2 }],
  },
  'golden-hour': {
    horizon: [
      [0, 0.4],
      [0.1, 0.44],
      [0.2, 0.57],
      [0.5, 0.6],
      [0.8, 0.58],
      [1, 0.56],
    ],
    drift: 4,
    still: [[0, 0.28, 0.17, 1]],
    wind: { from: 0.55, amp: 3.4, speed: 0.45, region: [0, 0.3, 1, 1], strong: [[0, 0.3, 0.17, 1]] },
    sun: { at: [0.355, 0.535], r: 0.06 },
    motes: { box: [0.1, 0.35, 0.9, 0.95], count: 34, color: '255, 226, 160', size: 4.6, rise: 9 },
    birds: [{ at: 1.5, dur: 7, from: [0.95, 0.32], to: [0.72, 0.17], size: 15, count: 2 }],
  },
  'night-aurora': {
    // (Traced: the mountains' ridges, the far ones between.)
    horizon: [
      [0, 0.369], [0.025, 0.4], [0.05, 0.418], [0.075, 0.44], [0.1, 0.503], [0.125, 0.539], [0.15, 0.579], [0.175, 0.575], [0.2, 0.57], [0.225, 0.575],
      [0.25, 0.548], [0.275, 0.53], [0.3, 0.544], [0.325, 0.557], [0.35, 0.588], [0.375, 0.611], [0.4, 0.611], [0.425, 0.624], [0.45, 0.651], [0.475, 0.674],
      [0.5, 0.656], [0.525, 0.647], [0.55, 0.642], [0.575, 0.651], [0.6, 0.651], [0.625, 0.647], [0.65, 0.638], [0.675, 0.602], [0.7, 0.606], [0.725, 0.597],
      [0.75, 0.566], [0.775, 0.539], [0.8, 0.53], [0.825, 0.544], [0.85, 0.553], [0.875, 0.557], [0.9, 0.494], [0.925, 0.472], [0.95, 0.405], [0.975, 0.373], [1, 0.351],
    ],
    drift: 5,
    water: { box: [0, 0.752, 1, 1], kind: 'mirror', amp: 1.2, fixed: true },
    // (Not the clouds to the right: their blue-gray shade reads as lights.)
    aurora: { box: [0, 0, 0.74, 0.62], strength: 0.38 },
    lamp: { at: [0.655, 0.716], r: 0.016 },
    stars: { box: [0, 0, 1, 0.5], count: 40 },
  },
}

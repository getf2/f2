import type { CSSProperties } from 'react'

// The paintings in public/img/paint.
export type Paint =
  | 'hero-valley'
  | 'still-lake'
  | 'night-aurora'
  | 'meadow'
  | 'study-window'
  | 'golden-hour'
  | 'terraces'
  | 'mist-forest'
  | 'dusk-sea'

// A real F2 screenshot from public/img/shots, placed on the painting in
// percentages of the plate (`at`).
export type Shot = { src: string; w: number; h: number; alt: string; at: CSSProperties }

// What a plate shows: a painting, and on it real screenshots, or one of the
// glass sketches below where there's no screenshot yet, or what's typed into
// Quick Open.
export type Art = {
  paint: Paint
  // The painting's vertical position, as background-position's.
  y?: string
  shots?: Shot[]
  glass?: 'menu' | 'snap' | 'marquee' | 'recording' | 'dictation' | 'notch'
  queries?: string[]
}

export function Plate({ art }: { art: Art }) {
  return (
    <div
      className="plate"
      style={{ backgroundImage: `url(/img/paint/${art.paint}.webp)`, backgroundPosition: `center ${art.y ?? '50%'}` }}
    >
      {art.shots?.map((s) => (
        <img
          key={s.src}
          className="win"
          src={`/img/shots/${s.src}.webp`}
          width={s.w}
          height={s.h}
          loading="lazy"
          alt={s.alt}
          style={s.at}
        />
      ))}
      {art.queries && (
        <div className="g-queries" aria-hidden="true">
          {art.queries.map((q) => (
            <span key={q}>{q}</span>
          ))}
        </div>
      )}
      {art.glass === 'menu' && <Menu />}
      {art.glass === 'snap' && <Snap />}
      {(art.glass === 'marquee' || art.glass === 'recording') && <Marquee recording={art.glass === 'recording'} />}
      {art.glass === 'dictation' && <Dictation />}
      {art.glass === 'notch' && <Notch />}
    </div>
  )
}

// A photo's Quick Actions, as F2's right-click menu lists them.
function Menu() {
  const items = ['Rotate Left', 'Create PDF', 'Convert to JPEG', 'Make Smaller', 'Remove Background', 'Copy Text']
  return (
    <div className="g-menu" aria-hidden="true">
      <b>Quick Actions</b>
      {items.map((i) => (
        <span key={i} className={i === 'Make Smaller' ? 'on' : undefined}>
          {i}
        </span>
      ))}
    </div>
  )
}

// Three windows snapped into a half and two quarters.
function Snap() {
  return (
    <div className="g-snap" aria-hidden="true">
      <div>Left Half</div>
      <div>Top Right Quarter</div>
      <div>Bottom Right Quarter</div>
    </div>
  )
}

// An area being picked for a screenshot, or being recorded.
function Marquee({ recording }: { recording: boolean }) {
  return (
    <div className={recording ? 'g-marquee rec' : 'g-marquee'} aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
      <span>{recording ? '0:42' : '1280 × 720'}</span>
    </div>
  )
}

// What was said, typed out, over the pill that listens at the bottom of the
// screen, its bars moving with a voice.
function Dictation() {
  return (
    <div className="g-dictation" aria-hidden="true">
      <p>Running ten minutes late, save me a seat by the window</p>
      <div>
        {Array.from({ length: 9 }, (_, i) => (
          <i key={i} style={{ animationDelay: `${-((i * 0.37) % 1.1).toFixed(2)}s` }} />
        ))}
      </div>
    </div>
  )
}

// The notch opened, as F2 draws it, hanging from the top of the screen under
// the menu bar: what's playing, a timer to stop, and the last note.
function Notch() {
  return (
    <div className="g-notch" aria-hidden="true">
      <div className="bar">
        <b>F2</b>
        <span>File</span>
        <span>Edit</span>
        <span>View</span>
        <span className="clock">9:41</span>
      </div>
      <div className="island">
        <div className="song">
          <i className="art" />
          <div className="words">
            <b>Northern Lights</b>
            <span>Aurora Hall</span>
          </div>
          <div className="keys">
            <svg viewBox="0 0 24 24">
              <path d="M11 6 3 12l8 6zM21 6l-8 6 8 6z" />
            </svg>
            <svg viewBox="0 0 24 24">
              <rect x="6" y="5" width="4" height="14" rx="1" />
              <rect x="14" y="5" width="4" height="14" rx="1" />
            </svg>
            <svg viewBox="0 0 24 24">
              <path d="m3 6 8 6-8 6zM13 6l8 6-8 6z" />
            </svg>
          </div>
        </div>
        <div className="going">
          <svg viewBox="0 0 24 24" className="timer">
            <circle cx="12" cy="13.5" r="7.5" />
            <path d="M12 13.5V9M10 3.5h4" />
          </svg>
          <b>Tea</b>
          <em>4:12</em>
          <svg viewBox="0 0 24 24" className="stop">
            <path d="m9 9 6 6m0-6-6 6" />
          </svg>
        </div>
        <div className="note">
          <svg viewBox="0 0 24 24" className="tile">
            <path d="m7 12.5 3.5 3.5L17 9" />
          </svg>
          <div className="words">
            <b>Copied</b>
            <span>1,204 characters</span>
          </div>
          <small>Now</small>
        </div>
      </div>
    </div>
  )
}

import { useRef, useState } from 'react'

import { VIDEO_SECONDS, VIDEO_URL } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'

// The feature video, right under the hero: its first moments as a still,
// played with sound only when asked (nothing's fetched before that), and the
// still back once it's over.
export function Watch() {
  const [ref, reveal] = useReveal<HTMLElement>()
  const video = useRef<HTMLVideoElement>(null)
  const [playing, setPlaying] = useState(false)
  const [watched, setWatched] = useState(false)

  // play() in the click itself: Safari plays with sound only then. Then all
  // of it in view, if it isn't (the heading above can push its foot out).
  const play = () => {
    const v = video.current
    if (!v) return
    v.play()
    setPlaying(true)
    const r = v.getBoundingClientRect()
    if (r.top < 0 || r.bottom > innerHeight) v.scrollIntoView({ block: 'center' })
  }

  return (
    <section ref={ref} className={`watch ${reveal}`} id="watch">
      <h2>See it in a minute.</h2>
      <p>Find a file, ask a question, free up space and tidy Downloads, with every key and click. Turn your sound on.</p>
      <div className={playing ? 'screen playing' : 'screen'}>
        <video
          ref={video}
          src={VIDEO_URL}
          width={1920}
          height={1080}
          preload="none"
          playsInline
          controls={playing}
          onEnded={() => {
            setPlaying(false)
            setWatched(true)
            if (video.current) video.current.currentTime = 0
          }}
          aria-label="F2's feature video"
        />
        <button
          className="poster"
          type="button"
          onClick={play}
          inert={playing}
          aria-label={`${watched ? 'Watch again' : 'Watch'}: F2's video, ${VIDEO_SECONDS} seconds, with sound`}
        >
          <img src="/img/video/f2-feature.webp" width={1920} height={1080} loading="lazy" alt="" />
          <span className="play" aria-hidden="true">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" />
            </svg>
            {watched ? 'Watch again' : 'Watch'}
            <span className="mono">0:{VIDEO_SECONDS}</span>
          </span>
        </button>
      </div>
    </section>
  )
}

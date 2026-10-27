import { useReveal } from '#/lib/useReveal'
import { VOICES, VOICES_ARE_PLACEHOLDERS } from '#/lib/voices'

export function Voices() {
  const [ref, reveal] = useReveal<HTMLElement>()

  // Not shown at all while they're made up: real quotes first.
  if (VOICES_ARE_PLACEHOLDERS) return null

  return (
    <section ref={ref} className={`voices ${reveal}`} aria-label="What people say">
      {VOICES.map((v) => (
        <figure className="quote" key={v.name}>
          <blockquote>{v.quote}</blockquote>
          <figcaption>
            <b>{v.name}</b>
            {v.role}
          </figcaption>
        </figure>
      ))}
    </section>
  )
}

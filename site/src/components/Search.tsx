import { useReveal } from '#/lib/useReveal'

const EXAMPLES = ['photos from last week', 'big videos', 'invoices from march', 'pdfs from today', 'screenshots in downloads']

export function Search() {
  const [ref, reveal] = useReveal<HTMLElement>()
  const [shotRef, shotReveal] = useReveal<HTMLDivElement>()

  return (
    <section ref={ref} className={`search ${reveal}`} id="search">
      <h2>Find anything, in plain words.</h2>
      <p>
        Type what you remember, the way you'd say it. Typos are forgiven, and your whole Mac is searched before you've
        finished typing.
      </p>
      <div className="chips">
        {EXAMPLES.map((e) => (
          <span key={e}>{e}</span>
        ))}
      </div>
      <div ref={shotRef} className={`spanel ${shotReveal}`}>
        <img className="win" src="/img/shots/search-photos.webp" width={2800} height={1680} loading="lazy" alt='F2 searching for "photos from last week" and finding the photos changed in the last seven days' />
      </div>
    </section>
  )
}

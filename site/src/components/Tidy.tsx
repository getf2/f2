import { useReveal } from '#/lib/useReveal'

export function Tidy() {
  const [ref, reveal] = useReveal<HTMLElement>()
  const [shotsRef, shotsReveal] = useReveal<HTMLDivElement>()

  return (
    <section ref={ref} className={`feat ${reveal}`} id="tidy">
      <div className="text">
        <h2>Downloads, tidied in one click.</h2>
        <p>
          Tidy Up sorts a folder's loose files into folders of their kinds: Screenshots, Images, Videos, Audio,
          Documents, Installers and Archives. Changed your mind? ⌘Z puts it all back.
        </p>
        <p>
          Or give a folder a few rules, and it stays tidy on its own: Downloads sorted after a day, old installers
          in the Trash after a week. It can keep your Desktop tidy too, filing screenshots away as you take them.
        </p>
      </div>
      <div ref={shotsRef} className={`panel forest ${shotsReveal}`}>
        <figure className="tidy-before">
          <img className="win" src="/img/shots/tidy-before.webp" width={2800} height={1224} loading="lazy" alt="A Downloads folder full of loose screenshots, photos, PDFs, installers and zips" />
          <figcaption>Before</figcaption>
        </figure>
        <figure className="tidy-after">
          <img className="win" src="/img/shots/tidy-after.webp" width={2800} height={594} loading="lazy" alt="The same Downloads folder after Tidy Up: a folder for each kind of file" />
          <figcaption>After</figcaption>
        </figure>
      </div>
    </section>
  )
}

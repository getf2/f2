import { useReveal } from '#/lib/useReveal'

export function Familiar() {
  const [ref, reveal] = useReveal<HTMLDivElement>()
  const [dualRef, dualReveal] = useReveal<HTMLElement>()
  const [clipRef, clipReveal] = useReveal<HTMLElement>()

  return (
    <section className="familiar" id="finder">
      <div ref={ref} className={`intro ${reveal}`}>
        <h2>Everything Finder does. Just quicker.</h2>
        <p>
          The same sidebar, tabs, Trash, AirDrop and iCloud Drive, so there's nothing new to learn, and Finder stays on
          your Mac. When you want more, it's there: two folders side by side, and zip files that open like folders.
        </p>
      </div>
      <div className="tiles">
        <article ref={dualRef} className={`tile ${dualReveal}`} id="dual-pane">
          <div className="art lake">
            <img className="win" src="/img/shots/dual-photos.webp" width={2800} height={1680} loading="lazy" alt="Two folders side by side: holiday photos on the left, a USB stick on the right" />
          </div>
          <div className="body">
            <h3>Two folders, side by side.</h3>
            <p>
              Drag from one to the other, or press F5 to copy across. Handy for sorting photos into albums, or filling a
              USB stick.
            </p>
          </div>
        </article>
        <article ref={clipRef} className={`tile ${clipReveal}`}>
          <div className="art aurora">
            <img className="win clip" src="/img/shots/clipboard.webp" width={680} height={548} loading="lazy" alt="Clipboard history: text, links, an image and files, newest first, with a preview of the one picked" />
          </div>
          <div className="body">
            <h3>Everything you copied, kept.</h3>
            <p>
              Text, links, images and files, newest first, with the whole of the one you pick beside the list. What
              password managers mark secret is skipped.
            </p>
          </div>
        </article>
      </div>
    </section>
  )
}

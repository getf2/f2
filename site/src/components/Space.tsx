import { useReveal } from '#/lib/useReveal'

export function Space() {
  const [ref, reveal] = useReveal<HTMLElement>()
  const [shotsRef, shotsReveal] = useReveal<HTMLDivElement>()

  return (
    <section ref={ref} className={`feat ${reveal}`} id="space">
      <div className="text">
        <h2>Storage almost full? Clear it in a few clicks.</h2>
        <p>
          F2 shows what's filling your Mac, and what's safe to let go of: installers you've already used, downloads you
          haven't opened in six months, old iPhone backups, apps' caches and duplicate files. Tick what you want gone. It
          goes to the Trash first, so you can always put it back.
        </p>
        <p>Uninstall an app, and what it left behind in your Library goes with it.</p>
      </div>
      <div ref={shotsRef} className={`panel terraces ${shotsReveal}`}>
        <img className="du-back win" src="/img/shots/diskusage.webp" width={2160} height={1504} loading="lazy" alt="Disk Usage: a ring chart of the startup disk beside a list of its folders, biggest first" />
        <img className="fus win" src="/img/shots/freeupspace.webp" width={1080} height={752} loading="lazy" alt="Free Up Space: installers, old downloads, iPhone backups, app caches and the Trash, each with what it holds" />
      </div>
    </section>
  )
}

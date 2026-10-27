import { useReveal } from '#/lib/useReveal'

export function Start() {
  const [ref, reveal] = useReveal<HTMLHeadingElement>()
  const [dmgRef, dmgReveal] = useReveal<HTMLLIElement>()
  const [helloRef, helloReveal] = useReveal<HTMLLIElement>()
  const [keysRef, keysReveal] = useReveal<HTMLLIElement>()

  return (
    <section className="start" id="start">
      <h2 ref={ref} className={reveal}>
        Up and running in a minute.
      </h2>
      <ol className="steps">
        <li ref={dmgRef} className={`tile ${dmgReveal}`}>
          <div className="art terraces">
            <img className="win" src="/img/shots/start-dmg.webp" width={1280} height={856} loading="lazy" alt="F2's disk image: F2 on the left, an arrow, and the Applications folder on the right" />
          </div>
          <div className="body">
            <h3>
              <span className="n">1</span>Download
            </h3>
            <p>Open F2.dmg and drag F2 into your Applications folder.</p>
          </div>
        </li>
        <li ref={helloRef} className={`tile ${helloReveal}`}>
          <div className="art dusk">
            <img className="win" src="/img/shots/start-welcome.webp" width={920} height={640} loading="lazy" alt="F2's welcome: a few permissions, each with what it's for and a button to allow it" />
          </div>
          <div className="body">
            <h3>
              <span className="n">2</span>Say hello
            </h3>
            <p>The welcome shows what F2 asks your Mac for, and why, with a button for each.</p>
          </div>
        </li>
        <li ref={keysRef} className={`tile ${keysReveal}`}>
          <div className="art aurora keys" aria-hidden="true">
            <kbd>⌘</kbd>
            <kbd>Space</kbd>
          </div>
          <div className="body">
            <h3>
              <span className="n">3</span>Press ⌘ Space
            </h3>
            <p>The welcome helps you move ⌘ Space over from Spotlight. Then type anything, in any app.</p>
          </div>
        </li>
      </ol>
    </section>
  )
}

import { FINE_PRINT, NUMBERS } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'
import { DownloadButton } from './DownloadButton'
import { LivingSky } from './LivingSky'
import { Nav } from './Nav'

// More of Quick Open's answers, floating around the window (the same shots
// as the answers panel further down).
const FLOATS = [
  { shot: 'translate', height: 201, alt: 'Quick Open translating "good morning" into Japanese' },
  { shot: 'remind', height: 179, alt: 'Quick Open adding a reminder to call mom tomorrow at 17:00' },
  { shot: 'time-london', height: 218, alt: 'Quick Open: 3pm here is 20:00 in London, and 15:00 in London, Canada' },
]

export function Hero() {
  const [proofRef, proofReveal] = useReveal<HTMLElement>()

  return (
    <>
      <header className="hero">
        <LivingSky paint="hero-valley" y={0.22} />
        <Nav />
        <div className="copy">
          <a className="pill" href="#space">
            <b>New</b>Free Up Space finds what's safe to delete{' '}
            <span className="arr" aria-hidden="true">
              →
            </span>
          </a>
          <h1>The Finder your Mac should have come with.</h1>
          <p className="sub">
            Find any file in a few letters, see what's filling your Mac and clear it, and tidy Downloads in one click.
            Screenshots, clipboard history and quick answers come with it.
          </p>
          <div className="ctas">
            <DownloadButton />
            <span className="ghost">
              then press <kbd>⌘</kbd>
              <kbd>Space</kbd>
            </span>
          </div>
          <p className="fine mono">{FINE_PRINT}</p>
        </div>
      </header>

      <div className="stage">
        <img
          className="main win"
          src="/img/shots/hero-dual.webp"
          width={2800}
          height={1680}
          fetchPriority="high"
          alt="F2 with two folders side by side: Documents as a list, and a folder of photos as icons"
        />
        <img
          className="qo win"
          src="/img/shots/quickopen-currency.webp"
          width={1360}
          height={358}
          alt="Quick Open converting 100 USD to 89.09 EUR at the ECB's rate"
        />
        {FLOATS.map((f) => (
          <img
            key={f.shot}
            className={`float ${f.shot} win`}
            src={`/img/shots/quickopen-${f.shot}.webp`}
            width={680}
            height={f.height}
            alt={f.alt}
          />
        ))}
      </div>

      <section ref={proofRef} className={`proof ${proofReveal}`} aria-label="F2 in numbers">
        {NUMBERS.map((n) => (
          <div key={n.label}>
            <strong>{n.value}</strong>
            <span>{n.label}</span>
          </div>
        ))}
      </section>
    </>
  )
}

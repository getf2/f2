import { FINE_PRINT, VERSION } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'
import { DownloadButton } from './DownloadButton'
import { LivingSky } from './LivingSky'

export function Closing() {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <>
      <section ref={ref} className={`close ${reveal}`}>
        <LivingSky paint="golden-hour" y={0.4} />
        <div className="copy">
          <h2>Give Finder the afternoon off.</h2>
          <p className="sub">Download F2, press ⌘Space, and type the name of anything on your Mac.</p>
          <div className="ctas">
            <DownloadButton place="closing" />
          </div>
          <p className="fine mono">
            <a href="/changelog">Version {VERSION}</a> · {FINE_PRINT}
          </p>
        </div>
      </section>
      <footer className="footer">
        <a className="brand" href="/">
          <img src="/icon.png" alt="" width={22} height={22} />
          F2
        </a>
        <nav className="foot-links" aria-label="More">
          <a href="/features">All features</a>
          <a href="/extensions">Extensions</a>
          <a href="/changelog">What's new</a>
          <a href="/#faq">Questions</a>
          <a href="/privacy">Privacy</a>
          <a href="https://github.com/getf2/f2">Source</a>
        </nav>
        <span>
          Built with <a href="https://gpui.rs">GPUI</a> and <a href="https://github.com/noahdunnagan/fsearch">fsearch</a>
        </span>
      </footer>
    </>
  )
}

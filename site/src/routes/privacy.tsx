import { createFileRoute } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import { Closing } from '#/components/Closing'
import { PageHead } from '#/components/PageHead'
import { openNotice } from '#/lib/analytics'
import { SITE_URL } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'

const TITLE = 'Privacy · F2 for Mac'
const DESCRIPTION = 'What F2 sends from your Mac, and when: never your files, and a crash report only if you say so.'
const UPDATED = '30 October 2026'
const CONTACT = 'leo@getf2.com'

export const Route = createFileRoute('/privacy')({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:url', content: `${SITE_URL}/privacy` },
      { name: 'twitter:title', content: TITLE },
      { name: 'twitter:description', content: DESCRIPTION },
    ],
    links: [
      { rel: 'canonical', href: `${SITE_URL}/privacy` },
      { rel: 'preload', as: 'image', href: '/img/paint/still-lake.webp', type: 'image/webp' },
    ],
  }),
  component: Privacy,
})

function Privacy() {
  return (
    <main>
      <PageHead paint="still-lake" y="45%" path="/privacy" title="Your files stay on your Mac.">
        <p className="sub">
          F2 works with everything on your Mac, so here is exactly what it ever sends anywhere, and when.
        </p>
      </PageHead>

      <div className="releases policy">
        <Part title="Your files">
          <p>
            F2 never sends your files anywhere: not what's in them, not their names, not where they are. Search,
            previews, your clipboard history, screenshots and Disk Usage all work on your Mac, and what they keep
            stays in your Mac's Library folder.
          </p>
        </Part>

        <Part title="Your voice">
          <p>
            Dictation hears your microphone only while you dictate, and turns what you say into text on your Mac,
            with Apple's own speech recognition. What it hears isn't sent anywhere or kept; only the last text
            stays, until F2 quits, for Paste Last Dictation.
          </p>
        </Part>
        <Part title="No tracking">
          <p>
            No analytics, no ads and no account. F2 doesn't count what you do or report how you use it, and there's
            nothing to sign up for.
          </p>
        </Part>

        <Part title="When F2 goes online">
          <p>Only for things you ask for, and only with what's needed to answer:</p>
          <ul>
            <li>
              <b>Updates.</b> F2 checks download.getf2.com for a new version, which tells it F2's version and macOS's.
            </li>
            <li>
              <b>Answers in Quick Open</b>, when you type them: the weather (from Open-Meteo, for the place you name,
              or your time zone's city, never your exact location), currency rates (Frankfurter), crypto and stock
              prices (CoinGecko, Coinbase, Yahoo Finance), public holidays (Nager.Date), sports scores (NBA), your
              internet address (ipify), and whether you're online (Apple). They learn what you asked about, and
              nothing else of yours.
            </li>
            <li>
              <b>Dictation's languages.</b> The first time you dictate in one, macOS downloads it from Apple.
            </li>
            <li>
              <b>Links</b> F2 offers, like maps, package tracking or a translation, open in your browser.
            </li>
          </ul>
        </Part>

        <Part title="Crash reports">
          <p>
            If F2 ever quits unexpectedly, it asks the next time it starts whether to send a report of it. Nothing
            is sent unless you say yes, or tell it to always send them (Settings › Advanced, where you can also
            choose never).
          </p>
          <p>
            A report holds where in F2 it stopped (the names of F2's own code, not your files), F2's and macOS's
            versions, and your Mac's model. Before it's sent, F2 takes out the ids macOS puts in it, which could tie
            reports to your Mac, and your name in folder paths. Show Report shows you exactly what will go.
          </p>
        </Part>

        <Part title="Feedback">
          <p>
            What you write in Help › Send Feedback, and your email address if you add one so you can get an answer,
            with F2's and macOS's versions and your Mac's model.
          </p>
        </Part>

        <Part title="Who sees it">
          <p>
            Crash reports and feedback go to getf2.com, which passes them straight on by email to F2's maker and
            keeps no copy. They're read to fix F2 and to answer you, and never sold, shared or used for anything
            else.
          </p>
          <p>
            To have yours deleted, or for any question, write to <a href={`mailto:${CONTACT}`}>{CONTACT}</a>.
          </p>
        </Part>

        <Part title="This website">
          <p>
            getf2.com has no cookies. It counts visits with Yaap: for each page you open, the page, the website that
            linked to it (just the site, not the full address), any campaign labels in the link, your browser, system
            and kind of device, and a rough location (country, region and city) worked out from your internet
            address. A click on Download is counted the same way. Yaap keeps no internet addresses.
          </p>
          <p>
            Yaap also keeps two random ids in your browser: one for your browser, for six months, and one for your
            visit, which ends when you close the tab or after 30 minutes away. They show how many people visit, how
            long they stay and which pages lead to a download. Yaap keeps them only in a scrambled form, and they
            don't say who you are.
          </p>
          <p>
            Opt out in the notice that comes up on your first visit, or{' '}
            <button type="button" className="choice" onClick={openNotice}>
              change your choice
            </button>
            , and the ids are deleted from your browser, which remembers only that you opted out. Your visits are
            then counted without them, so they can't be told apart from anyone else's.
          </p>
          <p>
            Cloudflare, which hosts the site, keeps short-lived logs of the requests it serves, to keep the site
            running and safe.
          </p>
        </Part>

        <Part title="Changes">
          <p>
            If any of this changes, this page will say so first. Last updated {UPDATED}.
          </p>
        </Part>
      </div>

      <Closing />
    </main>
  )
}

function Part({ title, children }: { title: string; children: ReactNode }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  // An id from the title ("This website" is #this-website), for links here.
  return (
    <section ref={ref} id={title.toLowerCase().replaceAll(' ', '-')} className={`rel ${reveal}`}>
      <div className="when">
        <h2>{title}</h2>
      </div>
      <div className="what">{children}</div>
    </section>
  )
}

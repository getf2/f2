import { createFileRoute } from '@tanstack/react-router'

import { Closing } from '#/components/Closing'
import { PageHead } from '#/components/PageHead'
import { Plate } from '#/components/Plate'
import { LATEST, RELEASES } from '#/lib/changelog'
import type { Release } from '#/lib/changelog'
import { SITE_URL } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'

const TITLE = "What's new · F2 for Mac"
const DESCRIPTION = "Every F2 update, newest first: what's new, what's better, and what's fixed."

export const Route = createFileRoute('/changelog')({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:url', content: `${SITE_URL}/changelog` },
      { name: 'twitter:title', content: TITLE },
      { name: 'twitter:description', content: DESCRIPTION },
    ],
    links: [
      { rel: 'canonical', href: `${SITE_URL}/changelog` },
      { rel: 'preload', as: 'image', href: '/img/paint/night-aurora.webp', type: 'image/webp' },
    ],
  }),
  component: Changelog,
})

// Release dates are days, so they're read and shown in UTC: the same day on
// the server as in any browser.
function day(date: string) {
  return new Date(date).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' })
}

function Changelog() {
  return (
    <main>
      <PageHead paint="night-aurora" y="30%" path="/changelog" title="What's new in F2.">
        <p className="sub">
          Every update, newest first. The latest is {LATEST.version}, from {day(LATEST.date!)}.
        </p>
      </PageHead>

      <div className="releases">
        {RELEASES.map((r) => (
          <Entry key={r.version} r={r} />
        ))}
      </div>

      <Closing />
    </main>
  )
}

const KINDS = [
  { key: 'added', label: 'New' },
  { key: 'improved', label: 'Better' },
  { key: 'fixed', label: 'Fixed' },
] as const

function Entry({ r }: { r: Release }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <article ref={ref} id={`v${r.version}`} className={`rel ${r.date ? '' : 'next '}${reveal}`}>
      <div className="when">
        <h2>{r.date ? r.version : 'Next'}</h2>
        <p className="mono">{r.date ? day(r.date) : 'Not out yet'}</p>
        {r === LATEST && <span className="badge new">Latest</span>}
      </div>
      <div className="what">
        <Plate art={r.art} />
        <h3>{r.title}</h3>
        <p className="lede">{r.summary}</p>
        <ul>
          {KINDS.flatMap(({ key, label }) =>
            (r[key] ?? []).map((item) => (
              <li key={item}>
                <span className={`tag ${key}`}>{label}</span>
                <span>{item}</span>
              </li>
            )),
          )}
        </ul>
      </div>
    </article>
  )
}

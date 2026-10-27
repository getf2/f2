import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { Closing } from '#/components/Closing'
import { PageHead } from '#/components/PageHead'
import { Plate } from '#/components/Plate'
import { compareVersions } from '#/lib/changelog'
import { CATEGORIES, FEATURE_COUNT } from '#/lib/features'
import type { Category, Feature } from '#/lib/features'
import { SITE_URL, VERSION } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'

const TITLE = 'Everything F2 does · F2 for Mac'
const DESCRIPTION = `All ${FEATURE_COUNT} things F2 does, from freeing up space and finding files to quick answers, screenshots and tidy windows.`

export const Route = createFileRoute('/features')({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:url', content: `${SITE_URL}/features` },
      { name: 'twitter:title', content: TITLE },
      { name: 'twitter:description', content: DESCRIPTION },
    ],
    links: [
      { rel: 'canonical', href: `${SITE_URL}/features` },
      { rel: 'preload', as: 'image', href: '/img/paint/still-lake.webp', type: 'image/webp' },
    ],
  }),
  component: Features,
})

// Every word typed has to be somewhere in the feature (or its group's name).
function matches(f: Feature, c: Category, words: string[]) {
  const text = [c.name, f.name, f.text, ...(f.examples ?? [])].join(' ').toLowerCase()
  return words.every((w) => text.includes(w))
}

function Features() {
  const [query, setQuery] = useState('')
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  const shown = CATEGORIES.map((c) => ({ ...c, features: c.features.filter((f) => matches(f, c, words)) }))
  const found = shown.reduce((n, c) => n + c.features.length, 0)

  return (
    <main>
      <PageHead paint="still-lake" y="38%" path="/features" title="Everything F2 does.">
        <p className="sub">
          All {FEATURE_COUNT} of them so far, sorted by what you'd use them for. More come with every update:{' '}
          <a href="/changelog">see what's new</a>.
        </p>
        <label className="find">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
            <circle cx="11" cy="11" r="6.5" />
            <path d="m16 16 4 4" />
          </svg>
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Find a feature: try “battery”"
            aria-label="Find a feature"
            spellCheck={false}
          />
        </label>
        {/* Every group stays, so the header keeps its height as you type: the
            ones with nothing matching just fade. */}
        <nav className="jump" aria-label="Groups">
          {shown.map((c) =>
            c.features.length ? (
              <a key={c.id} href={`#${c.id}`}>
                {c.name}
                <b>{c.features.length}</b>
              </a>
            ) : (
              <a key={c.id} className="empty" aria-disabled="true">
                {c.name}
                <b>0</b>
              </a>
            ),
          )}
        </nav>
      </PageHead>

      <div className="cats">
        {shown.map((c, i) => (
          <Group key={c.id} c={c} flip={i % 2 === 1} />
        ))}
        {!found && (
          <p className="none">
            Nothing here matches “{query.trim()}”. <button onClick={() => setQuery('')}>Show everything</button>
          </p>
        )}
      </div>

      <Closing />
    </main>
  )
}

function Group({ c, flip }: { c: Category; flip: boolean }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <section ref={ref} id={c.id} className={`cat ${flip ? 'flip ' : ''}${reveal}`} hidden={!c.features.length}>
      <Plate art={c.art} />
      <div className="text">
        <h2>{c.name}</h2>
        <p className="lede">{c.lede}</p>
        <ul>
          {c.features.map((f) => (
            <li key={f.name}>
              <h3>
                {f.name}
                {f.keys && (
                  <span className="keys">
                    {f.keys.map((k) => (
                      <kbd key={k}>{k}</kbd>
                    ))}
                  </span>
                )}
                <Badge since={f.since} />
              </h3>
              <p>{f.text}</p>
              {f.examples && (
                <div className="ex">
                  {f.examples.map((e) => (
                    <span key={e}>{e}</span>
                  ))}
                </div>
              )}
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}

// New in the current version, or built but not out yet.
function Badge({ since }: { since?: string }) {
  if (!since) return null
  const d = compareVersions(since, VERSION)
  if (d > 0) return <span className="badge soon">Next update</span>
  if (d === 0) return <span className="badge new">New</span>
  return null
}

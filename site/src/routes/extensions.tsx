import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

import { Closing } from '#/components/Closing'
import { PageHead } from '#/components/PageHead'
import { Plate } from '#/components/Plate'
import type { Art } from '#/components/Plate'
import { SITE_URL } from '#/lib/site'
import { needsAccount, SNAPSHOT, STORE, STORE_REPO } from '#/lib/store'
import type { Listed } from '#/lib/store'
import { useReveal } from '#/lib/useReveal'

const TITLE = 'Extensions · F2 for Mac'
const DESCRIPTION = 'Extensions for F2’s Quick Open: Wikipedia, Hacker News, GitHub, Linear, Notion and more, free from F2’s own store, and shut in so they can only do what they say. Or make your own, in plain JavaScript.'

export const Route = createFileRoute('/extensions')({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:url', content: `${SITE_URL}/extensions` },
      { name: 'twitter:title', content: TITLE },
      { name: 'twitter:description', content: DESCRIPTION },
    ],
    links: [
      { rel: 'canonical', href: `${SITE_URL}/extensions` },
      { rel: 'preload', as: 'image', href: '/img/paint/golden-hour.webp', type: 'image/webp' },
    ],
  }),
  component: Extensions,
})

function Extensions() {
  // (The store's list as it is now, once it's here: the snapshot till then.)
  const [list, setList] = useState<Listed[]>(SNAPSHOT)
  useEffect(() => {
    fetch(`${STORE}v1/index.json`)
      .then((r) => (r.ok ? r.json() : null))
      .then((index) => index?.extensions?.length && setList(index.extensions))
      .catch(() => {})
  }, [])
  const free = list.filter((e) => !needsAccount(e))
  const accounts = list.filter(needsAccount)

  return (
    <main>
      <PageHead paint="golden-hour" y="45%" path="/extensions" title="Extensions for Quick Open.">
        <p className="sub">
          Commands for the websites and services you use, from F2’s own store: {list.length} so far, all free. In F2, open Quick Open and
          type “extension store”, then Return to install one.
        </p>
      </PageHead>

      <div className="cats">
        <Listing
          id="free"
          title="No account needed"
          lede="Look things up without leaving what you’re doing: read the answer right in Quick Open, or open it in your browser."
          art={{ paint: 'golden-hour', y: '45%', queries: ['wiki alan turing', 'hn', 'mdn fetch', 'brew ffmpeg'] }}
          list={free}
        />
        <Listing
          id="accounts"
          title="With your accounts"
          lede="Your own issues, pages, tasks and deployments. Each asks for a token of yours once, kept in your Mac’s keychain, and talks only to the service it’s for."
          art={{ paint: 'mist-forest', y: '40%', queries: ['gh', 'linear', 'notion', 'todoist'] }}
          list={accounts}
          flip
        />
        <Points
          id="safe"
          title="Safe by design"
          lede="An extension can only do what it says, and the store shows you what that is before you install it."
          art={{ paint: 'still-lake', y: '45%' }}
          points={[
            ['Shut in', 'An extension runs in a sandbox of its own: it can’t open your files, see what you type elsewhere, or run anything on your Mac.'],
            ['Only the websites it names', 'Each says which websites it talks to, and F2 lets it reach those and no others. The store lists them for every one.'],
            ['Read before it’s in', 'Every extension in the store, and every update, is checked and read by a person before it’s added.'],
            ['Checked as it’s installed', 'The store’s list is signed, and F2 checks each file it downloads against it, so what you install is what was read.'],
          ]}
        />
        <Points
          id="make"
          title="Make your own"
          lede="An extension is a folder with a short manifest and a script in plain JavaScript. F2 sets one up for you, ready to run."
          art={{ paint: 'study-window', y: '50%', queries: ['new extension'] }}
          points={[
            ['New Extension', 'In Quick Open or Settings › Extensions: give it a name, and it’s made and running, with your editor knowing F2’s API.'],
            ['Save, and it runs again', 'Its page in Quick Open shows what’s new as you save, and says on which line anything went wrong.'],
            [
              'Everything it can do',
              <>
                Lists, searches, pages to read, settings and tokens, actions that copy, open or change something. It’s all in{' '}
                <a href={`${STORE_REPO}/blob/main/docs/EXTENSIONS.md`}>the guide</a>, and the store’s own are{' '}
                <a href={`${STORE_REPO}/tree/main/extensions`}>examples of each</a>.
              </>,
            ],
            [
              'Share it',
              <>
                Add it to <a href={STORE_REPO}>the store’s repository</a> with a pull request: once it’s read and merged, everyone with F2 can
                install it.
              </>,
            ],
          ]}
          flip
        />
      </div>

      <Closing />
    </main>
  )
}

function Listing({ id, title, lede, art, list, flip }: { id: string; title: string; lede: string; art: Art; list: Listed[]; flip?: boolean }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <section ref={ref} id={id} className={`cat ${flip ? 'flip ' : ''}${reveal}`}>
      <Plate art={art} />
      <div className="text">
        <h2>{title}</h2>
        <p className="lede">{lede}</p>
        <ul>
          {list.map((e) => (
            <li key={e.id}>
              <h3>{e.name}</h3>
              <p>{e.description}</p>
              <div className="ex">
                {e.commands.slice(0, 3).map((c) => (
                  <span key={c.title}>{c.keyword ?? c.title}</span>
                ))}
              </div>
              <p className="reach">{e.network.length ? `Reaches ${e.network.join(', ')}` : 'Works on your Mac alone'}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}

function Points({ id, title, lede, art, points, flip }: { id: string; title: string; lede: string; art: Art; points: [string, ReactNode][]; flip?: boolean }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <section ref={ref} id={id} className={`cat ${flip ? 'flip ' : ''}${reveal}`}>
      <Plate art={art} />
      <div className="text">
        <h2>{title}</h2>
        <p className="lede">{lede}</p>
        <ul>
          {points.map(([name, text]) => (
            <li key={name}>
              <h3>{name}</h3>
              <p>{text}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}

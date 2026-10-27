import { createFileRoute } from '@tanstack/react-router'

import { Ask } from '#/components/Ask'
import { Closing } from '#/components/Closing'
import { Familiar } from '#/components/Familiar'
import { Faq } from '#/components/Faq'
import { Hero } from '#/components/Hero'
import { Power } from '#/components/Power'
import { Search } from '#/components/Search'
import { SmallTools } from '#/components/SmallTools'
import { Space } from '#/components/Space'
import { Start } from '#/components/Start'
import { Tidy } from '#/components/Tidy'
import { Voices } from '#/components/Voices'
import { Watch } from '#/components/Watch'
import { faqPageJsonLd, videoJsonLd } from '#/lib/seo'
import { SITE_URL } from '#/lib/site'

export const Route = createFileRoute('/')({
  head: () => ({
    links: [
      { rel: 'canonical', href: `${SITE_URL}/` },
      { rel: 'preload', as: 'image', href: '/img/paint/hero-valley.webp', type: 'image/webp' },
    ],
    scripts: [
      { type: 'application/ld+json', children: JSON.stringify(faqPageJsonLd()) },
      { type: 'application/ld+json', children: JSON.stringify(videoJsonLd()) },
    ],
  }),
  component: Home,
})

// A minute of F2 in a video, then everyday problems (a full disk, a lost
// file, a question, a messy Downloads), then that it's still Finder, the
// extras, and how to start.
function Home() {
  return (
    <main>
      <Hero />
      <Watch />
      <Space />
      <Search />
      <Ask />
      <Tidy />
      <Voices />
      <Familiar />
      <SmallTools />
      <Power />
      <Start />
      <Faq />
      <Closing />
    </main>
  )
}

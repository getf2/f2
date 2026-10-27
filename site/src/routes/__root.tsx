import { HeadContent, Scripts, createRootRoute } from '@tanstack/react-router'

import { softwareApplicationJsonLd } from '#/lib/seo'
import { SITE_URL } from '#/lib/site'
import appCss from '../styles.css?url'

const TITLE = 'F2: Free Finder Alternative for Mac. Free Up Space, Find Files Fast'
const DESCRIPTION =
  "A free Finder for your Mac. Find any file in plain words, see what's filling your disk and clear it, tidy Downloads in one click, and ask your Mac anything with ⌘ Space."

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { name: 'theme-color', content: '#1d4aa8' },
      { name: 'robots', content: 'index, follow, max-image-preview:large' },
      { property: 'og:type', content: 'website' },
      { property: 'og:site_name', content: 'F2' },
      { property: 'og:locale', content: 'en_US' },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:image', content: `${SITE_URL}/og.jpg` },
      { property: 'og:image:width', content: '1200' },
      { property: 'og:image:height', content: '630' },
      {
        property: 'og:image:alt',
        content:
          'F2 over a painted valley: "The Finder your Mac should have come with." above an F2 window, with Quick Open converting 100 USD to EUR and translating good morning into Japanese',
      },
      { property: 'og:url', content: `${SITE_URL}/` },
      { name: 'twitter:card', content: 'summary_large_image' },
      { name: 'twitter:title', content: TITLE },
      { name: 'twitter:description', content: DESCRIPTION },
      { name: 'twitter:image', content: `${SITE_URL}/og.jpg` },
      {
        name: 'twitter:image:alt',
        content: 'F2 for Mac: "The Finder your Mac should have come with." above an F2 window with Quick Open',
      },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', type: 'image/png', href: '/favicon.png' },
      // Browsers and crawlers that ask for /favicon.ico get the PNG from src/server.ts.
      { rel: 'shortcut icon', href: '/favicon.ico' },
      { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' },
    ],
    scripts: [{ type: 'application/ld+json', children: JSON.stringify(softwareApplicationJsonLd()) }],
  }),
  notFoundComponent: NotFound,
  shellComponent: RootDocument,
})

// Unknown paths. src/server.ts also gives every 404 a noindex meta tag, an
// X-Robots-Tag header and its own <title>, so it never borrows the home
// page's snippet.
function NotFound() {
  return (
    <main className="not-found">
      <h1>Page not found</h1>
      <p>
        That page isn't here. <a href="/">Go to the F2 home page</a> or <a href="/features">see everything F2 does</a>.
      </p>
    </main>
  )
}

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}

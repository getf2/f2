import handler from '@tanstack/react-start/server-entry'
import { reports } from './reports'

// Canonical host. www and *.workers.dev 301 here (same path). getf2.com is
// never redirected, so this can't loop with itself or with any zone-level
// redirect that already points those hosts at the apex.
const HOST = 'getf2.com'

const NOINDEX = 'noindex, nofollow'

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)

    if (url.hostname === `www.${HOST}` || url.hostname.endsWith('.workers.dev')) {
      url.protocol = 'https:'
      url.hostname = HOST
      url.port = ''
      return Response.redirect(url.href, 301)
    }

    // F2's crash reports and feedback (src/reports.ts).
    if (url.pathname.startsWith('/api/')) return reports(request, env, url.pathname)

    let res: Response
    if (url.pathname === '/favicon.ico') {
      // The site only has a PNG; browsers and crawlers that ask for
      // /favicon.ico get it anyway.
      const png = await env.ASSETS.fetch(new Request(new URL('/favicon.png', url), request))
      res = new Response(png.body, png)
      res.headers.set('content-type', 'image/png')
      res.headers.set('cache-control', 'public, max-age=86400')
    } else {
      // Prerendered pages and public/ files first; anything else (unknown
      // paths) goes to TanStack Start, which renders the not-found page.
      res = await env.ASSETS.fetch(request)
      if (res.status === 404) res = await handler.fetch(request)
    }

    // 404s must stay out of search results and must not borrow the home
    // page's title/canonical. Canonical host pages otherwise pass through.
    if (res.status !== 404) return res

    res = new Response(res.body, res)
    res.headers.set('X-Robots-Tag', NOINDEX)
    if (!(res.headers.get('content-type') ?? '').includes('text/html')) return res

    return new HTMLRewriter()
      .on('meta[name="robots"]', {
        element(el) {
          el.remove()
        },
      })
      .on('head', {
        element(el) {
          el.append(`<meta name="robots" content="${NOINDEX}">`, { html: true })
        },
      })
      .on('title', {
        element(el) {
          el.setInnerContent('Page not found · F2')
        },
      })
      .on('link[rel="canonical"]', {
        element(el) {
          el.remove()
        },
      })
      .transform(res)
  },
}

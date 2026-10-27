# F2's site

The landing page, at https://getf2.com: TanStack Start on Cloudflare Workers
(made with `create cloudflare --framework=tanstack-start`), prerendered to
static HTML at build time. `src/server.ts` sends www.getf2.com and the
workers.dev address to getf2.com.

```
bun install
bun run dev       # http://localhost:3000
bun run preview   # the production build, in the Workers runtime
bun run deploy    # build, then wrangler deploy (Worker "f2-site")
```

Pushes to `main` that change something in `site/` (other than Markdown)
deploy it on their own: Workers Builds builds and deploys from this folder
(the Worker's Settings › Build in Cloudflare's dashboard). So what's on
`main` is what's live: `bun run deploy` is for deploying by hand, before a
push.

- `src/routes/index.tsx` the page, from `src/components/`, one per section:
  written for everyday Mac users first (a full disk, a lost file, a quick
  question, a messy Downloads), power users' features gathered in `Power`.
  `src/styles.css` its styles (Fraunces for headings, Source Sans 3 for text
  and Geist Mono, all from Fontsource).
- `src/routes/features.tsx` (/features) every feature, by group, with a
  search; `src/routes/changelog.tsx` (/changelog) every release.
- `src/lib/features.ts` what /features lists. When F2 gains a feature (and
  FEATURES.md at the repo's root gets it), add it here too, in plain words,
  with `since` set to the version it comes in: it's marked "Next update"
  until that version is out, then "New" while it's the latest.
- `src/lib/changelog.ts` what /changelog lists, newest first. An entry
  without a date is shown as coming next. **Releasing:** give the top entry
  its date. The site's version (the newest dated entry) and the features'
  badges follow from it.
- `src/routes/extensions.tsx` (/extensions) the extension store's
  extensions, and how to make one: its list is the store's own
  (getf2.github.io/extensions), asked for as the page opens;
  `src/lib/store-snapshot.json` is what's prerendered till then (refresh it
  with `bun scripts/store-snapshot.ts`). `public/schemas/f2.json` is
  extensions' manifest schema, at the address it says it's at: copy it from
  `extensions/js/f2.schema.json` when that changes.
- `src/lib/site.ts` the download link (the latest DMG,
  `https://download.getf2.com/F2.dmg`), the fine print under the buttons and the
  numbers quoted: keep them in step with releases.
- `src/lib/voices.ts` the quotes. **They're placeholders**, marked as such on
  the page: replace them with real ones (and set `VOICES_ARE_PLACEHOLDERS` to
  false) before the site is shared.
- `public/img/paint` the paintings (generated for F2), `public/img/shots` real
  F2 screenshots, both WebP. The everyday ones were taken in a demo home
  folder ("Anna"), never the real one.

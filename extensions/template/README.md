# __NAME__

An extension for F2's Quick Open, made by Settings › Extensions › New
Extension….

## Working on it

- **Open it:** type "__NAME__" in Quick Open, then Return.
- **Change it:** edit `index.js` (what its commands show) or `f2.json`
  (its commands, settings and the websites it may reach) and save. A
  page of it that's open runs again, with what's new.
- **When it goes wrong:** what went wrong is shown on its page, with the
  line (`index.js:12`). `console.log` goes to the page's ⌘K › Show
  Output.
- **Your editor knows F2:** `f2.d.ts` has F2's API and `f2.schema.json`
  has f2.json's fields, for completions, hover and errors as you type
  (`jsconfig.json` turns the checking on).

It runs in F2's sandbox: it can't read your files or start programs,
and `fetch` reaches only the hosts f2.json names.

## In TypeScript

Write `index.ts` and have bun build it to `index.js` each time you save
(npm packages that need nothing of Node's are bundled in):

    bun build index.ts --target=browser --format=iife --outfile index.js --watch

## All of it

`EXTENSIONS.md`, beside this: the manifest, what a command can show
(rows, pages to read, actions), settings, and the API.

# F2 extensions

An extension adds commands to Quick Open. It's a folder with a manifest
(`f2.json`) and either:

- **a script in JavaScript** (or TypeScript built to it), which F2 runs
  itself, shut in a sandbox, with nothing to install: what a store's
  extensions are to be (see "In JavaScript" below); or
- **a program in any language**, for your own: F2 runs it, tells it
  what's wanted on stdin, and builds Quick Open's rows (or a page to
  read) from what it prints on stdout, as it prints it. A program runs
  as you, not in F2's sandbox, so it runs only with Developer Mode on
  (Settings › Extensions), once you've allowed it.

Both show the same things: the messages, rows, pages and actions below.

They go in `~/Library/Application Support/NewFinder/Extensions/`, one
folder each ("Extensions Folder" in Quick Open opens it), called anything:
what an extension's known by is the `id` in its manifest. Those from the
store (Settings › Extensions › Browse) are installed there for you.

The store's are all in [github.com/getf2/extensions](https://github.com/getf2/extensions):
examples of every kind (lists, searches, pages to read, tokens, writes),
and where to add yours.

## Writing one

**New Extension…** (in Settings › Extensions, or "New Extension" in
Quick Open) asks for a name and makes one, in JavaScript, ready to run:
a folder in Extensions with a command that shows a few rows, for you to
change into yours. Its id is your account's name, a dot, then its
name's (`jane.linear-issues`). In it:

- `f2.json`, its manifest, and `index.js`, what its commands show;
- `f2.d.ts` (F2's API's types), `f2.schema.json` (f2.json's fields) and
  `jsconfig.json`: your editor knows F2, with completions, hover and
  errors as you type, in plain JavaScript (the template is the
  repo's `extensions/template`);
- a README, and this file.

Then:

- **Run it:** type its name in Quick Open, and Return.
- **Change it:** save `index.js` or `f2.json`, and a page of it that's
  open runs again with what's new (twice a second F2 looks at its
  manifest and its script, or the files its program's `run` names).
  A manifest saved half-done says what's wrong with it on the page.
- **When it goes wrong:** an error is shown on its page with where it
  was (`TypeError: … (index.js:12)`), a syntax error too. `console.log`
  and what was thrown go to the page's ⌘K › Show Output.
- **Open in your editor:** its page in Settings › Extensions has a
  button for the editor Settings › General's "Open in" has.

## In JavaScript

```json
{
  "api": 1,
  "id": "f2.npm",
  "name": "npm",
  "runtime": "javascript",
  "main": "index.js",
  "permissions": { "network": ["registry.npmjs.org"] },
  "commands": [{ "id": "search", "title": "Search npm Packages", "keyword": "npm", "search": "script" }]
}
```

```js
f2.command("search", async ({ args, query }) => {
  f2.loading(`Searching for “${query}”…`)
  const found = await (await fetch(`https://registry.npmjs.org/-/v1/search?text=${encodeURIComponent(query)}`)).json()
  f2.show({ items: found.objects.map((o) => ({ title: o.package.name, subtitle: o.package.description })), loading: false })
})
```

F2 runs `main` in macOS's own JavaScriptCore (Safari's engine: nothing to
install, a few MB while it runs), in a process of its own that's shut in
macOS's sandbox for pure computation before the script runs: it can't
open a file, make a connection or start a program. All it can do goes
through F2, which does it only if the manifest allows:

- `f2.command(id, handler)`: what a command does. `handler({ command,
  args, query })` may be async; when it's done, the run is.
- `f2.show(message)`: any message below (`{ items }`, `{ append }`,
  `{ detail }`, `{ notice }`…). And `f2.loading(text, progress)`,
  `f2.done()`, `f2.notice(title, text)`, `f2.copy(text)`, `f2.open(url)`.
- `fetch(url, { method, headers, body })`: only `https://`, only to the
  hosts in `permissions.network` (`"api.github.com"`, or `"*.github.com"`
  for any under it), redirects too. Nothing is kept between requests (no
  cookies, no cache). Answers are text, 20 MB at most, within 60 seconds:
  `response.status`, `.ok`, `.headers.get()`, `await .text()`, `await
  .json()`.
- `f2.preferences`: its settings, by id, its passwords too (see
  Preferences).
- `f2.storage.get(key)`, `.set(key, value)`, `.delete(key)`: JSON values
  kept between runs, 5 MB in all.
- `setTimeout`, `clearTimeout`, `console.log` (to ⌘K › Show Output),
  `URLSearchParams`, `btoa` and `atob`, and the language itself (`Date`,
  `Intl`, `JSON`, `Promise`…). No Node or browser APIs: no `require`,
  `process`, `fs`, `URL`, `TextEncoder` or DOM.
- `bun tools/run.js <its folder> <command> [what's typed]` (in the
  store's repo) runs a command without F2, as F2 would, and prints what it
  shows, and anything in it F2 wouldn't understand.

A thrown error, or a rejected promise, is shown in place of the list,
with where in the script it was. Write TypeScript with `f2.d.ts` for
the types, and build one file, again each time it's saved: `bun build
index.ts --target=browser --format=iife --outfile index.js --watch` (npm
packages that need nothing of Node's are bundled in).

## The manifest: f2.json

```json
{
  "api": 1,
  "id": "jane.github",
  "name": "GitHub",
  "description": "Your repos, pull requests and Actions runs.",
  "icon": "chevron.left.forwardslash.chevron.right",
  "color": "#24292f",
  "run": ["python3", "main.py"],
  "commands": [
    { "id": "repos", "title": "Search Repos", "keyword": "gh", "refresh": "30m", "root": true },
    { "id": "prs", "title": "Pull Requests", "hidden": true }
  ],
  "preferences": [
    { "id": "token", "title": "API Token", "type": "password", "required": true, "link": "https://example.com/tokens" },
    { "id": "size", "title": "Results", "type": "dropdown", "options": ["10", "25", "50"], "default": "25" }
  ]
}
```

- `api`: the version of F2's extension API it's written for: `1`. An F2
  that has only an older one says it needs updating, and doesn't run it.
- `id`: yours, a dot, then its name: `"jane.github"`, in lowercase
  letters, digits and hyphens. In a store, yours is your GitHub account's
  name; `f2.` is F2's own. Its settings, passwords, storage and what it
  showed are kept by it, so keep it once it's out. Two extensions with
  the same id can't both be installed: the second is listed as broken.
- `version`: `"1.2.0"`. In the store, raised each time it changes: those
  installed from it update to it.
- `run`: (a program's) the program and its arguments (an array, or one
  string split at spaces; no shell). It runs in the extension's folder. A program named
  without a `/` is looked for in the folder, then on your PATH (your login
  shell's, plus Homebrew's, bun's, deno's, `~/.local/bin` and
  `~/.cargo/bin`, which an app opened from the Dock otherwise wouldn't have).
- `icon`: an SF Symbol's name, an image in the folder (`icon.png`), or an
  emoji. `color` is the tile its symbols are drawn on.
- Each command:
  - `id`: what the program is told.
  - `title`, `subtitle`, `icon`: how it's listed in Quick Open.
  - `keyword`: typed first ("gh f2"), the command's rows are searched for
    the rest, right in Quick Open's results. Typed alone, the command
    comes first.
  - `refresh`: how long what it showed stays fresh ("45s", "30m", "2h",
    "1d"). What a page showed last is always shown at once when it's
    opened again; within `refresh`, the program isn't run again (⌘R runs
    it). Without it, it's run every time it's opened.
  - `search`: `"f2"` (the default): F2 filters the rows as you type, by
    title, then keywords, then subtitle, typos and all. `"script"`: the
    program is run again for each query (a quarter of a second after
    typing stops), for searches only a server can do.
  - `root`: its rows (the last ones it showed) are found by what's typed
    in Quick Open's own search too. They're asked for again in the
    background when Quick Open opens and they're older than `refresh` (an
    hour, without it). A command with a `keyword` is kept fresh the same
    way.
  - `hidden`: not a command of its own, only opened from another page
    (`push`).
- `preferences`: its settings, in Settings › Extensions (see below).

## Preferences

Each extension has a page in Settings › Extensions (⌘, on its page in
Quick Open), with an On switch, its preferences as a form, its commands
(each with an Open button), and what it may do. A preference:

```json
{ "id": "pages", "title": "Show Pages projects", "type": "checkbox", "default": true,
  "description": "Beside each account's Workers.", "required": false }
```

- `type`: `text` (the default), `password`, `checkbox`, `dropdown` (with
  `options`: values, or `{ "value", "title" }`) or `number`.
- `default`: what it is until it's set (a checkbox is off, a dropdown its
  first option, without one). `placeholder`, `description` and `link`
  (where to get one: a "Get One…" link beside it) are shown with it.
- `required`: opening one of its commands goes to its settings instead,
  what's missing marked, till it's set.
- A `password` is typed in a field that doesn't show it, and kept in
  your login keychain; the rest with F2's settings. What a page showed is
  forgotten when a preference changes. A password's only ever asked for
  this way: F2 does the asking, keeping and forgetting.

A script has them as `f2.preferences`, a program as `preferences` in what
it's told (and its passwords as environment variables too, named by
their `id`), each by its `id`: passwords as strings, checkboxes as
`true`/`false`, numbers as numbers, the rest as strings.

## What F2 tells the program

On stdin, one JSON object, then end of file:

```json
{ "command": "prs", "args": { "repo": "getf2/f2" }, "query": "", "preferences": { "size": "25" }, "f2": "0.1.10" }
```

`args` are what the action that opened the page gave (`{}` for a
command opened from Quick Open). `query` is what's typed (always empty
unless the command's `search` is `"script"`). The environment also has
`F2_EXTENSION`, `F2_EXTENSION_DIR`, `F2_COMMAND` and `F2_VERSION`.

## What the program says

JSON objects on stdout, one after another (one a line is easiest; pretty
or back to back works too). F2 acts on each as it arrives, so say what
you're doing while you do it. Any field can go in any message:

| Field | What F2 does |
| --- | --- |
| `"loading": "Fetching your repos…"` | Shows it's working, with that text (or `true`). `false` stops it. It also stops when the program ends. |
| `"progress": 0.4` | A bar along the field, 0 to 1, in place of the moving light. |
| `"title"`, `"placeholder"` | The page's title (by the field, on the right) and the field's placeholder. |
| `"items": [row, …]` | The page's rows, all of them (replacing what was there). |
| `"append": [row, …]` | More rows after them. The first `append` of a run replaces what was shown from last time. |
| `"detail": {…}` | The page is something to read, not a list. |
| `"notice": "Done"` or `{"title", "text"}` | A notification. |
| `"error": "Why"` | Said in place of the list. What the page shows then isn't kept for next time. |
| `"copy": "text"`, `"open": "url or path"` | Done at once, in a run you started: by opening the command, ⌘R, or choosing a `run` action. Ignored in runs F2 starts as you type (`"search": "script"`) or after `refresh`, so nothing's copied or opened on its own. |
| `"refresh": true`, `"pop": true`, `"close": true` | The page asked again; back a page; Quick Open closed. |

Plain text instead of JSON is shown as a page to read. When the program
exits non-zero without an `error`, its last line on stderr is shown.
Everything it prints on stderr is in ⌘K › Show Output.

When the run ends well, what the page shows is kept, and shown at once
the next time it's opened.

### A row

```json
{
  "title": "f2",
  "subtitle": "F2 for Mac",
  "icon": "book.closed.fill",
  "color": "#24292f",
  "section": "dagurleo",
  "accessory": "TypeScript · 2h ago",
  "tags": [{ "text": "Private", "color": "gray" }],
  "keywords": ["getf2/f2"],
  "detail": { "markdown": "## F2\n…", "metadata": [{ "label": "Stars", "text": "12" }] },
  "actions": [ … ]
}
```

Only `title` is needed. Rows with the same `section` are listed together
under it, in the order the sections first come. `icon` is as the
manifest's, or an absolute path to an image. Colors are `#rrggbb`, `#rgb`
or one of red, orange, yellow, green, mint, teal, blue, indigo, purple,
pink, gray, brown. When any row on a page has a `detail`, the list
narrows and the picked row's detail is shown beside it.

### A detail

```json
{ "markdown": "## Title\n\nText…", "metadata": [{ "label": "By", "text": "Ana", "color": "green" }], "actions": [ … ] }
```

The markdown scrolls, and the facts go under it. `actions` are a page
to read's own (a row's detail uses the row's). `"color": "#0ea5e9"` puts a
swatch of it above the text.

### Actions

The first action is Return's, the second ⌘Return's, and ⌘K lists them
all, then F2's own (Refresh ⌘R, Show Output). Each does one thing:

```json
{ "title": "Open on GitHub", "open": "https://github.com/getf2/f2", "icon": "safari" }
{ "title": "Copy Clone URL", "copy": "git@github.com:getf2/f2.git" }
{ "title": "Show in F2", "reveal": "~/Projects/f2" }
{ "title": "Pull Requests", "push": { "command": "prs", "args": { "repo": "getf2/f2" } } }
{ "title": "Delete Branch", "run": { "command": "delete-branch", "args": { "name": "old" } }, "confirm": "Delete old?", "destructive": true }
```

- `open`: a link, or a file or folder (a folder opens in F2).
- `push`: another page of the same extension, `args` going to the program.
- `run`: the program run to do something, the page staying. What it says
  then: `loading` shows on the page, `notice`, `error`, `copy` and `open`
  as above, and `refresh`, `pop` or `close` after. Its `items` and
  `detail` are ignored; plain text is a notice of its first line.
- `confirm`: asked first, in the row (in red); Return again does it.
- `destructive`: in red in ⌘K.

## A whole extension

`Extensions/hello/f2.json`:

```json
{ "api": 1, "id": "jane.hello", "name": "Hello", "run": "./hello.sh", "commands": [{ "id": "hello", "title": "Say Hello" }] }
```

`Extensions/hello/hello.sh` (made executable):

```sh
#!/bin/sh
echo '{"loading": "Thinking…"}'
sleep 1
echo '{"items": [{"title": "Hello", "subtitle": "from a shell script", "actions": [{"title": "Copy", "copy": "Hello"}]}]}'
```

It's a program, so it runs once Developer Mode is on and you've allowed
it, in Settings › Extensions › Hello.

## Programs and Developer Mode

A program runs as you: it can read and change your files and reach
anything you can, so F2 runs one only with Developer Mode on (Settings ›
Extensions, at the bottom), and only once you've allowed it on its page
there (opening it in Quick Open goes there till you have). It's allowed
in the folder it's in: moved, it asks again. Scripts in JavaScript need
neither: they can only do what their manifests say.

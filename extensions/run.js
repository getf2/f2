#!/usr/bin/env bun
// Runs a JavaScript extension's command as F2 does, without F2: its script
// with F2's own API (src/js_host.js), in bun's JavaScriptCore (Safari's
// engine, as F2's host), `fetch` only over https to its manifest's hosts,
// its storage kept in memory (or in a file). Prints what it shows, and
// what in it F2 wouldn't understand.
//
//   bun extensions/run.js extensions/js/wikipedia search "alan turing"
//   bun tools/run.js extensions/f2.hacker-news top --rows 30   (in the store's repo)
//   … --args '{"id": 123}'          what a `push` gives it
//   … --pref size=50                a preference (`--pref-env token=GITHUB_TOKEN`: from the environment)
//   … --storage /tmp/its.json       storage kept between runs
//   … --json                        every message, as F2 gets it
//
// It exits 1 if the run ended in an error (or said one), 2 if it couldn't
// be run at all.

import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'
import vm from 'node:vm'

const realFetch = fetch
const realSetTimeout = setTimeout
const realClearTimeout = clearTimeout
const out = (line = '') => process.stdout.write(`${line}\n`)
const fail = (why) => {
  process.stderr.write(`${why}\n`)
  process.exit(2)
}

// What it's asked.
const flags = { args: {}, prefs: {}, json: false, rows: 10, storage: null }
const plain = []
const argv = process.argv.slice(2)
for (let i = 0; i < argv.length; i++) {
  const a = argv[i]
  const next = () => argv[++i] ?? fail(`${a} needs a value.`)
  if (a === '--args') flags.args = JSON.parse(next())
  else if (a === '--pref' || a === '--pref-env') {
    const [id, ...rest] = next().split('=')
    flags.prefs[id] = a === '--pref' ? rest.join('=') : process.env[rest.join('=')]
  } else if (a === '--json') flags.json = true
  else if (a === '--rows') flags.rows = Number(next())
  else if (a === '--storage') flags.storage = next()
  else plain.push(a)
}
const [folder, command, typed = ''] = plain
if (!folder || !command) fail('bun extensions/run.js <its folder> <command> [what’s typed] [--args JSON] [--pref id=value] [--json]')

const dir = resolve(folder)
let manifest
try {
  manifest = JSON.parse(readFileSync(join(dir, 'f2.json'), 'utf8'))
} catch (e) {
  fail(`f2.json: ${e.message}`)
}
if (manifest.runtime !== 'javascript') fail(`${manifest.name ?? folder} is a program, not JavaScript: run it with F2.`)
const commands = manifest.commands ?? []
const spec = commands.find((c) => c.id === command) ?? fail(`f2.json has no command "${command}": ${commands.map((c) => c.id).join(', ')}.`)
// (As F2: what's typed goes to it only if it does its own searching.)
const query = spec.search === 'script' ? typed : ''
if (typed && !query) out(`(“${command}” is searched by F2, so it's run with nothing typed.)`)

// Its preferences, as F2 gives them: set, else their defaults; empty ones left out.
const preferences = {}
for (const p of manifest.preferences ?? []) {
  let value = flags.prefs[p.id] ?? (p.type === 'password' ? undefined : p.default)
  if (value === undefined && p.type === 'checkbox') value = false
  if (value === undefined && p.type === 'dropdown' && p.options?.length) value = typeof p.options[0] === 'string' ? p.options[0] : p.options[0].value
  if (p.type === 'checkbox' && typeof value === 'string') value = value === 'true'
  if (p.type === 'number' && typeof value === 'string') value = Number(value)
  if (value === undefined || value === null || (typeof value === 'string' && !value.trim())) {
    if (p.required) fail(`It needs “${p.title}” set (--pref ${p.id}=…), as F2 would ask in its settings.`)
    continue
  }
  preferences[p.id] = value
}

// Its fetch: https only, to its manifest's hosts, redirects too (as src/macos/http.m).
const hosts = (manifest.permissions?.network ?? []).map((h) => h.toLowerCase())
const allowed = (host) => {
  host = host.toLowerCase()
  return hosts.some((e) => (e.startsWith('*.') ? host === e.slice(2) || host.endsWith(e.slice(1)) : host === e))
}
async function fetched({ url, method, headers, body }) {
  let at
  try {
    at = new URL(url)
  } catch {
    return { error: "That isn't a URL." }
  }
  if (at.protocol !== 'https:') return { error: 'Only https:// addresses can be fetched.' }
  if (!allowed(at.hostname)) return { error: `${at.hostname} isn't one of the hosts its manifest allows.` }
  // (macOS's own, as F2's requests have, unless it says its own.)
  if (!Object.keys(headers).some((k) => k.toLowerCase() === 'user-agent')) headers = { ...headers, 'User-Agent': 'F2/1 CFNetwork/3860 Darwin/25.5.0' }
  for (let hops = 0; hops < 16; hops++) {
    let r
    try {
      r = await realFetch(at, { method, headers, body: body ?? undefined, redirect: 'manual', signal: AbortSignal.timeout(60_000) })
    } catch (e) {
      return { error: e.message }
    }
    const to = r.status >= 300 && r.status < 400 && r.headers.get('location')
    if (to) {
      const next = new URL(to, at)
      if (next.protocol !== 'https:' || !allowed(next.hostname)) return { error: `Redirected to ${next.hostname || next.href}, which its manifest doesn't allow.` }
      if (r.status === 303 || ((r.status === 301 || r.status === 302) && method !== 'GET' && method !== 'HEAD')) {
        method = 'GET'
        body = null
      }
      // (As macOS's: a sign-in isn't sent on to another host.)
      if (next.host !== at.host) headers = Object.fromEntries(Object.entries(headers).filter(([k]) => k.toLowerCase() !== 'authorization'))
      at = next
      continue
    }
    const text = await r.text()
    if (text.length > 20 << 20) return { error: 'The answer is bigger than 20 MB.' }
    return { result: { status: r.status, url: at.href, headers: Object.fromEntries([...r.headers].map(([k, v]) => [k.toLowerCase(), v])), body: text } }
  }
  return { error: 'Too many redirects.' }
}

// Its storage.
const kept = flags.storage && existsSync(flags.storage) ? JSON.parse(readFileSync(flags.storage, 'utf8')) : {}
function stored({ op, key, value }) {
  if (typeof key !== 'string') return { error: 'A key is a string.' }
  if (op === 'storage.get') return { result: kept[key] ?? null }
  if (op === 'storage.set') kept[key] = value
  else delete kept[key]
  const bytes = JSON.stringify(kept)
  if (bytes.length > 5 << 20) return { error: 'Its storage would be over 5 MB.' }
  if (flags.storage) writeFileSync(flags.storage, bytes)
  return { result: null }
}

// What it shows, as F2 keeps it.
const page = { title: null, placeholder: null, items: [], detail: null, error: null }
const problems = []
let appended = false
let lastLoading
const started = performance.now()
const ms = () => `${Math.round(performance.now() - started)} ms`
const known = new Set(commands.map((c) => c.id))
const KINDS = ['open', 'copy', 'reveal', 'push', 'run']
const COLOR = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i
const COLORS = new Set(['red', 'orange', 'yellow', 'green', 'mint', 'teal', 'blue', 'indigo', 'purple', 'pink', 'gray', 'grey', 'brown'])
const problem = (what) => problems.includes(what) || problems.push(what)
function checkColor(c, where) {
  if (c !== undefined && !(typeof c === 'string' && (COLOR.test(c) || COLORS.has(c)))) problem(`${where}: color ${JSON.stringify(c)} isn't one F2 knows.`)
}
function checkActions(actions, where) {
  for (const a of actions ?? []) {
    if (typeof a?.title !== 'string' || !a.title) problem(`${where}: an action without a title.`)
    const does = KINDS.filter((k) => a?.[k] !== undefined)
    if (does.length !== 1) problem(`${where}: action “${a?.title}” does ${does.length ? does.join(' and ') : 'nothing'} (one of ${KINDS.join(', ')}).`)
    for (const k of ['push', 'run']) {
      if (a?.[k] !== undefined && !known.has(a[k]?.command)) problem(`${where}: action “${a.title}” goes to “${a[k]?.command}”, which f2.json doesn't have.`)
    }
    for (const k of ['open', 'copy', 'reveal']) {
      if (a?.[k] !== undefined && typeof a[k] !== 'string') problem(`${where}: action “${a.title}”'s ${k} isn't a string.`)
    }
  }
}
function checkRows(rows) {
  for (const [i, row] of rows.entries()) {
    const where = `row ${i + 1}${typeof row?.title === 'string' ? ` (“${row.title.slice(0, 40)}”)` : ''}`
    if (typeof row?.title !== 'string') problem(`${where}: no title.`)
    for (const k of ['subtitle', 'accessory', 'section', 'icon']) {
      if (row?.[k] !== undefined && typeof row[k] !== 'string') problem(`${where}: ${k} isn't a string.`)
    }
    checkColor(row?.color, where)
    for (const t of row?.tags ?? []) {
      if (typeof t?.text !== 'string') problem(`${where}: a tag without text.`)
      checkColor(t?.color, where)
    }
    checkActions(row?.actions, where)
    checkColor(row?.detail?.color, `${where}'s detail`)
    for (const f of row?.detail?.metadata ?? []) {
      if (typeof f?.label !== 'string' || typeof f?.text !== 'string') problem(`${where}: a detail fact needs a label and a text (strings).`)
    }
  }
}
function said(m) {
  if (flags.json) out(JSON.stringify(m))
  const line = (s) => flags.json || out(`  ${ms().padStart(8)}  ${s}`)
  // (Loading's text when it changes, not each step of its progress.)
  if (m.loading !== undefined && m.loading !== false && m.loading !== lastLoading) line(`loading${typeof m.loading === 'string' ? `: ${m.loading}` : ''}${m.progress !== undefined ? ' (with progress)' : ''}`)
  if (m.loading !== undefined) lastLoading = m.loading
  if (m.title !== undefined) page.title = m.title
  if (m.placeholder !== undefined) page.placeholder = m.placeholder
  if (m.items) {
    checkRows(m.items)
    page.items = m.items
    page.detail = null
    appended = true
    line(`items: ${m.items.length}`)
  }
  if (m.append) {
    checkRows(m.append)
    page.items = appended ? [...page.items, ...m.append] : m.append
    appended = true
    line(`append: ${m.append.length} (${page.items.length} in all)`)
  }
  if (m.detail) {
    checkActions(m.detail.actions, 'detail')
    checkColor(m.detail.color, 'detail')
    page.detail = m.detail
    line('detail')
  }
  if (m.notice) line(`notice: ${typeof m.notice === 'string' ? m.notice : `${m.notice.title}${m.notice.text ? ` — ${m.notice.text}` : ''}`}`)
  if (m.error) {
    page.error = m.error
    line(`error: ${m.error}`)
  }
  for (const k of ['copy', 'open', 'refresh', 'pop', 'close']) if (m[k]) line(`${k}${m[k] === true ? '' : `: ${m[k]}`}`)
}

// F2's side of the host (as src/macos/jshost.m), in a world of its own
// with only what macOS's JavaScriptCore has (bun's has more, as Temporal).
const host = vm.createContext({})
for (const name of ['Temporal', 'ShadowRealm', 'SharedArrayBuffer', 'DisposableStack', 'AsyncDisposableStack', 'SuppressedError']) vm.runInContext(`delete globalThis.${name}`, host)
let finished
const done = new Promise((r) => (finished = r))
const timers = new Map()
host.__f2_post = (line) => {
  const m = JSON.parse(line)
  if (m.call !== undefined) {
    const answer = m.op === 'fetch' ? fetched(m) : m.op?.startsWith('storage.') ? Promise.resolve(stored(m)) : Promise.resolve({ error: `F2 doesn't do "${m.op}".` })
    answer.then((a) => host.__f2_reply(m.call, JSON.stringify(a)))
  } else if (m.say) said(m.say)
}
host.__f2_log = (line) => process.stderr.write(`  ${ms().padStart(8)}  console: ${line}\n`)
host.__f2_timer_set = (id, wait) =>
  timers.set(
    id,
    realSetTimeout(() => {
      timers.delete(id)
      host.__f2_timer(id)
    }, wait),
  )
host.__f2_timer_clear = (id) => {
  realClearTimeout(timers.get(id))
  timers.delete(id)
}
host.__f2_done = (error) => {
  if (error !== undefined && error !== null) said({ error: String(error), loading: false })
  finished()
}
host.__f2_preferences = preferences

// (F2's API: beside this, in the store's repo; in F2's own, in its source.)
const beside = new URL('./js_host.js', import.meta.url)
const prelude = readFileSync(existsSync(beside) ? beside : new URL('../src/js_host.js', import.meta.url), 'utf8')
const main = manifest.main ?? 'index.js'
let script
try {
  script = readFileSync(join(dir, main), 'utf8')
} catch (e) {
  fail(`${main}: ${e.message}`)
}
vm.runInContext(prelude, host, { filename: 'js_host.js' })
try {
  vm.runInContext(script, host, { filename: `extension:///${encodeURIComponent(basename(main))}` })
} catch (e) {
  said({ error: `${e}${e?.line ? ` (${basename(main)}:${e.line})` : ''}`, loading: false })
  finished()
}
if (!page.error) host.__f2_start(command, flags.args, query).then(host.__f2_done, host.__f2_failed)
await done
for (const t of timers.values()) realClearTimeout(t)

// The page it ended with.
if (!flags.json) {
  out()
  out(`${manifest.name} › ${page.title ?? spec.title}${query ? `  “${query}”` : ''}   (${ms()})`)
  if (page.error) out(`  ✗ ${page.error}`)
  else if (page.detail) {
    const lines = (page.detail.markdown ?? '').split('\n')
    for (const l of lines.slice(0, 40)) out(`  │ ${l}`)
    if (lines.length > 40) out(`  │ … ${lines.length - 40} lines more`)
    for (const f of page.detail.metadata ?? []) out(`  · ${f.label}: ${f.text}`)
    for (const a of page.detail.actions ?? []) out(`  → ${a.title}`)
  } else {
    if (!page.items.length) out(`  (no rows${page.placeholder ? `; placeholder “${page.placeholder}”` : ''})`)
    let section
    for (const row of page.items.slice(0, flags.rows)) {
      if (row.section !== section) {
        section = row.section
        if (section) out(`  ── ${section}`)
      }
      const tags = (row.tags ?? []).map((t) => `[${t.text}]`).join(' ')
      out(`  ${row.title}${row.subtitle ? ` — ${row.subtitle}` : ''}${row.accessory ? `   ${row.accessory}` : ''}${tags ? `   ${tags}` : ''}`)
      const acts = (row.actions ?? []).map((a) => `${a.title} (${KINDS.filter((k) => a[k] !== undefined).map((k) => (k === 'push' || k === 'run' ? `${k} ${a[k].command}${a[k].args ? ` ${JSON.stringify(a[k].args)}` : ''}` : `${k} ${String(a[k]).slice(0, 70)}`)).join('')})`)
      if (acts.length) out(`      ↵ ${acts.join('\n        ')}`)
      if (row.detail) out(`      ▸ detail: ${(row.detail.markdown ?? '').replace(/\s+/g, ' ').slice(0, 100)}${row.detail.metadata?.length ? `  · ${row.detail.metadata.map((f) => `${f.label}: ${f.text}`).join(' · ')}` : ''}`)
    }
    if (page.items.length > flags.rows) out(`  … ${page.items.length - flags.rows} rows more (--rows N)`)
  }
  for (const p of problems) out(`  ⚠ ${p}`)
}
process.exit(page.error ? 1 : 0)

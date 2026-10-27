// What an extension's script has, run by F2's JavaScript host (see
// src/macos/jshost.m, EXTENSIONS.md): F2's API, `fetch`, timers, a
// console, `URLSearchParams` and base64 (`btoa`, `atob`). All of it goes through F2: the host itself can't reach a file
// or the network. (Its passwords come with its preferences.)
(() => {
  const post = (message) => __f2_post(JSON.stringify(message))

  // F2 asked to do something: a promise of its answer.
  const waiting = new Map()
  let nextCall = 1
  const call = (op, params) =>
    new Promise((resolve, reject) => {
      const id = nextCall++
      waiting.set(id, { resolve, reject })
      post({ call: id, op, ...params })
    })
  globalThis.__f2_reply = (id, line) => {
    const w = waiting.get(id)
    if (!w) return
    waiting.delete(id)
    const reply = JSON.parse(line)
    if (reply.error !== undefined) w.reject(new Error(reply.error))
    else w.resolve(reply.result)
  }

  const timers = new Map()
  let nextTimer = 1
  globalThis.setTimeout = (fn, ms = 0, ...args) => {
    const id = nextTimer++
    timers.set(id, () => fn(...args))
    __f2_timer_set(id, Number(ms) || 0)
    return id
  }
  globalThis.clearTimeout = (id) => {
    timers.delete(id)
    __f2_timer_clear(id)
  }
  globalThis.__f2_timer = (id) => {
    const fn = timers.get(id)
    timers.delete(id)
    if (fn) fn()
  }

  const shown = (x) => (typeof x === 'string' ? x : x instanceof Error ? `${x.name}: ${x.message}` : JSON.stringify(x))
  const log = (...args) => __f2_log(args.map(shown).join(' '))
  globalThis.console = { log, info: log, warn: log, error: log, debug: log }

  class Headers {
    constructor(init = {}) {
      this.map = new Map()
      for (const [k, v] of Object.entries(init instanceof Headers ? Object.fromEntries(init.map) : init)) this.map.set(k.toLowerCase(), String(v))
    }
    get(name) {
      return this.map.get(String(name).toLowerCase()) ?? null
    }
    has(name) {
      return this.map.has(String(name).toLowerCase())
    }
    set(name, value) {
      this.map.set(String(name).toLowerCase(), String(value))
    }
    entries() {
      return this.map.entries()
    }
  }
  class Response {
    constructor(r) {
      this.status = r.status
      this.ok = r.status >= 200 && r.status < 300
      this.url = r.url
      this.headers = new Headers(r.headers)
      this.body = r.body
    }
    async text() {
      return this.body
    }
    async json() {
      return JSON.parse(this.body)
    }
  }
  globalThis.Headers = Headers
  globalThis.Response = Response

  // A query string's names and values, as the web's (`?q=a+b&page=2`).
  const formEncode = (s) => encodeURIComponent(s).replace(/[!'()~]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`).replace(/%20/g, '+')
  const formDecode = (s) => decodeURIComponent(s.replace(/\+/g, ' '))
  class URLSearchParams {
    constructor(init = '') {
      this.list = []
      if (typeof init === 'string') {
        for (const part of init.replace(/^\?/, '').split('&')) {
          if (!part) continue
          const at = part.indexOf('=')
          this.list.push(at < 0 ? [formDecode(part), ''] : [formDecode(part.slice(0, at)), formDecode(part.slice(at + 1))])
        }
      } else {
        for (const [k, v] of init instanceof URLSearchParams ? init.list : Symbol.iterator in Object(init) ? init : Object.entries(init)) this.list.push([String(k), String(v)])
      }
    }
    get size() {
      return this.list.length
    }
    append(name, value) {
      this.list.push([String(name), String(value)])
    }
    delete(name) {
      this.list = this.list.filter(([k]) => k !== String(name))
    }
    get(name) {
      return this.list.find(([k]) => k === String(name))?.[1] ?? null
    }
    getAll(name) {
      return this.list.filter(([k]) => k === String(name)).map(([, v]) => v)
    }
    has(name) {
      return this.list.some(([k]) => k === String(name))
    }
    set(name, value) {
      const at = this.list.findIndex(([k]) => k === String(name))
      if (at < 0) return this.append(name, value)
      this.list[at] = [String(name), String(value)]
      this.list = this.list.filter(([k], i) => i <= at || k !== String(name))
    }
    sort() {
      this.list.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    }
    forEach(fn) {
      for (const [k, v] of this.list) fn(v, k, this)
    }
    keys() {
      return this.list.map(([k]) => k)[Symbol.iterator]()
    }
    values() {
      return this.list.map(([, v]) => v)[Symbol.iterator]()
    }
    entries() {
      return this.list.map(([k, v]) => [k, v])[Symbol.iterator]()
    }
    [Symbol.iterator]() {
      return this.entries()
    }
    toString() {
      return this.list.map(([k, v]) => `${formEncode(k)}=${formEncode(v)}`).join('&')
    }
  }
  globalThis.URLSearchParams = URLSearchParams

  // Base64, as the web's: of text whose characters are bytes (0–255), as
  // "user:token" for a Basic sign-in.
  const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
  globalThis.btoa = (data) => {
    const s = String(data)
    if (/[^\0-\xff]/.test(s)) throw new Error('btoa: a character in it is beyond 255.')
    let out = ''
    for (let i = 0; i < s.length; i += 3) {
      const [a, b, c] = [s.charCodeAt(i), s.charCodeAt(i + 1), s.charCodeAt(i + 2)]
      const n = (a << 16) | ((b || 0) << 8) | (c || 0)
      out += B64[n >> 18] + B64[(n >> 12) & 63] + (i + 1 < s.length ? B64[(n >> 6) & 63] : '=') + (i + 2 < s.length ? B64[n & 63] : '=')
    }
    return out
  }
  globalThis.atob = (data) => {
    const s = String(data).replace(/[\t\n\f\r ]/g, '').replace(/={1,2}$/, '')
    if (s.length % 4 === 1 || /[^A-Za-z0-9+/]/.test(s)) throw new Error("atob: it isn't base64.")
    let out = ''
    let n = 0
    let bits = 0
    for (const ch of s) {
      n = (n << 6) | B64.indexOf(ch)
      bits += 6
      if (bits >= 8) {
        bits -= 8
        out += String.fromCharCode((n >> bits) & 255)
      }
    }
    return out
  }
  // Only https, only to the hosts its manifest's `permissions.network` names.
  globalThis.fetch = async (url, options = {}) => {
    const headers = Object.fromEntries(new Headers(options.headers ?? {}).map)
    const body = options.body == null ? null : typeof options.body === 'string' ? options.body : JSON.stringify(options.body)
    return new Response(await call('fetch', { url: String(url), method: options.method ?? 'GET', headers, body }))
  }

  const commands = {}
  globalThis.f2 = {
    /** What a command does: `handler({ command, args, query })`. */
    command: (id, handler) => {
      commands[id] = handler
    },
    /** Any message F2 shows: { items }, { append }, { detail }, { loading }… */
    show: (message) => post({ say: message }),
    loading: (text, progress) => post({ say: { loading: text ?? true, ...(progress === undefined ? {} : { progress }) } }),
    done: () => post({ say: { loading: false } }),
    notice: (title, text) => post({ say: { notice: text === undefined ? title : { title, text } } }),
    copy: (text) => post({ say: { copy: String(text) } }),
    open: (target) => post({ say: { open: String(target) } }),
    fetch: globalThis.fetch,
    /** Its settings (Settings › Extensions), by their ids: passwords too. */
    preferences: Object.freeze({ ...(globalThis.__f2_preferences ?? {}) }),
    /** What it keeps between runs: JSON values, by key. */
    storage: {
      get: (key) => call('storage.get', { key }),
      set: (key, value) => call('storage.set', { key, value }),
      delete: (key) => call('storage.delete', { key }),
    },
  }

  globalThis.__f2_start = (command, args, query) => {
    const handler = commands[command]
    if (!handler) return Promise.reject(`Nothing says what command "${command}" does: f2.command("${command}", …).`)
    return Promise.resolve()
      .then(() => handler({ command, args, query }))
      .then(() => undefined)
  }
  // What went wrong, and where in its script: "TypeError: … (index.js:12)"
  // (its stack's first place in the script, not in F2's API).
  const where = (e) => {
    const at = /extension:\/\/\/([^\s:]+):(\d+)/.exec(String(e.stack ?? ''))
    return at ? ` (${decodeURIComponent(at[1])}:${at[2]})` : ''
  }
  globalThis.__f2_failed = (e) =>
    __f2_done(e instanceof Error ? `${e.name && e.name !== 'Error' ? `${e.name}: ` : ''}${e.message}${where(e)}` : String(e))
})()

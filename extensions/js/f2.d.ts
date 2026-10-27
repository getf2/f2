// What an F2 extension in JavaScript has (see EXTENSIONS.md): F2's API,
// `fetch` (https, to its manifest's hosts), timers, a console,
// `URLSearchParams` and base64. (No `URL`, `TextEncoder` or DOM: it's
// JavaScript itself, not a browser.)

declare namespace F2 {
  type Color = string
  interface Tag { text: string; color?: Color }
  interface Fact { label: string; text: string; color?: Color }
  interface Call { command: string; args?: Record<string, unknown> }
  interface Action {
    title: string
    icon?: string
    open?: string
    copy?: string
    reveal?: string
    push?: Call
    run?: Call
    confirm?: string
    destructive?: boolean
  }
  /** `color`: a swatch of it above the text. */
  interface Detail { color?: Color; markdown?: string; metadata?: Fact[]; actions?: Action[] }
  interface Row {
    title: string
    subtitle?: string
    icon?: string
    color?: Color
    section?: string
    accessory?: string
    tags?: Tag[]
    keywords?: string[]
    detail?: Detail
    actions?: Action[]
  }
  interface Message {
    loading?: string | boolean
    progress?: number
    title?: string
    placeholder?: string
    items?: Row[]
    append?: Row[]
    detail?: Detail
    notice?: string | { title: string; text?: string }
    error?: string
    open?: string
    copy?: string
    refresh?: boolean
    pop?: boolean
    close?: boolean
  }
  interface Asked { command: string; args: Record<string, any>; query: string }
}

declare const f2: {
  /** What a command does, by its id in f2.json. */
  command(id: string, handler: (asked: F2.Asked) => void | Promise<void>): void
  show(message: F2.Message): void
  loading(text?: string, progress?: number): void
  done(): void
  notice(title: string, text?: string): void
  copy(text: string): void
  open(target: string): void
  fetch: typeof fetch
  /** Its settings from Settings › Extensions (f2.json's `preferences`), by id: its passwords too. */
  preferences: Readonly<Record<string, string | number | boolean>>
  storage: {
    get<T = unknown>(key: string): Promise<T | null>
    set(key: string, value: unknown): Promise<void>
    delete(key: string): Promise<void>
  }
}

interface F2Response {
  status: number
  ok: boolean
  url: string
  headers: { get(name: string): string | null; has(name: string): boolean }
  text(): Promise<string>
  json<T = any>(): Promise<T>
}
declare function fetch(url: string, options?: { method?: string; headers?: Record<string, string>; body?: string | object }): Promise<F2Response>
declare function setTimeout(fn: (...args: any[]) => void, ms?: number, ...args: any[]): number
declare function clearTimeout(id: number): void
declare const console: { log(...a: unknown[]): void; info(...a: unknown[]): void; warn(...a: unknown[]): void; error(...a: unknown[]): void; debug(...a: unknown[]): void }
/** Base64 of text whose characters are bytes (0–255): `btoa("user:token")`. */
declare function btoa(data: string): string
declare function atob(data: string): string
/** A query string's names and values: `new URLSearchParams({ q: "a b" }).toString()` is "q=a+b". */
declare class URLSearchParams implements Iterable<[string, string]> {
  constructor(init?: string | Record<string, string | number | boolean> | Iterable<[string, string]> | URLSearchParams)
  readonly size: number
  append(name: string, value: string | number | boolean): void
  delete(name: string): void
  get(name: string): string | null
  getAll(name: string): string[]
  has(name: string): boolean
  set(name: string, value: string | number | boolean): void
  sort(): void
  forEach(fn: (value: string, name: string, params: URLSearchParams) => void): void
  keys(): IterableIterator<string>
  values(): IterableIterator<string>
  entries(): IterableIterator<[string, string]>
  [Symbol.iterator](): IterableIterator<[string, string]>
  toString(): string
}

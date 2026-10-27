import snapshot from './store-snapshot.json'

// F2's extension store (github.com/getf2/extensions), as its list is
// published: what /extensions shows. The page asks for the list itself as
// it opens; this snapshot is what's prerendered, and shown till then
// (refresh it: curl the list into store-snapshot.json, see README).
export const STORE = 'https://getf2.github.io/extensions/'
export const STORE_REPO = 'https://github.com/getf2/extensions'

export type Listed = {
  id: string
  version: string
  name: string
  description: string
  author: string
  network: string[]
  commands: { title: string; keyword: string | null }[]
  preferences: { title: string; type: string; required: boolean }[]
}

export const SNAPSHOT: Listed[] = snapshot

// One that needs an account of yours (a token) to work.
export const needsAccount = (e: Listed) => e.preferences.some((p) => p.required && p.type === 'password')

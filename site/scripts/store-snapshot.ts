// The extension store's list as it's published, saved for /extensions to
// prerender (the page asks for it again as it opens): `bun scripts/store-snapshot.ts`.
const index = await (await fetch('https://getf2.github.io/extensions/v1/index.json')).json()
const slim = index.extensions.map(({ id, version, name, description, author, network, commands, preferences }: any) => ({
  id,
  version,
  name,
  description,
  author,
  network,
  commands: commands.map(({ title, keyword }: any) => ({ title, keyword })),
  preferences: preferences.map(({ title, type, required }: any) => ({ title, type, required })),
}))
await Bun.write(new URL('../src/lib/store-snapshot.json', import.meta.url), `${JSON.stringify(slim, null, 1)}\n`)
console.log(`${slim.length} extensions`)

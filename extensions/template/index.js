// @ts-check
/// <reference path="./f2.d.ts" />

// What __NAME__'s commands show. F2 runs this in its sandbox each time one
// of the commands in f2.json is opened. Save, and a page of it that's open
// runs again. README.md says how to work on it; EXTENSIONS.md says all F2
// understands.

f2.command("hello", async () => {
  f2.loading("Saying hello…")
  const words = ["Hello", "Hej", "Halló", "Hola", "Bonjour"]
  f2.show({
    items: words.map((word) => ({
      title: `${word}, world`,
      subtitle: "Return does its first action, ⌘K lists them all",
      icon: "hand.wave.fill",
      actions: [
        { title: "Say More", push: { command: "more", args: { word } }, icon: "text.bubble" },
        { title: "Copy", copy: `${word}, world`, icon: "doc.on.doc" },
      ],
    })),
    loading: false,
  })
})

// A page pushed from a row: something to read.
f2.command("more", async ({ args }) => {
  f2.show({ title: args.word, detail: { markdown: `# ${args.word}\n\nEdit index.js and save: this page shows what's new.` } })
})

// Asking a website: name its host in f2.json's "permissions.network"
// ("api.github.com"), then
//
//   const answer = await fetch("https://api.github.com/zen")
//   f2.show({ items: [{ title: await answer.text() }] })

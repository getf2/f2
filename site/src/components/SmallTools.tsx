import { useReveal } from '#/lib/useReveal'

// Each with where to find it, in place of shortcuts.
const TOOLS = [
  {
    title: 'Screenshots, marked up',
    art: 'dusk-sea',
    at: 'center 30%',
    text: "An area, a window or the whole screen. Draw on it, blur what's private, or pin it on top while you work.",
    how: 'From the menu bar, or a shortcut',
  },
  {
    title: 'Small enough to email',
    art: 'meadow',
    at: 'center 75%',
    text: 'Photos shrunk to 1600 pixels and videos to 720p, ready to send, without opening another app.',
    how: 'Right-click › Make Smaller',
  },
  {
    title: 'PDFs, put together',
    art: 'valley-plate',
    at: 'center 45%',
    text: 'Turn photos and scans into one PDF, combine a few PDFs into one, or squeeze a big one down to send.',
    how: 'Right-click › Combine into PDF, Compress PDF',
  },
  {
    title: 'Text and backgrounds',
    art: 'terraces',
    at: 'center 30%',
    text: "Copy the text out of a photo, a screenshot or a scan. Lift a photo's subject off its background.",
    how: 'Right-click › Copy Text, Remove Background',
  },
  {
    title: 'Windows, side by side',
    art: 'golden-hour',
    at: 'center 60%',
    text: 'Put two windows side by side, one in the middle, or one on the other screen, with a key.',
    how: 'Halves, thirds and quarters',
  },
  {
    title: 'Say it instead of typing',
    art: 'mist-forest',
    at: 'center 55%',
    text: "Hold a key in any app, say what you want to write, and let go: it's typed for you, worked out on your Mac.",
    how: 'Settings › Dictation, on macOS 26',
  },
]

export function SmallTools() {
  const [headRef, headReveal] = useReveal<HTMLDivElement>()

  return (
    <section className="tools" id="tools">
      <div ref={headRef} className={`grid-h ${headReveal}`}>
        <h2>The little apps you'd pay for, built in.</h2>
        <p>
          Most Macs end up with a screenshot app, a PDF tool and a window snapper, bought one at a time. These come with
          F2, free.
        </p>
      </div>
      <div className="grid">
        {TOOLS.map((t) => (
          <Card key={t.title} tool={t} />
        ))}
      </div>
    </section>
  )
}

function Card({ tool: t }: { tool: (typeof TOOLS)[number] }) {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <article ref={ref} className={`card ${reveal}`}>
      <div className="art" style={{ backgroundImage: `url(/img/paint/${t.art}-sm.webp)`, backgroundPosition: t.at }} />
      <div className="body">
        <h3>{t.title}</h3>
        <p>{t.text}</p>
        <div className="k">{t.how}</div>
      </div>
    </article>
  )
}

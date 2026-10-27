import { FEATURE_COUNT } from '#/lib/features'
import { useReveal } from '#/lib/useReveal'

const ITEMS = [
  { title: 'Two panes, by keyboard', text: 'Tab between them, F5 copies across, F6 moves. Each pane keeps its own tabs.' },
  { title: "Git's status", text: 'Modified, added and untracked files marked beside their names, as your editor marks them.' },
  { title: 'Terminal and editor', text: '⌥⌘T and ⌥⌘E open the selection in Ghostty, iTerm, Zed, Cursor or VS Code.' },
  { title: 'Ports', text: '"kill port 3000" finds what\'s listening there and stops it.' },
  { title: 'JSON, Base64, UUIDs', text: "Format the clipboard's JSON, encode and decode, or copy the Unix time." },
  { title: 'Snippets and quicklinks', text: 'Type ;date in any app for the date. "gh rust" searches GitHub.' },
]

export function Power() {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <section ref={ref} className={`power ${reveal}`} id="power">
      <h2>And for power users.</h2>
      <div className="list">
        {ITEMS.map((i) => (
          <div key={i.title}>
            <h3>{i.title}</h3>
            <p>{i.text}</p>
          </div>
        ))}
      </div>
      <a className="more" href="/features">
        See all {FEATURE_COUNT} things F2 does{' '}
        <span className="arr" aria-hidden="true">
          →
        </span>
      </a>
    </section>
  )
}

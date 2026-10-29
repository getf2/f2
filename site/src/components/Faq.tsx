import { MIN_MACOS } from '#/lib/site'
import { useReveal } from '#/lib/useReveal'

// Also read by faqPageJsonLd (src/lib/seo.ts), so the FAQPage markup always
// matches what's on the page.
export const QUESTIONS = [
  {
    q: 'Does it replace Finder?',
    a: "It works alongside it. Finder can't be taken off a Mac, and F2 doesn't try: open F2 from the Dock or the menu bar, and Finder's still there whenever you want it.",
  },
  {
    q: 'Is it safe for my files?',
    a: 'F2 works on your files just as Finder does. What it clears goes to the Trash, where Put Back works, and Tidy Up is undone with ⌘Z. Apple checks every version for malware before you open it.',
  },
  {
    q: 'Why does it ask for Full Disk Access?',
    a: "So search can find what's in your Desktop, Documents and Downloads, and so it can empty the Trash. Without it, macOS asks about each of those folders as you open them.",
  },
  { q: 'What does it cost?', a: 'Nothing. F2 is free, and open source: its code is at github.com/getf2/f2.' },
  {
    q: 'Which Macs does it run on?',
    a: `Any Mac on macOS ${MIN_MACOS} Ventura or later: Apple silicon (M1 and later) and Intel. Dictation needs macOS 26 Tahoe.`,
  },
  {
    q: 'Does it send my files anywhere?',
    a: "No. Its search index stays on your Mac, and so do your files. It goes online to check for updates, and for what you ask that needs the internet: exchange rates, the weather, a speed test.",
  },
]

export function Faq() {
  const [ref, reveal] = useReveal<HTMLElement>()

  return (
    <section ref={ref} className={`faq ${reveal}`} id="faq">
      <h2>Questions</h2>
      <div className="qs">
        {QUESTIONS.map((x) => (
          <div key={x.q}>
            <h3>{x.q}</h3>
            <p>{x.a}</p>
          </div>
        ))}
      </div>
    </section>
  )
}

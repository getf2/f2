import { useReveal } from '#/lib/useReveal'

const CHIPS = ['battery', 'speed test', 'keep awake', 'wifi password', 'connect airpods', 'uninstall zoom']

// Real Quick Open answers (each cropped to the query, its answer and the
// footer), in two columns: the second sits a little lower.
const ANSWERS = [
  [
    { shot: 'time-australia', height: 338, alt: 'Quick Open showing the time in Sydney, Adelaide, Brisbane, Darwin and Perth' },
    { shot: 'remind', height: 179, alt: 'Quick Open adding a reminder to call mom tomorrow at 17:00' },
    { shot: 'translate', height: 201, alt: 'Quick Open translating "good morning" into Japanese' },
    { shot: 'darkmode', height: 179, alt: 'Quick Open with Toggle Dark Mode' },
  ],
  [
    { shot: 'time-london', height: 218, alt: 'Quick Open: 3pm here is 20:00 in London, and 15:00 in London, Canada' },
    { shot: 'define', height: 225, alt: "Quick Open with the Dictionary's meaning of serendipity" },
    { shot: 'temp', height: 179, alt: 'Quick Open converting 30 °C to 86 °F' },
    { shot: 'wifi', height: 218, alt: 'Quick Open offering to turn Wi-Fi off, or open its settings' },
  ],
]

export function Ask() {
  const [ref, reveal] = useReveal<HTMLElement>()
  const [shotsRef, shotsReveal] = useReveal<HTMLDivElement>()
  const [answersRef, answersReveal] = useReveal<HTMLElement>()

  return (
    <>
      <section ref={ref} className={`feat rev ${reveal}`} id="ask">
        <div className="text">
          <h2>Press ⌘ Space. Ask your Mac.</h2>
          <p>
            Quick Open floats over whatever you're in. Type a few letters to open an app or a file. Or ask it: the time
            in Tokyo, 100 dollars in euros, what a word means, how to say it in Japanese. It sets reminders, checks your
            AirPods' battery, and keeps your Mac awake until a download's done.
          </p>
          <div className="chips">
            {CHIPS.map((c) => (
              <span key={c}>{c}</span>
            ))}
          </div>
        </div>
        <div ref={shotsRef} className={`panel meadow ${shotsReveal}`}>
          <img className="qo-empty win" src="/img/shots/quickopen-empty.webp" width={1360} height={1096} loading="lazy" alt="Quick Open with nothing typed: favorite folders and commands" />
          <img className="qo-calc win" src="/img/shots/quickopen-calc.webp" width={1360} height={496} loading="lazy" alt="Quick Open working out 12*7 + 15%" />
        </div>
      </section>

      <section ref={answersRef} className={`answers ${answersReveal}`} aria-label="Some of what Quick Open answers">
        {ANSWERS.map((column, i) => (
          <div className="col" key={i}>
            {column.map((a) => (
              <Answer key={a.shot} {...a} />
            ))}
          </div>
        ))}
      </section>
    </>
  )
}

function Answer({ shot, height, alt }: (typeof ANSWERS)[number][number]) {
  const [ref, reveal] = useReveal<HTMLImageElement>()

  return (
    <img
      ref={ref}
      className={`win ${reveal}`}
      src={`/img/shots/quickopen-${shot}.webp`}
      width={680}
      height={height}
      loading="lazy"
      alt={alt}
    />
  )
}

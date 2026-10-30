import { useEffect, useState } from 'react'

import { OPEN_NOTICE_EVENT, readChoice, saveChoice } from '#/lib/analytics'
import type { Choice } from '#/lib/analytics'

// Says once, in the bottom left corner, that the site collects analytics and
// the app doesn't, with a way to opt out of Yaap's visitor IDs
// (src/lib/analytics.ts). It opens again from the privacy page. Visits are
// counted either way, without IDs after opting out.
export function AnalyticsNotice() {
  const [open, setOpen] = useState(false)

  useEffect(() => {
    setOpen(readChoice() === null)
    const reopen = () => setOpen(true)
    window.addEventListener(OPEN_NOTICE_EVENT, reopen)
    return () => window.removeEventListener(OPEN_NOTICE_EVENT, reopen)
  }, [])

  if (!open) return null

  const choose = (choice: Choice) => {
    saveChoice(choice)
    setOpen(false)
  }

  return (
    <section className="notice" aria-label="Analytics">
      <p>
        This website collects analytics, to see how people find F2. The F2 app itself collects nothing.{' '}
        <a href="/privacy#this-website">Privacy</a>
      </p>
      <div className="notice-actions">
        <button type="button" className="ok" onClick={() => choose('granted')}>
          OK
        </button>
        <button type="button" onClick={() => choose('denied')}>
          Opt out
        </button>
      </div>
    </section>
  )
}

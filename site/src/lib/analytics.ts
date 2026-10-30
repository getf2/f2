// Visits and Download clicks go to Yaap (yaap.sh, the "F2" website there),
// with its visitor and session IDs, which count people rather than page
// views and keep where a visit came from as it moves between pages, so a
// Download click shows the referrer it started with. The notice
// (AnalyticsNotice.tsx) lets visitors opt out of the IDs: Yaap then counts
// their visits without them. The choice is kept in local storage ("denied"
// is opted out, "granted" closed the notice), and CHOICE_SCRIPT in
// __root.tsx turns the IDs off before the tracker starts. The site ID is
// public, not a secret: Yaap only takes events from https://getf2.com. Keep
// /privacy's "This website" in step with this.

export const TRACKER_SRC = 'https://yaap.sh/script.js'
export const YAAP_SITE_ID = 'c5deec01-6333-4c69-9b23-52a726be3b92'

export type Choice = 'granted' | 'denied'

const KEY = 'f2-analytics'
// The privacy page's "change your choice" asks the notice to open again.
export const OPEN_NOTICE_EVENT = 'f2:analytics-notice'

type Tracker = {
  track: (name: string, properties?: Record<string, string>) => Promise<boolean>
  setIdentifiers: (enabled: boolean) => void
}

declare global {
  interface Window {
    osAnalytics?: Tracker
  }
}

/** The script that runs right after the tracker's tag, before the deferred tracker reads it. */
export const CHOICE_SCRIPT = `try{localStorage.getItem(${JSON.stringify(KEY)})==="denied"&&document.querySelector('script[src="${TRACKER_SRC}"]').setAttribute("data-identifiers","false")}catch(e){}`

export function readChoice(): Choice | null {
  try {
    const value = localStorage.getItem(KEY)
    return value === 'granted' || value === 'denied' ? value : null
  } catch {
    return null
  }
}

export function saveChoice(choice: Choice) {
  const granted = choice === 'granted'
  try {
    localStorage.setItem(KEY, choice)
    // The tracker deletes its IDs when they're turned off. These are its
    // keys, in case it's blocked or hasn't loaded.
    if (!granted) {
      localStorage.removeItem(`os-analytics:${YAAP_SITE_ID}:visitor`)
      sessionStorage.removeItem(`os-analytics:${YAAP_SITE_ID}:session`)
    }
  } catch {
    // Storage is off: the choice lasts for this page.
  }
  if (window.osAnalytics) window.osAnalytics.setIdentifiers(granted)
  // Not started yet: it reads the attribute when it does.
  else document.querySelector(`script[src="${TRACKER_SRC}"]`)?.setAttribute('data-identifiers', String(granted))
}

export function openNotice() {
  window.dispatchEvent(new Event(OPEN_NOTICE_EVENT))
}

/** Counts a click on a Download button; `place` is which one (nav, hero, closing). */
export function trackDownload(place: string) {
  void window.osAnalytics?.track('download', { place })
}

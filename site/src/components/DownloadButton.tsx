import { DOWNLOAD_URL } from '#/lib/site'

export function DownloadButton() {
  return (
    <a className="btn" href={DOWNLOAD_URL}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M12 4v12m0 0-5-5m5 5 5-5M5 20h14" />
      </svg>
      Download for Mac
    </a>
  )
}

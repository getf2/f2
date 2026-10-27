import { DOWNLOAD_URL } from '#/lib/site'

// The same on every page: the home page's sections, then the other pages.
const LINKS = [
  { href: '/#space', label: 'Free up space' },
  { href: '/#search', label: 'Find files' },
  { href: '/#ask', label: 'Ask your Mac' },
  { href: '/features', label: 'All features' },
  { href: '/extensions', label: 'Extensions' },
  { href: '/changelog', label: "What's new" },
  { href: '/#faq', label: 'Questions' },
]

export function Nav({ current }: { current?: string }) {
  return (
    <nav className="nav" aria-label="Main">
      <a className="brand" href="/">
        <img src="/icon.png" alt="" width={30} height={30} />
        F2
      </a>
      <div className="links">
        {LINKS.map((l) => (
          <a key={l.href} href={l.href} aria-current={l.href === current ? 'page' : undefined}>
            {l.label}
          </a>
        ))}
      </div>
      <a className="navcta" href={DOWNLOAD_URL}>
        Download
      </a>
    </nav>
  )
}

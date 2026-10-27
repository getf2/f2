import { useEffect, useRef, useState } from 'react'

// Marks an element to rise into place the first time it scrolls into view.
// Returns its ref and its class: "reveal", then "reveal in". styles.css hides
// a .reveal that isn't .in only when scripts run, so the page reads the same
// without them.
export function useReveal<T extends Element>() {
  const ref = useRef<T>(null)
  const [shown, setShown] = useState(false)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setShown(true)
          io.disconnect()
        }
      },
      // Wait until it's a little way up from the bottom of the window.
      { rootMargin: '0px 0px -10% 0px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [])

  return [ref, shown ? 'reveal in' : 'reveal'] as const
}

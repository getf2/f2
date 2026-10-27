import { useEffect, useRef } from 'react'

import { living } from '#/lib/living/living'
import type { Paint } from './Plate'

// The painting behind a section, alive: its clouds drifting, its grass and
// water moving, birds now and then (src/lib/living). The painting itself is
// the .sky's background, from styles.css: that's what shows first, and
// what stays with less motion or where it can't be drawn.
export function LivingSky({ paint, y }: { paint: Paint; y: number }) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!ref.current || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    return living(ref.current, paint, [0.5, y])
  }, [paint, y])

  return <div ref={ref} className="sky" aria-hidden="true" />
}

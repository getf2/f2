import type { CSSProperties, ReactNode } from 'react'

import { LivingSky } from './LivingSky'
import { Nav } from './Nav'
import type { Paint } from './Plate'

// The top of the pages other than home: a painting with the nav, the title,
// and whatever comes under it.
export function PageHead({
  paint,
  y = '40%',
  path,
  title,
  children,
}: {
  paint: Paint
  y?: string
  path: string
  title: string
  children?: ReactNode
}) {
  return (
    <header className="page-head" style={{ '--paint': `url(/img/paint/${paint}.webp)`, '--y': y } as CSSProperties}>
      <LivingSky paint={paint} y={parseFloat(y) / 100} />
      <Nav current={path} />
      <div className="copy">
        <h1>{title}</h1>
        {children}
      </div>
    </header>
  )
}

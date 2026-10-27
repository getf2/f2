// Line symbols in the manner of SF Symbols, for the sidebar and toolbars.
import { type ReactNode } from 'react'

const line = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' } as const

export const SYMBOL: Record<string, ReactNode> = {
	recents: (
		<>
			<circle cx="12" cy="12" r="8.2" {...line} />
			<path d="M12 7.4V12l3 2" {...line} />
		</>
	),
	airdrop: (
		<>
			<circle cx="12" cy="12" r="1.6" fill="currentColor" />
			<path d="M8.6 15.4a4.8 4.8 0 0 1 0-6.8M15.4 8.6a4.8 4.8 0 0 1 0 6.8M5.8 18.2a8.8 8.8 0 0 1 0-12.4M18.2 5.8a8.8 8.8 0 0 1 0 12.4" {...line} />
		</>
	),
	desktop: (
		<>
			<rect x="3.6" y="5" width="16.8" height="14" rx="2.2" {...line} />
			<path d="M3.8 14.6h16.4" {...line} />
			<path d="M6 16.8h12" {...line} strokeWidth={2.2} />
		</>
	),
	documents: (
		<>
			<path d="M7 3.6h6.4l4.6 4.6v12.2H7z" {...line} />
			<path d="M13.2 3.8v4.6h4.6" {...line} />
		</>
	),
	downloads: (
		<>
			<circle cx="12" cy="12" r="8.2" {...line} />
			<path d="M12 7.6v8M8.8 12.6l3.2 3.2 3.2-3.2" {...line} />
		</>
	),
	folder: <path d="M3.6 7.2c0-.9.7-1.6 1.6-1.6h4.2l1.8 1.8h7.6c.9 0 1.6.7 1.6 1.6v8.4c0 .9-.7 1.6-1.6 1.6H5.2c-.9 0-1.6-.7-1.6-1.6z M3.6 9.6h16.8" {...line} />,
	applications: (
		<>
			<circle cx="12" cy="12" r="8.2" {...line} />
			<path d="M9.2 15.6 12 8.4l2.8 7.2M10 13.6h4" {...line} />
		</>
	),
	trash: (
		<>
			<path d="M5.4 6.6h13.2M9.6 6.4V4.6h4.8v1.8M7 6.8l.9 12.6h8.2l.9-12.6" {...line} />
			<path d="M10.4 9.6v7.2M13.6 9.6v7.2" {...line} strokeWidth={1.2} />
		</>
	),
	disk: (
		<>
			<rect x="3.4" y="8" width="17.2" height="9" rx="2" {...line} />
			<path d="M6.4 14h1.2M9.4 14h1.2" {...line} />
			<path d="M5 8.4 7 5.6h10l2 2.8" {...line} />
		</>
	),
	back: <path d="M14.6 5.4 8 12l6.6 6.6" {...line} strokeWidth={2} />,
	forward: <path d="M9.4 5.4 16 12l-6.6 6.6" {...line} strokeWidth={2} />,
	icons: (
		<>
			<rect x="5" y="5" width="5.6" height="5.6" rx="1.4" {...line} />
			<rect x="13.4" y="5" width="5.6" height="5.6" rx="1.4" {...line} />
			<rect x="5" y="13.4" width="5.6" height="5.6" rx="1.4" {...line} />
			<rect x="13.4" y="13.4" width="5.6" height="5.6" rx="1.4" {...line} />
		</>
	),
	list: (
		<>
			<path d="M9 6.6h10.4M9 12h10.4M9 17.4h10.4" {...line} />
			<circle cx="5.2" cy="6.6" r="1" fill="currentColor" />
			<circle cx="5.2" cy="12" r="1" fill="currentColor" />
			<circle cx="5.2" cy="17.4" r="1" fill="currentColor" />
		</>
	),
	columns: (
		<>
			<rect x="3.8" y="5" width="16.4" height="14" rx="2" {...line} />
			<path d="M9.4 5v14M14.6 5v14" {...line} />
		</>
	),
	gallery: (
		<>
			<rect x="3.8" y="4.4" width="16.4" height="10.4" rx="2" {...line} />
			<path d="M4.4 18.6h1.6M8 18.6h1.6M11.6 18.6h1.6M15.2 18.6h1.6M18.4 18.6h1.2" {...line} strokeWidth={2} />
		</>
	),
	group: (
		<>
			<rect x="3.6" y="5" width="16.8" height="4.4" rx="1.2" {...line} />
			<path d="M4.2 13.4h3.4M10.3 13.4h3.4M16.4 13.4h3.4M4.2 17.6h3.4M10.3 17.6h3.4M16.4 17.6h3.4" {...line} strokeWidth={2.4} />
		</>
	),
	share: (
		<>
			<path d="M12 3.6v10.6M8.6 7 12 3.6 15.4 7" {...line} />
			<path d="M8.6 10H7c-.9 0-1.6.7-1.6 1.6v7.2c0 .9.7 1.6 1.6 1.6h10c.9 0 1.6-.7 1.6-1.6v-7.2c0-.9-.7-1.6-1.6-1.6h-1.6" {...line} />
		</>
	),
	tag: (
		<>
			<path d="M11.4 4h6.6c1.1 0 2 .9 2 2v6.6l-8.2 8.2a1.4 1.4 0 0 1-2 0l-6.6-6.6a1.4 1.4 0 0 1 0-2z" {...line} />
			<circle cx="16.2" cy="7.8" r="1.2" fill="currentColor" />
		</>
	),
	more: (
		<>
			<circle cx="12" cy="12" r="8.4" {...line} />
			<circle cx="8.4" cy="12" r="1.1" fill="currentColor" />
			<circle cx="12" cy="12" r="1.1" fill="currentColor" />
			<circle cx="15.6" cy="12" r="1.1" fill="currentColor" />
		</>
	),
	search: (
		<>
			<circle cx="10.4" cy="10.4" r="6" {...line} strokeWidth={1.8} />
			<path d="m14.8 14.8 4.8 4.8" {...line} strokeWidth={1.8} />
		</>
	),
	xmark: <path d="M7 7l10 10M17 7 7 17" {...line} strokeWidth={1.6} />,
	help: (
		<>
			<circle cx="12" cy="12" r="8.4" {...line} />
			<path d="M9.6 9.6a2.4 2.4 0 1 1 3.4 2.2c-.6.3-1 .8-1 1.5v.5" {...line} />
			<circle cx="12" cy="16.6" r="0.9" fill="currentColor" />
		</>
	),
	chevronDown: <path d="M7 10l5 5 5-5" {...line} strokeWidth={2.2} />,
	chevronRight: <path d="M10 7l5 5-5 5" {...line} strokeWidth={2} />,
	refresh: (
		<>
			<path d="M18.4 12.8a6.4 6.4 0 1 1-2-5.4" {...line} />
			<path d="M17.6 3.8v4.2h-4.2" {...line} />
		</>
	),
	sparkles: (
		<>
			<path d="M10 4.5l1.4 4.1 4.1 1.4-4.1 1.4L10 15.5l-1.4-4.1-4.1-1.4 4.1-1.4z" fill="currentColor" />
			<path d="M17 13.5l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7z" fill="currentColor" />
		</>
	),
	largeFile: (
		<>
			<path d="M7 3.6h6.4l4.6 4.6v12.2H7z" {...line} />
			<circle cx="8.6" cy="17.2" r="3.4" {...line} fill="#fff" />
			<path d="M8.6 15.6v3.2M7 17.2h3.2" {...line} strokeWidth={1.2} />
		</>
	),
	duplicates: (
		<>
			<path d="M8.6 6.6h6l3.4 3.4v10.4H8.6z" {...line} />
			<path d="M6 17.4V3.6h6" {...line} />
		</>
	),
	systemData: (
		<>
			<rect x="4" y="5" width="16" height="6" rx="1.6" {...line} />
			<rect x="4" y="13" width="16" height="6" rx="1.6" {...line} />
			<circle cx="7.4" cy="8" r="0.9" fill="currentColor" />
			<circle cx="7.4" cy="16" r="0.9" fill="currentColor" />
		</>
	),
	box: (
		<>
			<path d="M12 3.8l7.6 4.1v8.2L12 20.2l-7.6-4.1V7.9z" {...line} />
			<path d="M4.6 8 12 12l7.4-4M12 12v8M8.2 5.9l7.5 4.1" {...line} />
		</>
	),
	downloadCircle: (
		<>
			<circle cx="12" cy="12" r="8.2" {...line} />
			<path d="M12 7.6v8M8.8 12.6l3.2 3.2 3.2-3.2" {...line} />
		</>
	),
	iphone: (
		<>
			<rect x="7" y="3.4" width="10" height="17.2" rx="2.4" {...line} />
			<path d="M10.6 5.6h2.8" {...line} />
		</>
	),
	archive: (
		<>
			<rect x="3.6" y="4.4" width="16.8" height="4.6" rx="1.2" {...line} />
			<path d="M5 9v9.4c0 .7.6 1.2 1.2 1.2h11.6c.7 0 1.2-.5 1.2-1.2V9M10 12.6h4" {...line} />
		</>
	),
	hammer: (
		<>
			<path d="M13.6 5.4 18.6 10.4M11.2 7.8l4.9 4.9M15.2 3.8l1.6-.4 3.8 3.8-.4 1.6" {...line} />
			<path d="M12.4 10.6 4.6 18.4a1.4 1.4 0 0 0 2 2l7.8-7.8" {...line} />
		</>
	),
	check: <path d="M6.4 12.4 10.2 16l7.4-8" {...line} strokeWidth={2.4} stroke="#fff" />,
}

export function Sym({ name, size = 16, color, style }: { name: keyof typeof SYMBOL; size?: number; color?: string; style?: React.CSSProperties }) {
	return (
		<svg width={size} height={size} viewBox="0 0 24 24" style={{ color, flexShrink: 0, ...style }}>
			{SYMBOL[name]}
		</svg>
	)
}

// Made-up files for F2's windows, generic, with photos as crops of the
// site's paintings.
import { Doc, Folder, Thumb } from './components/icons'
import { type Big } from './components/bigIcons'
import { type ListRow } from './components/FinderWindow'

export const PHOTOS: { name: string; paint: string; at?: string }[] = [
	{ name: 'Dusk at sea.jpg', paint: 'dusk-sea', at: 'center 40%' },
	{ name: 'Golden hour.jpg', paint: 'golden-hour', at: 'center 45%' },
	{ name: 'Misty forest.jpg', paint: 'mist-forest' },
	{ name: 'Northern lights.jpg', paint: 'night-aurora' },
	{ name: 'Reading nook.jpg', paint: 'study-window' },
	{ name: 'Rice terraces.jpg', paint: 'terraces' },
	{ name: 'River bend.jpg', paint: 'hero-valley', at: 'center 70%' },
	{ name: 'Still lake.jpg', paint: 'still-lake' },
	{ name: 'Summer valley.jpg', paint: 'hero-valley', at: 'center 40%' },
	{ name: 'Wildflowers.jpg', paint: 'meadow', at: 'center 80%' },
]

export const photoIcon = (name: string): Big => {
	const p = PHOTOS.find((p) => p.name === name)!
	return { kind: 'photo', paint: p.paint, at: p.at }
}
export const photoThumb = (name: string) => {
	const p = PHOTOS.find((p) => p.name === name)!
	return <Thumb size={16} paint={p.paint} at={p.at} />
}

const folder = (name: string, date: string, size: string): ListRow => ({ icon: <Folder size={16} />, name, date, size, folder: true })
const file = (name: string, date: string, size: string, label: string, color: string): ListRow => ({
	icon: <Doc size={16} label={label} color={color} />,
	name,
	date,
	size,
})

// Documents, as hero-dual.webp lists it.
export const DOCUMENTS: ListRow[] = [
	folder('Recipes', 'Oct 4, 2026 at 08:15', '9 KB'),
	folder('School', 'Oct 16, 2026 at 13:20', '4 KB'),
	folder('Taxes', 'Oct 19, 2026 at 17:42', '10 KB'),
	folder('Travel', 'Oct 9, 2026 at 06:45', '10 KB'),
	folder('Wedding', 'Aug 30, 2026 at 17:30', '9 KB'),
	folder('Work', 'Yesterday at 11:04', '4 KB'),
	file('Car insurance.pdf', 'Oct 12, 2026 at 09:30', '10 KB', 'PDF', '#e5483f'),
	file('Cover letter.docx', 'Yesterday at 11:05', '4 KB', 'DOCX', '#2f6fd6'),
	file('Lease agreement.pdf', 'Sep 2, 2026 at 10:15', '10 KB', 'PDF', '#e5483f'),
	file('Tax return 2025.pdf', 'Oct 19, 2026 at 17:42', '10 KB', 'PDF', '#e5483f'),
	file('Wedding speech.docx', 'Jul 28, 2026 at 19:12', '4 KB', 'DOCX', '#2f6fd6'),
]

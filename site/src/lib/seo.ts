import { QUESTIONS } from '#/components/Faq'
import { DOWNLOAD_URL, MIN_MACOS, SITE_URL, VERSION, VIDEO_SECONDS, VIDEO_URL } from './site'

// schema.org markup for search engines. Rich results are checked at
// https://search.google.com/test/rich-results

export function softwareApplicationJsonLd() {
  return {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'F2',
    alternateName: 'F2 for Mac',
    url: `${SITE_URL}/`,
    description:
      'A free, faster Finder for Mac: free up space, find files in plain words, Quick Open, Tidy Up, dual pane and clipboard history.',
    applicationCategory: 'UtilitiesApplication',
    operatingSystem: `macOS ${MIN_MACOS} or later`,
    processorRequirements: 'Apple silicon or Intel (x86-64)',
    softwareVersion: VERSION,
    downloadUrl: DOWNLOAD_URL,
    installUrl: DOWNLOAD_URL,
    image: `${SITE_URL}/og.jpg`,
    screenshot: `${SITE_URL}/img/shots/hero-dual.webp`,
    releaseNotes: `${SITE_URL}/changelog`,
    isAccessibleForFree: true,
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    author: { '@type': 'Person', name: 'Dagur Leó' },
  }
}

export function faqPageJsonLd() {
  return {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: QUESTIONS.map(({ q, a }) => ({
      '@type': 'Question',
      name: q,
      acceptedAnswer: { '@type': 'Answer', text: a },
    })),
  }
}

// The home page's video (components/Watch.tsx).
export function videoJsonLd() {
  return {
    '@context': 'https://schema.org',
    '@type': 'VideoObject',
    name: 'F2 for Mac in a minute',
    description:
      'Find a file from a typo, ask your Mac a question, free up space and tidy Downloads with F2, a free, faster Finder for Mac.',
    thumbnailUrl: `${SITE_URL}/img/video/f2-feature.webp`,
    uploadDate: '2026-10-25',
    duration: `PT${VIDEO_SECONDS}S`,
    contentUrl: VIDEO_URL,
    embedUrl: `${SITE_URL}/#watch`,
  }
}

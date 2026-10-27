import { Buffer } from 'node:buffer'

// What F2 sends from people's Macs when they say so (the app's
// src/reports.rs): a report of it having quit unexpectedly, or feedback
// from Help › Send Feedback. Each is emailed on to leo@getf2.com (the
// binding sends nowhere else: wrangler.jsonc) and kept nowhere else.

const FROM = { email: 'reports@mail.getf2.com', name: 'F2' }
const TO = 'leo@getf2.com'

/** Bigger than any F2 report (a crash report is 100–300 KB as sent). */
const MAX = 1_500_000

type Reports = Env

type Mail = {
  subject: string
  text: string
  replyTo?: string
  attachments?: { content: string; filename: string; type: string; disposition: 'attachment' }[]
}

function answer(body: object, status: number): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

/** POST /api/crash and /api/feedback. */
export async function reports(request: Request, env: Reports, path: string): Promise<Response> {
  const kind = path === '/api/crash' ? 'crash' : path === '/api/feedback' ? 'feedback' : null
  if (!kind) return answer({ error: 'There’s nothing here.' }, 404)
  if (request.method !== 'POST') return answer({ error: 'Send it with POST.' }, 405)

  const ip = request.headers.get('cf-connecting-ip') ?? 'unknown'
  if (!(await env.REPORT_LIMIT.limit({ key: ip })).success) {
    return answer({ error: 'Too many at once. Try again in a minute.' }, 429)
  }
  const raw = await request.text()
  if (raw.length > MAX) return answer({ error: 'That’s too big to send.' }, 413)
  let body: Record<string, unknown>
  try {
    body = JSON.parse(raw)
  } catch {
    return answer({ error: 'That isn’t a report.' }, 400)
  }
  const field = (key: string, max = 200) => (typeof body[key] === 'string' ? (body[key] as string).slice(0, max) : '')
  const version = field('version', 40)
  if (!version) return answer({ error: 'That isn’t a report.' }, 400)
  const about = `F2 ${version} · macOS ${field('macos', 60)} · ${field('model', 40)}`

  let mail: Mail
  if (kind === 'feedback') {
    const message = field('message', 20_000).trim()
    if (!message) return answer({ error: 'There’s nothing in it.' }, 400)
    const email = field('email').trim()
    const replyTo = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) ? email : undefined
    mail = {
      subject: `F2 feedback: ${headline(message)}`,
      text: `${message}\n\n—\nFrom: ${replyTo ?? 'no email given'}\n${about}`,
      replyTo,
    }
  } else {
    const report = field('report', MAX)
    if (!report) return answer({ error: 'There’s no report in it.' }, 400)
    const { crashed, title, lines } = summary(report)
    const panic = field('panic', 20_000)
    const name = field('name', 80).replace(/[^\w.-]/g, '') || 'F2.ips'
    mail = {
      subject: `F2 ${crashed ?? version} crashed: ${title}`,
      text: [`Sent from ${about}`, '', lines, panic ? `\nWhat F2 said as it stopped:\n${panic}` : '', '\nmacOS’s report is attached.'].join('\n'),
      attachments: [{ content: Buffer.from(report).toString('base64'), filename: name, type: 'application/json', disposition: 'attachment' }],
    }
  }

  // (Crash reports' fields are whatever was sent: a subject of one line.)
  mail.subject = mail.subject.replace(/[\u0000-\u001f\u007f\u2028\u2029]+/g, ' ').slice(0, 160)
  try {
    await env.EMAIL.send({ to: TO, from: FROM, ...mail })
  } catch (e) {
    const error = e as { code?: string; message?: string }
    console.error('reports: couldn’t email it', error.code, error.message)
    return answer({ error: 'It couldn’t be passed on just now. Try again later.' }, 502)
  }
  return answer({ ok: true }, 200)
}

/** A message's first line, short, for a subject. */
function headline(text: string): string {
  const line = text.split('\n')[0].trim()
  return line.length > 70 ? `${line.slice(0, 69)}…` : line
}

type Frame = { imageIndex?: number; symbol?: string }

/** Which F2 crashed, and where it stopped, from macOS's report: a line of
 * JSON, then the rest. */
function summary(report: string): { crashed?: string; title: string; lines: string } {
  try {
    const head = JSON.parse(report.slice(0, report.indexOf('\n')))
    const crashed = head.app_version ? `${head.app_version}${head.build_version ? ` (${head.build_version})` : ''}` : undefined
    const body = JSON.parse(report.slice(report.indexOf('\n') + 1))
    const images: { name?: string }[] = body.usedImages ?? []
    const thread = body.threads?.[body.faultingThread ?? 0]
    const frames: Frame[] = thread?.frames ?? []
    const shown = frames.slice(0, 20).map((f) => `  ${images[f.imageIndex ?? -1]?.name ?? '?'}  ${f.symbol ?? '?'}`)
    const ours = frames.find((f) => images[f.imageIndex ?? -1]?.name === 'F2' && f.symbol)?.symbol
    const exception = [body.exception?.type, body.exception?.signal].filter(Boolean).join(' ')
    const said = Object.values(body.asi ?? {}).flat().join(' ')
    const title = [exception || 'a crash', ours ? `in ${ours.slice(0, 60)}` : ''].filter(Boolean).join(' ')
    const lines = [
      `${exception}${body.termination?.indicator ? ` (${body.termination.indicator})` : ''}`,
      said ? `macOS said: ${said}` : '',
      `Thread ${body.faultingThread}${thread?.name ? ` (${thread.name})` : ''}:`,
      ...shown,
    ]
    return { crashed, title, lines: lines.filter(Boolean).join('\n') }
  } catch {
    return { title: 'a crash (its report couldn’t be read)', lines: '' }
  }
}

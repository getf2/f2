// F2's Cloudflare extension: your Workers and Pages projects, in every
// account you're in, and their deployments; signed in as wrangler is
// (`wrangler login`), through Cloudflare's API.
//
// F2 says what it wants on stdin (the command, its args, what's typed),
// and reads what this prints: JSON messages, one a line. See EXTENSIONS.md.

type Asked = { command: string; args: Record<string, string>; query: string }
type Row = Record<string, unknown>

const API = "https://api.cloudflare.com/client/v4"
const ORANGE = "#f38020"

const say = (message: Record<string, unknown>) => process.stdout.write(JSON.stringify(message) + "\n")

function fail(why: string): never {
  say({ error: why, loading: false })
  process.exit(0)
}

// wrangler's own sign-in: its token (refreshed if it's old).
async function token(): Promise<string> {
  const run = Bun.spawn(["bunx", "wrangler@4", "auth", "token"], { stdout: "pipe", stderr: "pipe" })
  const out = await new Response(run.stdout).text()
  await run.exited
  const last = out.trim().split("\n").pop()?.trim() ?? ""
  if (run.exitCode !== 0 || !last || last.includes(" ")) fail("Sign in to Cloudflare first: `npx wrangler login` in a terminal.")
  return last
}

async function api<T>(key: string, path: string): Promise<T> {
  const response = await fetch(API + path, { headers: { Authorization: `Bearer ${key}` } })
  const body = (await response.json().catch(() => ({}))) as { success?: boolean; result?: T; errors?: { message: string }[] }
  if (!response.ok || body.success === false) fail(`Cloudflare said: ${body.errors?.[0]?.message ?? response.statusText}`)
  return body.result as T
}

function ago(stamp?: string): string {
  if (!stamp) return ""
  const secs = (Date.now() - new Date(stamp).getTime()) / 1000
  for (const [unit, size] of [["y", 31536000], ["mo", 2592000], ["d", 86400], ["h", 3600], ["m", 60]] as const) {
    if (secs >= size) return `${Math.floor(secs / size)}${unit} ago`
  }
  return "just now"
}

type Account = { id: string; name: string }
type Script = { id: string; modified_on: string; created_on: string; last_deployed_from?: string; has_assets?: boolean; routes?: { pattern: string }[] }
type Project = { name: string; subdomain: string; domains?: string[]; latest_deployment?: { created_on: string; url: string; deployment_trigger?: { metadata?: { commit_message?: string; branch?: string } }; latest_stage?: { status: string } } }

const dash = (account: string, rest: string) => `https://dash.cloudflare.com/${account}/${rest}`

async function workers() {
  say({ loading: "Signing in with wrangler…" })
  const key = await token()
  say({ loading: "Finding your accounts…" })
  const accounts = await api<Account[]>(key, "/accounts?per_page=50")
  let first = true
  for (const [i, account] of accounts.entries()) {
    say({ loading: `Listing ${account.name}'s Workers…`, progress: i / accounts.length })
    const [scripts, projects] = await Promise.all([
      api<Script[]>(key, `/accounts/${account.id}/workers/scripts`),
      api<Project[]>(key, `/accounts/${account.id}/pages/projects`).catch(() => [] as Project[]),
    ])
    scripts.sort((a, b) => b.modified_on.localeCompare(a.modified_on))
    const rows: Row[] = scripts.map((s) => ({
      title: s.id,
      subtitle: s.routes?.[0]?.pattern ?? "",
      icon: s.has_assets ? "globe" : "gearshape.2.fill",
      color: ORANGE,
      section: `${account.name} · Workers`,
      accessory: `deployed ${ago(s.modified_on)}`,
      tags: s.last_deployed_from ? [{ text: s.last_deployed_from, color: "gray" }] : [],
      keywords: [account.name, ...(s.routes?.map((r) => r.pattern) ?? [])],
      actions: [
        { title: "Open in Dashboard", open: dash(account.id, `workers/services/view/${s.id}/production`), icon: "safari" },
        { title: "Deployments", push: { command: "deployments", args: { account: account.id, script: s.id } }, icon: "clock.arrow.circlepath" },
        { title: "Copy Name", copy: s.id, icon: "doc.on.doc" },
      ],
    }))
    for (const p of projects) {
      const latest = p.latest_deployment
      const status = latest?.latest_stage?.status
      rows.push({
        title: p.name,
        subtitle: latest?.deployment_trigger?.metadata?.commit_message?.split("\n")[0] ?? p.subdomain,
        icon: "doc.richtext.fill",
        color: ORANGE,
        section: `${account.name} · Pages`,
        accessory: latest ? `deployed ${ago(latest.created_on)}` : "",
        tags: status ? [{ text: status, color: status === "success" ? "green" : status === "failure" ? "red" : "yellow" }] : [],
        keywords: [account.name, p.subdomain, ...(p.domains ?? [])],
        actions: [
          { title: "Open Site", open: `https://${p.domains?.[0] ?? p.subdomain}`, icon: "globe" },
          { title: "Deployments", push: { command: "pages-deployments", args: { account: account.id, project: p.name } }, icon: "clock.arrow.circlepath" },
          { title: "Open in Dashboard", open: dash(account.id, `pages/view/${p.name}`), icon: "safari" },
        ],
      })
    }
    // (Each account's as it's found: the first in place of what was shown before.)
    say(first ? { items: rows } : { append: rows })
    first = false
  }
  say({ loading: false, progress: 1, placeholder: "Search your Workers and Pages…" })
}

type Deployment = { id: string; source: string; author_email?: string; created_on: string; annotations?: Record<string, string>; versions: { version_id: string; percentage: number }[] }

async function deployments(asked: Asked) {
  const { account, script } = asked.args
  say({ title: `${script}: Deployments`, loading: "Signing in with wrangler…" })
  const key = await token()
  say({ loading: "Fetching deployments…" })
  const found = (await api<{ deployments: Deployment[] }>(key, `/accounts/${account}/workers/scripts/${script}/deployments`)).deployments
  found.sort((a, b) => b.created_on.localeCompare(a.created_on))
  const items = found.map((d, i) => {
    const message = d.annotations?.["workers/message"]
    const versions = d.versions.map((v) => `- \`${v.version_id}\`: ${v.percentage}%`).join("\n")
    return {
      title: message ?? `Deployed ${ago(d.created_on)}`,
      subtitle: d.author_email ?? "",
      icon: i === 0 ? "checkmark.circle.fill" : "clock",
      color: i === 0 ? "green" : "gray",
      tags: [...(i === 0 ? [{ text: "Live", color: "green" }] : []), { text: d.source, color: "gray" }],
      accessory: ago(d.created_on),
      detail: {
        markdown: `## ${message ?? "Deployment"}\n\n${versions}`,
        metadata: [
          { label: "When", text: new Date(d.created_on).toLocaleString() },
          { label: "By", text: d.author_email ?? "" },
          { label: "From", text: d.source },
          { label: "Trigger", text: d.annotations?.["workers/triggered_by"] ?? "" },
        ],
      },
      actions: [
        { title: "Open in Dashboard", open: dash(account, `workers/services/view/${script}/production/deployments`), icon: "safari" },
        { title: "Copy Version ID", copy: d.versions[0]?.version_id ?? "", icon: "doc.on.doc" },
        { title: "Copy Deployment ID", copy: d.id, icon: "doc.on.doc" },
      ],
    }
  })
  say({ items, loading: false })
}

type PagesDeployment = { id: string; url: string; environment: string; created_on: string; latest_stage?: { status: string }; deployment_trigger?: { metadata?: { commit_message?: string; branch?: string; commit_hash?: string } } }

async function pagesDeployments(asked: Asked) {
  const { account, project } = asked.args
  say({ title: `${project}: Deployments`, loading: "Signing in with wrangler…" })
  const key = await token()
  say({ loading: "Fetching deployments…" })
  const found = await api<PagesDeployment[]>(key, `/accounts/${account}/pages/projects/${project}/deployments?per_page=25`)
  const items = found.map((d) => {
    const meta = d.deployment_trigger?.metadata ?? {}
    const status = d.latest_stage?.status ?? ""
    return {
      title: meta.commit_message?.split("\n")[0] ?? d.id.slice(0, 8),
      subtitle: [meta.branch, meta.commit_hash?.slice(0, 7)].filter(Boolean).join(" · "),
      icon: status === "success" ? "checkmark.circle.fill" : status === "failure" ? "xmark.circle.fill" : "clock.fill",
      color: status === "success" ? "green" : status === "failure" ? "red" : "yellow",
      section: d.environment === "production" ? "Production" : "Previews",
      tags: status ? [{ text: status, color: status === "success" ? "green" : status === "failure" ? "red" : "yellow" }] : [],
      accessory: ago(d.created_on),
      actions: [
        { title: "Open Deployment", open: d.url, icon: "globe" },
        { title: "Open in Dashboard", open: dash(account, `pages/view/${project}/${d.id}`), icon: "safari" },
        { title: "Copy URL", copy: d.url, icon: "link" },
      ],
    }
  })
  say({ items, loading: false })
}

const asked = (await Bun.stdin.json()) as Asked
switch (asked.command) {
  case "workers":
    await workers()
    break
  case "deployments":
    await deployments(asked)
    break
  case "pages-deployments":
    await pagesDeployments(asked)
    break
  default:
    fail(`No command "${asked.command}".`)
}

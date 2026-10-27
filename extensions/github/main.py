#!/usr/bin/env python3
"""F2's GitHub extension: your repos, pull requests, issues and Actions
runs, through the gh command (signed in already: `gh auth login`).

F2 says what it wants on stdin (the command, its args, what's typed), and
reads what this prints: JSON messages, one a line. See EXTENSIONS.md."""

import json
import subprocess
import sys
from datetime import datetime, timezone

GITHUB = "#24292f"


def say(**message):
    print(json.dumps(message), flush=True)


def gh(*args):
    """gh's JSON output, or a message saying why there's none."""
    try:
        done = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60)
    except FileNotFoundError:
        fail("The gh command isn't installed: `brew install gh`, then `gh auth login`.")
    except subprocess.TimeoutExpired:
        fail("GitHub took too long to answer.")
    if done.returncode != 0:
        why = done.stderr.strip().splitlines()[-1:] or ["gh didn't work"]
        if "auth login" in done.stderr:
            why = ["Sign in to GitHub first: `gh auth login` in a terminal."]
        fail(why[0])
    return json.loads(done.stdout or "null")


def fail(why):
    say(error=why, loading=False)
    sys.exit(0)


def ago(stamp):
    """"3h ago", from GitHub's "2026-10-26T02:01:27Z"."""
    if not stamp:
        return ""
    then = datetime.fromisoformat(stamp.replace("Z", "+00:00"))
    secs = (datetime.now(timezone.utc) - then).total_seconds()
    for unit, size in (("y", 31536000), ("mo", 2592000), ("d", 86400), ("h", 3600), ("m", 60)):
        if secs >= size:
            return f"{int(secs // size)}{unit} ago"
    return "just now"


def repos(asked):
    say(loading="Fetching your repos…")
    fields = "nameWithOwner,name,description,url,sshUrl,isPrivate,isFork,isArchived,pushedAt,stargazerCount,primaryLanguage,owner"
    found = gh("repo", "list", "--limit", "1000", "--json", fields)
    # (Your organizations' too, after yours.)
    orgs = [o["login"] for o in (gh("api", "user/orgs") or [])]
    for i, org in enumerate(orgs):
        say(loading=f"Fetching {org}'s repos…", progress=(i + 1) / (len(orgs) + 1))
        found += gh("repo", "list", org, "--limit", "1000", "--json", fields)
    found.sort(key=lambda r: r["pushedAt"] or "", reverse=True)
    items = []
    for r in found:
        name = r["nameWithOwner"]
        tags = []
        if r["isPrivate"]:
            tags.append({"text": "Private", "color": "gray"})
        if r["isFork"]:
            tags.append({"text": "Fork", "color": "purple"})
        if r["isArchived"]:
            tags.append({"text": "Archived", "color": "orange"})
        language = (r.get("primaryLanguage") or {}).get("name")
        stars = r["stargazerCount"]
        items.append({
            "title": r["name"],
            "subtitle": r["description"] or name,
            "icon": "lock.fill" if r["isPrivate"] else "book.closed.fill",
            "color": GITHUB,
            "section": r["owner"]["login"],
            "accessory": " · ".join(x for x in [language, f"★ {stars}" if stars else None, ago(r["pushedAt"])] if x),
            "tags": tags,
            "keywords": [name, language or ""],
            "actions": [
                {"title": "Open on GitHub", "open": r["url"], "icon": "safari"},
                {"title": "Pull Requests", "push": {"command": "prs", "args": {"repo": name}}, "icon": "arrow.triangle.pull"},
                {"title": "Issues", "push": {"command": "issues", "args": {"repo": name}}, "icon": "exclamationmark.circle"},
                {"title": "Actions Runs", "push": {"command": "runs", "args": {"repo": name}}, "icon": "play.circle"},
                {"title": "Copy Clone URL", "copy": r["sshUrl"], "icon": "doc.on.doc"},
                {"title": "Copy Name", "copy": name, "icon": "textformat"},
            ],
        })
    say(items=items, loading=False, placeholder="Search your repos…")


def pull_request_row(p, repo):
    tags = []
    if p.get("isDraft"):
        tags.append({"text": "Draft", "color": "gray"})
    review = p.get("reviewDecision")
    if review == "APPROVED":
        tags.append({"text": "Approved", "color": "green"})
    elif review == "CHANGES_REQUESTED":
        tags.append({"text": "Changes", "color": "red"})
    author = (p.get("author") or {}).get("login", "")
    facts = [{"label": "Repo", "text": repo}, {"label": "Author", "text": author}, {"label": "Updated", "text": ago(p.get("updatedAt"))}]
    if p.get("headRefName"):
        facts.append({"label": "Branch", "text": p["headRefName"]})
    if "additions" in p:
        facts.append({"label": "Changes", "text": f"+{p['additions']} −{p['deletions']}"})
    actions = [{"title": "Open on GitHub", "open": p["url"], "icon": "safari"}, {"title": "Copy Link", "copy": p["url"], "icon": "link"}]
    if p.get("headRefName"):
        actions.append({"title": "Copy Branch Name", "copy": p["headRefName"], "icon": "arrow.triangle.branch"})
    return {
        "title": p["title"],
        "subtitle": f"#{p['number']}",
        "icon": "arrow.triangle.pull",
        "color": "#8250df" if p.get("isDraft") else "#1a7f37",
        "tags": tags,
        "keywords": [str(p["number"]), author],
        "detail": {"markdown": f"## {p['title']}\n\n{p.get('body') or '_No description._'}", "metadata": facts},
        "actions": actions,
    }


def prs(asked):
    repo = asked["args"]["repo"]
    say(title=f"{repo.split('/')[-1]}: Pull Requests", loading="Fetching pull requests…")
    found = gh("pr", "list", "-R", repo, "--state", "open", "--limit", "100", "--json", "number,title,author,updatedAt,isDraft,url,headRefName,reviewDecision,body,additions,deletions")
    say(items=[pull_request_row(p, repo) for p in found], loading=False, placeholder="Search pull requests…")


def my_prs(asked):
    say(loading="Fetching your pull requests…")
    found = gh("search", "prs", "--author", "@me", "--state", "open", "--limit", "100", "--json", "number,title,repository,updatedAt,isDraft,url,body,author")
    items = []
    for p in found:
        repo = p["repository"]["nameWithOwner"]
        row = pull_request_row(p, repo)
        row["subtitle"] = f"{repo} #{p['number']}"
        row["section"] = repo
        items.append(row)
    say(items=items, loading=False)


def issues(asked):
    repo = asked["args"]["repo"]
    say(title=f"{repo.split('/')[-1]}: Issues", loading="Fetching issues…")
    found = gh("issue", "list", "-R", repo, "--state", "open", "--limit", "100", "--json", "number,title,author,updatedAt,url,labels,body")
    items = []
    for i in found:
        author = (i.get("author") or {}).get("login", "")
        items.append({
            "title": i["title"],
            "subtitle": f"#{i['number']}",
            "icon": "exclamationmark.circle.fill",
            "color": "#1a7f37",
            "tags": [{"text": l["name"], "color": "#" + l["color"] if l.get("color") else None} for l in i.get("labels", [])[:3]],
            "keywords": [str(i["number"]), author],
            "detail": {"markdown": f"## {i['title']}\n\n{i.get('body') or '_No description._'}", "metadata": [{"label": "Author", "text": author}, {"label": "Updated", "text": ago(i.get("updatedAt"))}]},
            "actions": [{"title": "Open on GitHub", "open": i["url"], "icon": "safari"}, {"title": "Copy Link", "copy": i["url"], "icon": "link"}],
        })
    say(items=items, loading=False, placeholder="Search issues…")


def runs(asked):
    repo = asked["args"]["repo"]
    say(title=f"{repo.split('/')[-1]}: Actions", loading="Fetching runs…")
    found = gh("run", "list", "-R", repo, "--limit", "50", "--json", "databaseId,displayTitle,status,conclusion,workflowName,headBranch,event,createdAt,url")
    look = {
        "success": ("checkmark.circle.fill", "green", "Passed"),
        "failure": ("xmark.circle.fill", "red", "Failed"),
        "cancelled": ("slash.circle", "gray", "Cancelled"),
        "skipped": ("arrow.uturn.right.circle", "gray", "Skipped"),
    }
    items = []
    for r in found:
        if r["status"] != "completed":
            icon, color, said = "clock.fill", "yellow", r["status"].replace("_", " ").title()
        else:
            icon, color, said = look.get(r["conclusion"], ("questionmark.circle", "gray", (r["conclusion"] or "").title()))
        actions = [{"title": "Open on GitHub", "open": r["url"], "icon": "safari"}]
        if r["conclusion"] == "failure":
            actions.append({"title": "Show Failed Steps' Log", "push": {"command": "run-log", "args": {"repo": repo, "id": r["databaseId"]}}, "icon": "doc.text.magnifyingglass"})
        actions.append({"title": "Copy Link", "copy": r["url"], "icon": "link"})
        items.append({
            "title": r["displayTitle"],
            "subtitle": f"{r['workflowName']} · {r['headBranch']}",
            "icon": icon,
            "color": color,
            "section": r["workflowName"],
            "tags": [{"text": said, "color": color}],
            "accessory": ago(r["createdAt"]),
            "keywords": [r["headBranch"], r["event"]],
            "actions": actions,
        })
    say(items=items, loading=False, placeholder="Search runs…")


def run_log(asked):
    repo, run = asked["args"]["repo"], str(asked["args"]["id"])
    say(title="Failed Steps", loading="Fetching the log…")
    done = subprocess.run(["gh", "run", "view", run, "-R", repo, "--log-failed"], capture_output=True, text=True, timeout=120)
    lines = done.stdout.splitlines()[-300:] or ["(No log: the run may have failed before any step ran.)"]
    # (Each line "job\tstep\ttime text": the step's name once, then its text.)
    out, step = [], None
    for line in lines:
        parts = line.split("\t", 2)
        if len(parts) == 3:
            if parts[1] != step:
                step = parts[1]
                out.append(f"\n── {step} ──")
            out.append(parts[2].split(" ", 1)[-1])
        else:
            out.append(line)
    url = f"https://github.com/{repo}/actions/runs/{run}"
    say(detail={"markdown": "```\n" + "\n".join(out).strip() + "\n```", "metadata": [{"label": "Repo", "text": repo}, {"label": "Run", "text": run}], "actions": [{"title": "Open on GitHub", "open": url, "icon": "safari"}, {"title": "Copy Log", "copy": "\n".join(out).strip(), "icon": "doc.on.doc"}]}, loading=False)


COMMANDS = {"repos": repos, "my-prs": my_prs, "prs": prs, "issues": issues, "runs": runs, "run-log": run_log}

if __name__ == "__main__":
    asked = json.load(sys.stdin)
    COMMANDS[asked["command"]](asked)

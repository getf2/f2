#!/bin/bash
# The themes the notch's terminal comes with (src/terminal/theme.rs), made
# again: iTerm2-Color-Schemes' (Mark Badolato's, MIT; as Ghostty's own
# themes are), those its licences.yml says may be passed on, under a
# licence that goes with F2's GPL-3.0 (not "unknown", "nonfree" or
# GPL-2.0-only). assets/terminal/themes.tsv has their colors, one a line;
# THEMES.md each one's licence and source.
#
# ./scripts/terminal-themes.sh (downloads about 30 MB)
set -euo pipefail
cd "$(dirname "$0")/.."

commit=3d8a6a1a983d7fe721aa6cb9a5b98066cd74303a
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/repo.tar.gz" "https://codeload.github.com/mbadolato/iTerm2-Color-Schemes/tar.gz/$commit"
tar xzf "$work/repo.tar.gz" -C "$work"
repo=$(ls -d "$work"/iTerm2-Color-Schemes-*)
mkdir -p assets/terminal
cp "$repo/LICENSE" assets/terminal/LICENSE-iTerm2-Color-Schemes.txt

python3 -I - "$repo" "$commit" <<'EOF'
import os, sys
repo, commit = sys.argv[1], sys.argv[2]

# licenses.yml: a scheme a line, then its licence, source and note.
schemes = {}
name = None
for line in open(os.path.join(repo, "licenses.yml"), encoding="utf-8"):
    if not line.strip() or line.lstrip().startswith("#"):
        continue
    if not line.startswith(" "):
        name = line.rstrip().rstrip(":").strip().strip("'\"")
        schemes[name] = {}
    else:
        key, _, value = line.strip().partition(":")
        schemes[name][key] = value.strip().strip("'\"")

passed_on = {"MIT", "MIT OR Apache-2.0", "Apache-2.0", "ISC", "CC0-1.0", "WTFPL", "Zlib", "Unlicense", "0BSD", "CC-BY-SA-4.0",
             "GPL-2.0-or-later", "GPL-3.0-only", "GPL-3.0-or-later", "LGPL-2.1-or-later", "LGPL-3.0-or-later"}
keys = ["background", "foreground", "cursor-color", "cursor-text", "selection-background", "selection-foreground"]
rows, credits = [], []
folder = os.path.join(repo, "ghostty")
for theme in sorted(os.listdir(folder), key=str.lower):
    licence = schemes.get(theme, {}).get("license", "unknown")
    if licence not in passed_on:
        continue
    values, palette = {}, [""] * 16
    for line in open(os.path.join(folder, theme), encoding="utf-8"):
        key, _, value = line.partition("=")
        key, value = key.strip(), value.strip()
        if key == "palette":
            n, _, color = value.partition("=")
            if n.strip().isdigit() and int(n) < 16:
                palette[int(n)] = color.strip().lower()
        elif key in keys:
            values[key] = value.lower()
    if not values.get("background") or not values.get("foreground") or not all(palette):
        continue
    rows.append("\t".join([theme] + [values.get(k, "") for k in keys] + [",".join(palette)]))
    source = schemes[theme].get("source", "")
    credits.append(f"| {theme} | {licence} | {source} |")

with open("assets/terminal/themes.tsv", "w", encoding="utf-8") as f:
    f.write("# name\tbackground\tforeground\tcursor\tcursor text\tselection\tselected text\tpalette 0-15\n")
    f.write("\n".join(rows) + "\n")
with open("assets/terminal/THEMES.md", "w", encoding="utf-8") as f:
    f.write("# The notch's terminal's themes\n\n")
    f.write(f"From [iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes) at {commit[:7]}, Mark Badolato's collection, ")
    f.write("under the MIT License ([LICENSE-iTerm2-Color-Schemes.txt](LICENSE-iTerm2-Color-Schemes.txt)). Only the themes its licences.yml ")
    f.write("says may be passed on are here, each under its own licence, as below; `scripts/terminal-themes.sh` makes this again.\n\n")
    f.write("| Theme | Licence | Source |\n|---|---|---|\n")
    f.write("\n".join(credits) + "\n")
print(len(rows), "themes")
EOF
ls -l assets/terminal/

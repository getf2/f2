"""Every dependency's license, as one text: F2's Acknowledgements.

Run from the repo (python3 -I scripts/licenses.py [extra license files…]):
cargo metadata says what's built in; each crate's own LICENSE, COPYING
and NOTICE files are copied out (else its license's name is given).
Files given after are added at the end (Sparkle's).
"""
import json
import os
import subprocess
import sys

# (What's built for this Mac only: not Windows' and Linux's crates.)
host = next(l.split(": ", 1)[1] for l in subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout.splitlines() if l.startswith("host: "))
meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", host], capture_output=True, text=True, check=True).stdout)
root = meta["resolve"]["root"]
used = {node["id"] for node in meta["resolve"]["nodes"]} - {root}
packages = sorted((p for p in meta["packages"] if p["id"] in used), key=lambda p: (p["name"].lower(), p["version"]))

RAYCAST = """MIT License

Copyright (c) 2021 Raycast

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

HEX = """MIT License

Copyright (c) 2026 Christopher Langton

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

out = [
    "F2 is built on open-source software. Thank you to everyone who made it.",
    "",
    "Unicode's emoji data (emoji-test.txt) and CLDR's annotations: © Unicode, Inc.,",
    "under the Unicode License v3 (https://www.unicode.org/license.txt).",
    "",
    "Cities and their time zones: GeoNames (https://www.geonames.org), under the",
    "Creative Commons Attribution 4.0 License (https://creativecommons.org/licenses/by/4.0/).",
    "",
    "Public holidays from Nager.Date (https://date.nager.at).",
    "",
    "Crypto prices: data provided by CoinGecko (https://www.coingecko.com), else",
    "Coinbase (https://www.coinbase.com). Stock prices from Yahoo Finance",
    "(https://finance.yahoo.com); they may be delayed.",
    "",
    "Air quality: Open-Meteo.com, from the Copernicus Atmosphere Monitoring",
    "Service's information (CAMS, https://atmosphere.copernicus.eu).",
    "",
    "Weather data by Open-Meteo.com (https://open-meteo.com), under the Creative",
    "Commons Attribution 4.0 License (https://creativecommons.org/licenses/by/4.0/).",
    "",
    "Geist (https://github.com/vercel/geist-font), the welcome's typeface, under the",
    "SIL Open Font License 1.1:",
    "",
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "fonts", "OFL.txt"), encoding="utf-8").read().strip(),
    "",
    "The Do Not Disturb shortcut is after Raycast's do-not-disturb extension's",
    "(https://github.com/raycast/extensions), under its MIT License:",
    "",
    RAYCAST,
    "",
    "Dictation is after Hex's (https://github.com/anomalyco/hex), by Kit Langton;",
    "its speech recognition part (src/macos/speech.swift) is adapted from Hex's",
    "src/apple_speech.swift, under its MIT License:",
    "",
    HEX,
    "",
    "Typing Icelandic and Korean on ABC is after Vikey (https://vikey.app), Tony Dinh's",
    "Vietnamese keyboard, built on Tuyen Mai's OpenKey.",
    "",
    "Its Icelandic words are BÍN's, under the Creative Commons Attribution-ShareAlike",
    "4.0 License (https://creativecommons.org/licenses/by-sa/4.0/):",
    "Beygingarlýsing íslensks nútímamáls. Stofnun Árna Magnússonar í íslenskum fræðum.",
    "Höfundur og ritstjóri Kristín Bjarnadóttir.",
    "",
    "Its Korean words are from Hermit Dave's FrequencyWords",
    "(https://github.com/hermitdave/FrequencyWords), made from OpenSubtitles",
    "(https://www.opensubtitles.org), under the Creative Commons",
    "Attribution-ShareAlike 4.0 License (https://creativecommons.org/licenses/by-sa/4.0/).",
    "",
    "Its English words are SCOWL's (http://wordlist.aspell.net):",
    "",
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "typing", "SCOWL-Copyright.txt"), encoding="utf-8").read().strip(),
    "",
    "The notch's terminal's themes are from iTerm2-Color-Schemes",
    "(https://github.com/mbadolato/iTerm2-Color-Schemes), Mark Badolato's collection,",
    "under its MIT License, each theme under its own licence (as below):",
    "",
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "terminal", "LICENSE-iTerm2-Color-Schemes.txt"), encoding="utf-8").read().strip(),
    "",
    # (THEMES.md's table: each theme's licence and source.)
    "| Theme |" + open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "terminal", "THEMES.md"), encoding="utf-8").read().split("| Theme |", 1)[1].rstrip(),
    "",
]
out.append("%d crates:" % len(packages))
out.extend("  %s %s (%s)" % (p["name"], p["version"], p.get("license") or "see its license") for p in packages)
out.append("")
# Each license text once, with the crates under it (Apache's is the same
# for hundreds of them; MIT's differ by their copyright lines).
texts = {}
none = []
for p in packages:
    folder = os.path.dirname(p["manifest_path"])
    files = sorted(f for f in os.listdir(folder) if f.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE")) and os.path.isfile(os.path.join(folder, f)))
    if not files:
        none.append(p)
    for f in files:
        with open(os.path.join(folder, f), encoding="utf-8", errors="replace") as text:
            texts.setdefault(text.read().strip(), []).append("%s %s" % (p["name"], p["version"]))
for text, crates in sorted(texts.items(), key=lambda t: t[1][0].lower()):
    out.append("=" * 72)
    out.append(", ".join(crates))
    out.append("")
    out.append(text)
    out.append("")
if none:
    out.append("=" * 72)
    out.append("Under the license named above, without its text in the crate:")
    out.extend("  %s %s (%s)" % (p["name"], p["version"], p.get("license") or "see its source") for p in none)
    out.append("")
for extra in sys.argv[1:]:
    out.append("=" * 72)
    out.append("Sparkle (https://sparkle-project.org)")
    out.append("")
    with open(extra, encoding="utf-8", errors="replace") as text:
        out.append(text.read().strip())
print("\n".join(out))

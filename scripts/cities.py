"""The cities Quick Open knows the time in: assets/cities.tsv.

Run from the repo (python3 -I scripts/cities.py): GeoNames' cities of
15,000 people and more (https://www.geonames.org, CC BY 4.0) kept to those
of 100,000 and more, every country's capital and every state's or
province's, with their countries' names and their states', where they
are (to a kilometer or so: the weather's), and what
they're called in Japanese, Chinese and Korean (their names in those
scripts: "東京", "도쿄").
"""
import io
import os
import urllib.request
import zipfile

DUMP = "https://download.geonames.org/export/dump/"
PEOPLE = 100_000


def cjk(name):
    """Whether `name` is written in Han, kana or Hangul."""
    return any(0x3040 <= ord(c) <= 0x30FF or 0x3400 <= ord(c) <= 0x9FFF or 0xAC00 <= ord(c) <= 0xD7AF for c in name)


def get(name):
    with urllib.request.urlopen(DUMP + name) as r:
        return r.read()


cities = zipfile.ZipFile(io.BytesIO(get("cities15000.zip"))).read("cities15000.txt").decode()
countries = {}
for line in get("countryInfo.txt").decode().splitlines():
    if line and not line.startswith("#"):
        f = line.split("\t")
        countries[f[0]] = f[4]
states = {}
for line in get("admin1CodesASCII.txt").decode().splitlines():
    f = line.split("\t")
    states[f[0]] = f[1]

rows = []
for line in cities.splitlines():
    f = line.split("\t")
    name, ascii_name, code, country, state, people, zone = f[1], f[2], f[7], f[8], f[10], int(f[14]), f[17]
    others = []
    for other in f[3].split(","):
        if cjk(other) and other not in others and len(others) < 10:
            others.append(other)
    if not zone or country not in countries:
        continue
    if people < PEOPLE and code not in ("PPLC", "PPLA"):
        continue
    lat, lon = round(float(f[4]), 2), round(float(f[5]), 2)
    rows.append((name, "" if ascii_name == name else ascii_name, country, states.get(f"{country}.{state}", ""), people, zone, lat, lon, ",".join(others)))
rows.sort(key=lambda r: -r[4])

out = [
    "# Cities: GeoNames' (https://www.geonames.org, CC BY 4.0), made by scripts/cities.py.",
    "# Countries (@code, name), then cities, the biggest first: name, its ASCII spelling (if it differs), country, state or province, people, time zone, latitude, longitude,",
    "# and its names in Japanese, Chinese and Korean scripts.",
]
used = sorted({r[2] for r in rows})
out += [f"@{c}\t{countries[c]}" for c in used]
out += ["\t".join(map(str, r)) for r in rows]
path = os.path.join(os.path.dirname(__file__), "..", "assets", "cities.tsv")
with open(path, "w", encoding="utf-8") as f:
    f.write("\n".join(out) + "\n")
print(f"{len(rows)} cities in {len(used)} countries")

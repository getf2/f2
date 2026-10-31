#!/bin/bash
# The word lists Typing (src/typing.rs) looks words up in, made again:
# assets/typing/icelandic.fst, every word form in BÍN (Beygingarlýsing
# íslensks nútímamáls, Stofnun Árna Magnússonar í íslenskum fræðum, under
# CC BY-SA 4.0), and assets/typing/english.fst, SCOWL's words to size 60
# (American, British and Canadian spellings and contractions, not its
# names: "Etta" would make "/etta" English; Kevin Atkinson's), lowercase,
# one each; and assets/typing/korean.fst, the 200,000 words said most in
# OpenSubtitles' Korean (Hermit Dave's FrequencyWords, under CC BY-SA 4.0),
# whole Hangul only, each with how often. See assets/typing/README.md.
#
# ./scripts/typing-words.sh (downloads about 46 MB, then builds them)
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/bin.zip" "https://bin.arnastofnun.is/django/api/nidurhal/?file=SHsnid.csv.zip"
mkdir "$work/bin"
unzip -q "$work/bin.zip" -d "$work/bin"
curl -fsSL -o "$work/scowl.tar.gz" "https://downloads.sourceforge.net/wordlist/scowl-2020.12.07.tar.gz"
mkdir "$work/scowl"
tar xzf "$work/scowl.tar.gz" -C "$work/scowl"
curl -fsSL -o "$work/ko_full.txt" "https://raw.githubusercontent.com/hermitdave/FrequencyWords/525f9b560de45753a5ea01069454e72e9aa541c6/content/2018/ko/ko_full.txt"

python3 -I - "$work" <<'EOF'
import os, sys
work = sys.argv[1]

# BÍN, as Sigrúnarsnið: lemma;id;class;domain;form;tag.
icelandic = set()
with open(os.path.join(work, "bin", "SHsnid.csv"), encoding="utf-8") as f:
    for line in f:
        parts = line.rstrip("\n").split(";")
        if len(parts) >= 6 and parts[4]:
            icelandic.add(parts[4].lower())

# SCOWL's final lists, to size 60 (as spell checkers have it).
english = set()
final = next(os.path.join(root, "final") for root, dirs, _ in os.walk(os.path.join(work, "scowl")) if "final" in dirs)
for name in os.listdir(final):
    kind, _, size = name.rpartition(".")
    region, _, part = kind.partition("-")
    if size.isdigit() and int(size) <= 60 and region in ("english", "american", "british", "canadian") and part in ("words", "upper", "contractions"):
        with open(os.path.join(final, name), encoding="latin-1") as f:
            english.update(w.strip().lower() for w in f if w.strip())

for name, words in (("icelandic", icelandic), ("english", english)):
    with open(os.path.join(work, name + ".txt"), "w", encoding="utf-8") as f:
        f.writelines(w + "\n" for w in sorted(words))
    print(name, len(words))

# Korean: "word count" a line, most said first; whole syllables only (not
# ㅋㅋ, not Latin), the 200,000 said most.
korean = []
with open(os.path.join(work, "ko_full.txt"), encoding="utf-8") as f:
    for line in f:
        parts = line.split()
        if len(parts) == 2 and parts[1].isdigit() and all("\uac00" <= c <= "\ud7a3" for c in parts[0]):
            korean.append((parts[0], int(parts[1])))
korean.sort(key=lambda wc: (-wc[1], wc[0]))
with open(os.path.join(work, "korean.txt"), "w", encoding="utf-8") as f:
    f.writelines(f"{w}\t{c}\n" for w, c in sorted(korean[:200000]))
print("korean", min(len(korean), 200000))
EOF

TYPING_WORDS="$work" cargo test --quiet typing::words::tests::build -- --ignored --exact
ls -l assets/typing/*.fst

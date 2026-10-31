# Typing's word lists

Typing (src/typing.rs) looks words up in these to tell which language a word is in: English, Icelandic or Korean. Each file is a set of lowercase words, one per entry, stored as an FST (the `fst` crate's format). F2 builds them in and looks words up where they sit, without reading either file whole. `scripts/typing-words.sh` downloads the sources and makes both files again, byte for byte.

## icelandic.fst

Every word form in BÍN, the Database of Icelandic Morphology: 3.7 million forms, made from the basic format (Sigrúnarsnið, SHsnid.csv). This file is under the [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode) license, as BÍN is. See the conditions [in English](https://bin.arnastofnun.is/DMII/LTdata/conditions/) or [in Icelandic](https://bin.arnastofnun.is/gogn/mimisbrunnur/).

As BÍN's terms ask, credit is given as follows:

*Beygingarlýsing íslensks nútímamáls. Stofnun Árna Magnússonar í íslenskum fræðum. Höfundur og ritstjóri Kristín Bjarnadóttir.*

## english.fst

SCOWL's word lists (Spell Checker Oriented Word Lists), version 2020.12.07, up to size 60: 115,000 words. They include American, British and Canadian spellings, contractions and possessives. SCOWL's lists of names are left out: a name like "Etta" would make `/etta` (þetta) read as English. SCOWL is copyright 2000–2018 Kevin Atkinson and others; its notice is in [SCOWL-Copyright.txt](SCOWL-Copyright.txt).

## korean.fst

The 200,000 Korean words said most in films' and shows' subtitles, each with how often it's said (an FST map). Typing uses the counts so that a rarely said Korean word can't beat a common English one, as 랙 for "for". Only whole Hangul syllables are kept. The words come from Hermit Dave's [FrequencyWords](https://github.com/hermitdave/FrequencyWords), made from [OpenSubtitles](https://www.opensubtitles.org) through [OPUS](https://opus.nlpl.eu/OpenSubtitles.php). Its lists are under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode), and so is this file.

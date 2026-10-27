# Credits

Everything in the video that F2 didn't make itself. All the sounds are CC0
(public domain): no attribution needed, listed here anyway.

## Sounds (public/sfx)

| File | From | Author | License |
| --- | --- | --- | --- |
| `key-1.wav` … `key-5.wav`, `key-space.wav` | "Keyboard single key presses.wav", <https://freesound.org/people/5ro4/sounds/609534/> | 5ro4 | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `whoosh.wav` | "digital_whoosh_soft.wav", <https://freesound.org/people/ludvique/sounds/71852/> | ludvique | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `click.wav`, `press.wav`, `release.wav` | `mouseclick1.ogg` and `mouserelease1.ogg` from UI Audio, <https://kenney.nl/assets/ui-audio> | Kenney | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| `pop.wav` | `drop_003.ogg` from Interface Sounds, <https://kenney.nl/assets/interface-sounds> | Kenney | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |

How they were cut (from freesound's high-quality previews, with ffmpeg):

- `key-1` to `key-5`: the first 120 ms of five presses (at 1.06, 5.04, 8.14,
  25.37 and 26.21 s), faded out from 50 ms and leveled to one loudness.
  `key-space`: a lower press, at 7.09 s, for Space, Return and Delete.
- `click`: the press, and the release 70 ms later. `press` and `release`:
  each on its own, for a drag's start and its drop.
- `whoosh`: 0.1–0.9 s, faded in and out.
- `pop`: as it is.

## Ambience (public/ambience)

Under each painting, quietly, and a call as birds fly past. All from
freesound's high-quality previews, high-passed at 70 Hz and leveled (the
beds to −23 LUFS, the calls to −20); the quietest also lightly de-hissed,
and the forest, mostly hush between calls, compressed
so its calls sit with the rest.

| File | From | Author | License | Part used |
| --- | --- | --- | --- | --- |
| `valley-birds.mp3` | "Birds Outdoors", <https://freesound.org/people/BurghRecords/sounds/456122/> | BurghRecords | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) | 0–21 s |
| `wind-grass.mp3` | "Wind is blowing in the grass of a patagonian plain…", <https://freesound.org/people/felix.blume/sounds/139337/> | felix.blume | CC0 1.0 | 37.5–59.5 s |
| `meadow.mp3` | "summer meadow with wind", <https://freesound.org/people/Garuda1982/sounds/639459/> | Garuda1982 | CC0 1.0 | 31–53 s |
| `forest.mp3` | "Quiet Spring Woodland Ambience.wav", <https://freesound.org/people/ecfike/sounds/160893/> | ecfike | CC0 1.0 | 21–43 s |
| `lake.mp3` | "Gentle waves on a lake", <https://freesound.org/people/TheFlyFishingFilmmaker/sounds/614299/> | TheFlyFishingFilmmaker | CC0 1.0 | 108–130 s |
| `sea.mp3` | "Calm ocean waves", <https://freesound.org/people/SamsterBirdies/sounds/578524/> | SamsterBirdies | CC0 1.0 | 42–64 s |
| `evening.mp3` | "Southern Summer Evening Ambience with Crickets 5", <https://freesound.org/people/RyanKingArt/sounds/746366/> | RyanKingArt | CC0 1.0 | 0.5–22.5 s |
| `call-1.mp3` | "Birds Outdoors" (as above) | BurghRecords | CC0 1.0 | a call at 18.05 s |
| `call-2.mp3`, `call-3.mp3` | "Bird song early morning long.wav", <https://freesound.org/people/Rimmer/sounds/614924/> | Rimmer | CC0 1.0 | calls at 18.45 and 20.55 s |
| `gull-1.mp3` | "Seagulls distant.wav", <https://freesound.org/people/Ambientsoundapp/sounds/537854/> | Ambientsoundapp | CC0 1.0 | 1.05–2.95 s |

## Pictures

- The paintings (`public/paint`) and the app icon (`public/icon.png`) are
  F2's own, from `site/public/img/paint` and `assets/icon.png`. Their
  motion (drifting clouds, wind, water, light, birds, motes, stars) is
  drawn over them in `src/living/`, nothing added to the files.
- `public/ref` holds the site's screenshots, only to check the rebuilt
  screens against (the Check compositions); they aren't in the video.

## Fonts

Fraunces, Source Sans 3 and Geist Mono, from `@fontsource-variable`, under
the SIL Open Font License 1.1. F2's own windows are drawn in the Mac's
system font, as the app draws them.

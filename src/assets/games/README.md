# Game icons

The side menu shows each game's own logo here, falling back to a Material glyph
when the file is absent — which is the state of a fresh clone, because these are
the games' artwork rather than KovOBS's and aren't redistributed with the source.

To use them, drop the two files in beside this README:

| File | Section |
| --- | --- |
| `kovaaks.png` | KovaaK's |
| `aimbeast.png` | Aimbeast |

Square, transparent, and 64×64 or larger — they are drawn at 19×19 with
`object-fit: contain`, so anything square looks right and anything wider gets
letterboxed rather than squashed. The Steam library icon for each game is the
obvious source; on Windows the client keeps them under
`Steam\appcache\librarycache`.

Nothing else needs changing: `SECTIONS` in `home.component.ts` already points at
these paths, and the fallback disappears as soon as the files load.

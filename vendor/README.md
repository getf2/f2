# Vendored crates

## fsearch

[fsearch](https://github.com/noahdunnagan/fsearch) at rev
`88e63fdf850cf7749deae8198b187accd21f8ba5` (MIT, see `fsearch/LICENSE`),
used in place of the git dependency through `[patch]` in `Cargo.toml`.

Changes from upstream:

- `Engine::with_live` (`src/engine.rs`): read access to the live name
  index, which Finder2 adds folder sizes up from (`src/search.rs`,
  `Search::folder_sizes`).
- `frecency` and `git` (`src/frecency.rs`, `src/git.rs`) are modules of the
  library (`src/lib.rs`), not only of the `fsearch` binary (`src/main.rs`
  imports them from it; their `fsearch::query` paths are `crate::query`),
  for Finder2's git badges and Quick Open.
- `Cargo.toml`: its `[profile.*]` sections dropped (Cargo ignores them in a
  dependency, with a warning).
- `Part::of` (`src/query.rs`): a pattern with `?` or `[` and a character
  wider than a byte ("blár?", "파래요?") parses, where it panicked (it
  sliced one byte past every character, looking for a class's `]`).

- `apply_loop` (`src/engine.rs`): a replay that took over 2 s
  (`LONG_REPLAY`) is saved as soon as the live index has applied all of
  it. Upstream saves only after 50,000 changes or 12 hours in one run, so
  an app quit and started often (Finder2's dev builds) replayed the same
  growing history at every start (10 s by evening).
- `Engine::rebuild` and `Status::rebuilding` (`src/engine.rs`,
  `rebuild_all`): the owner crawls the whole disk afresh, as on the first
  run, and swaps that in, searches answering from the old index meanwhile
  (the apply loop waits, so what changes during the crawl is applied
  after); then the content index syncs all of home. For Finder2's
  Settings > Search, "Rebuild Index".
- `Listed` and `Engine::search_listed` (`src/engine.rs`), with
  `walk::scan_open` and `walk::list_open` (`src/walk.rs`): a name search
  in a folder the index doesn't hold (on another disk, or one skipped
  without Full Disk Access) lists that folder for it, as a first run's
  early index does, but into other volumes and skipped folders too, and
  without recording refusals in `DENIED`. Finder2 keeps a listing while
  someone types (`src/search.rs`, `Search::listed`).

- `apply_loop` (`src/engine.rs`): it folds the overlay into the base
  (a whole new index.bin) after 50,000 changes only if 20 minutes have
  passed since the last save (`COMPACT_GAP`), or at once past 400,000
  (`COMPACT_PENDING_MAX`): on a developer's Mac 50,000 came every 2 to 4
  minutes, each fold 2 s of CPU and ~280 MB written. And its thread runs at
  utility QoS (`utility_qos`).

- `Git::changed` (`src/git.rs`): a change in a repo gets its status read
  only if it was asked for (`Git::load`, recorded in `Repo::asked`), not
  when it was only looked up (`Git::repo`). Upstream read every repo ever
  looked up at its next change: Finder2's badges look up each repo shown,
  so a folder of 60 repos had all 60 read (git status and log each, 32 s
  in all) at the start's replay of what changed since the index's save.

- `parse_entry` (`src/walk.rs`): a file with more than one name (hard
  links: Rust's builds and pnpm's packages are full of them) has its size
  shared among them (`ATTR_FILE_LINKCOUNT`), so a folder holding them all
  adds up to the file once. Upstream gave each name the whole size: a
  Rust project's target folder of 8 GB added up to 35. An index saved
  before this (no `shares` file beside it, `SHARES` in `src/engine.rs`) is
  crawled again once, a minute after starting, in the background.

To move to a newer fsearch: copy its `src/`, `Cargo.toml`, `LICENSE` and
`README.md` here, apply the changes above again, and update the rev in
`Cargo.toml` and this file.

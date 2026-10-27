//! Whole-disk file search for macOS: a fuzzy name index over every entry on
//! disk, kept live from FSEvents, plus a trigram content index for text
//! files. `Engine` runs it all in-process; the `fsearch` binary wraps one in
//! a daemon with a JSON-lines socket.

pub mod alloc;
pub mod content;
mod engine;
// (Finder2: in the library, from the binary, for its git badges and
// Quick Open.)
pub mod frecency;
mod fsevents;
pub mod git;
pub mod index;
pub mod live;
pub mod query;
pub mod walk;

pub use content::{Case, FileMatches, Grep, GrepResult, Hit};
pub use engine::{Engine, Found, Listed, Options, Status, default_dir, gated, has_full_disk_access, no_materialize};
pub use query::{GrepMode, Query};

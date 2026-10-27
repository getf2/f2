#!/bin/zsh
# F2's side of the extension store's repo (github.com/getf2/extensions)
# copied into a clone of it: F2's API's types and the manifest's schema,
# the docs, the JavaScript host's API (for its tools/run.js) and run.js.
# Run when any of them changes, then commit there.
#
#   ./scripts/sync-store.sh ../f2-extensions
set -e
store=${1:?which clone of the store repo}
here=${0:A:h:h}
[ -d "$store/extensions" ] && [ -d "$store/tools" ] || { echo "$store isn't the store's repo" >&2; exit 1; }
cp "$here/extensions/js/f2.d.ts" "$here/extensions/js/f2.schema.json" "$store/types/"
cp "$here/EXTENSIONS.md" "$store/docs/EXTENSIONS.md"
cp "$here/src/js_host.js" "$here/extensions/run.js" "$store/tools/"
git -C "$store" status --short

#!/bin/sh
# F2 for other Macs: built and signed with your Developer ID, notarized by
# Apple and stapled, in a DMG; with Sparkle's appcast.xml for updates.
# Into target/release/dist. With --publish, uploaded to the f2-updates R2
# bucket, which https://download.getf2.com serves: the DMG as F2-<version>.dmg
# and as F2.dmg (the latest, to link to), and appcast.xml, which F2 checks
# once a day.
#
# Once, before the first release:
#   1. Notary credentials, in your keychain (with an app-specific password
#      from account.apple.com > Sign-In and Security):
#        xcrun notarytool store-credentials F2APP --apple-id <your Apple ID> --team-id FTASJZ6Z7N
#   2. Sparkle's signing key, into your keychain (it prints the public
#      half, which goes in the app; keep the keychain item: updates are
#      signed with it, and without it F2 can't be updated):
#        target/sparkle-2.10.0/bin/generate_keys
#   3. For --publish, Cloudflare: npx wrangler login
#
# Each release: bump the version in Cargo.toml, commit, then
#   ./scripts/release-macos.sh --publish
set -eu
cd "$(dirname "$0")/.."

NOTARY_PROFILE=${NOTARY_PROFILE:-F2APP}
BASE_URL=${BASE_URL:-https://download.getf2.com}
R2_BUCKET=${R2_BUCKET:-f2-updates}
# (The account the bucket is in: wrangler sees more than one.)
export CLOUDFLARE_ACCOUNT_ID=${CLOUDFLARE_ACCOUNT_ID:-913ea66e4789612e4d55d396e3ffed24}
FEED_URL=$BASE_URL/appcast.xml
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
PUBLISH=""
[ "${1:-}" = "--publish" ] && PUBLISH=1
WRANGLER="npx --yes wrangler@4"

security find-identity -v -p codesigning | grep -q "Developer ID Application" || { echo "no Developer ID Application certificate in the keychain"; exit 1; }
xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1 || { echo "no notary credentials named $NOTARY_PROFILE: see step 1 above"; exit 1; }
if [ -n "$PUBLISH" ]; then
    # (The build number is the commit count: it should be this commit's.)
    [ -z "$(git status --porcelain)" ] || { echo "commit first: a release is built from a commit"; exit 1; }
    # (By the appcast, which isn't cached: asking for a DMG that isn't
    # there yet would leave Cloudflare holding its 404 for hours.)
    if curl -fsS "$FEED_URL" 2>/dev/null | grep -q "F2-$VERSION.dmg"; then
        echo "F2 $VERSION is already out: bump the version in Cargo.toml"
        exit 1
    fi
    $WRANGLER whoami >/dev/null 2>&1 || { echo "wrangler isn't logged in to Cloudflare: npx wrangler login"; exit 1; }
fi

RELEASE=1 FEED_URL="$FEED_URL" ./scripts/bundle-macos.sh
APP=target/release/bundle/F2.app
/usr/libexec/PlistBuddy -c "Print :SUPublicEDKey" "$APP/Contents/Info.plist" >/dev/null 2>&1 || { echo "no Sparkle key in the keychain: see step 2 above"; exit 1; }
# (By its hash, as bundle-macos.sh picks it: two certificates can have
# the same name.)
IDENTITY=${SIGN_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*) \([0-9A-F]\{40\}\) "Developer ID Application: .*/\1/p' | head -1)}
codesign -dvv "$APP" 2>&1 | grep -q "^Authority=Developer ID Application" || { echo "F2.app isn't signed with a Developer ID"; exit 1; }
BUILD=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$APP/Contents/Info.plist")

DIST=target/release/dist
rm -rf "$DIST"
mkdir -p "$DIST"

# The app, notarized and stapled (so it opens offline too, as Sparkle
# installs it).
echo "notarizing the app…"
ditto -c -k --keepParent "$APP" "$DIST/F2.zip"
xcrun notarytool submit "$DIST/F2.zip" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$APP"
rm "$DIST/F2.zip"

# The DMG: the app, and Applications to drag it to, over an arrow (laid
# out by dmgbuild, which writes Finder's .DS_Store itself: see
# scripts/dmg-settings.py); signed, notarized, stapled.
DMGBUILD=target/dmgbuild/bin/dmgbuild
if [ ! -x "$DMGBUILD" ]; then
    python3 -m venv target/dmgbuild
    target/dmgbuild/bin/pip install -q dmgbuild==1.6.7 ds_store==1.3.3 mac_alias==2.2.3
fi
DMG="$DIST/F2-$VERSION.dmg"
STAGE=$(mktemp -d)
swift scripts/render-dmg-background.swift "$STAGE/background"
"$DMGBUILD" -s scripts/dmg-settings.py -D app="$APP" -D background="$STAGE/background.png" F2 "$DMG" >/dev/null
rm -rf "$STAGE"
# (F2 as it'll be installed, from the DMG, checked strictly.)
hdiutil attach -readonly -nobrowse -mountpoint "$DIST/mnt" "$DMG" >/dev/null
STRICT=$(codesign --verify --deep --strict "$DIST/mnt/F2.app" 2>&1 && echo ok || true)
hdiutil detach "$DIST/mnt" >/dev/null
[ "$STRICT" = ok ] || { echo "F2.app in the DMG: $STRICT"; exit 1; }
codesign --force --timestamp --sign "$IDENTITY" "$DMG"
echo "notarizing the DMG…"
xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$DMG"

# As Gatekeeper sees them, on a Mac that's never seen F2.
spctl --assess --type open --context context:primary-signature --verbose "$DMG"
spctl --assess --type execute --verbose "$APP"

# Sparkle's appcast: the DMG, signed with the key in your keychain.
target/sparkle-2.10.0/bin/generate_appcast --download-url-prefix "$BASE_URL/" -o "$DIST/appcast.xml" "$DIST"
grep -q "sparkle:edSignature" "$DIST/appcast.xml" || { echo "the appcast isn't signed: see step 2 above"; exit 1; }
echo "built $DMG and $DIST/appcast.xml: F2 $VERSION ($BUILD)"

if [ -n "$PUBLISH" ]; then
    put() {
        $WRANGLER r2 object put "$R2_BUCKET/$1" --file "$2" --content-type "$3" --cache-control "$4" --remote >/dev/null
    }
    # (The DMG before the appcast that points to it.)
    echo "uploading to ${BASE_URL}…"
    put "F2-$VERSION.dmg" "$DMG" application/x-apple-diskimage "public, max-age=31536000, immutable"
    put "F2.dmg" "$DMG" application/x-apple-diskimage "public, max-age=300"
    put appcast.xml "$DIST/appcast.xml" application/xml no-cache
    curl -fsS "$FEED_URL" | grep -q "F2-$VERSION.dmg" || { echo "$FEED_URL doesn't list F2 $VERSION yet"; exit 1; }
    git tag "v$VERSION" 2>/dev/null || true
    echo "released F2 $VERSION: $BASE_URL/F2.dmg (updates from $FEED_URL)"
fi

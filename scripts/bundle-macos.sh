#!/bin/sh
# Build "F2.app" into target/release/bundle; with --install, also
# copy it to /Applications (replacing an earlier install).
#
# A release (RELEASE set, as release-macos.sh does) runs on Apple silicon
# and Intel alike: built for both, the two put in one app with lipo (each
# Mac runs its own). Other builds are for this Mac's only, as quick to
# make as before; UNIVERSAL=1 makes one for both too. (Needs Rust's target
# for the other: rustup target add x86_64-apple-darwin, or aarch64-.)
#
# Signed with your Developer ID (with the hardened runtime, as
# notarization wants), else your Apple Development certificate, else ad
# hoc. Sparkle (updates) goes in Contents/Frameworks; the open-source
# licenses in Contents/Resources/Acknowledgements.txt. Updates (from the
# releases' appcast, or FEED_URL's) are on when Sparkle's key is in your
# keychain (see scripts/release-macos.sh): a build of yours is offered the
# first release made after it (build numbers are commit counts).
set -eu
cd "$(dirname "$0")/.."

NAME="F2"
# (Kept from when it was "New Finder": macOS keys privacy grants and
# "Show in Finder" to it.)
BUNDLE_ID="dev.newfinder.NewFinder"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# (What it's built for: see .cargo/config.toml.)
MIN_MACOS=$(sed -n 's/^MACOSX_DEPLOYMENT_TARGET = "\(.*\)"/\1/p' .cargo/config.toml)
# (Builds before 0.1.7 check https://f2.dagurleo.com/appcast.xml: the bucket
# keeps that domain too, for them.)
FEED_URL=${FEED_URL:-https://download.getf2.com/appcast.xml}
# Sparkle compares this: it only goes up (a commit each). (The history
# began again when F2 went open source, after 0.2.0's build 189: counted
# on from there.)
BUILD=$((189 + $(git rev-list --count HEAD 2>/dev/null || echo 1)))
OUT=target/release/bundle
APP="$OUT/$NAME.app"
SPARKLE_VERSION=2.10.0
SPARKLE_SHA256=c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c
SPARKLE="target/sparkle-$SPARKLE_VERSION"

cargo build --release
BINARY=target/release/new-finder
if [ -n "${RELEASE:-}" ] || [ -n "${UNIVERSAL:-}" ]; then
    case "$(uname -m)" in
        arm64) OTHER=x86_64-apple-darwin ;;
        *) OTHER=aarch64-apple-darwin ;;
    esac
    if ! rustup target list --installed 2>/dev/null | grep -qx "$OTHER"; then
        echo "a build for Apple silicon and Intel needs Rust's $OTHER target: rustup target add $OTHER" >&2
        exit 1
    fi
    cargo build --release --target "$OTHER"
    mkdir -p target/universal
    lipo -create target/release/new-finder "target/$OTHER/release/new-finder" -output target/universal/new-finder
    BINARY=target/universal/new-finder
fi

# Sparkle, once: its release, checked against the hash it was vetted with.
if [ ! -d "$SPARKLE/Sparkle.framework" ]; then
    mkdir -p target/sparkle-dl "$SPARKLE"
    curl -sSL -o "target/sparkle-dl/Sparkle-$SPARKLE_VERSION.tar.xz" \
        "https://github.com/sparkle-project/Sparkle/releases/download/$SPARKLE_VERSION/Sparkle-$SPARKLE_VERSION.tar.xz"
    echo "$SPARKLE_SHA256  target/sparkle-dl/Sparkle-$SPARKLE_VERSION.tar.xz" | shasum -a 256 -c - >/dev/null
    tar -xf "target/sparkle-dl/Sparkle-$SPARKLE_VERSION.tar.xz" -C "$SPARKLE"
fi

rm -rf "$APP" "$OUT/AppIcon.iconset"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks" "$OUT/AppIcon.iconset"
cp "$BINARY" "$APP/Contents/MacOS/$NAME"

swift scripts/render-icon.swift "$OUT/AppIcon.iconset"
iconutil -c icns "$OUT/AppIcon.iconset" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$OUT/AppIcon.iconset"

# Sparkle, less its XPC services (only sandboxed apps need them).
ditto "$SPARKLE/Sparkle.framework" "$APP/Contents/Frameworks/Sparkle.framework"
rm -rf "$APP/Contents/Frameworks/Sparkle.framework/Versions/B/XPCServices" "$APP/Contents/Frameworks/Sparkle.framework/XPCServices"

# Do Not Disturb's shortcut, for Shortcuts to add (see src/toggles.rs).
cp "assets/F2 Do Not Disturb.shortcut" "$APP/Contents/Resources/"

# Every dependency's license, as the licenses ask.
python3 -I scripts/licenses.py "$SPARKLE/LICENSE" > "$APP/Contents/Resources/Acknowledgements.txt"

# Updates: where to look, and the key their archives are signed with (in
# your keychain, made once with Sparkle's generate_keys).
UPDATES=""
# (Without one, generate_keys prints its error where the key would be:
# only a key, 32 bytes in base64.)
PUBLIC_KEY=$("$SPARKLE/bin/generate_keys" -p 2>/dev/null | grep -E '^[A-Za-z0-9+/]{43}=$' || true)
if [ -n "${FEED_URL:-}" ] && [ -n "$PUBLIC_KEY" ]; then
    UPDATES="<key>SUFeedURL</key><string>$FEED_URL</string>
    <key>SUPublicEDKey</key><string>$PUBLIC_KEY</string>
    <key>SUEnableAutomaticChecks</key><true/>"
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$NAME</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$BUILD</string>
    <key>NSHumanReadableCopyright</key><string>© $(date +%Y) Dagur Leó Bergsson</string>
    <key>LSMinimumSystemVersion</key><string>$MIN_MACOS</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <!-- Folders: dropped on the Dock icon, or Open With. -->
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>Folder</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.folder</string></array>
        </dict>
    </array>
    <!-- malloc's space-efficient mode: see space_efficient in src/main.rs. -->
    <key>LSEnvironment</key><dict><key>MallocSpaceEfficient</key><string>1</string></dict>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
    <key>NSDesktopFolderUsageDescription</key><string>$NAME lists and searches the files in your Desktop folder.</string>
    <key>NSDocumentsFolderUsageDescription</key><string>$NAME lists and searches the files in your Documents folder.</string>
    <key>NSDownloadsFolderUsageDescription</key><string>$NAME lists and searches the files in your Downloads folder.</string>
    <key>NSAppleEventsUsageDescription</key><string>$NAME asks Finder what's in the Trash, and to empty it, when it may not look itself; and Music or Spotify what's playing, to play, skip and love songs when you ask.</string>
    <key>NSRemovableVolumesUsageDescription</key><string>$NAME lists the files on removable drives.</string>
    <key>NSNetworkVolumesUsageDescription</key><string>$NAME lists the files on network drives.</string>
    <key>NSBluetoothAlwaysUsageDescription</key><string>$NAME turns Bluetooth on and off when you ask it to, from Quick Open.</string>
    <key>NSRemindersUsageDescription</key><string>$NAME adds the reminders you make in Quick Open to Reminders.</string>
    <key>NSRemindersFullAccessUsageDescription</key><string>$NAME adds the reminders you make in Quick Open to Reminders.</string>
    <key>NSCalendarsUsageDescription</key><string>$NAME shows what's on your calendar, and adds the events you make, in Quick Open.</string>
    <key>NSCalendarsFullAccessUsageDescription</key><string>$NAME shows what's on your calendar, and adds the events you make, in Quick Open.</string>
    <key>NSLocationUsageDescription</key><string>$NAME only needs this to tell which Wi-Fi network you're on, to show its password. It doesn't keep or send where you are.</string>
    <key>NSLocationWhenInUseUsageDescription</key><string>$NAME only needs this to tell which Wi-Fi network you're on, to show its password. It doesn't keep or send where you are.</string>
    <key>NSMicrophoneUsageDescription</key><string>$NAME hears you when you dictate, and records your voice in a screen recording when Settings say to.</string>
    <key>NSContactsUsageDescription</key><string>$NAME finds the people you call, message and email from Quick Open.</string>
    <key>NSCalendarsWriteOnlyAccessUsageDescription</key><string>$NAME adds the events you make in Quick Open to your calendar.</string>
    $UPDATES
</dict>
</plist>
PLIST

# macOS ties privacy grants (Full Disk Access, folders) to the signature:
# an ad-hoc one changes with every build, so they'd be asked for again
# each time. Your Developer ID (or SIGN_IDENTITY) stays the same; an Apple
# Development certificate does too, but only runs on your own Macs.
IDENTITY=${SIGN_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*) \([0-9A-F]\{40\}\) "Developer ID Application: .*/\1/p' | head -1)}
IDENTITY=${IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*) \([0-9A-F]\{40\}\) "Apple Development: .*/\1/p' | head -1)}
if [ -n "$IDENTITY" ]; then
    # (Inside out: Sparkle's helpers, Sparkle, then the app; the hardened
    # runtime, and for a release a secure timestamp (from Apple's server),
    # as notarization wants.)
    TIMESTAMP=$([ -n "${RELEASE:-}" ] && echo --timestamp || echo --timestamp=none)
    SIGN="codesign --force $TIMESTAMP --options runtime --sign $IDENTITY"
    FRAMEWORK="$APP/Contents/Frameworks/Sparkle.framework"
    $SIGN "$FRAMEWORK/Versions/B/Autoupdate"
    $SIGN "$FRAMEWORK/Versions/B/Updater.app"
    $SIGN "$FRAMEWORK"
    $SIGN --entitlements scripts/F2.entitlements "$APP"
else
    codesign --force --deep --sign - "$APP"
fi
codesign --verify --deep --strict "$APP"
SIGNER=$(codesign -dvv "$APP" 2>&1 | sed -n 's/^Authority=//p' | head -1)
echo "built $APP $VERSION ($BUILD) for $(lipo -archs "$APP/Contents/MacOS/$NAME" | sed 's/ / and /'), signed ${SIGNER:-ad hoc}$([ -n "$UPDATES" ] && echo ", updates from $FEED_URL")"

if [ "${1:-}" = "--install" ]; then
    # It was "New Finder", then "Finder2", before: not two of it (only
    # if it's this app: its bundle identifier).
    for OLD in "/Applications/New Finder.app" "/Applications/Finder2.app"; do
        if [ -d "$OLD" ] && [ "$(defaults read "$OLD/Contents/Info" CFBundleIdentifier 2>/dev/null)" = "$BUNDLE_ID" ]; then
            rm -rf "$OLD"
            echo "removed $OLD"
        fi
    done
    rm -rf "/Applications/$NAME.app"
    ditto "$APP" "/Applications/$NAME.app"
    # (So LaunchServices reads Info.plist anew: LSEnvironment, say.)
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "/Applications/$NAME.app"
    echo "installed /Applications/$NAME.app"
fi

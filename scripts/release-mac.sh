#!/usr/bin/env bash
# Build, sign, notarize and publish the macOS app from this computer.
#
# Faster than the GitHub build. Produces Held-In-Common-macOS-AppleSilicon.dmg
# (signed with your Developer ID, notarized by Apple, ticket stapled) and
# publishes it to the release for this version (v<version>, marked Latest).
#
# Needs, once per computer:
#   - your "Developer ID Application" certificate in the login keychain
#     (security find-identity -v -p codesigning)
#   - a notarytool keychain profile:  xcrun notarytool store-credentials hic-notary
#   - Rust, the Tauri CLI (cargo install tauri-cli --version "^2" --locked), gh
#
# Usage:  ./scripts/release-mac.sh            build, sign, notarize, upload
#         ./scripts/release-mac.sh --no-upload  everything except the upload
#
# Settings (environment variables, all optional):
#   HIC_NOTARY_PROFILE   notarytool profile name        (default: hic-notary)
#   HIC_SIGNING_IDENTITY signing identity               (default: the first Developer ID in the keychain)
#   HIC_SHARE_DEFAULT    commons pre-filled in the app  (default: the repo's Actions variable)
#
# Publishes to the numbered release v<version> (from src-tauri/Cargo.toml),
# creating it and marking it Latest if needed. Edit its notes on GitHub after.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

UPLOAD=1; [ "${1:-}" = "--no-upload" ] && UPLOAD=0
REPO=HeldInCommon/FederatedBios
PROFILE="${HIC_NOTARY_PROFILE:-hic-notary}"
ID="${HIC_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' | head -1)}"
[ -n "$ID" ] || { echo "No Developer ID Application certificate found in the keychain." >&2; exit 1; }
SHARE="${HIC_SHARE_DEFAULT:-$(gh variable get HIC_SHARE_DEFAULT --repo "$REPO" 2>/dev/null || true)}"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
DMG="$OUT/Held-In-Common-macOS-AppleSilicon.dmg"
step(){ printf '\n== %s\n' "$1"; }

step "Version $VERSION, signing as: $ID"
[ -z "$(git status --porcelain -- commons src-tauri)" ] || echo "warning: uncommitted changes in commons/ or src-tauri/ will be included"

step "Building the server"
cargo build --release --locked --manifest-path commons/Cargo.toml
TRIPLE=$(rustc -vV | sed -n 's/host: //p')
mkdir -p src-tauri/binaries
cp commons/target/release/hic-commons "src-tauri/binaries/hic-commons-$TRIPLE"

step "Building and signing the app"
rm -rf src-tauri/target/release/bundle/macos
(cd src-tauri && APPLE_SIGNING_IDENTITY="$ID" HIC_SHARE_DEFAULT="$SHARE" cargo tauri build --bundles app)
APP="src-tauri/target/release/bundle/macos/Held In Common.app"
codesign --verify --deep --strict "$APP"

step "Notarizing the app (Apple scans it; a few minutes)"
ditto -c -k --keepParent "$APP" "$OUT/app.zip"
xcrun notarytool submit "$OUT/app.zip" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"

step "Making the .dmg (drag to Applications), signing and notarizing it"
mkdir "$OUT/stage"
ditto "$APP" "$OUT/stage/Held In Common.app"
ln -s /Applications "$OUT/stage/Applications"
hdiutil create -volname "Held In Common" -srcfolder "$OUT/stage" -ov -format UDZO "$DMG" >/dev/null
codesign --sign "$ID" --timestamp "$DMG"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"

step "Checking"
xcrun stapler validate "$DMG"
spctl -a -vv -t open --context context:primary-signature "$DMG" 2>&1 | grep -E "origin|accepted|rejected|override" || true

if [ "$UPLOAD" = 1 ]; then
  # Each version has one release, v$VERSION, marked Latest; created on the
  # first upload for that version. Only the most recent release is kept, so
  # delete the previous one on GitHub once this is out.
  REL="v$VERSION"
  step "Publishing to the $REL release"
  if gh release view "$REL" --repo "$REPO" >/dev/null 2>&1; then
    gh release upload "$REL" "$DMG" --repo "$REPO" --clobber
  else
    gh release create "$REL" "$DMG" --repo "$REPO" --target main --latest \
      --title "Held In Common $VERSION" \
      --notes "Held in Common $VERSION. macOS: signed and notarized; open the .dmg and drag Held In Common into Applications. Install guide: https://github.com/$REPO/blob/main/docs/INSTALL.md"
  fi
  echo "Published: https://github.com/$REPO/releases/tag/$REL"
else
  cp "$DMG" ./Held-In-Common-macOS-AppleSilicon.dmg
  echo "Not uploaded. The .dmg is at ./Held-In-Common-macOS-AppleSilicon.dmg"
fi

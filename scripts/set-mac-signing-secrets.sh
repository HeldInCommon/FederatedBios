#!/usr/bin/env bash
# Check a Developer ID .p12 and store the macOS signing secrets GitHub needs.
#
# GitHub's macOS build signs the app with the six secrets in the protected
# `macos-signing` environment (docs/MAINTAINER.md §3, ADR 0015). This script
# sets them from your own Terminal. Nothing secret is printed, and nothing
# goes through chat or into a file other than the .p12 you exported.
#
# Before running it:
#   1. Keychain Access → login → My Certificates → right-click
#      "Developer ID Application: … (TEAMID)" → Export… → save as .p12 with a
#      new password. (Export just that one certificate.)
#   2. Have an app-specific password ready if you want to replace APPLE_PASSWORD
#      (account.apple.com → Sign-In and Security → App-Specific Passwords).
#   3. Sign in to gh as an admin of HeldInCommon.
#
# Usage:  ./scripts/set-mac-signing-secrets.sh ~/Desktop/hic-developer-id.p12
#
# What it does, in order:
#   - asks for the .p12's password (hidden) and imports it into a throwaway
#     keychain, the same way the GitHub runner does, so a wrong password or a
#     .p12 without its private key fails here, before anything is stored;
#   - reads the signing identity and Team ID from the certificate;
#   - sets APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY
#     and APPLE_TEAM_ID;
#   - asks (hidden) for the Apple ID and app-specific password; press Return to
#     keep the ones already stored;
#   - deletes the throwaway keychain and offers to delete the .p12.
set -euo pipefail

REPO=HeldInCommon/FederatedBios
ENV=macos-signing
P12="${1:-}"
[ -f "$P12" ] || { echo "Usage: $0 path/to/developer-id.p12" >&2; exit 1; }
command -v gh >/dev/null || { echo "Needs the GitHub CLI (gh)." >&2; exit 1; }

read -r -s -p "Password you chose when exporting the .p12: " P12_PASSWORD; echo

# Import into a throwaway keychain, as the runner does.
KC="$(mktemp -d)/check.keychain-db"
KC_PASSWORD="$(openssl rand -hex 16)"
cleanup() { security delete-keychain "$KC" 2>/dev/null || true; }
trap cleanup EXIT
security create-keychain -p "$KC_PASSWORD" "$KC"
if ! security import "$P12" -k "$KC" -f pkcs12 -P "$P12_PASSWORD" -T /usr/bin/codesign >/dev/null 2>&1; then
  echo "That .p12 didn't import: the password is wrong, or it isn't a .p12 with its private key." >&2
  echo "Nothing was changed on GitHub." >&2
  exit 1
fi
IDENTITY="$(security find-identity -v -p codesigning "$KC" | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' | head -1)"
[ -n "$IDENTITY" ] || { echo "The .p12 has no Developer ID Application certificate with its key. Nothing was changed." >&2; exit 1; }
TEAM_ID="$(printf '%s' "$IDENTITY" | sed -n 's/.*(\([A-Z0-9]\{10\}\))$/\1/p')"
echo "Checked: $IDENTITY"

set_secret() { gh secret set "$1" --repo "$REPO" --env "$ENV" --body "$2" >/dev/null && echo "  set $1"; }
echo "Storing in $REPO, environment $ENV:"
set_secret APPLE_CERTIFICATE "$(base64 -i "$P12" | tr -d '\n')"
set_secret APPLE_CERTIFICATE_PASSWORD "$P12_PASSWORD"
set_secret APPLE_SIGNING_IDENTITY "$IDENTITY"
set_secret APPLE_TEAM_ID "$TEAM_ID"

read -r -s -p "Apple ID email (Return to keep the stored one): " APPLE_ID; echo
[ -z "$APPLE_ID" ] || set_secret APPLE_ID "$APPLE_ID"
read -r -s -p "App-specific password (Return to keep the stored one): " APPLE_PASSWORD; echo
[ -z "$APPLE_PASSWORD" ] || set_secret APPLE_PASSWORD "$APPLE_PASSWORD"

read -r -p "Delete $P12 now? [Y/n] " yn
case "$yn" in [Nn]*) echo "Kept it. Delete it yourself once you're done." ;; *) rm -P "$P12" && echo "Deleted $P12." ;; esac
echo "Done. Run Actions → desktop-app → macos to check the build signs."

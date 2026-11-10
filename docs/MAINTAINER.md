# Maintainer guide — releasing, signing and hosting

For maintainers of **HeldInCommon/FederatedBios**. All work happens in this
repository (not personal forks): branch from `main`, open a pull request, merge
when the checks pass. App identity: `org.heldincommon.node`, product name
**Held In Common**.

---

## 1. Checks on every pull request

`.github/workflows/pr-checks.yml` runs:

- formatting (`cargo fmt --check`) and lints (`cargo clippy -D warnings`);
- the server's unit, HTTP and format-vector tests (`cargo test`);
- the independent Python check of the format vectors;
- the end-to-end test (`scripts/test-federation.sh`): real nodes and commons,
  sharing, tampering, impostors, vouching and steward approval, withdrawal, key
  replacement and recovery;
- a macOS build of the desktop app (unsigned; attached to the run as an
  artifact so reviewers can try it), and its tests.

Security-sensitive code needs a second reviewer: `federation.rs`, `identity.rs`,
`keys.rs`, `passphrase.rs`, `sessions.rs`, `store.rs`, `vouching.rs`,
`outbound.rs`, `web/steward.rs`.

---

## 2. Releasing the desktop app

Each version gets one release, **vX.Y.Z**, marked **Latest**; the docs and the
app link to `releases/latest`. **Only the most recent release is kept:** once a
new one is out, delete the previous one (`gh release delete vA.B.C --cleanup-tag`).

1. **Bump the version** in `commons/Cargo.toml`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`, through a pull request.
2. **Release the Mac app from a Mac** (fast; about 5–10 minutes, mostly
   Apple's notarization):
   ```
   ./scripts/release-mac.sh
   ```
   It builds, signs with your Developer ID, notarizes the app and the `.dmg`,
   checks them, and publishes `Held-In-Common-macOS-AppleSilicon.dmg` to the
   `vX.Y.Z` release (creating it, marked Latest). Then edit that release's
   notes on GitHub to say what's new. `--no-upload` stops before uploading. It needs the certificate in
   your keychain and a `notarytool` profile (see the script's header).
3. **Release Linux on GitHub:** Actions → **desktop-app** → Run workflow on
   `main` (it builds Linux by default; choose `macos` or `all` to have GitHub
   build the Mac app too, which is slower). It uploads to the `vX.Y.Z`
   release. A pushed `node-app-vX.Y.Z` tag builds both.
4. **Check** the release page, and download the `.dmg` on another Mac to
   confirm it opens without warnings.

### What the script does

For reference, or to do it step by step:

```
cargo build --release --manifest-path commons/Cargo.toml
cp commons/target/release/hic-commons src-tauri/binaries/hic-commons-$(rustc -vV | sed -n 's/host: //p')
cd src-tauri && APPLE_SIGNING_IDENTITY="Developer ID Application: NAME (TEAMID)" \
  HIC_SHARE_DEFAULT="<default commons>" cargo tauri build --bundles app
# notarize and staple the app, wrap it in a .dmg (with an Applications link),
# sign, notarize and staple the .dmg:
xcrun notarytool submit app.zip --keychain-profile <profile> --wait && xcrun stapler staple "Held In Common.app"
xcrun notarytool submit Held-In-Common-macOS-AppleSilicon.dmg --keychain-profile <profile> --wait
xcrun stapler staple Held-In-Common-macOS-AppleSilicon.dmg
gh release upload vX.Y.Z Held-In-Common-macOS-AppleSilicon.dmg --clobber
```

Store the notarization profile once with
`xcrun notarytool store-credentials <profile>` (it asks for an app-specific
password; never paste it anywhere else).

---

## 3. macOS signing secrets

GitHub signs with a **Developer ID Application** certificate. The six secrets
live in the GitHub environment **`macos-signing`** (Settings → Environments),
which only `main` and `node-app-v*` tags can use, so a workflow on another
branch can't read them ([decision record 0015](adr/0015-macos-signing.md)). If
any is missing, the macOS job stops instead of publishing an unsigned build.

Set them from your own Terminal, with `gh` signed in as an admin. Run each
command **on its own**, and paste a value only at the "Paste your secret"
prompt:

1. **Export the certificate:** Keychain Access → *login* → *My Certificates* →
   right-click **Developer ID Application: … (TEAMID)** → **Export…** → `.p12`
   with a strong password.
2. **Make an app-specific password** at account.apple.com → Sign-In and
   Security → App-Specific Passwords.
3. **Add the secrets** (`R=HeldInCommon/FederatedBios`):
   ```
   base64 -i ~/Desktop/hic-developer-id.p12 | gh secret set APPLE_CERTIFICATE --repo $R --env macos-signing
   gh secret set APPLE_CERTIFICATE_PASSWORD --repo $R --env macos-signing
   gh secret set APPLE_PASSWORD --repo $R --env macos-signing
   gh secret set APPLE_ID --repo $R --env macos-signing
   gh secret set APPLE_TEAM_ID --repo $R --env macos-signing --body TEAMID
   gh secret set APPLE_SIGNING_IDENTITY --repo $R --env macos-signing --body "Developer ID Application: NAME (TEAMID)"
   ```
4. **Delete the `.p12`.**

The workflow tidies stray spaces and line breaks in pasted secrets. When the
certificate expires, repeat steps 1, 3 and 4.

---

## 4. Docker image

`.github/workflows/node-image.yml` publishes a multi-arch image
`ghcr.io/heldincommon/hic-node:latest` on every push to `main` that touches
`commons/`. Make the package public once: **org → Packages → `hic-node` →
Package settings → Change visibility → Public**. How to run a node or a commons
from it: [commons/README.md](../commons/README.md).

---

## 5. Architecture slides

`docs/architecture.html` is a self-contained reveal.js deck, currently served
from the original fork at
https://phoenixperry.github.io/FederatedBios/architecture.html. To serve it from
the organisation instead: Settings → **Pages** → deploy from `main`, folder
`/docs`; it will be at `https://heldincommon.github.io/FederatedBios/architecture.html`.

---

## 6. Not yet

- **Automatic updates** (issue #17): needs an updater signing key.
- **Windows** builds (and a Windows code-signing certificate) and **Intel Mac**
  builds.

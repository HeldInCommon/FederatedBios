# Maintainer guide — reviewing & shipping from the org

Instructions for a **HeldInCommon/FederatedBios** maintainer to review PR #2 and
publish Held in Common from the org. The work currently lives on the fork
`phoenixperry/FederatedBios` (branch `feat/federation`); contributors there have
read-only access to the org, so the final hosting steps need a maintainer.

> **Signing status: unsigned for now.** Desktop builds are intentionally unsigned
> this release. Users open them with the one-time "right-click → Open" (macOS) or
> "More info → Run anyway" (Windows) flow — documented in `docs/INSTALL.md`.
> Code signing (Apple notarization, Windows cert) is a follow-up, not a blocker.

---

## 1. Review PR #2

https://github.com/HeldInCommon/FederatedBios/pull/2 — adds the commons/node
server, node↔commons federation, encrypted members-only, CC governance, the
desktop app, the Docker image, the install guide, the architecture slides, and
`docs/ARCHITECTURE.md`.

**Verify locally:**
```
git fetch origin pull/2/head:pr-2 && git checkout pr-2
cargo build --manifest-path commons/Cargo.toml     # server builds
./scripts/test-federation.sh                        # 15/15: publish→sign→federate, tamper→400, members guard
```

**Give these a close read** (security-sensitive):
- `commons/main.rs` → `inbox` / `verify_inbound` (cid recompute + Ed25519 verify + TOFU key pinning).
- `commons/main.rs` → `encrypt_to_members` / `decrypt_for` (members-only, Option B: ChaChaBox/X25519).
- Auth: passphrase (salted hash, no recovery) + `Secure` session cookie + per-session CSRF on `publish`/`share`.

---

## 2. Merge

Merge `feat/federation` → `main`. App identity is `org.heldincommon.node`,
product name **"Held In Common"**.

---

## 3. Publish installers from the org

CI (`.github/workflows/desktop-app.yml`) builds **macOS + Linux** via
`tauri-action`. To cut a release on the org:

1. Settings → Actions → General → **Workflow permissions = Read and write**
   (lets `tauri-action` create the release).
2. Push a tag: `git tag node-app-v0.1.0 && git push origin node-app-v0.1.0`.
   CI builds and publishes to a `node-app-preview` release on the **org**.
3. (Optional polish) Rename the uploaded assets to version-less names
   (`Held-In-Common-macOS-AppleSilicon.dmg`, `Held-In-Common-Linux.{deb,AppImage,rpm}`)
   and retitle the release "Held In Common — app". Then update the download links
   in `docs/INSTALL.md` to the org release.

A working reference release (built from this branch) is on the fork:
https://github.com/HeldInCommon/FederatedBios/releases/tag/node-app-preview

> **Not built yet:** Windows and Intel-mac. Add `windows-latest` / an
> `x86_64-apple-darwin` target to the matrix when wanted (Windows needs a sign
> step for a clean SmartScreen experience).

---

## 4. Docker image

`.github/workflows/node-image.yml` publishes a multi-arch image on push to
`main`/`feat/federation`. The fork's image is already **public** and anonymously
pullable:
```
docker pull ghcr.io/phoenixperry/hic-node:latest
```
To host it under the org, let CI publish `ghcr.io/heldincommon/hic-node`, then set
that package public once: **org → Packages → `hic-node` → Package settings →
Danger Zone → Change visibility → Public**. (Package visibility is a UI action;
there is no REST endpoint for it.)

---

## 5. Architecture slides (GitHub Pages)

`docs/architecture.html` is a self-contained reveal.js deck. To serve it from the
org: Settings → **Pages** → Deploy from branch `main`, folder `/docs`. It will be
at `https://heldincommon.github.io/FederatedBios/architecture.html`.
(Currently served from the fork.)

---

## 6. macOS signing

macOS builds from `desktop-app.yml` are signed with a **Developer ID
Application** certificate and notarized by Apple. The six secrets live in the
GitHub environment **`macos-signing`** (Settings → Environments), which only
`main` and `node-app-v*` tags can use, so a workflow pushed to another branch
cannot read them. If any secret is missing, the macOS job fails instead of
publishing an unsigned build.

Set them from **your own Terminal** (not a shared screen or chat), with the
`gh` CLI signed in as a repo admin:

1. **Export the certificate.** Keychain Access → *login* → *My Certificates* →
   expand **Developer ID Application: … (TEAMID)** to check a private key sits
   under it → right-click the certificate → **Export…** → save as `.p12` with a
   strong password.
2. **Make an app-specific password** at account.apple.com → Sign-In and
   Security → App-Specific Passwords (label it e.g. `hic-github`).
3. **Add the secrets** (each `gh secret set` without `--body` asks for the
   value with hidden input):
   ```bash
   R=HeldInCommon/FederatedBios
   base64 -i ~/Desktop/hic-developer-id.p12 | gh secret set APPLE_CERTIFICATE --repo $R --env macos-signing
   gh secret set APPLE_CERTIFICATE_PASSWORD --repo $R --env macos-signing
   gh secret set APPLE_PASSWORD             --repo $R --env macos-signing
   gh secret set APPLE_ID                   --repo $R --env macos-signing
   gh secret set APPLE_TEAM_ID              --repo $R --env macos-signing
   gh secret set APPLE_SIGNING_IDENTITY     --repo $R --env macos-signing
   ```
   `APPLE_SIGNING_IDENTITY` is the full name shown by
   `security find-identity -v -p codesigning` (e.g. `Developer ID Application: NAME (TEAMID)`).
4. **Delete the `.p12`** from your disk once the secrets are set.
5. **Run a build:** Actions → desktop-app → Run workflow (on `main`), or push a
   `node-app-v*` tag. The last macOS step checks the signature and notarization
   and replaces `Held-In-Common-macOS-AppleSilicon.dmg` on the release.

When the certificate expires or is revoked, repeat steps 1, 3 and 4.

**Windows:** not signed yet; needs a code-signing certificate and a sign step.

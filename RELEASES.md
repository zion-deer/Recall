# Releases

## Versioning

Semantic versioning: `MAJOR.MINOR.PATCH`. Pre-1.0, minor versions may change behavior; the storage format is still protected by migrations at every version.

The version lives in three places and must match:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Every release gets a [CHANGELOG.md](CHANGELOG.md) entry with **Features**, **Fixes**, **Security**, and **Breaking changes**.

## Pipeline

`.github/workflows/ci.yml` runs on every push / PR: typecheck, lint, frontend tests, `cargo fmt --check`, `clippy`, `cargo test` on Windows, macOS and Linux, plus a debug build of the desktop app on Windows and macOS.

`.github/workflows/release.yml` runs when a `v*` tag is pushed and builds platform installers with `tauri-action`:

| Platform | Artifacts |
| --- | --- |
| Windows (x64) | `Recall_<version>_x64-setup.exe` (NSIS), `Recall_<version>_x64_en-US.msi` |
| macOS (Apple Silicon + Intel) | `Recall_<version>_universal.dmg` |

Artifacts are attached to a draft release for review before publishing.

### Cutting a release

1. Update the version in the three files above and add a CHANGELOG entry.
2. Merge to `main`.
3. `git tag v0.1.0 && git push origin v0.1.0`
4. Review the draft release and artifacts, then publish.

Builds are reproducible from the tag: dependencies are pinned by `package-lock.json` and `Cargo.lock`, and CI uses `npm ci`.

## Code signing (required before public distribution)

Configure these as encrypted CI secrets; the release workflow already reads them:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | Developer ID Application certificate (base64 .p12) |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Recall (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Notarization |
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Authenticode certificate (base64 .pfx) |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater signing key (see below) |

Without these, CI still produces unsigned builds that are useful for internal testing.

## Auto-update plan (Phase 8 — required for v0.1 public release)

Not yet implemented. Design:

1. Generate an updater key pair: `npx tauri signer generate -w ~/.tauri/recall.key`. The **public** key goes into `tauri.conf.json` (`plugins.updater.pubkey`); the private key only into CI secrets.
2. Add `tauri-plugin-updater`. Release builds then emit `.sig` files and a `latest.json` manifest.
3. Host `latest.json` and artifacts over HTTPS at a fixed endpoint we control.
4. The app checks the endpoint (version + platform only), downloads the matching package, **verifies the Ed25519 signature** against the embedded public key, asks the user, installs, and restarts.
5. Updates from any other source, or with an invalid signature, are rejected. Rollback: the previous installer stays available, and database backups are made before any schema migration.

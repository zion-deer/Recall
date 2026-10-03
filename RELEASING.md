# Releasing Recall

The authoritative version is `package.json` → `version`. These commands copy it to `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`, then commit and tag it:

```bash
npm run release:patch
npm run release:minor
npm run release:major
```

Publish the tag:

```bash
git push --follow-tags
```

`.github/workflows/release.yml` runs on every `v*` tag. It builds the Windows and macOS installers with `tauri-action`, creates updater artifacts, and publishes a GitHub Release. Users do not clone the repository or build Recall.

## Updater

On startup, and from Settings → Updates, Recall asks the endpoint in `src-tauri/tauri.conf.json`:

```text
https://github.com/zion-deer/Recall/releases/latest/download/latest.json
```

Change that URL if the GitHub repository path is different. The app downloads the platform artifact described there and verifies its minisign signature against `plugins.updater.pubkey`. If the signature, download, or install fails, the current installation stays in place. The user must choose **Update now**; updates are not forced.

## Signing secrets

Generate a keypair on a trusted machine. Do not commit the private key.

```bash
npx tauri signer generate -w ~/.tauri/recall.key
```

Replace `plugins.updater.pubkey` with the contents of the `.pub` file, then store these GitHub Actions secrets:

| Secret | Required for |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Every update. The private minisign key. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Only if the key has a password. |
| `APPLE_CERTIFICATE` | Optional macOS Developer ID `.p12`, base64. |
| `APPLE_CERTIFICATE_PASSWORD` | Optional macOS certificate password. |
| `APPLE_SIGNING_IDENTITY` | Optional, for example `Developer ID Application: Name (TEAMID)`. |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Optional notarization. |
| `WINDOWS_CERTIFICATE` | Optional Authenticode `.pfx`, base64. |
| `WINDOWS_CERTIFICATE_PASSWORD` | Optional Windows certificate password. |

The public key currently in the repository does not have its private key stored anywhere. Generate your own pair and replace the public key before the first real release; until then Recall correctly rejects update packages.

## First release checklist

1. Create the GitHub repository that matches the updater endpoint.
2. Generate the updater keypair and replace the public key.
3. Add the secrets above.
4. Add Apple and Windows signing secrets when those certificates exist. Unsigned builds still run for testing, but signed updates are required for the updater.
5. Run `npm run release:patch` (or minor/major) and `git push --follow-tags`.
6. Confirm the GitHub Release contains the Windows installer, macOS disk image, signatures, and `latest.json`.
7. Install the previous build and confirm it offers the new version, then install it.

Normal releases after that are the two commands above. The release script does not push, so a tag is not published accidentally.

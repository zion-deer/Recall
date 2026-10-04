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

`.github/workflows/release.yml` runs on every `v*` tag. It builds a Windows x64 installer and an Apple Silicon macOS installer with `tauri-action`, creates updater artifacts, and publishes a GitHub Release. Intel Macs are not included in this build. Users do not clone the repository or build Recall.

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

Replace `plugins.updater.pubkey` with the contents of **`recall.key.pub`** only:

```bash
cat ~/.tauri/recall.key.pub
```

That line decodes to `minisign public key: …`. Do **not** paste the **“Public signature:”** line printed by `tauri signer sign` (that decodes to `signature from tauri secret key` and will break updates).

Then store these **repository** secrets (Settings → Secrets and variables → Actions → Repository secrets):

| Secret | Required for |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Every release. The **private** key file (`recall.key`), not the `.pub` file. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Only if you set a password when generating the key. If the key has **no** password, **do not** create this secret (an empty or guessed value causes “Wrong password for that key”). |

### Pasting the private key correctly

The private key is a small text file (often ~348 bytes). Paste **everything** into `TAURI_SIGNING_PRIVATE_KEY`:

```text
untrusted comment: minisign encrypted secret key
<base64 lines…>
```

Include both comment lines and the base64 block. Do not add quotes, `%`, or **an extra blank line after the last line** (GitHub will show `Invalid symbol 10, offset 348` if there is a trailing newline too many). On macOS you can copy the file exactly with:

```bash
pbcopy < ~/.tauri/recall.key
```

The password secret must match **exactly** what you typed at `tauri signer generate` (case-sensitive). If you are not sure of the password, regenerate:

```bash
npx tauri signer generate -w ~/.tauri/recall.key -f --ci --password ""
```

Then update `plugins.updater.pubkey` from `recall.key.pub`, set the new private key in GitHub, delete `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if you used an empty password, and cut a new release tag.

Release workflow runs `scripts/verify-tauri-signing.mjs` **before** the long compile so a bad password fails in under a minute instead of after bundling.

Updater bundles (`latest.json`, signatures) are only built on **Release** (tag push), via `src-tauri/tauri.release.conf.json`. Regular CI does not need signing secrets.
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

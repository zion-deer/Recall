# Reset updater signing (fresh start)

Use this when GitHub Release keeps failing on signing. You can ship **installers without auto-update** first; turn signed updates on after this checklist.

## 1. Generate a new key (no password — simplest)

On your Mac:

```bash
cd ~/Recall
npx tauri signer generate -w ~/.tauri/recall.key -f --ci --password ""
```

## 2. Put the public key in the app

```bash
cat ~/.tauri/recall.key.pub
```

Copy the **one line** into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.

Commit and push:

```bash
git add src-tauri/tauri.conf.json
git commit -m "Rotate updater public key"
git pull origin main --rebase
git push origin main
git push github main
```

## 3. GitHub secrets (repository secrets only)

**Settings → Secrets and variables → Actions → Repository secrets**

1. **Delete** `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (important for empty-password keys).
2. **Update** `TAURI_SIGNING_PRIVATE_KEY`:
   ```bash
   pbcopy < ~/.tauri/recall.key
   ```
   Paste once — no extra blank line at the bottom.

## 4. Prove it locally

```bash
printf 'x' > /tmp/probe.bin
npx tauri signer sign -f ~/.tauri/recall.key -p "" /tmp/probe.bin
ls /tmp/probe.bin.sig
```

If `.sig` exists, secrets are ready for CI.

## 5. Ship installers (no signed updater yet)

Leave **Settings → Secrets and variables → Actions → Variables** empty (do **not** set `RECALL_SIGNED_UPDATES` yet).

Cut a release:

```bash
cd ~/Recall
git pull origin main --rebase
git push github main
npm run release:patch
git push github main --follow-tags
git push origin main --follow-tags
```

Wait for **Actions → Release** → green → **Releases** → download `.dmg` / `.exe`.

## 6. Turn on in-app auto-update (later)

When downloads work and local probe signing works:

1. **Settings → Secrets and variables → Actions → Variables → New repository variable**
2. Name: `RECALL_SIGNED_UPDATES` Value: `true`
3. Cut another release (`npm run release:patch` + push tags).

Release will then build `latest.json` and signed updater bundles.

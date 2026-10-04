#!/usr/bin/env bash
# Write updater key file + export signing env (safe for special characters in password).
set -euo pipefail
KEY_FILE="${RUNNER_TEMP:?}/recall-updater.key"
install -m 600 /dev/null "$KEY_FILE"
node scripts/write-updater-signing-key.mjs "$KEY_FILE"
{
  echo "TAURI_SIGNING_PRIVATE_KEY_PATH<<EOF"
  echo "$KEY_FILE"
  echo "EOF"
  echo "TAURI_SIGNING_PRIVATE_KEY_PASSWORD<<EOF"
  echo -n "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
  echo
  echo "EOF"
} >> "$GITHUB_ENV"

#!/usr/bin/env node
/**
 * Write TAURI_SIGNING_PRIVATE_KEY to a temp file without corrupting minisign format.
 * GitHub secret paste often adds an extra trailing newline (breaks base64 decode at EOF).
 */
import { writeFileSync } from "node:fs";

const dest = process.argv[2];
const raw = process.env.TAURI_SIGNING_PRIVATE_KEY;
if (!dest || !raw?.trim()) {
  console.error("Usage: TAURI_SIGNING_PRIVATE_KEY=... node write-updater-signing-key.mjs <path>");
  process.exit(1);
}

let key = raw.replace(/\r\n/g, "\n");
// Remove accidental blank lines / extra newlines at the end only.
while (key.endsWith("\n\n")) {
  key = key.slice(0, -1);
}
key = key.replace(/\n+$/, "\n");

writeFileSync(dest, key, { mode: 0o600 });

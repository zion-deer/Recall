#!/usr/bin/env node
/** Write the updater password exactly as provided, with no added newline. */
import { writeFileSync } from "node:fs";

const dest = process.argv[2];
if (!dest) {
  console.error("Usage: node write-updater-password.mjs <path>");
  process.exit(1);
}
let password = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "";
password = password.replace(/\r/g, "").replace(/\n+$/, "");
writeFileSync(dest, password, { mode: 0o600 });

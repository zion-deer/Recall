#!/usr/bin/env node
/**
 * Invoked as `npm run tauri -- build ...` from GitHub Actions.
 * Loads the updater password from a file so `!` and extra newlines cannot change it.
 */
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const require = createRequire(import.meta.url);
const tauriJs = require.resolve("@tauri-apps/cli/tauri.js");

const passFile = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD_FILE;
const env = { ...process.env };
delete env.TAURI_SIGNING_PRIVATE_KEY;

if (passFile) {
  let password = readFileSync(passFile);
  while (
    password.length > 0 &&
    (password[password.length - 1] === 10 || password[password.length - 1] === 13)
  ) {
    password = password.subarray(0, -1);
  }
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = password.toString("utf8");
}

const args = process.argv.slice(2);
const result = spawnSync(process.execPath, [tauriJs, ...args], {
  stdio: "inherit",
  env,
});

if (result.error) {
  console.error(result.error);
  process.exit(1);
}
process.exit(result.status ?? 1);

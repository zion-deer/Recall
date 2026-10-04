#!/usr/bin/env node
/**
 * Invoked as `npm run tauri -- build ...` from GitHub Actions.
 * Loads the updater password from a file written with exact bytes so a `!`
 * or a trailing newline in GITHUB_ENV cannot change it.
 */
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

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
const result = spawnSync("npx", ["tauri", ...args], {
  stdio: "inherit",
  env,
});
process.exit(result.status ?? 1);

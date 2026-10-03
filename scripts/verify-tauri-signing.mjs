#!/usr/bin/env node
/**
 * Fail fast in CI when updater signing secrets are missing or the password is wrong.
 * Matches what `tauri build` does after bundling when createUpdaterArtifacts is true.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const rawKey = process.env.TAURI_SIGNING_PRIVATE_KEY?.trim();
const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH?.trim();
const password = (process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "").replace(
  /\r?\n$/,
  "",
);

if (!rawKey && !keyPath) {
  console.error(
    "Missing TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH.",
  );
  process.exit(1);
}

const dir = mkdtempSync(join(tmpdir(), "recall-sign-"));
const probe = join(dir, "probe.bin");
writeFileSync(probe, "recall-updater-signing-probe");

const args = ["tauri", "signer", "sign", "-p", password, probe];
if (keyPath) {
  args.splice(3, 0, "-f", keyPath);
} else {
  args.splice(3, 0, "-k", rawKey);
}

const result = spawnSync("npx", args, {
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
  env: {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: undefined,
    TAURI_SIGNING_PRIVATE_KEY_PATH: undefined,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: undefined,
  },
});

try {
  unlinkSync(`${probe}.sig`);
} catch {
  // ignore
}

if (result.status !== 0) {
  const detail = (result.stderr || result.stdout || "").trim();
  if (detail) {
    console.error(detail);
  }
  console.error(
    "\nUpdater signing check failed. In GitHub → Settings → Secrets → Actions, set TAURI_SIGNING_PRIVATE_KEY to the full contents of ~/.tauri/recall.key (both comment lines and the base64 block). Set TAURI_SIGNING_PRIVATE_KEY_PASSWORD to the exact password from `tauri signer generate`, or delete that secret if the key has no password. If the password is lost, regenerate the keypair and update the public key in tauri.conf.json.",
  );
  process.exit(1);
}

console.log("Updater signing credentials verified.");

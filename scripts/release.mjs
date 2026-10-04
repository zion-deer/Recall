import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const kind = process.argv[2];
if (!["patch", "minor", "major"].includes(kind)) {
  console.error("Usage: npm run release:patch | release:minor | release:major");
  process.exit(1);
}

function git(...args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

if (git("status", "--porcelain")) {
  console.error("Commit or stash your changes before creating a release.");
  process.exit(1);
}

const pkgPath = "package.json";
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const [major, minor, patch] = pkg.version.split(".").map(Number);
const version =
  kind === "major" ? `${major + 1}.0.0` : kind === "minor" ? `${major}.${minor + 1}.0` : `${major}.${minor}.${patch + 1}`;

pkg.version = version;
writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

const lock = JSON.parse(readFileSync("package-lock.json", "utf8"));
lock.version = version;
if (lock.packages?.[""]) lock.packages[""].version = version;
writeFileSync("package-lock.json", `${JSON.stringify(lock, null, 2)}\n`);

for (const file of ["src-tauri/Cargo.toml", "src-tauri/tauri.conf.json"]) {
  const source = readFileSync(file, "utf8");
  const updated = source
    .replace(/version = "\d+\.\d+\.\d+"/, `version = "${version}"`)
    .replace(/"version": "\d+\.\d+\.\d+"/, `"version": "${version}"`);
  if (updated === source) throw new Error(`Could not update the version in ${file}`);
  writeFileSync(file, updated);
}

const lockfile = readFileSync("src-tauri/Cargo.lock", "utf8").replace(
  /name = "recall"\nversion = "\d+\.\d+\.\d+"/,
  `name = "recall"\nversion = "${version}"`,
);
writeFileSync("src-tauri/Cargo.lock", lockfile);

git("add", "package.json", "package-lock.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "src-tauri/tauri.conf.json");
git("commit", "-m", `Release ${version}`);
try {
  git("tag", "-d", `v${version}`);
} catch {
  // no local tag yet
}
git("tag", "-a", `v${version}`, "-m", `Release ${version}`);

console.log(`Tagged v${version}.`);
console.log("Publish it with:\n");
console.log("  git push --follow-tags");
console.log("\nGitHub Actions then builds, signs, and publishes the release.");

# Recall

**Your computer finally remembers.**

Recall is a private, local-first memory for your computer. It quietly remembers which apps and windows you use so you can find your way back to what you were doing. Everything stays on your device: there is no account, no cloud, and no telemetry.

Recall is proprietary software, free for users. This repository is private.

## Status — v0.1.1 (in development)

| Area | State |
| --- | --- |
| Application activity recording (app, window title, duration, idle detection) | Working |
| Timeline with day navigation and app/type filters | Working |
| Pause / resume (sidebar, tray, timed pauses) | Working |
| Exclusions (apps, title keywords, websites, folders) with privacy-leaning defaults | Working |
| Delete one memory, a time range, a day, or everything | Working |
| Retention (7 days → forever), JSON export | Working |
| First-run onboarding with staged, optional permissions | Working |
| Browser memory: Chrome, Edge, Firefox, Safari (macOS) | Working |
| Local full-text search (SQLite FTS5) | Working |
| Optional screenshots | Working |
| Local Llama 3.2 1B answers grounded in retrieved memories | Working |
| Signed update checks | Configured |
| Local AI answers, "Continue where I left off" | Planned |
| Agent with permissioned tools | Planned |
| Signed auto-updates | Planned (required before the v0.1 public release) |

Anything not built yet is labeled as such in the app. Recall never fakes results.

## Platforms

- **Windows 10/11** and **macOS 10.15+** — supported targets.
- **Linux (X11)** — development only; used to run the app in CI and dev containers.

## Quick start

Prerequisites: Node.js 20+, Rust (stable), and the [Tauri system dependencies](https://tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev
```

This starts the Vite dev server on `http://127.0.0.1:51820` and opens the Recall desktop window. (Opening that URL in a regular browser only shows a notice — Recall's data lives in the desktop backend.)

Run all checks:

```bash
npm run typecheck && npm run lint && npm test
cd src-tauri && cargo test
```

Build an installer for the current OS:

```bash
npm run tauri build
```

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — how Recall is put together
- [PRIVACY.md](PRIVACY.md) — what is recorded, where it lives, how to export or delete it
- [SECURITY.md](SECURITY.md) — threat model and security controls
- [DEVELOPMENT.md](DEVELOPMENT.md) — setup, workflow, testing, conventions
- [RELEASES.md](RELEASES.md) — versioning, CI/CD, signing, update plan
- [CHANGELOG.md](CHANGELOG.md)

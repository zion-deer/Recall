# Development

## Prerequisites

- Node.js 20+ and npm
- Rust stable (`rustup`); MSRV is declared in `src-tauri/Cargo.toml`
- Tauri system dependencies — <https://tauri.app/start/prerequisites/>
  - **Windows:** Microsoft C++ Build Tools, WebView2 (preinstalled on Windows 11)
  - **macOS:** Xcode Command Line Tools
  - **Linux (dev only):** `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev build-essential`, an X11 session

## Running

```bash
npm install
npm run tauri dev        # desktop app with hot reload (Vite on 127.0.0.1:51820)
```

Data from dev builds goes to the same per-user app data folder as release builds. To start fresh, quit Recall and delete that folder (see PRIVACY.md for locations).

## Checks

```bash
npm run typecheck        # tsc
npm run lint             # oxlint
npm test                 # vitest (frontend utilities)
cd src-tauri && cargo test   # Rust unit tests: storage, migrations, settings,
                             # privacy rules, recorder sessions, export, validation
cd src-tauri && cargo clippy --all-targets
```

CI runs all of these on Windows, macOS and Linux for every push and pull request.

## Project layout

```text
src/                     React UI
  components/            app shell, shared widgets, components/ui = shadcn primitives
  hooks/                 use-recall (app state), use-navigation, use-async, use-theme
  lib/                   api.ts (typed IPC), format.ts, status.ts
  pages/                 home, memory, onboarding, settings/*, coming-soon
src-tauri/
  src/                   Rust core (see ARCHITECTURE.md)
  capabilities/          IPC permission grants
  build.rs               IPC command allow-list
  tauri.conf.json        app, window, CSP and bundle config
.github/workflows/       CI and release pipelines
```

## Adding an IPC command

1. Implement it in `src-tauri/src/commands.rs`, validating every argument.
2. Register it in `generate_handler!` in `lib.rs`.
3. Add its name to `COMMANDS` in `build.rs`.
4. Grant `allow-<name-with-dashes>` in `capabilities/default.json`.
5. Add a typed wrapper in `src/lib/api.ts`.

## Adding a schema change

Append a new `Migration` to `storage/migrations.rs` with the next version number. Never edit a migration that has shipped. Add a test.

## Adding a collector

Create a module under `memory/` that produces `NewEvent`s with a new `EventKind`, runs every candidate through `PrivacyFilter`, and is gated by its own setting. Keep OS calls in `platform/`.

## Conventions

- **Privacy in logs:** never log window titles, URLs, file paths or app names.
- **No fake functionality:** unbuilt features are labeled "Not available in this version" / "Coming later".
- Prefer small modules; avoid new dependencies unless they clearly pay for themselves.
- UI primitives come from shadcn/ui (`npx shadcn@latest add <component>`); don't add a second component library.

## Branches

- `main` — released / releasable code
- `develop` — integration branch
- `feature/*`, `fix/*` — short-lived branches merged via pull request

Commit in small, logical steps. Never commit secrets.

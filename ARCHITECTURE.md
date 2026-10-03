# Architecture

Recall is a [Tauri 2](https://tauri.app) desktop app: a Rust core that does all recording, storage and policy enforcement, and a React + TypeScript UI that talks to it over Tauri's IPC.

```text
┌──────────────────────── Frontend (src/) ────────────────────────┐
│ React 19 · TypeScript · Tailwind 4 · shadcn/ui (Base UI)         │
│ pages/  home · memory (timeline) · settings · onboarding         │
│ lib/api.ts  ← the only place that calls invoke()/listen()        │
└──────────────────────────────┬───────────────────────────────────┘
                 IPC commands  │  events: recorder:status,
              (allow-listed)   │  memory:changed, settings:changed
┌──────────────────────────────┴───── Rust core (src-tauri/src/) ──┐
│ commands.rs   IPC boundary: validates every argument             │
│ state.rs      AppState: db, live config, recorder, platform      │
│ settings.rs   typed settings + validation                        │
│ memory/                                                          │
│   mod.rs        event model (NewEvent, MemoryEvent, EventKind)   │
│   recorder.rs   app-activity collector (background thread)       │
│   privacy.rs    exclusion rules + PrivacyFilter                  │
│   store.rs      event queries, deletion, retention, stats        │
│   export.rs     streaming JSON export                            │
│ storage/      SQLite connection + versioned migrations           │
│ platform/     PlatformAdapter trait                              │
│   windows.rs · macos.rs · linux.rs (dev only)                    │
│ tray.rs       system tray / menu bar                             │
└──────────────────────────────────────────────────────────────────┘
```

## Memory pipeline

```text
Platform adapter ──► Collector ──► Privacy filter ──► Store (SQLite) ──► UI
 (active window,     (recorder.rs:   (exclusions;      (events table)     (timeline,
  idle time)          sessions)       drop on match)                       home)
```

- **Collectors** produce `NewEvent`s. v0.1 has one: application activity. Browser history and screenshots will be separate collectors that emit the same event model with their own `kind`.
- **The privacy filter runs before anything is written.** Excluded activity is never stored, not even as a placeholder. Adding an exclusion also deletes existing matching memories.
- **Recall never records itself** (matched by process id), so what you look at inside Recall doesn't end up in your memory.

### Application activity collector

A dedicated thread samples the foreground window every `pollIntervalSecs` (default 2 s):

1. Recording off / paused → close any open session.
2. Idle longer than `idleThresholdSecs` (default 5 min) → close the session at the time of last input.
3. Foreground window excluded or is Recall → close the session.
4. Same app + title as the open session → extend it. The new end time is flushed to disk at most every 15 s.
5. Different window → close the open session and start a new one.

Each session is one row. A crash loses at most ~15 s of duration. Retention runs hourly on the same thread.

Settings and exclusions are shared with the thread through `SharedConfig` (an `RwLock` snapshot); IPC commands update it and send a `Refresh` message so changes (like pausing) apply immediately rather than on the next tick.

## Storage

One SQLite database: `<app data dir>/recall.db` (`%APPDATA%\com.recallapp.desktop` on Windows, `~/Library/Application Support/com.recallapp.desktop` on macOS).

- WAL mode, `secure_delete=ON` (deleted content is overwritten), `VACUUM` after "delete all".
- File permissions restricted to the current user (0600 / 0700 on Unix; per-user AppData ACLs on Windows).
- Timestamps are Unix milliseconds, UTC. The UI converts to local time.

### Schema (migration 1)

| Table | Purpose |
| --- | --- |
| `events` | One row per memory. `kind`, `source`, `started_at`, `ended_at`, `app_name`, `app_id`, `window_title`, `url`, `file_path`, `metadata` (JSON). Indexed by `started_at` and `app_name`. |
| `settings` | Key/value. User settings are stored as one JSON document so new settings don't need migrations; missing fields fall back to defaults, invalid documents fall back to defaults. |
| `exclusions` | `kind` ∈ {app, website, folder, title}, `pattern`; unique per kind case-insensitively. Seeded with password managers, Signal, and private-browsing title markers. |

`url`, `file_path` and `metadata` exist now so upcoming collectors don't need a destructive change.

### Migrations

`storage/migrations.rs` holds an append-only list tracked with `PRAGMA user_version`.

- Each migration runs in a transaction.
- Before upgrading an existing database, Recall writes a backup (`recall.db.v<N>.bak`) with `VACUUM INTO`.
- A database from a **newer** Recall version is refused with a clear message instead of being opened (no silent downgrade).
- Never edit a shipped migration; add a new one.

## IPC boundary

- Every command is declared in `src-tauri/build.rs` and granted explicitly in `capabilities/default.json`. Undeclared commands can't be invoked from the webview.
- Commands treat input as untrusted: ids must be positive, ranges ordered, limits bounded (≤ 1000), patterns validated and length-limited, export paths must be absolute `.json` files in an existing directory with no `..`.
- Errors cross the boundary as `{ code, message }` and are shown to the user; the UI never pretends an operation succeeded.
- Strict CSP; no remote content is loaded.

## Platform layer

`platform::PlatformAdapter` is the only place OS APIs are used:

| | Active window | Idle time | Permissions |
| --- | --- | --- | --- |
| Windows | `active-win-pos-rs` (Win32) | `GetLastInputInfo` | none needed |
| macOS | `active-win-pos-rs` (NSWorkspace + CGWindowList), wrapped in an autorelease pool | `CGEventSourceSecondsSinceLastEventType` | Screen Recording (for window titles; app names work without it) |
| Linux (dev) | `active-win-pos-rs` (X11) | not available | — |

## Frontend

- No router: a small navigation context switches between Home, Memory, Search, Agent, Settings. `Ctrl/⌘ + 1…5` jumps between them; `Ctrl/⌘ + ,` opens Settings.
- `RecallProvider` loads settings, status and app info once, then stays current through backend events. `memoryVersion` increments on `memory:changed` so views refetch.
- If the UI is opened outside Tauri it shows a notice rather than mock data.

## Decisions log

| Decision | Choice | Why |
| --- | --- | --- |
| Desktop framework | Tauri 2 + Rust | Small footprint, native performance for a background recorder, strong IPC permission model. |
| Storage | SQLite (bundled via `rusqlite`) | Single portable file, FTS5 available for search, mature. |
| Settings format | JSON document in SQLite | Adding settings never requires a migration. |
| Active-window detection | `active-win-pos-rs` behind our adapter | Battle-tested on all three OSes; avoids hand-written Objective-C FFI. Replaceable per platform. |
| Encryption at rest | **Not yet** — relies on OS disk encryption (BitLocker / FileVault) and user-only file permissions | SQLCipher with a key in the OS keychain is the leading option, but it changes the permanent storage format, so it needs a product decision first. |
| Session granularity | One row per (app, window title) focus run, flushed every 15 s | Low write volume, crash-tolerant. |

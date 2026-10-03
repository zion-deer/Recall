# Changelog

All notable changes to Recall are documented here. This project uses [semantic versioning](https://semver.org).

## [Unreleased]

### Features
- Browser memory for Chrome, Edge, Firefox, and Safari on macOS: URL, page title, browser, and visit time.
- Browser-level controls and installation/status reporting in Memory and Privacy settings.
- Normal browser-history databases only; private/incognito visits are never imported.
- Website exclusions are applied before URL or title is stored and retroactively delete matching memories.
- SQLite FTS5 search across apps, window titles, URLs, and file paths, with date and memory-type filters.
- Browser memories appear in the timeline, have a detailed view, and can be opened with the OS default browser.

### Security
- Only HTTP(S) URLs are accepted. Credentials and fragments are rejected/removed, and common authentication query parameters are redacted.
- Browser databases are read through private temporary snapshots; cookie, login, autofill, payment, and preference stores are never opened.

## [0.1.0]

### Features
- Desktop app foundation for Windows and macOS (Tauri 2, Rust, React, TypeScript).
- Local SQLite storage with versioned, transactional migrations and automatic pre-upgrade backups.
- Application activity recording: app name, window title, start/end time, idle detection.
- Memory timeline with day navigation, date picker, and app / memory-type filters; memory detail view.
- Home screen with recent activity, today's time per app, and quick actions.
- Global pause (15 min, 1 h, 4 h, until resumed) from the sidebar, settings, and system tray.
- Exclusions for apps, window-title keywords, websites, and folders, with privacy-leaning defaults.
- Deletion of a single memory, the last 15 minutes / hour, a whole day, or everything.
- Retention setting (7 days, 30 days, 6 months, 1 year, forever).
- JSON data export.
- Staged first-run onboarding with optional permissions.
- Settings: General, Memory, Privacy, AI, Agent, Permissions, Appearance (light/dark/system), Updates, Data, About.
- System tray: keeps recording when the window is closed; pause/resume from the menu.

### Security
- IPC commands are allow-listed and every argument is validated.
- Strict content security policy; no remote content.
- Database restricted to the current user; SQLite `secure_delete` enabled.
- Logs never contain memory contents.

### Not yet available
- Screenshots, AI answers, agent, and signed auto-updates.

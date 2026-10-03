# Changelog

All notable changes to Recall are documented here. This project uses [semantic versioning](https://semver.org).

## [Unreleased] — 0.1.0

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
- Search, screenshots, browser history, AI answers, agent, and signed auto-updates.

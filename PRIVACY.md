# Privacy

Recall exists to remember sensitive things about how you use your computer. It is designed so that this information never leaves your device unless you explicitly move it.

## The short version

- Everything Recall records is stored **only on your computer**.
- Recall has **no account, no servers, no analytics, and no telemetry**. The current version makes no network requests at all.
- Recording is **off until you turn it on** during setup.
- You can **pause, exclude, delete, and export** at any time.

## What Recall records (v0.1)

When recording is on, for the window in front:

| Recorded | Notes |
| --- | --- |
| Application name | e.g. "Visual Studio Code" |
| Application path / bundle | Used to tell apps apart and to match exclusions |
| Window title | Can be turned off (Settings → Memory → Window titles) |
| Start and end time | Idle periods (no keyboard/mouse input) are not counted |

Recall does **not** record keystrokes, clipboard contents, screen contents, audio, or file contents. Screenshots and browser history are not part of this version; when added they will be separate, off-by-default switches with their own retention.

## What Recall never records

- **Recall itself.** Your searches and browsing inside Recall are not added to your memory.
- **Excluded apps, websites, folders and title keywords.** Excluded activity is dropped before it's written — not hidden, never stored. Adding an exclusion also deletes existing memories that match.
- **Defaults:** password managers (1Password, Bitwarden, KeePass, KeePassXC, LastPass, Dashlane, Enpass, Proton Pass, Keychain Access, Passwords, Credential Manager), Signal, and windows whose title contains "Private Browsing", "Incognito" or "InPrivate". You can change these in Settings → Privacy.
- While **paused** or with **recording off**, nothing is recorded.

## Where your data lives

A single SQLite database, readable only by your user account:

- Windows: `%APPDATA%\com.recallapp.desktop\recall.db`
- macOS: `~/Library/Application Support/com.recallapp.desktop/recall.db`

Settings → Data → Location → Show opens this folder.

The database is not separately encrypted in this version. We recommend enabling full-disk encryption (BitLocker on Windows, FileVault on macOS), which protects it at rest.

Diagnostic logs are stored in the app's log folder. **Logs never contain window titles, websites, file names, or other memory contents** — only events like "recorder started" or "migration applied". Logs are never uploaded.

## Retention

Settings → Memory → Keep memories for: 7 days, 30 days, 6 months (default), 1 year, or forever. Older memories are deleted automatically within an hour.

## Deleting data

- **One memory:** open it in the timeline → Delete memory.
- **A time range:** Memory → Delete… → last 15 minutes, last hour, or a whole day.
- **Everything:** Settings → Privacy (or Data) → Delete all memory. You must type `delete` to confirm.

Deletion is permanent. Recall uses SQLite `secure_delete`, so deleted content is overwritten in the database file, and "delete all" compacts the file afterwards.

Uninstalling Recall does not automatically remove the data folder on every platform; delete it manually if you want to remove all traces.

## Exporting your data

Settings → Privacy → Export data (or Settings → Data) saves a JSON file wherever you choose.

Format (`formatVersion` 1):

```json
{
  "format": "recall-export",
  "formatVersion": 1,
  "appVersion": "0.1.0",
  "schemaVersion": 1,
  "exportedAt": 1791052800000,
  "settings": { "...": "your current settings" },
  "exclusions": [
    { "id": 1, "kind": "app", "pattern": "1Password", "createdAt": 0 }
  ],
  "events": [
    {
      "id": 42,
      "kind": "app_activity",
      "source": "app_activity",
      "startedAt": 1791052800000,
      "endedAt": 1791052860000,
      "appName": "Visual Studio Code",
      "appId": "C:\\Program Files\\Microsoft VS Code\\Code.exe",
      "windowTitle": "search.ts — recall",
      "url": null,
      "filePath": null
    }
  ]
}
```

All timestamps are Unix epoch milliseconds (UTC). Events are in chronological order. The export file is not encrypted — store it carefully.

## Future features and this policy

- **AI:** local models by default. Only the memories relevant to a question will be given to the model. Any cloud AI option will be opt-in, clearly labeled, and show what is sent.
- **Crash reporting:** if ever added, it will be opt-in, will explain what is sent, and will never include memory contents or screenshots.
- **Updates:** checking for updates will contact our update server with only the app version and platform.

This document will be updated whenever what Recall collects changes.

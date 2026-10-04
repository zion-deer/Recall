# Privacy

Recall exists to remember sensitive things about how you use your computer. It is designed so that this information never leaves your device unless you explicitly move it.

## The short version

- Everything Recall records is stored on your computer.
- There is no account, no analytics, and no telemetry.
- Recording stays off until you turn it on during setup.
- You can pause, exclude, delete, and export at any time.
- Recall only uses the network for two things you start yourself: downloading the local AI model, and checking GitHub for an app update. Your activity is not part of either request.

## What Recall records

When recording is on, for the window in front:

| Recorded | Notes |
| --- | --- |
| Application name | e.g. "Visual Studio Code" |
| Application path / bundle | Used to tell apps apart and to match exclusions |
| Window title | Can be turned off (Settings → Memory → Window titles) |
| Start and end time | Idle periods (no keyboard/mouse input) are not counted |
| Normal browser visits (optional) | Browser name, page title, sanitized HTTP(S) URL, and visit time from Chrome, Edge, Firefox, and Safari on macOS |

Recall does **not** record keystrokes, clipboard contents, audio, file contents, cookies, saved logins, autofill, payment information, or browser preferences.

Screenshots are off until you enable them in Settings. They are JPEG files in the Recall data folder, not uploaded anywhere. Pause, idle time, excluded apps, and Recall's own window skip capture. Screenshot retention is separate from other memories, and deleting a screenshot deletes its image file.

Local AI is also off until you enable it and choose to download Llama 3.2 1B Instruct (about 770 MB). The download is verified with a pinned SHA-256 checksum. A question searches your local memory first; only a short list of relevant records is given to the model. If those records do not contain an answer, Recall says so instead of inventing one. The model is removed from disk when you choose Remove model.

Browser memory is off by default. When enabled, Recall reads only the browser's normal history SQLite database every 15 seconds. It makes a private, short-lived snapshot because browsers may keep the original file locked. It never modifies the browser database.

Private/incognito sessions do not write visits to normal browser history, so Recall cannot import them. When browser memory, global recording, or a specific browser is disabled—or Recall is paused—Recall does not open that history database. Re-enabling establishes a new high-water mark, so activity from the disabled period is not imported later.

Only `http://` and `https://` URLs are accepted. URL credentials are rejected; fragments are removed; values for common authentication parameters such as `token`, `code`, `session`, and `password` are replaced with `[redacted]`.

## What Recall never records

- **Recall itself.** Your searches and browsing inside Recall are not added to your memory.
- **Excluded apps, websites, folders and title keywords.** Excluded activity is dropped before it's written — not hidden, never stored. Adding an exclusion also deletes existing memories that match.
- **Defaults:** password managers (1Password, Bitwarden, KeePass, KeePassXC, LastPass, Dashlane, Enpass, Proton Pass, Keychain Access, Passwords, Credential Manager), Signal, and windows whose title contains "Private Browsing", "Incognito" or "InPrivate". You can change these in Settings → Privacy.
- While **paused** or with **recording off**, nothing is recorded.
- **Excluded websites.** URL and page title are discarded together before storage, including subdomains.

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
  "appVersion": "0.1.1",
  "schemaVersion": 2,
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

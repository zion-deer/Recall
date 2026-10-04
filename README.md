# Recall

Recall is a desktop app that keeps a local history of the apps and windows you use, so you can find your way back to something later. It runs on your computer. There is no account.

Installers for Windows and Mac are on the [latest release](https://github.com/zion-deer/Recall/releases/latest).

| System | File |
| --- | --- |
| Windows 10/11 | `Recall_*_x64-setup.exe` or the `.msi` |
| Mac (Apple Silicon) | `Recall_*_aarch64.dmg` |

Intel Macs are not built. Linux is only for development.

## Opening it on a Mac

A browser download is not signed with an Apple Developer ID, so macOS says the app is damaged. Drag Recall to Applications, then in Terminal:

```bash
xattr -cr /Applications/Recall.app
open /Applications/Recall.app
```

## Updates

The installed app checks GitHub for a newer release. It verifies the signature before installing, and it waits for you to choose **Update now**. You can also check from Settings → Updates.

## What it records

Recording is off until you turn it on.

While it is on, Recall stores the frontmost app, the window title, and how long you stayed there. Time with no keyboard or mouse input is left out. Closing the window does not stop recording; quit from the system tray or menu bar icon when you want it to stop. Pause is in the sidebar.

These are off until you enable them:

- **Browser history** for Chrome, Edge, Firefox, and Safari. Safari is Mac only. Recall reads normal history, not private or incognito windows.
- **Screenshots**, either on a timer or when you switch windows. The images stay in Recall's data folder and have their own retention setting.
- **Ask**, which answers from your saved activity using Llama 3.2 1B on this computer. Settings → AI downloads the model (about 770 MB) only if you ask it to. If your history does not contain an answer, Recall says so.

Search covers app names, window titles, and page addresses.

You can exclude apps, websites, folders, and words in window titles. Password managers and a few messaging apps are excluded already. You can delete one memory, a stretch of time, a day, or everything, and you can export the data as JSON.

## What it does not do

Recall does not record keystrokes, clipboard contents, audio, file contents, cookies, or saved passwords. It does not record its own window. It does not upload your activity.

It does not take actions for you. There is no agent in this version.

The only network use is the model download, if you request it, and the update check.

## Building it

You need Node.js 22, a stable Rust toolchain, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev
```

That opens the Recall window. `http://127.0.0.1:51820` in a browser is only the interface, with no memory behind it.

```bash
npm run typecheck && npm run lint && npm test
cd src-tauri && cargo test
```

[PRIVACY.md](PRIVACY.md) describes storage and deletion. [DEVELOPMENT.md](DEVELOPMENT.md) is the setup notes. [RELEASING.md](RELEASING.md) is how a version is tagged and published.

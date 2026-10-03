# Security

Recall's database is effectively a diary of the user's computer use. We treat it as highly sensitive.

## Threat model (v0.1)

| Threat | Mitigation |
| --- | --- |
| Other local users reading the database | Data dir 0700 / file 0600 on Unix; per-user AppData ACL on Windows. |
| Offline disk theft | Rely on OS full-disk encryption today. Application-level encryption (SQLCipher + OS keychain) is planned and requires a storage-format decision. |
| Malicious or compromised web content in the UI | The UI loads only bundled assets. Strict CSP (`default-src 'self'`), no remote scripts, `freezePrototype`. |
| Webview calling privileged functionality | Every IPC command is declared in `build.rs` and explicitly granted in `capabilities/default.json`; the dialog plugin is limited to `save`. |
| Malformed or hostile IPC arguments | All commands validate input (ids, ranges, limits, string lengths, control characters, website/folder syntax, export paths). |
| SQL injection | All SQL uses bound parameters; covered by tests with SQL metacharacters in titles. |
| Path traversal on export | Export path must be absolute, end in `.json`, contain no `..`, and its parent must exist. Writes go to a temp file and are renamed. |
| Command injection | No shell is ever invoked. Opening folders passes a single path argument to `explorer` / `open` / `xdg-open`, only for Recall's own data/log folders. |
| Sensitive data in logs | Logging policy: never log titles, URLs, app names or file paths. The recorder logs only state transitions. |
| Data loss from upgrades | Versioned, transactional migrations; automatic backup before upgrading; refusal to open newer schemas. |
| Two instances writing concurrently | Single-instance plugin focuses the existing window instead. |

## Future: the agent

When the agent is built it will use only structured tools, each with a JSON schema, a permission level (`READ_ONLY`, `LOW_RISK`, `CONFIRM_REQUIRED`, `HIGH_RISK`, `BLOCKED`), argument validation, path sanitization, and an audit log. Model output will never be executed as a shell command. Credential stores, password databases, private keys and payment flows are blocked.

## Future: updates

Auto-updates will use Tauri's updater: release metadata over HTTPS, packages signed with an Ed25519 key whose public half ships in the app, signature verification before install, and user consent before restart. The private signing key lives only in CI secrets. See [RELEASES.md](RELEASES.md).

## Secrets

- No API keys or secrets are committed or hard-coded. `.gitignore` blocks common key formats and `.env` files.
- Release signing material (Apple Developer ID, Windows code-signing certificate, updater key) is provided to CI through encrypted secrets only.

## Reporting a vulnerability

Email the security contact (to be published before public release). Please don't open public issues for security reports.

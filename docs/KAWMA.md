# Kawma ↔ Kawma Launcher contract

Server: `https://kawma.app` (the Kawma-Bot repo, Fastify on Railway). Everything below is new unless marked existing.

## Kawma account sign-in (existing desktop flow, extended)

Same flow Kawma Studio uses (`desktop/src-tauri/src/account.rs` in Kawma-Bot is the reference client):

1. Client makes a PKCE pair: `verifier` = 43+ url-safe base64 chars, `challenge` = base64url(sha256(verifier)) (43 chars, no padding).
2. `POST /auth/desktop/start` JSON `{ challenge, provider: "twitch" | "discord", app: "launcher" }` → `{ id, code, expiresIn }`. (`app` is new; "studio" is the default. It only changes the wording of the browser pages.)
3. Open the browser at `GET /auth/desktop/login?id=<id>`.
4. Poll `POST /auth/desktop/poll` JSON `{ id, verifier }` no faster than every 1.5 s (429 if faster) → `{ pending: true }` until approved, then `{ token, profile }`. 400 = expired/invalid.
5. Token is a 43-char bearer: `Authorization: Bearer <token>`, valid 30 days. Store it in Windows Credential Manager (`keyring` crate, service `com.aurora.launcher`, user `kawma-account`).
6. `GET /auth/desktop/profile` (Bearer) → profile, 401 = sign in again. `POST /auth/desktop/logout` (Bearer) → 204.

Profile shape:
```json
{ "id": "...", "displayName": "vKarmah", "avatar": "https://..." | null,
  "twitch": "login" | null, "discord": "username" | null,
  "tester": false, "testMode": false,
  "releases": { "launcher-now-playing": true, "launcher-community": false, ... },
  "minecraft": [{ "uuid": "32 hex, no dashes", "name": "vKarmah" }] }
```
`minecraft` is new: the Minecraft accounts the launcher linked.

## Launcher endpoints (all Bearer desktop token; 401 = sign in again)

- `PUT /auth/desktop/launcher/minecraft` `{ accounts: [{ uuid, name }] }` (max 10; uuid 32 hex lowercase without dashes, name 1–16 `[A-Za-z0-9_]`) → profile. Replaces the linked list. Send it after sign-in and whenever Minecraft accounts are added/removed.
- `POST /auth/desktop/launcher/activity` `{ game: "minecraft"|"skyrim"|"eldenring"|"cyberpunk", title: string ≤60 (e.g. "Minecraft"), detail?: string ≤80 (e.g. "All the Mods 9 · 1.20.1") }` → `{ ok: true }`; `{ game: null }` clears. 404 when the account does not have the `launcher-now-playing` release. Server forgets it 15 minutes after the last post, so re-post every 5 minutes while the game runs.
- `GET /auth/desktop/launcher/activity` → `{ playing: { game, title, detail, since } | null }` (Studio uses this).
- `POST /auth/desktop/launcher/session` `{ game, name: string ≤60 (server/world name), address: string ≤120 ("bore.pub:41234"), version?: string ≤40, whitelist: boolean }` → `{ ok: true, discord: boolean, twitch: boolean }` (which places it was announced). 404 without the `launcher-community` release. Re-posting replaces the session.
- `DELETE /auth/desktop/launcher/session` → `{ ok: true }`.
- `GET /auth/desktop/launcher/session/requests` → `{ requests: [{ id: string, name: string (Minecraft name), from: "twitch"|"discord", user: string (display name), at: number }] }` pending whitelist requests. Poll every 10 s while a session with `whitelist: true` is open.
- `POST /auth/desktop/launcher/session/requests/:id` `{ ok: boolean, error?: string ≤200 }` → `{ ok: true }` after the launcher ran `whitelist add <name>` on the hosted server (the bot tells the viewer).

## Updates (replaces GitHub releases)

- `GET /launcher/releases/latest.json` (stable, everyone) and `GET /launcher/test/latest.json` (testers: profile.tester && profile.testMode) →
  ```json
  { "version": "1.8.3", "notes": "line\nline", "pub_date": "ISO",
    "platforms": { "windows-x86_64": { "url": "https://kawma.app/launcher/builds/1.8.3/Kawma-Launcher-1.8.3-x64-setup.exe", "sha256": "64 hex", "size": 12345 } } }
  ```
  404 when nothing is published yet. The launcher must verify the installer's SHA-256 before running it and only accept https URLs on kawma.app.
- `GET /launcher/releases.json` → `{ releases: [{ version, title, notes: string[], date: "YYYY-MM-DD" }] }` released builds, newest first (in-app patch notes).
- `GET /launcher/download` → 302 to the newest stable installer.
- CI (MCLauncher `.github/workflows/launcher.yml`, GitHub OIDC token with audience `kawma-launcher-builds`, repository `camwooloo/MCLauncher`, ref `refs/heads/main`):
  - `POST /api/launcher-builds` `{ floor: "1.8.0", title?: string, notes?: string[] }` → `{ version, title, notes }` (next version to build).
  - `PUT /api/launcher-builds/:version` body = installer bytes (`Content-Type: application/octet-stream`), headers `x-kawma-sha256: <hex>` → test channel.
  - `POST /api/launcher-builds/:version/failed`.
  - An admin releases a test build to everyone from kawma.app admin → Updates.

## Finding each other on the PC

Both apps are Tauri NSIS installs. Each writes `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>` with `DisplayIcon` = `"<install dir>\<binary>.exe"` (quoted). Also check HKLM for per-machine installs.
- Studio: productName `Kawma Studio`. Download page `https://kawma.app/studio`.
- Launcher: productName `Kawma Launcher` (older installs: `Aurora Launcher`), binary `aurora-launcher.exe`. Download page `https://kawma.app/launcher`.

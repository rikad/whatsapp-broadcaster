# Rust Migration Plan — whatsapp-rust + gpui

Status: **implemented on branch `rust`**, pending live testing on Windows · Target release: v2.0.0 · Platform: **Windows x64 only**

## Goals

- **Drop the Chrome dependency.** [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust) speaks the WhatsApp protocol directly — no Puppeteer, no browser. Removes `findSystemChrome()` / `PUPPETEER_EXECUTABLE_PATH` and the #1 install issue.
- **Native Windows desktop app** built with [gpui](https://www.gpui.rs/) instead of a localhost page opened in the browser. The HTTP API stays for scripting.
- **Smaller, faster single `.exe`**, no Bun runtime, session stored in one SQLite file.

## Scope

- **Shipped:** Windows 10/11 x64 (`x86_64-pc-windows-msvc`), native gpui app.
- **Not shipped:** macOS and Linux builds, and headless mode. gpui is cross-platform, so macOS can still be used for day-to-day UI development, but every release is built and tested on Windows.

## Library notes (verified against whatsapp-rust 0.7.0)

- **whatsapp-rust 0.7.0** — MIT, edition 2024, MSRV stable 1.94 (the repo pins a nightly only for its own development), tokio runtime.
  - Setup: `Bot::builder().with_backend(SqliteStore::open(..)).on_qr_code(..).on_pair_code(..).on_connected(..).on_logged_out(..).build()`
  - Send: `client.send_message(Jid::pn(digits), wa::Message::text(..))`
  - Media: `client.upload(bytes, MediaType, UploadOptions::new())`
  - Other: `client.logout()`, `contacts().is_on_whatsapp(&[Jid])`, `groups().get_participating()`, `client.push_name()`, `client.pair_with_code()`
  - The default `simd` feature needs nightly Rust, so it is turned off; the other default features are enabled one by one.
  - Events: `Connected`, `Disconnected`, `PairSuccess`, `PairError`, `LoggedOut`, `PairingQrCode`, `PairingCode`, `ClientOutdated`, …
- **gpui** — pre-1.0, no built-in text input or table. The app depends on **gpui-kit 0.7** only: it pins the matching gpui snapshot (`gpui-pre`) and re-exports gpui-component (Input, Textarea, Button, notifications, theming). The broadcast table uses gpui's `uniform_list` for virtualized rows.

## Architecture

```
┌──────────── main thread ────────────┐      ┌────────── tokio runtime thread ──────────┐
│ gpui App                            │ cmds │ WaService (supervisor)                    │
│  AppState entity ◄── event pump ◄───┼──────┤  └─ whatsapp-rust Bot + SqliteStore       │
│  Login / Shell / Send / Broadcast   │events│ BroadcastEngine (task, cancel token)      │
│                                     ├─────►│ axum HTTP API + SSE on 127.0.0.1:1111     │
└─────────────────────────────────────┘      └───────────────────────────────────────────┘
```

gpui has its own executor and whatsapp-rust needs tokio, so tokio runs on a separate thread. The two sides talk through `tokio::sync` channels (mpsc + oneshot for commands, broadcast/watch for events). These channels don't depend on a runtime, so gpui can `.await` them directly inside `cx.spawn`. The event pump updates the `AppState` entity and calls `cx.notify()`.

## Workspace layout

```
Cargo.toml                 # workspace
rust-toolchain.toml        # stable, >= 1.94, target x86_64-pc-windows-msvc
crates/
  core/   lib.rs  service.rs (WaService/WaHandle, supervisor, re-pair loop)
          events.rs  phone.rs  media.rs  broadcast.rs  report.rs  paths.rs
  api/    lib.rs  routes.rs  sse.rs            # axum, same routes as api.js
  app/    main.rs  build.rs (winresource)  app.rs (root + sidebar)  connection.rs  theme.rs
          views/{login,send,broadcast}.rs
assets/   (icon.ico + icon.png reused; icon.icns no longer needed)
docs/broadcast-message.json
```

`core` has no UI code, so it can be tested on its own with plain `cargo test`.

## Mapping from the current code

| Current (JS) | Rust replacement | Notes |
|---|---|---|
| `whatsapp.js` `Client` + `LocalAuth` | `Bot` + `SqliteStore` in `%APPDATA%\WhatsApp Broadcaster\` (`directories` crate) | Users scan the QR once more; a `.wwebjs_auth` session can't be migrated. One DB file per session: logout switches to a new file instead of deleting an open one (Windows refuses that) |
| `qr` event → data URL | `on_qr_code(code, timeout)` → `qrcode` crate → PNG → gpui `img()` | Bonus: pair-code login via `with_pair_code` |
| `ready` / `authenticated` / `auth_failure` / `disconnected` | `Connected` / `PairSuccess` / `PairError` / `Disconnected` + `LoggedOut` | |
| `loading_screen` (percent) | No equivalent; show "Connecting…/Syncing…" | No browser, so no loading screen |
| `sendMessage(phone)` with `@c.us` | Strip non-digits → `Jid::pn(digits)` → `send_message(jid, Message::text)` | Optional `is_on_whatsapp` pre-check for a clear "not on WhatsApp" error |
| `sendMedia(path)` | Read file → `mime_guess` → `MediaType` → `upload()` → build Image/Video/Document message with caption | Biggest piece of new code; accepts Windows paths (`C:\…`) |
| `getChats()` | Groups only, via `groups().get_participating()` | whatsapp-rust has no server-side chat list; full chats via history sync is out of scope for v2 |
| `logout()` + `reinitialize()` | `client.logout()` → supervisor rebuilds the Bot with a fresh store → new QR | |
| Broadcast loop in `renderer.js` | `core::broadcast` tokio task emitting a `RowUpdated` event per row | Same semantics: skip rows already `success`, delay ≥ 500 ms, stop after the current message |
| Row parsing reads `customerName`, `invoiceCode`, … | Only `message` and the phone (`phone`, or `customerPhone` as fallback) are read; see [Broadcast input](#broadcast-input) | Every other field is ignored by the engine and copied unchanged into the report |
| Download report (Blob) | gpui `cx.prompt_for_new_path` (native Windows save dialog) → write JSON | Same file names: `{name}-report-{stamp}.json`, `{name}-failed-{stamp}.json` |
| Express on :1111 + SSE | axum with the same routes, response shapes and status codes; SSE via `axum::response::sse` | On by default; bind to **127.0.0.1** (Express currently listens on all interfaces) |
| `/`, `/renderer.js`, `/favicon.svg` | Removed | UI is native |
| Console window + auto-open browser | `#![windows_subsystem = "windows"]`: no console, the app window opens directly | Logs go to `%APPDATA%\WhatsApp Broadcaster\logs\` |
| `NO_OPEN=1` | Removed | Native app only |

### Core API sketch

```rust
pub enum WaEvent { Qr(String), PairCode(String), Connected(AccountInfo),
                   Disconnected(String), LoggedOut(String), Error(String),
                   Broadcast(BroadcastUpdate) }

impl WaHandle {
    pub fn subscribe(&self) -> broadcast::Receiver<WaEvent>;
    pub fn state(&self) -> watch::Receiver<ConnState>;   // replaces `lastEvents` replay for SSE
    pub async fn send_text(&self, phone: &str, text: &str) -> Result<SendOutcome>;
    pub async fn send_media(&self, phone: &str, path: &Path, caption: Option<&str>) -> Result<SendOutcome>;
    pub async fn logout(&self) -> Result<()>;
    pub fn start_broadcast(&self, rows: Vec<Row>, delay: Duration) -> BroadcastHandle; // .stop()
}
```

whatsapp-rust is 0.x and will change; keeping it behind `WaHandle` limits that churn to `core/service.rs`.

### Broadcast input

The engine processes only two keys per row:

| Key | Rule |
|---|---|
| `message` | Required, non-empty string |
| `phone` | Default phone key. Required unless `customerPhone` is present |
| `customerPhone` | Optional fallback, used only when `phone` is missing or empty (keeps v1 files working) |

```rust
pub struct Row {
    pub phone: String,
    pub message: String,
    pub result: RowResult,                 // broadcastStatus / SentAt / Error / MessageId
    raw: serde_json::Map<String, Value>,   // original object, written back untouched
}
```

- The root must be an array; a row missing `message` or both phone keys fails the whole file with `Entry #N is missing phone or message`.
- No other key (`customerName`, `invoiceCode`, `amount`, …) is read, validated or displayed. They stay in `raw` and are written back as-is in the report.
- The engine's own `broadcast*` fields are still read on load so the resume rule (skip rows with `broadcastStatus: "success"`) keeps working.

Minimal valid input:

```json
[
  { "phone": "628123456789", "message": "Hello" },
  { "customerPhone": "+62 812-3456-7890", "message": "Hi again", "anything": "ignored" }
]
```

## UI (gpui)

Same screens and the WhatsApp dark palette (`#0b141a`, `#111b21`, `#2a3942`, `#00a884`, `#e9edef`) as a gpui-component theme.

1. **Login** — card with QR image and status line; optional "Link with phone number" mode showing the 8-character pair code.
2. **Shell** — sidebar with avatar initial, push name, `+phone`, nav (Send Message, Broadcast), Logout.
3. **Send Message** — phone input, multi-line message input, Send button, result banner.
4. **Broadcast** — choose JSON file (`cx.prompt_for_paths`), delay input (min 500), Start / Stop / Download Report / Download Failed Only, live progress line, virtualized table (`#`, Phone, Message (one-line preview), Status badge, Sent At, Error) so thousands of rows stay smooth.

Windows details:

- Window and taskbar icon from `assets/icon.ico`.
- Single instance: if port 1111 or the SQLite file is already in use, show a dialog ("WhatsApp Broadcaster is already running") and exit instead of corrupting the session.
- Check rendering at 100 %, 125 % and 150 % display scaling.

## Phases

**Phase 0 — Spike (1–2 days).** Branch `rust`. Small CLI that pairs via a terminal QR and sends one text and one image to your own number. Then a "hello window" with a gpui-component Input. Both must build and run on **Windows** (MSVC toolchain); pin exact versions of whatsapp-rust, gpui and gpui-component.
*Go/no-go:* if media upload, the SQLite store or the gpui window fails on Windows, stop before investing more.

**Phase 1 — Core (3–4 days).** WaService supervisor (reconnect, re-pair after logout), phone normalization, media sending, broadcast engine, JSON report.
Unit tests:
- phone normalization (`+62 812-…` → `62812…`)
- `phone` is used by default; `customerPhone` only when `phone` is missing or empty
- rows missing `message` or a phone are rejected with the entry number
- other fields are never read but survive a load → save round trip
- rows already `success` are skipped
- stop takes effect after the current message
- 500 ms delay floor

**Phase 2 — API (1–2 days).** axum routes matching the README, `/api/chats` returning groups, SSE with last-state replay, contract tests mirroring the README curl examples (400 / 404 / 503).

**Phase 3 — gpui app (5–7 days).** Theme, AppState + event pump, the four views, file dialogs, error states, single-instance check.

**Phase 4 — Packaging & CI (1 day).**
- `winresource` in `app/build.rs`: icon, product name, publisher, version (replaces the `--windows-icon` / `--windows-version` flags in `build:win:release`).
- Release asset: portable `whatsapp-broadcaster-windows-x64.exe`, same as today. An installer (Inno Setup or `cargo-wix`) can come later.
- Rewrite `.github/workflows/build.yml` as one `windows-latest` job: `cargo test`, `cargo build --release`, upload the `.exe`, publish on `v*` tags.
- Unsigned `.exe` triggers a SmartScreen warning, as today; an Authenticode certificate is optional.

**Phase 5 — Cutover (1 day).**
- Rewrite the README: Windows only, no Chrome, data in `%APPDATA%\WhatsApp Broadcaster\`, breaking changes.
- Switch [docs/broadcast-message.json](broadcast-message.json) from `customerPhone` to `phone`.
- Tag `v2.0.0`. Keep the JS version on a `v1` branch. Delete `*.js`, `index.html`, `package.json`, `bun.lock`.

**Total: ~2–3 weeks** for one developer.

## Breaking changes (v2 release notes)

- **Windows only.** No macOS or Linux builds; macOS/Linux users stay on v1.
- Everyone re-scans the QR once.
- Broadcast input only needs `message` + `phone` (`customerPhone` still accepted as fallback); the table no longer shows Name and Invoice columns.
- `/api/send` response: `id` becomes a plain message ID (e.g. `3EB0…`) instead of `true_…@c.us_…`; `to` ends in `@s.whatsapp.net`.
- `info.platform` and the `loading` SSE event are removed.
- `/api/chats` returns groups only.
- API listens on localhost only. `NO_OPEN` is removed.

## Risks

| Risk | Mitigation |
|---|---|
| whatsapp-rust 0.x API churn | Pin exact version; isolate behind `WaHandle` |
| gpui pre-1.0, sparse docs; Windows backend younger than macOS | Pin gpui + gpui-component together; Phase 0 proves the build on Windows |
| gpui needs a working GPU driver; may fail over RDP or in some VMs | Test on the actual target PCs and over RDP during Phase 0 |
| Developing on macOS hides Windows-only bugs | Run every PR through the Windows CI job; test on a Windows machine before each release |
| Account bans | Same ToS risk as today; keep the 500 ms floor; random jitter can be added later |
| SmartScreen warning on unsigned `.exe` | Same as today; buy an Authenticode certificate if it becomes a problem |

## Decisions

1. `/api/chats` returns **groups only**.
2. The HTTP API **stays**, on by default, localhost only.
3. **Native gpui app, Windows only.**

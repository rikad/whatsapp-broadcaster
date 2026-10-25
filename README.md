# WhatsApp Broadcaster

A native Windows app for linking a WhatsApp account, sending single messages, broadcasting from a JSON file, and sending messages from scripts through a local HTTP API.

Version 2 is a rewrite in Rust: [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust) talks to WhatsApp directly and [gpui](https://www.gpui.rs/) (through [GPUI Kit](https://gpui-kit.com)) draws the window. **No Chrome, browser or runtime is needed.** v1 (Bun + whatsapp-web.js) lives on the `v1` branch.

## Features

- Native desktop window: link by QR code or by phone-number pairing code
- Persistent session: no re-scan on restart
- Send a single text message
- Broadcast from a JSON file with per-row status, stop, resume and report export
- Local HTTP API on `127.0.0.1:1111`, route-compatible with v1, plus a Server-Sent Events stream
- One portable `.exe`

## Requirements

- Windows 10 or 11, x64
- A graphics driver that supports DirectX 11 (any normal PC; some VMs and Remote Desktop sessions may not)

## Install

Download `whatsapp-broadcaster-windows-x64.exe` from the [latest release](../../releases/latest) and run it. Windows SmartScreen may warn because the file is not code-signed; choose **More info → Run anyway**.

On first launch:

1. Wait for the QR code.
2. On your phone open WhatsApp → **Settings → Linked devices → Link a device**.
3. Scan the code. Or click **Link with phone number instead**, enter your number with country code and type the 8-character code on your phone.

The app then shows **Send Message** and **Broadcast** in the sidebar. Only one copy of the app can run at a time.

## Send a single message

1. Open **Send Message**.
2. Enter the phone number with country code. Spaces, `+` and dashes are ignored.
3. Type the message and click **Send Message**.

Numbers are checked first; a number without WhatsApp fails with "… is not on WhatsApp".

## Broadcast from a JSON file

1. Open **Broadcast** and click **Choose JSON file…**. See [docs/broadcast-message.json](docs/broadcast-message.json).
2. Set the delay between messages (minimum 500 ms, default 3000 ms) to reduce the chance of being rate-limited.
3. Click **Start Broadcast**. Each row goes `pending` → `sending` → `success` / `failed`.
4. **Stop** ends the run after the current message.
5. **Download Report** saves every row with `broadcastStatus`, `broadcastSentAt`, `broadcastError` and `broadcastMessageId`. **Download Failed Only** saves just the failures.

### Input format

The root must be an array. Only two keys are read from each entry:

| Key | Rule |
|---|---|
| `message` | Required, non-empty string |
| `phone` | Phone number with country code. Required unless `customerPhone` is present |
| `customerPhone` | Fallback used only when `phone` is missing or empty (v1 files keep working) |

Every other field is ignored by the app and copied unchanged into the report.

```json
[
  { "phone": "628123456789", "message": "Hello Jane, your invoice INV-0001 is due.", "invoiceCode": "INV-0001" }
]
```

### Resume and retry

Rows already marked `"broadcastStatus": "success"` are skipped. To retry failures, load a downloaded report (or the `-failed-` export) again.

## HTTP API

The API runs inside the app on `http://127.0.0.1:1111` and only accepts connections from the same PC. Set the `PORT` environment variable to use another port. If the port is taken, the app still runs and shows a notice that the API is off.

| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/status` | `{"status": "ready" \| "not_ready", "info": …}` |
| GET | `/api/info` | `{"phoneNumber", "pushname"}` |
| POST | `/api/send` | `{"phone", "message"}` → send a text |
| POST | `/api/send-media` | `{"phone", "mediaPath", "caption"?}` → send a file from this PC |
| GET | `/api/chats` | Groups the account is in: `[{"id", "name", "isGroup": true}]` |
| POST | `/api/logout` | Unlink and show a new QR code |
| GET | `/api/events` | Server-Sent Events stream |

```bash
curl http://localhost:1111/api/status

curl -X POST http://localhost:1111/api/send \
  -H "Content-Type: application/json" \
  -d '{"phone":"628123456789","message":"Hello from the API"}'

curl -X POST http://localhost:1111/api/send-media \
  -H "Content-Type: application/json" \
  -d '{"phone":"628123456789","mediaPath":"C:\\Users\\me\\Pictures\\promo.jpg","caption":"Check this out"}'
```

A successful send returns `{"success": true, "id", "to", "timestamp"}`. Images, videos and audio are sent as media; anything else is sent as a document with its file name.

### Events

`/api/events` emits `qr` (PNG data URL), `pair-code`, `authenticated`, `ready`, `auth-failure`, `disconnected` and `error`. A new subscriber first receives the events matching the current state.

```bash
curl -N http://localhost:1111/api/events
```

### Status codes

- `200` success
- `400` missing fields or invalid phone number
- `404` unknown endpoint
- `500` send failed (for example the number is not on WhatsApp)
- `503` WhatsApp not connected yet

## Where data is stored

Everything lives in `%APPDATA%\WhatsApp Broadcaster\`:

- `whatsapp*.db`: the linked session. Delete the folder to start over.
- `logs\whatsapp-broadcaster.log`: log of the last run.

## Changes from v1

- Windows only. macOS and Linux users stay on v1.
- No Chrome needed.
- Everyone scans the QR code once more; v1 sessions cannot be migrated.
- Broadcast input uses `phone` (with `customerPhone` as fallback); the table no longer shows Name and Invoice columns.
- `/api/send`: `id` is a plain message ID (e.g. `3EB0…`) and `to` ends in `@s.whatsapp.net`.
- `/api/chats` lists groups only, without `unreadCount`. `info.platform` and the `loading` event are gone.
- The API listens on localhost only. `NO_OPEN` is gone (there is no browser to open).

## Development

Needs Rust (the version is pinned in `rust-toolchain.toml`; rustup installs it automatically). gpui is cross-platform, so the app also runs on macOS for UI work, but releases are built and tested on Windows.

```bash
cargo run -p whatsapp-broadcaster          # the app
cargo test --workspace                     # unit and API tests
cargo run -p wb-core --example spike -- 628123456789 "Hello" [file]   # link by terminal QR and send, no UI
```

Set `WB_DATA_DIR` to keep a development session separate from the installed app.

```
crates/
├── core/   # WhatsApp service, phone normalization, media, broadcast engine (no UI)
├── api/    # axum HTTP API + SSE
└── app/    # gpui desktop app (Windows .exe; icon embedded by build.rs)
assets/     # icon sources and rasters
docs/       # example broadcast file, migration plan
```

### Releases

Push a `v*` tag. CI (`.github/workflows/build.yml`) runs clippy and tests on Windows, builds the release `.exe` and publishes it.

### Regenerating the icon

`assets/icon.svg` is the source; `icon-small.svg` and `icon-tiny.svg` are simplified variants for small sizes. `./scripts/build-icons.sh` (needs Google Chrome and ImageMagick) rewrites `icon.png`, `icon-512.png`, `icon.ico` and `icon.icns`.

## Disclaimer

This project uses whatsapp-rust, an unofficial client not affiliated with or endorsed by WhatsApp or Meta. Automated messaging may violate WhatsApp's Terms of Service and can get a number banned. Use at your own risk.

## License

ISC

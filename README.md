# WhatsApp Broadcaster

A local web app for WhatsApp Web login, bulk broadcasting from a JSON file, and an HTTP API for sending messages programmatically. Runs as a single Bun-compiled binary that starts a local server and opens the UI in your default browser.

## Features

- Web UI served at `http://localhost:1111` (auto-opens in your default browser)
- Persistent session (no need to re-scan QR on restart)
- Send single text messages from the UI
- Bulk broadcast from a JSON file with per-row status, retry, and report export
- Send media files (images, documents) via API
- RESTful API on the same port for programmatic use
- Real-time UI updates via Server-Sent Events
- Builds into a standalone executable per platform with `bun build --compile`

## Prerequisites

- [Bun](https://bun.sh/) (v1.1+)
- **Google Chrome (or Chromium / Microsoft Edge) installed locally.** `whatsapp-web.js` drives WhatsApp Web through Puppeteer; the standalone binary does **not** ship a browser. The app auto-detects Chrome in the standard install locations on Windows, macOS, and Linux. If yours is somewhere unusual, set `PUPPETEER_EXECUTABLE_PATH`:
  - Windows: `set PUPPETEER_EXECUTABLE_PATH=C:\Program Files\Google\Chrome\Application\chrome.exe`
  - macOS: `export PUPPETEER_EXECUTABLE_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"`
  - Linux: `export PUPPETEER_EXECUTABLE_PATH=/usr/bin/google-chrome`

## Installation

```bash
bun install
```

## Running from source

```bash
bun start
```

This starts the local server on port 1111 and opens `http://localhost:1111` in your default browser. To suppress auto-open (for example on a headless machine), set `NO_OPEN=1`.

On first launch:
1. Wait for the QR code to appear in the browser.
2. Open WhatsApp on your phone.
3. Go to **Settings > Linked Devices > Link a Device**.
4. Scan the QR code.

After successful login, the app shell appears with two views in the sidebar: **Send Message** and **Broadcast**.

## Building standalone binaries

`bun build --compile` produces a single self-contained executable per platform. The `index.html` and `renderer.js` are embedded into the binary at build time.

```bash
bun run build           # current platform
bun run build:mac       # darwin-arm64 + darwin-x64
bun run build:win       # windows-x64
bun run build:linux     # linux-x64
```

Output goes to `dist/`. The CI workflow at `.github/workflows/build.yml` produces all four targets on every push.

> **Note:** The binary still requires Chrome/Chromium at runtime (Puppeteer launches it as a subprocess). Set `PUPPETEER_EXECUTABLE_PATH` if the auto-discovered location is wrong.

## Send a single message (UI)

1. Open the **Send Message** view.
2. Enter a phone number (digits only — no `+` or spaces).
3. Type the message and click **Send Message**.

## Broadcast from a JSON file (UI)

1. Open the **Broadcast** view.
2. Click **Choose JSON file** and pick a file shaped like [docs/broadcast-message.json](docs/broadcast-message.json).
3. Set the per-message delay (minimum 500 ms, default 3000 ms) — used to throttle sending and reduce the chance of being rate-limited.
4. Click **Start broadcast**. Each row updates live with `pending` → `sending` → `success` / `failed`.
5. Click **Stop** to halt after the current message.
6. Click **Download report** to export every row (with `broadcastStatus`, `broadcastSentAt`, `broadcastError`, `broadcastMessageId`) as JSON. Click **Download failed only** to export just the failures.

### Resume / retry semantics

- Rows already marked `broadcastStatus: "success"` in the input file are **skipped** when broadcasting. This means you can re-feed a previously downloaded report to retry only the failed entries — or feed the `*-failed-*.json` export back in directly.

### Input JSON format

The root must be an array. Each entry must have at least `customerPhone` (string) and `message` (string). Other fields are preserved through to the report. Minimal example:

```json
[
  {
    "customerName": "Jane Doe",
    "customerPhone": "+628123456789",
    "invoiceCode": "INV-0001",
    "message": "Hello Jane, your invoice INV-0001 is due."
  }
]
```

See [docs/broadcast-message.json](docs/broadcast-message.json) for a fuller example.

## HTTP API

The API and the UI are served from the same Bun process. The default port is `1111` (override with `PORT`).

### Check status

```bash
curl http://localhost:1111/api/status
```

```json
{
  "status": "ready",
  "info": {
    "phoneNumber": "1234567890",
    "platform": "android",
    "pushname": "Your Name"
  }
}
```

### Send text message

```bash
curl -X POST http://localhost:1111/api/send \
  -H "Content-Type: application/json" \
  -d '{"phone":"1234567890","message":"Hello from API!"}'
```

### Send media

```bash
curl -X POST http://localhost:1111/api/send-media \
  -H "Content-Type: application/json" \
  -d '{"phone":"1234567890","mediaPath":"/path/to/image.jpg","caption":"Check this out"}'
```

### Get all chats

```bash
curl http://localhost:1111/api/chats
```

### Get client info

```bash
curl http://localhost:1111/api/info
```

### Stream events

The UI subscribes to a Server-Sent Events stream that emits `qr`, `loading`, `authenticated`, `ready`, `auth-failure`, `disconnected`, and `error` events. You can subscribe from any client:

```bash
curl -N http://localhost:1111/api/events
```

### Endpoint summary

| Method | Endpoint          | Description                                  |
|--------|-------------------|----------------------------------------------|
| GET    | `/api/status`     | Check if WhatsApp is ready                   |
| GET    | `/api/info`       | Get client info (phone, name)                |
| POST   | `/api/send`       | Send a text message                          |
| POST   | `/api/send-media` | Send a media file                            |
| GET    | `/api/chats`      | List all chats                               |
| POST   | `/api/logout`     | Log out and re-show the QR code              |
| GET    | `/api/events`     | Server-Sent Events stream of client state    |

### Phone number format

- Digits only — no `+` symbol or spaces.
- The app strips non-digits and appends `@c.us` for individual chats.

### HTTP status codes

- `200` — Success
- `400` — Bad Request (missing parameters)
- `404` — Endpoint not found
- `500` — Internal Server Error
- `503` — Service Unavailable (client not ready)

## Project structure

```
whatsapp-broadcaster/
├── server.js     # Bun entry: starts HTTP server, opens browser, embeds UI
├── api.js        # Express HTTP API + SSE stream + static UI routes
├── whatsapp.js   # WhatsApp client + EventEmitter for state changes
├── renderer.js   # Browser UI (fetch + EventSource)
├── index.html    # UI layout
├── docs/
│   └── broadcast-message.json  # Example broadcast input
├── package.json
└── README.md
```

## Session storage

Sessions are stored locally using the `LocalAuth` strategy from `whatsapp-web.js`:

```
.wwebjs_auth/
.wwebjs_cache/
```

Both directories are gitignored.

## Disclaimer

This project uses [`whatsapp-web.js`](https://github.com/pedroslopez/whatsapp-web.js), an unofficial library that automates WhatsApp Web. It is not affiliated with or endorsed by WhatsApp. Use of automated messaging may violate WhatsApp's Terms of Service — use at your own risk.

## License

ISC

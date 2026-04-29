# WhatsApp Broadcaster

A desktop application built with Electron that provides a GUI for WhatsApp Web login, bulk broadcasting from a JSON file, and an HTTP API for sending messages programmatically.

## Features

- Desktop GUI with QR code login
- Persistent session (no need to re-scan QR on restart)
- Send single text messages from the GUI
- Bulk broadcast from a JSON file with per-row status, retry, and report export
- Send media files (images, documents) via API
- RESTful API on port 1111 for programmatic use
- Context isolation and CSP enabled

## Prerequisites

- [Node.js](https://nodejs.org/) (v18 or higher)
- [Bun](https://bun.sh/) (recommended) or npm/pnpm

## Installation

```bash
bun install
```

## Usage

### Start the application

```bash
bun start
```

On first launch:
1. Wait for the QR code to appear.
2. Open WhatsApp on your phone.
3. Go to **Settings > Linked Devices > Link a Device**.
4. Scan the QR code.

After successful login, the app shell appears with two views in the sidebar: **Send Message** and **Broadcast**.

### Send a single message (GUI)

1. Open the **Send Message** view.
2. Enter a phone number (digits only — no `+` or spaces).
3. Type the message and click **Send Message**.

### Broadcast from a JSON file (GUI)

1. Open the **Broadcast** view.
2. Click **Choose JSON file** and pick a file shaped like [docs/broadcast-message.json](docs/broadcast-message.json).
3. Set the per-message delay (minimum 500 ms, default 3000 ms) — used to throttle sending and reduce the chance of being rate-limited.
4. Click **Start broadcast**. Each row updates live with `pending` → `sending` → `success` / `failed`.
5. Click **Stop** to halt after the current message.
6. Click **Download report** to export every row (with `broadcastStatus`, `broadcastSentAt`, `broadcastError`, `broadcastMessageId`) as JSON. Click **Download failed only** to export just the failures.

#### Resume / retry semantics

- Rows already marked `broadcastStatus: "success"` in the input file are **skipped** when broadcasting. This means you can re-feed a previously downloaded report to retry only the failed entries — or feed the `*-failed-*.json` export back in directly.

#### Input JSON format

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

The API server runs on `http://localhost:1111` and is started automatically alongside the Electron app.

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

### Endpoint summary

| Method | Endpoint          | Description                       |
|--------|-------------------|-----------------------------------|
| GET    | `/api/status`     | Check if WhatsApp is ready        |
| GET    | `/api/info`       | Get client info (phone, name)     |
| POST   | `/api/send`       | Send a text message               |
| POST   | `/api/send-media` | Send a media file                 |
| GET    | `/api/chats`      | List all chats                    |

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
├── main.js       # Electron main process
├── whatsapp.js   # WhatsApp client logic
├── api.js        # HTTP API server
├── preload.js    # Context bridge for security
├── renderer.js   # UI logic (send + broadcast views)
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

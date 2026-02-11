# WhatsApp Electron App

A desktop application built with Electron that provides a GUI for WhatsApp Web login and an HTTP API for sending messages programmatically.

## Features

- 🖥️ Desktop GUI with QR code login
- 🔐 Persistent session (no need to re-scan QR on restart)
- 📩 Send text messages via GUI or HTTP API
- 📎 Send media files (images, documents)
- 👥 Get all chats
- 📡 RESTful API on port 1111
- 🔒 Secure with context isolation and CSP

## Prerequisites

- [Node.js](https://nodejs.org/) (v18 or higher)
- [Bun](https://bun.sh/) (recommended) or npm/pnpm

## Installation

```bash
# Install dependencies
bun install
```

## Usage

### Start the Application

```bash
bun start
```

On first launch:
1. Wait for the QR code to appear
2. Open WhatsApp on your phone
3. Go to **Settings > Linked Devices > Link a Device**
4. Scan the QR code

After successful login, the message panel will appear where you can:
- Enter a phone number (without + or country code)
- Type your message
- Click "Send Message"

## HTTP API

The API server runs on `http://localhost:1111`

### Check Status

```bash
curl http://localhost:1111/api/status
```

**Response:**
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

### Send Text Message

```bash
curl -X POST http://localhost:1111/api/send \
  -H "Content-Type: application/json" \
  -d '{"phone":"1234567890","message":"Hello from API!"}'
```

**Response:**
```json
{
  "success": true,
  "id": "true_1234567890@c.us_3A01234567890ABCDEF01234567890ABCDEF",
  "to": "1234567890@c.us",
  "timestamp": "2025-01-11T12:00:00.000Z"
}
```

### Send Media

```bash
curl -X POST http://localhost:1111/api/send-media \
  -H "Content-Type: application/json" \
  -d '{"phone":"1234567890","mediaPath":"/path/to/image.jpg","caption":"Check this out"}'
```

### Get All Chats

```bash
curl http://localhost:1111/api/chats
```

**Response:**
```json
[
  {
    "id": "1234567890@c.us",
    "name": "Contact Name",
    "isGroup": false,
    "unreadCount": 0
  }
]
```

### Get Client Info

```bash
curl http://localhost:1111/api/info
```

**Response:**
```json
{
  "phoneNumber": "1234567890",
  "platform": "android",
  "pushname": "Your Name"
}
```

## Project Structure

```
my-electron-app/
├── main.js       # Electron main process
├── whatsapp.js   # WhatsApp client logic
├── api.js        # HTTP API server
├── preload.js    # Context bridge for security
├── renderer.js   # UI logic
├── index.html    # UI layout
├── package.json
└── README.md
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/status` | Check if WhatsApp is ready |
| `GET` | `/api/info` | Get client info (phone, name) |
| `POST` | `/api/send` | Send text message |
| `POST` | `/api/send-media` | Send media file |
| `GET` | `/api/chats` | Get all chats |

## Phone Number Format

- Use numbers only (no + symbol, no country code prefix)
- Example: `1234567890` for a US number
- The app automatically appends `@c.us` for individual chats

## Error Responses

All endpoints return appropriate HTTP status codes:

- `200` - Success
- `400` - Bad Request (missing parameters)
- `404` - Endpoint not found
- `503` - Service Unavailable (client not ready)
- `500` - Internal Server Error

## Session Storage

Sessions are stored locally using `LocalAuth` strategy from `whatsapp-web.js`. The session data is saved in:
```
.wwebjs_auth/
wwebjs_cache/
```

## License

ISC

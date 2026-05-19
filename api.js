const express = require('express')
const bodyParser = require('body-parser')

let whatsapp = null
let apiServer = null
let staticAssets = { indexHtml: null, rendererJs: null }

const sseClients = new Set()
const lastEvents = {}

const apiApp = express()
apiApp.use(bodyParser.json({ limit: '10mb' }))

function setWhatsapp(mod) {
  whatsapp = mod
}

function setStaticAssets(assets) {
  staticAssets = { ...staticAssets, ...assets }
}

function bindEvents(emitter) {
  const names = [
    'qr',
    'ready',
    'authenticated',
    'auth-failure',
    'disconnected',
    'loading',
    'error',
  ]
  names.forEach((name) => {
    emitter.on(name, (payload) => {
      const data = payload === undefined ? null : payload
      lastEvents[name] = data
      if (name === 'ready') delete lastEvents.qr
      if (name === 'qr') {
        delete lastEvents.ready
        delete lastEvents.authenticated
      }
      if (name === 'disconnected') {
        delete lastEvents.ready
        delete lastEvents.authenticated
      }
      broadcastSSE(name, data)
    })
  })
}

function broadcastSSE(event, data) {
  const payload = `event: ${event}\ndata: ${JSON.stringify(data ?? null)}\n\n`
  for (const res of sseClients) {
    try {
      res.write(payload)
    } catch {
      // client gone; will be cleaned on close
    }
  }
}

// ==================== SSE event stream ====================

apiApp.get('/api/events', (req, res) => {
  res.setHeader('Content-Type', 'text/event-stream')
  res.setHeader('Cache-Control', 'no-cache')
  res.setHeader('Connection', 'keep-alive')
  res.setHeader('X-Accel-Buffering', 'no')
  if (typeof res.flushHeaders === 'function') res.flushHeaders()
  res.write(': connected\n\n')

  for (const [event, data] of Object.entries(lastEvents)) {
    res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`)
  }

  sseClients.add(res)
  const ping = setInterval(() => {
    try {
      res.write(': ping\n\n')
    } catch {
      // ignore
    }
  }, 25000)

  req.on('close', () => {
    clearInterval(ping)
    sseClients.delete(res)
  })
})

// ==================== HTTP API ====================

apiApp.get('/api/status', (req, res) => {
  const ready = !!(whatsapp && whatsapp.isReady())
  res.json({
    status: ready ? 'ready' : 'not_ready',
    info: ready ? whatsapp.getClientInfo() : null,
  })
})

apiApp.get('/api/info', (req, res) => {
  const info = whatsapp && whatsapp.getClientInfo()
  if (!info) return res.status(503).json({ error: 'Client not ready' })
  res.json(info)
})

apiApp.post('/api/send', async (req, res) => {
  if (!whatsapp || !whatsapp.isReady()) {
    return res.status(503).json({ error: 'Client not ready' })
  }
  const { phone, message } = req.body || {}
  if (!phone || !message) {
    return res.status(400).json({ error: 'Missing phone or message' })
  }
  try {
    const result = await whatsapp.sendMessage(phone, message)
    res.json({ ...result, timestamp: new Date().toISOString() })
  } catch (err) {
    res.status(500).json({ error: err.message || String(err) })
  }
})

apiApp.post('/api/send-media', async (req, res) => {
  if (!whatsapp || !whatsapp.isReady()) {
    return res.status(503).json({ error: 'Client not ready' })
  }
  const { phone, mediaPath, caption } = req.body || {}
  if (!phone || !mediaPath) {
    return res.status(400).json({ error: 'Missing phone or mediaPath' })
  }
  try {
    const result = await whatsapp.sendMedia(phone, mediaPath, caption)
    res.json({ ...result, timestamp: new Date().toISOString() })
  } catch (err) {
    res.status(500).json({ error: err.message || String(err) })
  }
})

apiApp.get('/api/chats', async (req, res) => {
  if (!whatsapp || !whatsapp.isReady()) {
    return res.status(503).json({ error: 'Client not ready' })
  }
  try {
    res.json(await whatsapp.getChats())
  } catch (err) {
    res.status(500).json({ error: err.message || String(err) })
  }
})

apiApp.post('/api/logout', async (req, res) => {
  if (!whatsapp) return res.status(503).json({ error: 'Client not initialized' })
  try {
    await whatsapp.logout()
    delete lastEvents.ready
    delete lastEvents.authenticated
    await whatsapp.reinitialize()
    res.json({ success: true })
  } catch (err) {
    res.status(500).json({ error: err.message || String(err) })
  }
})

// ==================== Static UI ====================

apiApp.get('/', (req, res) => {
  res.set('Content-Type', 'text/html; charset=utf-8')
  if (staticAssets.indexHtml) {
    res.send(staticAssets.indexHtml)
  } else {
    res.sendFile(require('path').join(__dirname, 'index.html'))
  }
})

apiApp.get('/renderer.js', (req, res) => {
  res.set('Content-Type', 'application/javascript; charset=utf-8')
  if (staticAssets.rendererJs) {
    res.send(staticAssets.rendererJs)
  } else {
    res.sendFile(require('path').join(__dirname, 'renderer.js'))
  }
})

// 404 for unknown /api endpoints (must come after the routes above)
apiApp.use('/api', (req, res) => {
  res.status(404).json({ error: 'Endpoint not found' })
})

function start(port = 1111) {
  if (apiServer) {
    console.log('API server already running')
    return
  }
  apiServer = apiApp.listen(port, () => {
    console.log(`HTTP server listening on http://localhost:${port}`)
  })
}

function stop() {
  if (apiServer) {
    apiServer.close(() => {
      console.log('HTTP server stopped')
      apiServer = null
    })
  }
}

module.exports = {
  setWhatsapp,
  setStaticAssets,
  bindEvents,
  start,
  stop,
}

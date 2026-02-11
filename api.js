const express = require('express')
const bodyParser = require('body-parser')

let client = null
let apiServer = null

// Create Express app
const apiApp = express()
apiApp.use(bodyParser.json())

// Set the WhatsApp client (called from main.js)
function setClient(whatsappClient) {
  client = whatsappClient
}

// ==================== HTTP API Endpoints ====================

// Check if WhatsApp client is ready
apiApp.get('/api/status', (req, res) => {
  const isReady = client && client.info !== undefined
  res.json({
    status: isReady ? 'ready' : 'not_ready',
    info: isReady ? {
      phoneNumber: client.info.wid.user,
      platform: client.info.platform,
      pushname: client.info.pushname
    } : null
  })
})

// Get client info
apiApp.get('/api/info', (req, res) => {
  if (!client || !client.info) {
    return res.status(503).json({ error: 'Client not ready' })
  }

  res.json({
    phoneNumber: client.info.wid.user,
    platform: client.info.platform,
    pushname: client.info.pushname
  })
})

// Send text message
apiApp.post('/api/send', async (req, res) => {
  if (!client || !client.info) {
    return res.status(503).json({ error: 'Client not ready' })
  }

  const { phone, message } = req.body

  if (!phone || !message) {
    return res.status(400).json({ error: 'Missing phone or message' })
  }

  try {
    const formattedNumber = phone.replace(/\D/g, '') + '@c.us'
    const result = await client.sendMessage(formattedNumber, message)

    res.json({
      success: true,
      id: result.id._serialized,
      to: result.to,
      timestamp: new Date().toISOString()
    })
  } catch (err) {
    res.status(500).json({ error: err.message })
  }
})

// Send media message
apiApp.post('/api/send-media', async (req, res) => {
  if (!client || !client.info) {
    return res.status(503).json({ error: 'Client not ready' })
  }

  const { phone, mediaPath, caption } = req.body

  if (!phone || !mediaPath) {
    return res.status(400).json({ error: 'Missing phone or mediaPath' })
  }

  try {
    const formattedNumber = phone.replace(/\D/g, '') + '@c.us'
    const { MessageMedia } = require('whatsapp-web.js')
    const media = MessageMedia.fromFilePath(mediaPath)

    const result = await client.sendMessage(formattedNumber, media, { caption })

    res.json({
      success: true,
      id: result.id._serialized,
      to: result.to,
      timestamp: new Date().toISOString()
    })
  } catch (err) {
    res.status(500).json({ error: err.message })
  }
})

// Get all chats
apiApp.get('/api/chats', async (req, res) => {
  if (!client || !client.info) {
    return res.status(503).json({ error: 'Client not ready' })
  }

  try {
    const chats = await client.getChats()
    res.json(chats.map(chat => ({
      id: chat.id._serialized,
      name: chat.name,
      isGroup: chat.isGroup,
      unreadCount: chat.unreadCount
    })))
  } catch (err) {
    res.status(500).json({ error: err.message })
  }
})

// 404 handler
apiApp.use((req, res) => {
  res.status(404).json({ error: 'Endpoint not found' })
})

// Start API server
function start(port = 1111) {
  if (apiServer) {
    console.log('API server already running')
    return
  }

  apiServer = apiApp.listen(port, () => {
    console.log(`WhatsApp API server running on http://localhost:${port}`)
  })
}

// Stop API server
function stop() {
  if (apiServer) {
    apiServer.close(() => {
      console.log('API server stopped')
      apiServer = null
    })
  }
}

module.exports = {
  setClient,
  start,
  stop
}

const { Client, LocalAuth } = require('whatsapp-web.js')
const QRCode = require('qrcode')

let client = null
let window = null

// Set the window reference for sending events to renderer
function setWindow(win) {
  window = win
}

// Initialize WhatsApp client
async function initialize() {
  if (client) {
    return client
  }

  client = new Client({
    authStrategy: new LocalAuth(),
    puppeteer: {
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox']
    }
  })

  // QR code event
  client.on('qr', async (qr) => {
    if (window && window.webContents.isLoading() === false) {
      const qrDataUrl = await QRCode.toDataURL(qr, { width: 200 })
      window.webContents.send('qr', qrDataUrl)
    }
  })

  // Ready event
  client.on('ready', () => {
    if (window) window.webContents.send('ready')
  })

  // Authenticated event
  client.on('authenticated', () => {
    if (window) window.webContents.send('authenticated')
  })

  // Auth failure event
  client.on('auth_failure', (msg) => {
    if (window) window.webContents.send('auth-failure', msg)
  })

  // Disconnected event
  client.on('disconnected', (reason) => {
    if (window) window.webContents.send('disconnected', reason)
  })

  // Loading screen event
  client.on('loading_screen', (percent, message) => {
    if (window) window.webContents.send('loading', { percent, message })
  })

  try {
    await client.initialize()
    return client
  } catch (err) {
    console.error('WhatsApp init error:', err)
    if (window) window.webContents.send('error', err.message)
    throw err
  }
}

// Get the WhatsApp client instance
function getClient() {
  return client
}

// Check if client is ready
function isReady() {
  return client && client.info !== undefined
}

// Get client info
function getClientInfo() {
  if (!client || !client.info) {
    return null
  }

  return {
    phoneNumber: client.info.wid.user,
    platform: client.info.platform,
    pushname: client.info.pushname
  }
}

// Send text message
async function sendMessage(phoneNumber, message) {
  if (!client) {
    throw new Error('Client not initialized')
  }

  const formattedNumber = phoneNumber.replace(/\D/g, '') + '@c.us'
  const result = await client.sendMessage(formattedNumber, message)

  return {
    success: true,
    id: result.id._serialized,
    to: result.to
  }
}

// Send media message
async function sendMedia(phoneNumber, mediaPath, caption) {
  if (!client) {
    throw new Error('Client not initialized')
  }

  const formattedNumber = phoneNumber.replace(/\D/g, '') + '@c.us'
  const { MessageMedia } = require('whatsapp-web.js')
  const media = MessageMedia.fromFilePath(mediaPath)

  const result = await client.sendMessage(formattedNumber, media, { caption })

  return {
    success: true,
    id: result.id._serialized,
    to: result.to
  }
}

// Get all chats
async function getChats() {
  if (!client) {
    throw new Error('Client not initialized')
  }

  const chats = await client.getChats()
  return chats.map(chat => ({
    id: chat.id._serialized,
    name: chat.name,
    isGroup: chat.isGroup,
    unreadCount: chat.unreadCount
  }))
}

// Logout
async function logout() {
  if (!client) {
    return
  }

  await client.logout()
}

// Re-initialize after logout
async function reinitialize() {
  if (client) {
    await client.initialize()
  }
}

// Destroy client
function destroy() {
  if (client) {
    client.destroy()
    client = null
  }
}

module.exports = {
  setWindow,
  initialize,
  getClient,
  isReady,
  getClientInfo,
  sendMessage,
  sendMedia,
  getChats,
  logout,
  reinitialize,
  destroy
}

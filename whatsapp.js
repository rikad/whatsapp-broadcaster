const { Client, LocalAuth, MessageMedia } = require('whatsapp-web.js')
const QRCode = require('qrcode')
const { EventEmitter } = require('node:events')

const events = new EventEmitter()
events.setMaxListeners(50)

let client = null

async function initialize() {
  if (client) {
    return client
  }

  const puppeteerOptions = {
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  }
  if (process.env.PUPPETEER_EXECUTABLE_PATH) {
    puppeteerOptions.executablePath = process.env.PUPPETEER_EXECUTABLE_PATH
  }

  client = new Client({
    authStrategy: new LocalAuth(),
    puppeteer: puppeteerOptions,
  })

  client.on('qr', async (qr) => {
    try {
      const qrDataUrl = await QRCode.toDataURL(qr, { width: 200 })
      events.emit('qr', qrDataUrl)
    } catch (err) {
      events.emit('error', `QR generation failed: ${err.message}`)
    }
  })

  client.on('ready', () => events.emit('ready'))
  client.on('authenticated', () => events.emit('authenticated'))
  client.on('auth_failure', (msg) => events.emit('auth-failure', msg))
  client.on('disconnected', (reason) => events.emit('disconnected', reason))
  client.on('loading_screen', (percent, message) =>
    events.emit('loading', { percent, message })
  )

  try {
    await client.initialize()
    return client
  } catch (err) {
    console.error('WhatsApp init error:', err)
    events.emit('error', err.message)
    throw err
  }
}

function getClient() {
  return client
}

function isReady() {
  return !!(client && client.info)
}

function getClientInfo() {
  if (!client || !client.info) {
    return null
  }
  return {
    phoneNumber: client.info.wid.user,
    platform: client.info.platform,
    pushname: client.info.pushname,
  }
}

async function sendMessage(phoneNumber, message) {
  if (!client) {
    throw new Error('Client not initialized')
  }
  const formattedNumber = phoneNumber.replace(/\D/g, '') + '@c.us'
  const result = await client.sendMessage(formattedNumber, message)
  return {
    success: true,
    id: result.id._serialized,
    to: result.to,
  }
}

async function sendMedia(phoneNumber, mediaPath, caption) {
  if (!client) {
    throw new Error('Client not initialized')
  }
  const formattedNumber = phoneNumber.replace(/\D/g, '') + '@c.us'
  const media = MessageMedia.fromFilePath(mediaPath)
  const result = await client.sendMessage(formattedNumber, media, { caption })
  return {
    success: true,
    id: result.id._serialized,
    to: result.to,
  }
}

async function getChats() {
  if (!client) {
    throw new Error('Client not initialized')
  }
  const chats = await client.getChats()
  return chats.map((chat) => ({
    id: chat.id._serialized,
    name: chat.name,
    isGroup: chat.isGroup,
    unreadCount: chat.unreadCount,
  }))
}

async function logout() {
  if (!client) return
  await client.logout()
}

async function reinitialize() {
  if (client) {
    await client.initialize()
  }
}

function destroy() {
  if (client) {
    client.destroy()
    client = null
  }
}

module.exports = {
  events,
  initialize,
  getClient,
  isReady,
  getClientInfo,
  sendMessage,
  sendMedia,
  getChats,
  logout,
  reinitialize,
  destroy,
}

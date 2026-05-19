const { Client, LocalAuth, MessageMedia } = require('whatsapp-web.js')
const QRCode = require('qrcode')
const { EventEmitter } = require('node:events')
const fs = require('node:fs')
const path = require('node:path')

const events = new EventEmitter()
events.setMaxListeners(50)

let client = null

function findSystemChrome() {
  const candidates = []
  if (process.platform === 'win32') {
    const programFiles = process.env['PROGRAMFILES'] || 'C:\\Program Files'
    const programFilesX86 = process.env['PROGRAMFILES(X86)'] || 'C:\\Program Files (x86)'
    const localAppData = process.env['LOCALAPPDATA'] || ''
    candidates.push(
      path.join(programFiles, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      path.join(programFilesX86, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      localAppData && path.join(localAppData, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      path.join(programFiles, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
      path.join(programFilesX86, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
    )
  } else if (process.platform === 'darwin') {
    candidates.push(
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing',
      '/Applications/Chromium.app/Contents/MacOS/Chromium',
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    )
  } else {
    candidates.push(
      '/usr/bin/google-chrome',
      '/usr/bin/google-chrome-stable',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser',
      '/snap/bin/chromium',
      '/usr/bin/microsoft-edge',
    )
  }

  for (const p of candidates) {
    if (!p) continue
    try {
      if (fs.existsSync(p)) return p
    } catch {
      // ignore
    }
  }
  return null
}

async function initialize() {
  if (client) {
    return client
  }

  const puppeteerOptions = {
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  }

  const executablePath = process.env.PUPPETEER_EXECUTABLE_PATH || findSystemChrome()
  if (executablePath) {
    puppeteerOptions.executablePath = executablePath
    console.log(`Using Chrome at: ${executablePath}`)
  } else {
    const msg = [
      'Could not find Google Chrome / Chromium on this machine.',
      'WhatsApp Broadcaster needs a Chromium-based browser to drive WhatsApp Web.',
      '',
      'Fix one of:',
      '  - Install Google Chrome: https://www.google.com/chrome/',
      '  - Or set PUPPETEER_EXECUTABLE_PATH to your Chrome/Chromium executable',
      '    e.g. PUPPETEER_EXECUTABLE_PATH="C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"',
    ].join('\n')
    console.error(msg)
    events.emit('error', 'Chrome / Chromium not found. Install Chrome or set PUPPETEER_EXECUTABLE_PATH.')
    throw new Error('Chrome not found')
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

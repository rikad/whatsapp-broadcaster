const { app, BrowserWindow, ipcMain } = require('electron')
const { Client, LocalAuth } = require('whatsapp-web.js')
const QRCode = require('qrcode')
const path = require('path')

let win
let client

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection:', reason)
  if (win) {
    win.webContents.send('error', reason?.message || String(reason))
  }
})

const createWindow = () => {
  win = new BrowserWindow({
    width: 400,
    height: 500,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  })

  win.loadFile('index.html')
}

const initWhatsApp = async () => {
  client = new Client({
    authStrategy: new LocalAuth(),
    puppeteer: {
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox']
    }
  })

  client.on('qr', async (qr) => {
    if (win && win.webContents.isLoading() === false) {
      const qrDataUrl = await QRCode.toDataURL(qr, { width: 200 })
      win.webContents.send('qr', qrDataUrl)
    }
  })

  client.on('ready', () => {
    if (win) win.webContents.send('ready')
  })

  client.on('authenticated', () => {
    if (win) win.webContents.send('authenticated')
  })

  client.on('auth_failure', (msg) => {
    if (win) win.webContents.send('auth-failure', msg)
  })

  client.on('disconnected', (reason) => {
    if (win) win.webContents.send('disconnected', reason)
  })

  client.on('loading_screen', (percent, message) => {
    if (win) win.webContents.send('loading', { percent, message })
  })

  try {
    await client.initialize()
  } catch (err) {
    console.error('WhatsApp init error:', err)
    if (win) win.webContents.send('error', err.message)
  }
}

app.whenReady().then(() => {
  createWindow()
  initWhatsApp()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (client) {
    client.destroy()
  }
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

ipcMain.handle('logout', async () => {
  if (client) {
    await client.logout()
    client.initialize()
  }
})

// Check if client is ready
ipcMain.handle('isReady', () => {
  return client && client.info !== undefined
})

// Send text message
ipcMain.handle('sendMessage', async (event, phoneNumber, message) => {
  if (!client) {
    throw new Error('Client not initialized')
  }

  // Format phone number (remove non-numeric chars, ensure format)
  const formattedNumber = phoneNumber.replace(/\D/g, '') + '@c.us'

  const result = await client.sendMessage(formattedNumber, message)
  return {
    success: true,
    id: result.id._serialized,
    to: result.to
  }
})

// Send message with media
ipcMain.handle('sendMedia', async (event, phoneNumber, mediaPath, caption) => {
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
})

// Get all chats
ipcMain.handle('getChats', async () => {
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
})

// Get client info (phone number, etc)
ipcMain.handle('getClientInfo', async () => {
  if (!client || !client.info) {
    return null
  }

  return {
    phoneNumber: client.info.wid.user,
    platform: client.info.platform,
    pushname: client.info.pushname
  }
})

const { app, BrowserWindow, ipcMain } = require('electron')
const api = require('./api')
const whatsapp = require('./whatsapp')
const path = require('path')

let win

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection:', reason)
  if (win) {
    win.webContents.send('error', reason?.message || String(reason))
  }
})

const createWindow = () => {
  win = new BrowserWindow({
    width: 1100,
    height: 760,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  })

  win.loadFile('index.html')
  whatsapp.setWindow(win)
}

app.whenReady().then(async () => {
  createWindow()

  // Initialize WhatsApp
  const client = await whatsapp.initialize()

  // Set up API and HTTP server
  api.setClient(client)
  api.start()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  app.quit()
})

// IPC handlers for renderer process
ipcMain.handle('logout', async () => {
  await whatsapp.logout()
  await whatsapp.reinitialize()
})

ipcMain.handle('isReady', () => {
  return whatsapp.isReady()
})

ipcMain.handle('sendMessage', async (event, phoneNumber, message) => {
  return await whatsapp.sendMessage(phoneNumber, message)
})

ipcMain.handle('sendMedia', async (event, phoneNumber, mediaPath, caption) => {
  return await whatsapp.sendMedia(phoneNumber, mediaPath, caption)
})

ipcMain.handle('getChats', async () => {
  return await whatsapp.getChats()
})

ipcMain.handle('getClientInfo', async () => {
  return whatsapp.getClientInfo()
})

const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('whatsapp', {
  // Events
  onQr: (callback) => ipcRenderer.on('qr', (event, qr) => callback(qr)),
  onReady: (callback) => ipcRenderer.on('ready', () => callback()),
  onAuthenticated: (callback) => ipcRenderer.on('authenticated', () => callback()),
  onAuthFailure: (callback) => ipcRenderer.on('auth-failure', (event, msg) => callback(msg)),
  onDisconnected: (callback) => ipcRenderer.on('disconnected', (event, reason) => callback(reason)),
  onLoading: (callback) => ipcRenderer.on('loading', (event, data) => callback(data)),
  onError: (callback) => ipcRenderer.on('error', (event, msg) => callback(msg)),

  // Actions
  logout: () => ipcRenderer.invoke('logout'),
  isReady: () => ipcRenderer.invoke('isReady'),
  sendMessage: (phoneNumber, message) => ipcRenderer.invoke('sendMessage', phoneNumber, message),
  sendMedia: (phoneNumber, mediaPath, caption) => ipcRenderer.invoke('sendMedia', phoneNumber, mediaPath, caption),
  getChats: () => ipcRenderer.invoke('getChats'),
  getClientInfo: () => ipcRenderer.invoke('getClientInfo')
})

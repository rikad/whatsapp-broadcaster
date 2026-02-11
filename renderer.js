// Elements
const loginSection = document.getElementById('login-section')
const messagePanel = document.getElementById('message-panel')
const qrcode = document.getElementById('qrcode')
const status = document.getElementById('status')
const logoutBtn = document.getElementById('logout-btn')
const phoneInput = document.getElementById('phone')
const messageInput = document.getElementById('message')
const sendBtn = document.getElementById('send-btn')
const resultDiv = document.getElementById('result')
const clientInfo = document.getElementById('client-info')

let qrImage = null

// Display QR code from data URL
function displayQRCode(dataUrl) {
  if (qrImage) {
    qrImage.remove()
  }
  qrImage = document.createElement('img')
  qrImage.src = dataUrl
  qrImage.alt = 'WhatsApp QR Code'
  qrImage.width = 200
  qrImage.height = 200
  qrcode.innerHTML = ''
  qrcode.appendChild(qrImage)
}

// Show message panel
async function showMessagePanel() {
  loginSection.style.display = 'none'
  messagePanel.style.display = 'block'

  // Get and display client info
  const info = await window.whatsapp.getClientInfo()
  if (info) {
    clientInfo.textContent = `Logged in as: ${info.pushname} (+${info.phoneNumber})`
  }
}

// Show login section
function showLoginSection() {
  loginSection.style.display = 'block'
  messagePanel.style.display = 'none'
}

// Handle loading event
window.whatsapp.onLoading((data) => {
  status.textContent = `${data.message || 'Loading'}... ${data.percent}%`
  status.className = ''
})

// Handle QR code event
window.whatsapp.onQr((qrDataUrl) => {
  showLoginSection()
  displayQRCode(qrDataUrl)
  status.textContent = 'Scan the QR code with WhatsApp'
  status.className = ''
  logoutBtn.style.display = 'none'
})

// Handle ready event
window.whatsapp.onReady(() => {
  status.textContent = 'Connected to WhatsApp!'
  status.className = 'success'
  qrcode.innerHTML = '<p style="font-size: 48px;">✓</p>'
  logoutBtn.style.display = 'block'
  showMessagePanel()
})

// Handle authenticated event
window.whatsapp.onAuthenticated(() => {
  status.textContent = 'Authenticated! Loading...'
  status.className = 'success'
})

// Handle auth failure event
window.whatsapp.onAuthFailure((msg) => {
  status.textContent = `Authentication failed: ${msg}`
  status.className = 'error'
})

// Handle disconnected event
window.whatsapp.onDisconnected((reason) => {
  status.textContent = `Disconnected: ${reason}`
  status.className = 'error'
  logoutBtn.style.display = 'none'
  showLoginSection()
})

// Handle error event
window.whatsapp.onError((msg) => {
  status.textContent = `Error: ${msg}`
  status.className = 'error'
})

// Logout function
async function logout() {
  await window.whatsapp.logout()
  status.textContent = 'Logging out...'
  status.className = ''
  logoutBtn.style.display = 'none'
  showLoginSection()
}

// Send message function
async function sendMessage() {
  const phone = phoneInput.value.trim()
  const message = messageInput.value.trim()

  if (!phone || !message) {
    showResult('Please enter both phone number and message', 'error')
    return
  }

  sendBtn.disabled = true
  resultDiv.style.display = 'none'

  try {
    const result = await window.whatsapp.sendMessage(phone, message)
    showResult(`Message sent successfully! ID: ${result.id}`, 'success')
    phoneInput.value = ''
    messageInput.value = ''
  } catch (err) {
    showResult(`Failed to send message: ${err.message || err}`, 'error')
  } finally {
    sendBtn.disabled = false
  }
}

// Show result message
function showResult(message, type) {
  resultDiv.textContent = message
  resultDiv.className = type
  resultDiv.style.display = 'block'
}

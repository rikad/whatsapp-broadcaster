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
const userAvatar = document.getElementById('user-avatar')
const userName = document.getElementById('user-name')
const userPhone = document.getElementById('user-phone')
const pageTitle = document.getElementById('page-title')
const navItems = document.querySelectorAll('.nav-item')
const views = {
  send: document.getElementById('view-send'),
  broadcast: document.getElementById('view-broadcast'),
}
const viewTitles = {
  send: 'Send Message',
  broadcast: 'Broadcast',
}

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
  messagePanel.style.display = 'flex'

  // Get and display client info in the sidebar
  const info = await window.whatsapp.getClientInfo()
  if (info) {
    const name = info.pushname || 'WhatsApp User'
    userName.textContent = name
    userPhone.textContent = `+${info.phoneNumber}`
    userAvatar.textContent = (name.trim()[0] || 'W').toUpperCase()
  }
}

// Show login section
function showLoginSection() {
  loginSection.style.display = 'flex'
  messagePanel.style.display = 'none'
}

// Switch active view in the app shell
function switchView(viewName) {
  if (!views[viewName]) return
  Object.entries(views).forEach(([name, el]) => {
    el.classList.toggle('hidden', name !== viewName)
  })
  navItems.forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.view === viewName)
  })
  pageTitle.textContent = viewTitles[viewName] || ''
}

navItems.forEach((btn) => {
  btn.addEventListener('click', () => switchView(btn.dataset.view))
})

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

// ==================== Broadcast (bulk send from JSON) ====================

const fileInput = document.getElementById('json-file')
const fileSummary = document.getElementById('file-summary')
const delayInput = document.getElementById('delay-ms')
const startBroadcastBtn = document.getElementById('start-broadcast-btn')
const stopBroadcastBtn = document.getElementById('stop-broadcast-btn')
const downloadReportBtn = document.getElementById('download-report-btn')
const downloadFailedBtn = document.getElementById('download-failed-btn')
const broadcastProgress = document.getElementById('broadcast-progress')
const broadcastTableWrap = document.getElementById('broadcast-table-wrap')
const broadcastTbody = document.getElementById('broadcast-tbody')

// rows[i] = original entry + broadcastStatus / broadcastError / broadcastSentAt / broadcastMessageId
let broadcastRows = []
let broadcastSourceName = 'broadcast-report'
let stopRequested = false
let isBroadcasting = false

fileInput.addEventListener('change', handleFileChange)
startBroadcastBtn.addEventListener('click', startBroadcast)
stopBroadcastBtn.addEventListener('click', stopBroadcast)
downloadReportBtn.addEventListener('click', () => downloadReport(false))
downloadFailedBtn.addEventListener('click', () => downloadReport(true))
sendBtn.addEventListener('click', sendMessage)
logoutBtn.addEventListener('click', logout)
document.getElementById('logout-btn-bottom').addEventListener('click', logout)

function handleFileChange(event) {
  const file = event.target.files && event.target.files[0]
  if (!file) return

  broadcastSourceName = file.name.replace(/\.json$/i, '') || 'broadcast-report'

  const reader = new FileReader()
  reader.onload = (e) => {
    try {
      const parsed = JSON.parse(e.target.result)
      if (!Array.isArray(parsed)) {
        throw new Error('JSON root must be an array')
      }
      const invalid = parsed.findIndex(
        (row) => !row || typeof row.customerPhone !== 'string' || typeof row.message !== 'string'
      )
      if (invalid !== -1) {
        throw new Error(`Entry #${invalid + 1} is missing customerPhone or message`)
      }

      broadcastRows = parsed.map((row) => ({
        ...row,
        broadcastStatus: row.broadcastStatus === 'success' ? 'success' : 'pending',
        broadcastError: row.broadcastStatus === 'success' ? row.broadcastError || null : null,
        broadcastSentAt: row.broadcastStatus === 'success' ? row.broadcastSentAt || null : null,
        broadcastMessageId: row.broadcastStatus === 'success' ? row.broadcastMessageId || null : null,
      }))

      const successCount = broadcastRows.filter((r) => r.broadcastStatus === 'success').length
      const pendingCount = broadcastRows.length - successCount
      fileSummary.textContent =
        `Loaded ${broadcastRows.length} entries — ${pendingCount} to send, ${successCount} already marked success (will be skipped).`
      fileSummary.style.color = '#8696a0'

      renderBroadcastTable()
      broadcastTableWrap.style.display = 'block'
      startBroadcastBtn.disabled = pendingCount === 0
      downloadReportBtn.disabled = false
      downloadFailedBtn.disabled = !broadcastRows.some((r) => r.broadcastStatus === 'failed')
    } catch (err) {
      broadcastRows = []
      fileSummary.textContent = `Invalid JSON: ${err.message}`
      fileSummary.style.color = '#ea4335'
      broadcastTableWrap.style.display = 'none'
      startBroadcastBtn.disabled = true
      downloadReportBtn.disabled = true
      downloadFailedBtn.disabled = true
    }
  }
  reader.readAsText(file)
}

function renderBroadcastTable() {
  broadcastTbody.innerHTML = ''
  broadcastRows.forEach((row, idx) => {
    const tr = document.createElement('tr')
    tr.id = `bc-row-${idx}`
    tr.innerHTML = `
      <td>${idx + 1}</td>
      <td>${escapeHtml(row.customerName || '')}</td>
      <td>${escapeHtml(row.customerPhone || '')}</td>
      <td>${escapeHtml(row.invoiceCode || '')}</td>
      <td><span class="badge badge-${row.broadcastStatus}">${row.broadcastStatus}</span></td>
      <td>${row.broadcastSentAt ? formatTime(row.broadcastSentAt) : '—'}</td>
      <td class="error-cell" title="${escapeHtml(row.broadcastError || '')}">${escapeHtml(row.broadcastError || '')}</td>
    `
    broadcastTbody.appendChild(tr)
  })
}

function updateBroadcastRow(idx) {
  const row = broadcastRows[idx]
  const tr = document.getElementById(`bc-row-${idx}`)
  if (!tr) return
  tr.innerHTML = `
    <td>${idx + 1}</td>
    <td>${escapeHtml(row.customerName || '')}</td>
    <td>${escapeHtml(row.customerPhone || '')}</td>
    <td>${escapeHtml(row.invoiceCode || '')}</td>
    <td><span class="badge badge-${row.broadcastStatus}">${row.broadcastStatus}</span></td>
    <td>${row.broadcastSentAt ? formatTime(row.broadcastSentAt) : '—'}</td>
    <td class="error-cell" title="${escapeHtml(row.broadcastError || '')}">${escapeHtml(row.broadcastError || '')}</td>
  `
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function formatTime(iso) {
  try {
    return new Date(iso).toLocaleTimeString()
  } catch {
    return iso
  }
}

async function startBroadcast() {
  if (isBroadcasting || broadcastRows.length === 0) return

  const delayMs = Math.max(500, parseInt(delayInput.value, 10) || 3000)
  isBroadcasting = true
  stopRequested = false
  startBroadcastBtn.disabled = true
  stopBroadcastBtn.disabled = false
  fileInput.disabled = true
  delayInput.disabled = true

  let successCount = 0
  let failCount = 0
  let skipCount = 0

  broadcastProgress.classList.add('active')

  for (let i = 0; i < broadcastRows.length; i++) {
    if (stopRequested) break

    const row = broadcastRows[i]
    if (row.broadcastStatus === 'success') {
      skipCount++
      continue
    }

    row.broadcastStatus = 'sending'
    row.broadcastError = null
    updateBroadcastRow(i)
    broadcastProgress.textContent =
      `Sending ${i + 1} of ${broadcastRows.length}... (success: ${successCount}, failed: ${failCount}, skipped: ${skipCount})`

    try {
      const result = await window.whatsapp.sendMessage(row.customerPhone, row.message)
      row.broadcastStatus = 'success'
      row.broadcastMessageId = result && result.id ? result.id : null
      row.broadcastSentAt = new Date().toISOString()
      row.broadcastError = null
      successCount++
    } catch (err) {
      row.broadcastStatus = 'failed'
      row.broadcastError = err && err.message ? err.message : String(err)
      row.broadcastSentAt = new Date().toISOString()
      failCount++
    }
    updateBroadcastRow(i)
    downloadFailedBtn.disabled = !broadcastRows.some((r) => r.broadcastStatus === 'failed')

    // Don't sleep after the last item
    const isLast = i === broadcastRows.length - 1
    if (!isLast && !stopRequested) {
      await sleep(delayMs)
    }
  }

  isBroadcasting = false
  stopBroadcastBtn.disabled = true
  fileInput.disabled = false
  delayInput.disabled = false
  startBroadcastBtn.disabled = !broadcastRows.some((r) => r.broadcastStatus !== 'success')

  const stoppedNote = stopRequested ? ' (stopped)' : ''
  broadcastProgress.textContent =
    `Done${stoppedNote}. Success: ${successCount}, Failed: ${failCount}, Skipped: ${skipCount}.`
}

function stopBroadcast() {
  if (!isBroadcasting) return
  stopRequested = true
  stopBroadcastBtn.disabled = true
  broadcastProgress.textContent += ' — stopping after current message...'
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function downloadReport(failedOnly) {
  if (broadcastRows.length === 0) return
  const data = failedOnly
    ? broadcastRows.filter((r) => r.broadcastStatus === 'failed')
    : broadcastRows

  if (data.length === 0) {
    showResult('No entries to export.', 'error')
    return
  }

  const json = JSON.stringify(data, null, 2)
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  const stamp = new Date().toISOString().replace(/[:.]/g, '-')
  const suffix = failedOnly ? '-failed' : '-report'
  a.href = url
  a.download = `${broadcastSourceName}${suffix}-${stamp}.json`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

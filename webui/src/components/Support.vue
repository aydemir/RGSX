<script setup>
import { ref } from 'vue'
import { t, STRINGS, getLocale } from '../i18n.js'

const locale = ref(getLocale())
const busy = ref(false)
const errorMsg = ref('')
const warnMsg = ref('')
const infoMsg = ref('')

function loc(key) {
  const table = STRINGS[locale.value] || STRINGS.tr
  return (table && table[key]) || (STRINGS.en && STRINGS.en[key]) || STRINGS.tr[key] || key
}

function filenameFrom(disposition) {
  if (!disposition) return 'rgsx_support.zip'
  const m = /filename="?([^";]+)"?/.exec(disposition)
  return m && m[1] ? m[1] : 'rgsx_support.zip'
}

async function generate() {
  if (busy.value) return
  busy.value = true
  errorMsg.value = ''
  warnMsg.value = ''
  infoMsg.value = ''

  try {
    const response = await fetch('/api/support', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    })

    if (!response.ok) {
      let detail = ''
      try {
        const data = await response.json()
        detail = data && (data.error || data.message) ? String(data.error || data.message) : ''
      } catch (_) { /* yanıt JSON değil */ }
      if (!detail) {
        try { detail = (await response.text()).slice(0, 200) } catch (_) {}
      }
      errorMsg.value = loc('support_error') + (detail || ('HTTP ' + response.status))
      return
    }

    const blob = await response.blob()
    if (!blob || blob.size === 0) {
      warnMsg.value = loc('support_empty')
      return
    }

    const url = window.URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filenameFrom(response.headers.get('Content-Disposition'))
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    window.URL.revokeObjectURL(url)

    infoMsg.value = loc('support_message')
  } catch (err) {
    errorMsg.value = loc('support_error') + (err && err.message ? err.message : String(err))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="support-wrap">
    <button
      class="support-btn"
      :disabled="busy"
      @click="generate"
      :title="loc('support')"
    >{{ busy ? '⏳ ' + loc('support_generating') : '🆘 ' + loc('support') }}</button>

    <p v-if="errorMsg" class="support-msg support-err">{{ errorMsg }}</p>
    <p v-else-if="warnMsg" class="support-msg support-warn">{{ warnMsg }}</p>
    <p v-else-if="infoMsg" class="support-msg support-ok">{{ infoMsg }}</p>
  </div>
</template>

<style scoped>
.support-wrap { display: inline-flex; flex-direction: column; gap: 4px; }
.support-btn {
  background: #007bff;
  color: #fff;
  border: 0;
  border-radius: 6px;
  padding: 0 12px;
  height: 32px;
  font-size: 14px;
  cursor: pointer;
  white-space: nowrap;
}
.support-btn:hover { background: #0069d9; }
.support-btn:disabled { background: #17a2b8; cursor: progress; }

.support-msg {
  margin: 0;
  font-size: 12px;
  max-width: 320px;
  padding: 6px 10px;
  border-radius: 6px;
}
.support-err { color: #dc3545; background: rgba(220, 53, 69, 0.12); border: 1px solid #dc3545; }
.support-warn { color: #ffc107; background: rgba(255, 193, 7, 0.12); border: 1px solid #ffc107; }
.support-ok { color: #28a745; background: rgba(40, 167, 69, 0.12); border: 1px solid #28a745; }
</style>

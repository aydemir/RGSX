<script setup>
import { ref, onMounted } from 'vue'
import { apiGet, apiPost } from '../api.js'

// gap-18 — qBittorrent yönetim paneli (hibrit-mod scope'lu).
// ZORUNLU: librqbit/saf-Rust modunda backend `mode:'embedded'` (veya available:false, url:'')
// placeholder döner; panel bunu HATA gibi göstermez, nötr "kullanımda değil" mesajı verir.
const PY = 'RGSX_TORRENT_ENGINE=python gerekli'

const loading = ref(false)
const status = ref(null)
const notInUse = ref(false)
const infoMsg = ref('')
const errMsg = ref('')
const okMsg = ref('')

const showChange = ref(false)
const newPw = ref('')
const newPw2 = ref('')
const pwErr = ref('')

function isEmbedded(d) {
  return d && (d.mode === 'embedded' || d.mode === 'embedded_mode' ||
    (d.available === false && !d.webui_url))
}
function markNotInUse(msg) {
  notInUse.value = true
  status.value = null
  infoMsg.value = msg || ('qBittorrent bu modda kullanımda değil (' + PY + ')')
  okMsg.value = ''; errMsg.value = ''
}

async function loadStatus() {
  loading.value = true
  errMsg.value = ''; infoMsg.value = ''; okMsg.value = ''
  try {
    const d = await apiGet('/api/qbittorrent/password-status')
    if (isEmbedded(d)) { markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')'); return }
    status.value = d
    notInUse.value = false
  } catch (e) {
    // Ağ hatası bile olsa kırmızı hata yerine nötr bilgi.
    markNotInUse('qBittorrent durumu alınamadı (' + PY + ')')
  } finally {
    loading.value = false
  }
}

async function regenerate() {
  if (!confirm('qBittorrent parolası yenilensin mi?')) return
  errMsg.value = ''; okMsg.value = ''
  try {
    const d = await apiPost('/api/qbittorrent/regenerate-password', {})
    if (!d || d.success === false) {
      if (d && (d.message === 'bridge_unavailable' || d.message === 'embedded_mode')) {
        markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')'); return
      }
      errMsg.value = (d && d.message) || 'Parola yenileme başarısız'; return
    }
    okMsg.value = 'Yeni parola: ' + (d.password || '')
    loadStatus()
  } catch (e) {
    markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')')
  }
}

function openChange() { newPw.value = ''; newPw2.value = ''; pwErr.value = ''; showChange.value = true }
function closeChange() { showChange.value = false }

async function savePassword() {
  if (!newPw.value || newPw.value.length < 6) { pwErr.value = 'Parola en az 6 karakter olmalı'; return }
  if (newPw.value !== newPw2.value) { pwErr.value = 'Parolalar eşleşmiyor'; return }
  pwErr.value = ''
  try {
    const d = await apiPost('/api/qbittorrent/change-password', { password: newPw.value })
    if (!d || d.success === false) {
      if (d && d.message === 'embedded_mode') {
        showChange.value = false
        markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')'); return
      }
      if (d && d.message === 'password_too_short') { pwErr.value = 'Parola en az 8 karakter olmalı'; return }
      pwErr.value = (d && d.message) || 'Parola kaydedilemedi'; return
    }
    showChange.value = false
    okMsg.value = 'Parola kaydedildi'
    loadStatus()
  } catch (e) {
    showChange.value = false
    markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')')
  }
}

async function startWebUi() {
  errMsg.value = ''; okMsg.value = ''
  try {
    const d = await apiPost('/api/qbittorrent/start', {})
    if (!d || d.success === false) { markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')'); return }
    const url = (d.url || '').trim()
    if (!url) { markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')'); return }
    const u = new URL(url, window.location.href); u.pathname = '/'; u.search = ''; u.hash = ''
    window.open(u.toString(), '_blank', 'noopener,noreferrer')
  } catch (e) {
    markNotInUse('qBittorrent bu modda kullanımda değil (' + PY + ')')
  }
}

onMounted(loadStatus)
</script>

<template>
  <div class="qb">
    <h4 class="qb-h">🧲 qBittorrent WebUI</h4>

    <div v-if="loading" class="qb-loading">yükleniyor…</div>

    <div v-else-if="notInUse" class="qb-info">{{ infoMsg }}</div>

    <div v-else class="qb-body">
      <div class="qb-status">
        <span class="qb-dot" :class="status.secured ? 'ok' : 'warn'">{{ status.secured ? '🟢' : '🟡' }}</span>
        <span><strong>Durum:</strong> {{ status.using_default ? 'varsayılan parola' : 'özel parola' }}</span>
        <span v-if="status.webui_url" class="qb-url">· <strong>URL:</strong>
          <a :href="status.webui_url" target="_blank" rel="noopener noreferrer">{{ status.webui_url }}</a>
        </span>
      </div>

      <div class="qb-actions">
        <button class="btn run" @click="startWebUi">▶ WebUI aç</button>
        <button class="btn info" @click="regenerate">🎲 Parolayı yenile</button>
        <button class="btn info" @click="openChange">🔑 Parola belirle</button>
      </div>
    </div>

    <p v-if="okMsg" class="qb-ok">{{ okMsg }}</p>
    <p v-if="errMsg" class="qb-err">{{ errMsg }}</p>

    <div v-if="showChange" class="qb-modal" @click.self="closeChange">
      <div class="qb-modal-box">
        <h3>🔑 qBittorrent parolası</h3>
        <label>Yeni parola</label>
        <input type="password" v-model="newPw" />
        <label>Parola (tekrar)</label>
        <input type="password" v-model="newPw2" />
        <p v-if="pwErr" class="qb-err">{{ pwErr }}</p>
        <div class="qb-modal-actions">
          <button class="btn info" @click="savePassword">Kaydet</button>
          <button class="btn ghost" @click="closeChange">İptal</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.qb { margin-top: 20px; padding: 15px; background: #161b22; border: 1px solid #21262d; border-radius: 8px; }
.qb-h { margin: 0 0 12px; font-size: 14px; }
.qb-loading { color: #8b949e; font-size: 13px; }
.qb-info { color: #17a2b8; font-size: 13px; }
.qb-status { font-size: 13px; color: #c9d1d9; display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
.qb-dot.ok { color: #28a745; }
.qb-dot.warn { color: #ffc107; }
.qb-url a { color: #007bff; }
.qb-actions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; }
.btn { border: 0; border-radius: 6px; padding: 8px 14px; font-size: 13px; font-weight: bold; cursor: pointer; color: #fff; }
.btn.run { background: #007bff; }
.btn.info { background: #17a2b8; }
.btn.ghost { background: #30363d; color: #c9d1d9; }
.qb-ok { color: #28a745; font-size: 13px; margin-top: 8px; }
.qb-err { color: #dc3545; font-size: 13px; margin-top: 8px; }
.qb-modal { position: fixed; inset: 0; background: rgba(0,0,0,.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.qb-modal-box { background: #0e1116; border: 1px solid #30363d; border-radius: 10px; padding: 20px; width: 320px; }
.qb-modal-box h3 { margin: 0 0 12px; font-size: 14px; }
.qb-modal-box label { display: block; font-size: 12px; color: #8b949e; margin: 8px 0 4px; }
.qb-modal-box input { width: 100%; background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 8px; color: inherit; font-size: 13px; }
.qb-modal-actions { display: flex; gap: 8px; margin-top: 14px; }
</style>

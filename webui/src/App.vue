<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { connectSSE, apiGet, apiPost } from './api.js'
import { t as _t, getLocale, setLocale, STRINGS } from './i18n.js'
import Support from './components/Support.vue'
import BrowseDirectories from './components/BrowseDirectories.vue'

const connected = ref(false)
const lastEvent = ref('')
const snapshot = reactive({ history: [], queue: [], active: false, progress: {}, downloaded: {} })
const progress = reactive({})

const tt = (k, vars) => {
  let s = (STRINGS[locale.value] && STRINGS[locale.value][k]) || STRINGS.tr[k] || k
  if (vars) for (const [kk, vv] of Object.entries(vars)) s = s.replace(new RegExp('\\{' + kk + '\\}', 'g'), vv)
  return s
}
const locale = ref(getLocale())
function changeUiLang(l) { setLocale(l); locale.value = l }

// --- Tabs ---
const tab = ref('platforms') // platforms | downloaded | queue | history | settings

// --- Catalog ---
const platforms = ref([])
const selectedPlatform = ref(null)
const games = ref([])
const gameStatuses = ref({}) // stem -> { status, progress, platform }
const catalogLoading = ref(false)
const catalogError = ref('')

// --- Per-platform game filters ---
const REGIONS = ['USA', 'Canada', 'Europe', 'France', 'Germany', 'Japan', 'Korea', 'World', 'Other']
const REGION_PRIORITY = ['USA', 'Canada', 'World', 'Europe', 'Japan', 'Other']
const regionFilters = reactive({}) // region -> 'include' | 'exclude' | undefined
const gameSearch = ref('')
const hideDownloaded = ref(false)
const hideNonRelease = ref(false)
const regexMode = ref(false)
const oneRomPerGame = ref(false)
const sortMode = ref('name_asc')

// --- Global search ---
const searchTerm = ref('')
const searchResults = ref(null)

// --- Settings ---
const DEFAULT_SETTINGS = {
  language: 'Turkish',
  music_enabled: false,
  display: { grid: '3x4', light_mode: false, font_family: 'pixel', monitor: '', fullscreen: false },
  show_unsupported_platforms: false,
  global_sort_option: 'name',
  max_simultaneous_downloads: 3,
  allow_unknown_extensions: false,
  web_service_at_boot: false,
  custom_dns_at_boot: false,
  sources: { mode: 'rgsx', custom_url: '' },
  symlink: { enabled: false, target_directory: '' },
  accessibility: { font_scale: 1.0, footer_font_scale: 1.0 },
  roms_folder: '',
  auto_extract: true,
  api_keys: {}
}
function normalizeSettings(s) {
  s = s || {}
  const d = JSON.parse(JSON.stringify(DEFAULT_SETTINGS))
  Object.keys(d).forEach(k => { if (k !== 'display' && k !== 'sources' && k !== 'symlink' && k !== 'accessibility' && s[k] !== undefined) d[k] = s[k] })
  d.display = Object.assign({}, DEFAULT_SETTINGS.display, s.display || {})
  d.sources = Object.assign({}, DEFAULT_SETTINGS.sources, s.sources || {})
  d.symlink = Object.assign({}, DEFAULT_SETTINGS.symlink, s.symlink || {})
  d.accessibility = Object.assign({}, DEFAULT_SETTINGS.accessibility, s.accessibility || {})
  return d
}
const settings = ref(normalizeSettings(null))
const systemInfo = ref(null)
const languages = ref([])
const dataLang = ref('')
const openBrowse = ref(false)

// ===================== Toasts =====================
const toasts = ref([])
let toastSeq = 0
function pushToast(msg, type = 'info') {
  const id = ++toastSeq
  toasts.value.push({ id, msg, type })
  setTimeout(() => { toasts.value = toasts.value.filter(t => t.id !== id) }, 4000)
}

// ===================== Confirm modal =====================
const confirmModal = reactive({ open: false, title: '', message: '', onConfirm: null })
const confirmOkBtn = ref(null)
function openConfirm(title, message, onConfirm) {
  confirmModal.title = title
  confirmModal.message = message
  confirmModal.onConfirm = onConfirm
  confirmModal.open = true
  nextTick(() => { if (confirmOkBtn.value) confirmOkBtn.value.focus() })
}
function confirmOk() {
  const f = confirmModal.onConfirm
  confirmModal.open = false
  confirmModal.onConfirm = null
  if (f) f()
}
function confirmCancel() {
  confirmModal.open = false
  confirmModal.onConfirm = null
}
function onKeydown(e) {
  if (e.key === 'Escape' && confirmModal.open) confirmCancel()
}

// ===================== SSE =====================
let es = null
let seenHistory = new Set()
function applySnapshot(data) {
  connected.value = true
  if (data.history) snapshot.history = data.history
  if (data.queue) snapshot.queue = data.queue
  if (typeof data.active === 'boolean') snapshot.active = data.active
  if (data.progress) Object.assign(progress, data.progress)
  if (data.downloaded) snapshot.downloaded = data.downloaded
  lastEvent.value = 'snapshot'
}
onMounted(async () => {
  es = connectSSE({
    snapshot: applySnapshot,
    progress: (data) => {
      lastEvent.value = 'progress'
      Object.assign(progress, data.progress || data)
    },
    queue: (data) => {
      lastEvent.value = 'queue'
      if (Array.isArray(data)) snapshot.queue = data
      else if (data && data.queue) {
        snapshot.queue = data.queue
        if (typeof data.active === 'boolean') snapshot.active = data.active
      }
    },
    history: (data) => {
      lastEvent.value = 'history'
      const list = Array.isArray(data) ? data : (data && data.history) || snapshot.history
      snapshot.history = list
      for (const h of (list || [])) {
        const key = h.task_id || h.url || h.name
        if (seenHistory.has(key)) continue
        seenHistory.add(key)
        const s = String(h.status || '').toUpperCase()
        const nm = h.name || h.game_name || (h.url ? String(h.url).split('/').pop() : '')
        if (s === 'COMPLETED' || s === 'DOWNLOAD_OK' || s === 'ALREADY_PRESENT') pushToast(tt('download_complete', { n: nm }), 'success')
        else if (s === 'FAILED' || s === 'FAILED_PERMANENT') pushToast(tt('download_failed_item', { n: nm }), 'error')
      }
    },
    downloaded: (data) => {
      lastEvent.value = 'downloaded'
      snapshot.downloaded = (data && data.downloaded) || data || snapshot.downloaded
    },
  })
  await loadPlatforms()
  loadFiltersFromSettings()
  loadSettings()
  window.addEventListener('keydown', onKeydown)
})
onUnmounted(() => { es && es.close(); window.removeEventListener('keydown', onKeydown) })

// ===================== Catalog =====================
async function loadPlatforms() {
  catalogError.value = ''
  try {
    const p = await apiGet('/api/platforms')
    platforms.value = (p.platforms || []).slice(0, 200)
  } catch (e) { catalogError.value = tt('catalog_error') }
}

async function updateGamesList() {
  try {
    await apiPost('/api/update-cache', {})
  } catch (e) { /* sessiz */ }
  // Katalog yenilendikten sonra görünümü tazele
  await loadPlatforms()
  if (selectedPlatform.value) await selectPlatform(selectedPlatform.value)
  pushToast(tt('catalog_refreshed'), 'success')
}

async function selectPlatform(name) {
  selectedPlatform.value = name
  searchResults.value = null
  games.value = []
  catalogLoading.value = true
  try {
    const [g, st] = await Promise.all([
      apiGet('/api/games/' + encodeURIComponent(name)),
      apiGet('/api/game-status').catch(() => ({ statuses: {} })),
    ])
    games.value = g.games || []
    gameStatuses.value = (st && st.statuses) || {}
  } catch (e) { catalogError.value = tt('game_list_failed') }
  finally { catalogLoading.value = false }
}

function backToPlatforms() { selectedPlatform.value = null; games.value = [] }

// ===================== Filtering helpers =====================
const REGION_ALIASES = {
  USA: ['usa', 'u.s.', 'us ', '(us)'],
  Canada: ['canada'],
  Europe: ['europe', 'euro', '(eu)'],
  France: ['france', '(fr)'],
  Germany: ['germany', '(de)'],
  Japan: ['japan', 'jpn', '(jp)'],
  Korea: ['korea', '(kr)'],
  World: ['world'],
}
const NON_RELEASE = /\((demo|beta|proto|prototype|sample|unl|alt|hack|test|promo|bootleg|oem|preview|kiosk|review|pirate)\)/i
const REGION_WORDS = /usa|europe|japan|france|germany|canada|korea|world|euro|jpn|fr|de|kr|eu|us|brazil|spain|italy|australia|uk/gi

function getRegions(name) {
  const n = ' ' + name + ' '
  const found = []
  for (const [r, al] of Object.entries(REGION_ALIASES))
    if (al.some((a) => n.toLowerCase().includes(a))) found.push(r)
  return found
}
function isNonRelease(name) { return NON_RELEASE.test(name) }
function stem(name) { return String(name).toLowerCase().replace(/\.[^.]+$/, '') }
function getBaseName(name) {
  let s = String(name).replace(/\.[^.]+$/, '').replace(/\([^)]*\)/g, ' ')
  s = s.replace(REGION_WORDS, ' ')
  return s.toLowerCase().replace(/\s+/g, ' ').trim()
}
function regionPriority(name) {
  const regs = getRegions(name)
  if (!regs.length) return REGION_PRIORITY.indexOf('Other')
  let best = -1
  for (const r of regs) best = Math.max(best, REGION_PRIORITY.indexOf(r))
  return best
}
function parseSize(str) {
  if (!str) return 0
  const m = String(str).match(/([\d.]+)\s*([KMGT]?)B?/i)
  if (!m) return 0
  const n = parseFloat(m[1]); const u = (m[2] || '').toUpperCase()
  return n * ({ K: 1e3, M: 1e6, G: 1e9, T: 1e12 }[u] || 1)
}
function cmpGames(a, b) {
  if (sortMode.value === 'name_desc') return String(b.name).localeCompare(String(a.name))
  if (sortMode.value === 'size_desc') return parseSize(b.size) - parseSize(a.size)
  if (sortMode.value === 'size_asc') return parseSize(a.size) - parseSize(b.size)
  return String(a.name).localeCompare(String(b.name)) // name_asc
}
function gameStatusOf(g) {
  const s = gameStatuses.value
  return (s && (s[stem(g.name)] || s[String(g.name).toLowerCase()])) || null
}

// Son başarısız indirme denemesi yapılmış oyun adları (stem + lowercase).
// History ve kuyruktaki FAILED/ERROR kayıtlarından türetilir; README'in
// kırmızı `[X]` göstergesi için kullanılır.
const failedNames = computed(() => {
  const set = new Set()
  const add = (n) => { if (!n) return; set.add(stem(n)); set.add(String(n).toLowerCase()) }
  for (const it of historyItems.value) {
    const s = String(it.status || '').toUpperCase()
    if (s === 'FAILED' || s === 'FAILED_PERMANENT' || s === 'ERROR' || s === 'ERREUR')
      add(it.name || it.game_name)
  }
  for (const it of queueItems.value) {
    const s = String(it.status || '').toUpperCase()
    if (s === 'FAILED' || s === 'ERROR') add(it.game_name || it.name)
  }
  return set
})

// Katalog satır göstergesi — README.md "Game List Status Indicators" ile uyumlu:
//   Downloaded  `[>]`   🟢 yeşil   (dosya disk'te)
//   Downloading `[~] %` 🟡 sarı    (aktif indirme, progress haritasından)
//   Failed      `[X]`   🔴 kırmızı (son deneme başarısız)
// Python parity: önce indiriliyor, sonra indirildi, sonra başarısız.
function catalogStatus(g) {
  const p = g.url ? progress[g.url] : null
  const active = p && typeof p.status === 'string' &&
    ['Downloading', 'Extracting', 'Connecting', 'Verifying', 'Seeding'].includes(p.status)
  if (active) {
    const pct = typeof p.progress === 'number' ? Math.round(p.progress) : 0
    return { marker: `[~] ${pct}%`, color: '#ffcc00', cls: 'st-run' }
  }
  if (gameStatusOf(g) && gameStatusOf(g).status === 'downloaded')
    return { marker: '[>]', color: '#28a745', cls: 'st-ok' }
  if (failedNames.value.has(stem(g.name)) || failedNames.value.has(String(g.name).toLowerCase()))
    return { marker: '[X]', color: '#dc3545', cls: 'st-err' }
  return null
}

const filteredGames = computed(() => {
  if (!selectedPlatform.value) return []
  let list = games.value.slice()
  const term = gameSearch.value.trim()
  list = list.filter((g) => {
    const name = String(g.name || '').toLowerCase()
    if (term) {
      if (regexMode.value) { try { if (!(new RegExp(term, 'i')).test(name)) return false } catch { return false } }
      else if (!name.includes(term.toLowerCase())) return false
    }
    const regs = getRegions(g.name)
    for (const [r, mode] of Object.entries(regionFilters)) {
      if (mode === 'include' && !regs.includes(r)) return false
      if (mode === 'exclude' && regs.includes(r)) return false
    }
    // "Other" filter: include => games with NO specific region; exclude => games WITH a region
    if (regionFilters.Other === 'include' && regs.length) return false
    if (regionFilters.Other === 'exclude' && !regs.length) return false
    if (hideNonRelease.value && isNonRelease(g.name)) return false
    if (hideDownloaded.value && gameStatusOf(g) && gameStatusOf(g).status === 'downloaded') return false
    return true
  })
  if (oneRomPerGame.value) {
    const best = {}
    for (const g of list) {
      const base = getBaseName(g.name)
      const cur = best[base]
      if (!cur || regionPriority(g.name) > regionPriority(cur.name)) best[base] = g
    }
    list = Object.values(best)
  }
  list.sort(cmpGames)
  return list
})

function cycleRegion(r) {
  const cur = regionFilters[r]
  regionFilters[r] = cur === undefined ? 'include' : cur === 'include' ? 'exclude' : undefined
  if (regionFilters[r] === undefined) delete regionFilters[r]
  saveFilters()
}
function resetFilters() {
  for (const k of Object.keys(regionFilters)) delete regionFilters[k]
  gameSearch.value = ''; hideDownloaded.value = false; hideNonRelease.value = false
  regexMode.value = false; oneRomPerGame.value = false; sortMode.value = 'name_asc'
  saveFilters()
}
async function saveFilters() {
  try {
    await apiPost('/api/save_filters', {
      region_filters: { ...regionFilters },
      hide_non_release: hideNonRelease.value,
      one_rom_per_game: oneRomPerGame.value,
      hide_downloaded: hideDownloaded.value,
      regex_mode: regexMode.value,
      region_priority: REGION_PRIORITY,
    })
  } catch (e) { /* sessiz */ }
}
async function loadFiltersFromSettings() {
  try {
    const s = await apiGet('/api/settings')
    const f = s && s.settings && s.settings.game_filters
    if (f) {
      if (f.region_filters) for (const [k, v] of Object.entries(f.region_filters)) regionFilters[k] = v
      if (typeof f.hide_non_release === 'boolean') hideNonRelease.value = f.hide_non_release
      if (typeof f.one_rom_per_game === 'boolean') oneRomPerGame.value = f.one_rom_per_game
      if (typeof f.hide_downloaded === 'boolean') hideDownloaded.value = f.hide_downloaded
      if (typeof f.regex_mode === 'boolean') regexMode.value = f.regex_mode
    }
  } catch (e) { /* sessiz */ }
}

// ===================== Downloads =====================
async function downloadGame(g, mode) {
  if (!g.url) return
  try {
    await apiPost('/api/download', {
      url: g.url,
      platform: selectedPlatform.value || g.platform || '',
      game_name: g.name || g.game_name || '',
      mode: mode || 'queue',
    })
    pushToast(tt('download_started'), 'info')
  } catch (e) { pushToast(tt('download_failed'), 'error') }
}
async function downloadAll() {
  const names = filteredGames.value.map((g) => g.name)
  if (!names.length) return
  openConfirm(tt('confirm_download_all_title'), tt('confirm_download_all_msg', { platform: selectedPlatform.value, n: names.length }), async () => {
    try {
      const r = await apiPost('/api/download/batch', { platform: selectedPlatform.value, game_names: names })
      if (r && r.success) pushToast(tt('download_queued', { n: r.queued || names.length }), 'success')
      else pushToast((r && r.error) || 'Hata', 'error')
    } catch (e) { pushToast(tt('download_failed'), 'error') }
  })
}

// ===================== Queue / Progress =====================
const queueItems = computed(() => snapshot.queue || [])
function queuePct(item) {
  const p = progress[item.url]
  if (!p) return null
  if (typeof p.progress === 'number') return p.progress
  if (p.total && p.downloaded) return Math.round((p.downloaded / p.total) * 100)
  return null
}
function queueSpeed(item) {
  const p = progress[item.url]
  if (!p || !p.speed) return ''
  return (p.speed / 1048576).toFixed(1) + ' MB/s'
}
async function removeFromQueue(taskId) {
  try { await apiPost('/api/queue/remove', { task_id: taskId }) } catch (e) {}
}
async function clearQueue() {
  openConfirm(tt('confirm_clear_queue_title'), tt('confirm_clear_queue_msg'), async () => {
    try { await apiPost('/api/queue/clear', {}) } catch (e) {}
  })
}
async function cancelDownload(item) {
  try { await apiPost('/api/cancel', item.task_id ? { task_id: item.task_id } : { url: item.url }) } catch (e) {}
}
async function pauseAll() { try { await apiPost('/api/pause', {}) } catch (e) {} }
async function resumeAll() { try { await apiPost('/api/resume', {}) } catch (e) {} }

// ===================== History =====================
const historyItems = computed(() => snapshot.history || [])

// Rust backend durum dizgeleri (büyük/küçük harf + entity_state) -> rozet eşlemesi.
// Eski Python tasarımının renkleri birebir korunur (app.js 1966-1989):
//   DOWNLOADING/EXTRACTING -> #007bff (mavi)
//   COMPLETED              -> #28a745 (yeşil)
//   FAILED                 -> #dc3545 (kırmızı)
//   QUEUED                 -> #6c757d (gri)
//   CANCELED               -> #ffc107 (turuncu)
//   ALREADY_PRESENT/SEEDING-> #17a2b8 (camgöbeği)
function statusMeta(raw) {
  const s = String(raw || '')
  const up = s.toUpperCase()
  if (up === 'COMPLETED' || up === 'DOWNLOAD_OK' || s === 'Completed' || s === 'Download_OK' || s === 'downloaded')
    return { label: 'COMPLETED', color: '#28a745', cls: 'st-ok' }
  if (up === 'FAILED' || up === 'FAILED_PERMANENT' || s === 'Erreur' || s === 'error' || s === 'failed')
    return { label: 'FAILED', color: '#dc3545', cls: 'st-err' }
  if (up === 'CANCELED' || s === 'Canceled')
    return { label: 'CANCELED', color: '#ffc107', cls: 'st-cancel' }
  if (up === 'QUEUED' || s === 'Queued')
    return { label: 'QUEUED', color: '#6c757d', cls: 'st-queue' }
  if (s === 'Already_Present')
    return { label: 'ALREADY PRESENT', color: '#17a2b8', cls: 'st-info' }
  if (s === 'Extracting')
    return { label: 'EXTRACTING', color: '#ffcc00', cls: 'st-run' }
  if (s === 'Seeding')
    return { label: 'SEEDING', color: '#17a2b8', cls: 'st-run' }
  if (s === 'Downloading' || s === 'Connecting' || s === 'Verifying' || s.startsWith('Try') || s === 'downloading')
    return { label: 'DOWNLOADING', color: '#ffcc00', cls: 'st-run' }
  return { label: s || 'UNKNOWN', color: '#6c757d', cls: 'st-info' }
}

// Kuyruk öğesinin canlı durumu: SSE progress haritasından türet (indirme sırasında
// kuyruk kaydı hâlâ "Queued" olabilir; ilerleme olayı bunu DOWNLOADING yapar).
function queueStatus(item) {
  const p = progress[item.url]
  if (p && typeof p.status === 'string' &&
      ['Downloading', 'Extracting', 'Connecting', 'Verifying'].includes(p.status)) {
    return p.status
  }
  return item.status || 'Queued'
}
async function clearHistory() {
  openConfirm(tt('confirm_clear_history_title'), tt('confirm_clear_history_msg'), async () => {
    try { await apiPost('/api/clear-history', {}) } catch (e) {}
  })
}

// ===================== Downloaded (İndirilenler) =====================
// Faz A parity: aynı dosya gameStatuses'ta 2 anahtarla (stem + lowercase) ve
// snapshot.downloaded'da gerçek adıyla düşer → 3 ayrı satır görünür. Normalize
// anahtar (platform | stem(name).toLowerCase) ile birleştirip TEK satıra indir.
const downloadedItems = computed(() => {
  const map = {}
  const norm = (plat, name) => (plat || '') + '|' + stem(name)
  const st = gameStatuses.value || {}
  for (const [k, v] of Object.entries(st)) {
    if (v && v.status === 'downloaded') {
      const key = norm(v.platform, v.name || k)
      if (!map[key]) map[key] = { name: v.name || k, platform: v.platform || '' }
    }
  }
  const dl = snapshot.downloaded || {}
  for (const [plat, names] of Object.entries(dl)) {
    for (const n of (names || [])) {
      const key = norm(plat, n)
      if (!map[key]) map[key] = { name: n, platform: plat }
    }
  }
  return Object.values(map)
})
async function refreshDownloaded() {
  try { const r = await apiGet('/api/game-status'); gameStatuses.value = (r && r.statuses) || {} } catch (e) {}
}

// ===================== Search =====================
async function doSearch() {
  const q = searchTerm.value.trim()
  if (!q) { searchResults.value = null; return }
  catalogLoading.value = true
  try {
    const r = await apiGet('/api/search?q=' + encodeURIComponent(q))
    searchResults.value = (r && r.results) || { platforms: [], games: [] }
  } catch (e) { catalogError.value = tt('search_failed') }
  finally { catalogLoading.value = false }
}
function clearSearch() { searchResults.value = null; searchTerm.value = '' }
function searchDownload(g) {
  if (!g.url) return
  apiPost('/api/download', { url: g.url, platform: g.platform || '', game_name: g.game_name || g.name || '', mode: 'queue' })
    .then(() => pushToast(tt('download_started'), 'info'))
    .catch(() => pushToast(tt('download_failed'), 'error'))
}

// ===================== Settings =====================
async function loadSettings() {
  try { const ls = await apiGet('/api/languages'); languages.value = ls.languages || [] } catch (e) {}
  try {
    const s = await apiGet('/api/settings')
    settings.value = normalizeSettings(s && s.settings ? s.settings : null)
    systemInfo.value = s && s.system_info
  } catch (e) { settings.value = null }
  try { const tr = await apiGet('/api/translations'); dataLang.value = tr.language || 'en' } catch (e) { dataLang.value = 'en' }
}
async function saveSettings() {
  if (!settings.value) return
  try {
    await apiPost('/api/settings', { settings: settings.value })
    pushToast(tt('saved'), 'success')
    await loadSettings()
  } catch (e) { pushToast(tt('save_failed'), 'error') }
}
function changeDataLang(l) {
  dataLang.value = l
  apiGet('/api/translations?lang=' + encodeURIComponent(l)).catch(() => {})
  if (settings.value) { settings.value.language = l; saveSettings() }
}
async function onBrowseSelect(p) {
  if (settings.value) settings.value.roms_folder = p
  openBrowse.value = false
  await saveSettings()
  pushToast(tt('browse_restart_note'), 'info')
}
function onApiKey(service, val) {
  if (!settings.value) return
  if (!settings.value.api_keys) settings.value.api_keys = {}
  if (val) settings.value.api_keys[service] = val
  else delete settings.value.api_keys[service]
  saveSettings()
}

// ===================== Tab switching =====================
async function switchTab(t) {
  tab.value = t
  if (t === 'queue') { try { const r = await apiGet('/api/queue'); snapshot.queue = r.queue || []; snapshot.active = !!r.active } catch (e) {} }
  if (t === 'history') { try { const r = await apiGet('/api/history'); snapshot.history = r.history || [] } catch (e) {} }
  if (t === 'downloaded') await refreshDownloaded()
  if (t === 'settings') await loadSettings()
}
</script>

<template>
  <div class="app" :style="{ '--font-scale': settings.accessibility ? settings.accessibility.font_scale : 1 }">
    <header>
      <h1>{{ tt('app_title') }}</h1>
      <span class="status" :class="{ on: connected }">{{ connected ? tt('status_connected') : tt('status_connecting') }}</span>
      <span class="active" v-if="snapshot.active">● aktif indirme</span>
      <button class="gear" @click="updateGamesList" title="Oyun listesini güncelle" aria-label="Oyun listesini güncelle">🔄</button>
      <button class="gear" :class="{ on: tab === 'settings' }" @click="switchTab('settings')" title="Ayarlar" aria-label="Ayarlar">⚙</button>
      <Support />
    </header>

    <!-- Global search -->
    <div class="searchbar">
      <input v-model="searchTerm" @keyup.enter="doSearch" :placeholder="tt('search_placeholder')" />
      <button @click="doSearch">{{ tt('search_button') }}</button>
      <button v-if="searchResults" @click="clearSearch">{{ tt('clear') }}</button>
    </div>

    <!-- Tabs -->
    <nav class="tabs">
      <button :class="{ active: tab === 'platforms' }" @click="switchTab('platforms')">Platformlar</button>
      <button :class="{ active: tab === 'downloaded' }" @click="switchTab('downloaded')">İndirilenler ({{ downloadedItems.length }})</button>
      <button :class="{ active: tab === 'queue' }" @click="switchTab('queue')">Kuyruk ({{ queueItems.length }})</button>
      <button :class="{ active: tab === 'history' }" @click="switchTab('history')">Geçmiş ({{ historyItems.length }})</button>
      <button :class="{ active: tab === 'settings' }" @click="switchTab('settings')">Ayarlar</button>
    </nav>

    <p v-if="catalogError" class="err">{{ catalogError }}</p>

    <!-- Search results -->
    <section v-if="searchResults" class="panel">
      <h2>{{ tt('search_results') }} <a class="back" @click="clearSearch">{{ tt('clear') }}</a></h2>
      <h3 v-if="searchResults.platforms.length">Platformlar</h3>
      <div class="grid">
        <button v-for="p in searchResults.platforms" :key="p.platform_name" class="card"
                @click="clearSearch(); selectPlatform(p.platform_name)">
          <span class="pname">{{ p.platform_name }}</span>
          <span class="count" v-if="p.games_count != null">{{ p.games_count }} {{ tt('games') }}</span>
        </button>
      </div>
      <h3 v-if="searchResults.games.length">Oyunlar</h3>
      <ul class="games">
        <li v-for="(g, i) in searchResults.games" :key="g.game_name + i">
          <div class="row"><span class="name">{{ g.game_name }} <small>({{ g.platform }})</small></span><span class="size">{{ g.size || '' }}</span></div>
          <button class="dlbtn" :disabled="!g.url" @click="searchDownload(g)">{{ tt('download') }}</button>
        </li>
      </ul>
      <p v-if="!searchResults.platforms.length && !searchResults.games.length" class="muted">{{ tt('no_results') }}</p>
    </section>

    <!-- PLATFORMS TAB -->
    <section v-if="tab === 'platforms'" class="panel">
      <!-- Platform grid -->
      <div v-if="!selectedPlatform">
        <h2>{{ tt('platforms') }} <small>({{ platforms.length }})</small></h2>
        <div class="grid">
          <button v-for="(p, i) in platforms" :key="p.platform_name || p.name" class="card"
                  @click="selectPlatform(p.platform_name || p.name)">
            <img v-if="p.platform_image" :src="'/api/image/' + encodeURIComponent(p.platform_name || p.name)" class="box" alt="" />
            <span class="pname">{{ p.platform_name || p.name }}</span>
            <span class="count" v-if="p.games_count != null">{{ p.games_count }} {{ tt('games') }}</span>
          </button>
        </div>
      </div>

      <!-- Games list + filters -->
      <div v-else>
        <h2>
          {{ selectedPlatform }}
          <small>({{ filteredGames.length }} / {{ games.length }} oyun)</small>
          <a class="back" @click="backToPlatforms">{{ tt('back') }}</a>
        </h2>

        <!-- Filter bar -->
        <div class="filters">
          <input class="gfilt" v-model="gameSearch" :placeholder="'Oyun ara…'" />
          <div class="regions">
            <button v-for="r in REGIONS" :key="r" class="rbtn" :class="regionFilters[r]"
                    @click="cycleRegion(r)">{{ r }}</button>
          </div>
          <label class="chk"><input type="checkbox" v-model="hideDownloaded" @change="saveFilters()" /> İndirilenleri gizle</label>
          <label class="chk"><input type="checkbox" v-model="hideNonRelease" @change="saveFilters()" /> Demo/beta gizle</label>
          <label class="chk"><input type="checkbox" v-model="regexMode" @change="saveFilters()" /> Regex</label>
          <label class="chk"><input type="checkbox" v-model="oneRomPerGame" @change="saveFilters()" /> 1 ROM/oyun</label>
          <select v-model="sortMode">
            <option value="name_asc">Ada göre (A→Z)</option>
            <option value="name_desc">Ada göre (Z→A)</option>
            <option value="size_asc">Boya göre (küçük→büyük)</option>
            <option value="size_desc">Boya göre (büyük→küçük)</option>
          </select>
          <button class="reset" @click="resetFilters">Filtreyi sıfırla</button>
          <button class="dlall" @click="downloadAll">Tümünü indir ({{ filteredGames.length }})</button>
        </div>
        <p v-if="catalogLoading" class="muted">{{ tt('loading') }}</p>

        <ul class="games">
          <li v-for="(g, i) in filteredGames" :key="g.name || g.url || i">
            <span class="badge sm" v-if="catalogStatus(g)" :style="{ background: catalogStatus(g).color }">{{ catalogStatus(g).marker }}</span>
            <div class="row"><span class="name">{{ g.name }}</span><span class="size">{{ g.size || '' }}</span></div>
            <div class="dlgrp">
              <button class="dlbtn" :disabled="!g.url" @click="downloadGame(g, 'now')" title="Şimdi indir" aria-label="Şimdi indir">⬇️</button>
              <button class="dlbtn q" :disabled="!g.url" @click="downloadGame(g, 'queue')" title="Kuyruğa ekle" aria-label="Kuyruğa ekle">➕</button>
            </div>
          </li>
        </ul>
      </div>
    </section>

    <!-- DOWNLOADED TAB -->
    <section v-if="tab === 'downloaded'" class="panel">
      <h2>İndirilenler <small>({{ downloadedItems.length }})</small></h2>
      <p v-if="!downloadedItems.length" class="muted">Henüz indirilen oyun yok.</p>
      <ul class="games">
        <li v-for="(g, i) in downloadedItems" :key="g.name + i">
          <span class="st s-downloaded">✓</span>
          <div class="row"><span class="name">{{ g.name }}</span><span class="size">{{ g.platform }}</span></div>
        </li>
      </ul>
    </section>

    <!-- QUEUE TAB -->
    <section v-if="tab === 'queue'" class="panel">
      <h2>Kuyruk <small>({{ queueItems.length }})</small>
        <span class="qacts">
          <button @click="pauseAll">⏸ Duraklat</button>
          <button @click="resumeAll">▶ Devam</button>
          <button class="danger" @click="clearQueue">Temizle</button>
        </span>
      </h2>
      <p v-if="!queueItems.length" class="muted">Kuyruk boş.</p>
      <ul class="dl">
        <li v-for="(item, i) in queueItems" :key="item.task_id || item.url || i">
          <div class="row">
            <span class="name">{{ item.game_name || item.name || item.url }}</span>
            <span class="badge" :style="{ background: statusMeta(queueStatus(item)).color }">{{ statusMeta(queueStatus(item)).label }}</span>
            <span class="pct" v-if="queuePct(item) != null">{{ queuePct(item) }}%</span>
          </div>
          <div class="bar" v-if="queuePct(item) != null"><div class="fill" :style="{ width: queuePct(item) + '%' }"></div></div>
          <div class="qmeta">
            <span class="muted">{{ item.platform }}</span>
            <span class="muted" v-if="queueSpeed(item)">{{ queueSpeed(item) }}</span>
            <button class="dlbtn danger" @click="cancelDownload(item)">İptal</button>
          </div>
        </li>
      </ul>
    </section>

    <!-- HISTORY TAB -->
    <section v-if="tab === 'history'" class="panel">
      <h2>Geçmiş <small>({{ historyItems.length }})</small>
        <button class="danger" v-if="historyItems.length" @click="clearHistory">Geçmişi temizle</button>
      </h2>
      <p v-if="!historyItems.length" class="muted">Geçmiş boş.</p>
      <ul class="hist">
        <li v-for="(item, i) in historyItems" :key="item.task_id || i" :class="statusMeta(item.status).cls">
          <div class="row">
            <span class="name">{{ item.game_name || item.name }}</span>
            <span class="badge" :style="{ background: statusMeta(item.status).color }">{{ statusMeta(item.status).label }}</span>
          </div>
          <div class="qmeta">
            <span class="muted">{{ item.platform }}</span>
            <span class="muted" v-if="item.total_size">{{ item.total_size }}</span>
            <span class="muted" v-if="item.timestamp">{{ item.timestamp }}</span>
          </div>
          <div class="muted" v-if="item.message">{{ item.message }}</div>
        </li>
      </ul>
    </section>

    <!-- SETTINGS TAB -->
    <section v-if="tab === 'settings'" class="panel">
      <h2>Ayarlar</h2>
      <div class="field">
        <label>Arayüz Dili</label>
        <select :value="locale" @change="changeUiLang($event.target.value)">
          <option v-for="(v, k) in STRINGS" :key="k" :value="k">{{ k }}</option>
        </select>
      </div>
      <div class="field" v-if="languages.length">
        <label>Veri Dili (sunucu)</label>
        <select :value="dataLang" @change="changeDataLang($event.target.value)">
          <option v-for="l in languages" :key="l" :value="l">{{ l }}</option>
        </select>
      </div>
      <template v-if="settings">
        <div class="field">
          <label>{{ tt('light_mode') }}</label>
          <input type="checkbox" v-model="settings.display.light_mode" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>{{ tt('grid') }}</label>
          <select v-model="settings.display.grid" @change="saveSettings()">
            <option value="2x4">2x4</option><option value="3x4">3x4</option>
            <option value="4x3">4x3</option><option value="5x3">5x3</option>
          </select>
        </div>
        <div class="field">
          <label>Yazı tipi (font family)</label>
          <select v-model="settings.display.font_family" @change="saveSettings()">
            <option value="pixel">Pixel</option><option value="dejavu">DejaVu</option>
          </select>
        </div>
        <div class="field">
          <label>{{ tt('max_downloads') }}</label>
          <input type="number" min="1" max="20" v-model.number="settings.max_simultaneous_downloads" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>{{ tt('music') }}</label>
          <input type="checkbox" v-model="settings.music_enabled" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>{{ tt('show_unsupported') }}</label>
          <input type="checkbox" v-model="settings.show_unsupported_platforms" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>Bilinmeyen uzantılara izin ver</label>
          <input type="checkbox" v-model="settings.allow_unknown_extensions" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>Sıralama</label>
          <select v-model="settings.global_sort_option" @change="saveSettings()">
            <option value="name_asc">{{ tt('sort_name_asc') }}</option>
            <option value="name_desc">{{ tt('sort_name_desc') }}</option>
            <option value="size_desc">{{ tt('sort_size_desc') }}</option>
            <option value="added_desc">{{ tt('sort_added_desc') }}</option>
          </select>
        </div>
        <div class="field">
          <label>Kaynak modu</label>
          <select v-model="settings.sources.mode" @change="saveSettings()">
            <option value="rgsx">rgsx</option><option value="custom">custom</option>
          </select>
        </div>
        <div class="field" v-if="settings.sources.mode === 'custom'">
          <label>Özel URL</label>
          <input type="text" v-model="settings.sources.custom_url" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>Sembolik bağ (symlink)</label>
          <input type="checkbox" v-model="settings.symlink.enabled" @change="saveSettings()" />
        </div>
        <div class="field" v-if="settings.symlink.enabled">
          <label>🔗 {{ tt('symlink_target') }}</label>
          <input type="text" v-model="settings.symlink.target_directory" @change="saveSettings()" placeholder="/mnt/roms" />
        </div>
        <div class="field">
          <label>{{ tt('auto_extract') }}</label>
          <input type="checkbox" v-model="settings.auto_extract" @change="saveSettings()" />
        </div>
        <template v-if="systemInfo && (systemInfo.system || '').toLowerCase() === 'linux'">
          <h3>Linux / Batocera</h3>
          <div class="field">
            <label>Açılışta web servisi</label>
            <input type="checkbox" v-model="settings.web_service_at_boot" @change="saveSettings()" />
          </div>
          <div class="field">
            <label>Açılışta özel DNS</label>
            <input type="checkbox" v-model="settings.custom_dns_at_boot" @change="saveSettings()" />
          </div>
        </template>
        <div class="field">
          <label>🔑 {{ tt('api_keys') }}</label>
          <div class="keyrows">
            <input type="text" :value="settings.api_keys['archive.org'] || ''" @input="onApiKey('archive.org', $event.target.value)" placeholder="archive.org" />
            <input type="text" :value="settings.api_keys['realdebrid'] || ''" @input="onApiKey('realdebrid', $event.target.value)" placeholder="RealDebrid" />
            <input type="text" :value="settings.api_keys['1fichier'] || ''" @input="onApiKey('1fichier', $event.target.value)" placeholder="1fichier" />
            <input type="text" :value="settings.api_keys['alldebrid'] || ''" @input="onApiKey('alldebrid', $event.target.value)" placeholder="AllDebrid" />
            <input type="text" :value="settings.api_keys['debridlink'] || ''" @input="onApiKey('debridlink', $event.target.value)" placeholder="Debrid-Link" />
            <input type="text" :value="settings.api_keys['torbox'] || ''" @input="onApiKey('torbox', $event.target.value)" placeholder="TorBox" />
          </div>
        </div>
        <div class="field" v-if="systemInfo">
          <label>🖥️ {{ tt('system_info') || 'Sistem Bilgisi' }}</label>
          <div class="sysinfo">
            <div v-for="(v, k) in systemInfo" :key="k" class="sysrow"><span>{{ k }}</span><span>{{ v }}</span></div>
          </div>
        </div>
        <div class="field">
          <label>🔤 {{ tt('font_scale') }} ({{ settings.accessibility.font_scale }})</label>
          <input type="range" min="0.5" max="2.0" step="0.1" v-model.number="settings.accessibility.font_scale" @change="saveSettings()" />
        </div>
        <div class="field">
          <label>{{ tt('roms_folder') }}</label>
          <div class="browse-row">
            <input type="text" v-model="settings.roms_folder" @change="saveSettings()" placeholder="varsayılan" />
            <button class="browse-btn" @click="openBrowse = true">📂 {{ tt('browse') }}</button>
          </div>
        </div>
        <BrowseDirectories v-if="openBrowse" :current-path="settings.roms_folder" @select="onBrowseSelect" @close="openBrowse = false" />
      </template>
      <p v-else class="muted">{{ tt('no_settings') }}</p>
    </section>

    <footer class="muted">{{ tt('last_event') }}: {{ lastEvent }}</footer>
    <div class="sr-only" role="status" aria-live="polite" aria-atomic="true">{{ lastEvent }}</div>

    <!-- Toasts -->
    <div class="toasts" role="region" aria-label="Bildirimler" aria-live="polite">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="'toast-' + t.type" role="status">{{ t.msg }}</div>
    </div>

    <!-- Confirm modal -->
    <div v-if="confirmModal.open" class="modal-overlay" @click.self="confirmCancel" @keydown.esc="confirmCancel">
      <div class="modal" role="dialog" aria-modal="true" :aria-label="confirmModal.title">
        <h3 class="modal-title">{{ confirmModal.title }}</h3>
        <p class="modal-msg">{{ confirmModal.message }}</p>
        <div class="modal-actions">
          <button ref="confirmOkBtn" class="btn primary" @click="confirmOk">{{ tt('confirm_ok') }}</button>
          <button class="btn" @click="confirmCancel">{{ tt('confirm_cancel') }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
:root { color-scheme: light; }
* { box-sizing: border-box; }
body {
  margin: 0; padding: 20px; min-height: 100vh;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: #333;
}
/* Eski Python WebUI konteyner görünümü (static/css/app.css) */
.app {
  max-width: 960px; margin: 0 auto;
  background: #fff; border-radius: 16px;
  box-shadow: 0 20px 60px rgba(0,0,0,0.3);
  overflow: hidden; padding: 24px;
}
header {
  display: flex; align-items: center; gap: 12px; flex-wrap: wrap;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: #fff; padding: 18px 20px; border-radius: 12px; margin-bottom: 12px;
}
h1 { font-size: 20px; margin: 0; }
.status { font-size: 12px; padding: 2px 10px; border-radius: 999px; background: rgba(255,255,255,0.25); color: #fff; }
.status.on { background: rgba(255,255,255,0.45); }
.active { color: #fff; font-size: 12px; opacity: 0.9; }
.gear {
  background: rgba(255,255,255,0.2); border: 1px solid rgba(255,255,255,0.4);
  border-radius: 6px; color: #fff; font-size: 16px; width: 32px; height: 32px; cursor: pointer;
}
.gear:hover { background: rgba(255,255,255,0.32); }
.gear.on { background: #fff; color: #667eea; }

.searchbar { display: flex; gap: 8px; margin: 16px 0; }
.searchbar input { flex: 1; background: #fff; border: 2px solid #ddd; border-radius: 8px; padding: 8px 12px; color: #333; font-size: 13px; }
.searchbar input:focus { outline: none; border-color: #667eea; }
.searchbar button { background: #667eea; border: 0; border-radius: 8px; padding: 6px 16px; color: #fff; cursor: pointer; }
.searchbar button:hover { background: #5568d3; }

.tabs { display: flex; gap: 4px; flex-wrap: wrap; background: #f5f5f5; border-radius: 8px; padding: 4px; }
.tabs button { background: #f5f5f5; border: none; color: #333; border-radius: 6px; padding: 8px 14px; cursor: pointer; font-size: 13px; }
.tabs button:hover { background: #e0e0e0; }
.tabs button.active { background: #fff; border-bottom: 3px solid #667eea; font-weight: bold; }

.panel { margin-top: 12px; }
h2 { font-size: 15px; margin: 16px 0 8px; display: flex; align-items: center; gap: 8px; flex-wrap: wrap; color: #333; }
h3 { font-size: 13px; color: #666; margin: 12px 0 4px; }
small { color: #666; font-weight: normal; }
.muted { color: #666; font-size: 13px; }
.err { color: #dc3545; background: #f8d7da; padding: 8px 12px; border-radius: 8px; font-size: 13px; }

.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 20px; }
.card { display: flex; flex-direction: column; align-items: center; gap: 10px; background: linear-gradient(135deg, #2c3e50 0%, #34495e 100%); padding: 20px; border: none; border-radius: 12px; cursor: pointer; color: #fff; transition: transform 0.3s, box-shadow 0.3s; text-align: center; }
.card:hover { transform: translateY(-5px); box-shadow: 0 10px 30px rgba(0,0,0,0.4); }
.card .box { width: 200px; height: 200px; object-fit: contain; border-radius: 8px; background: rgba(255,255,255,0.05); filter: drop-shadow(0 4px 6px rgba(0,0,0,0.3)); }
.card .pname { font-size: 15px; text-align: center; color: #fff; min-height: 2.5em; display: flex; align-items: center; justify-content: center; }
.card .count { font-size: 13px; color: #fff; background: #667eea; padding: 5px 15px; border-radius: 20px; display: inline-block; margin-top: 10px; }
@media (max-width: 900px) {
  .grid { grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 15px; }
  .card { padding: 15px; }
  .card .box { width: 80px; height: 80px; }
  .card .pname { font-size: 13px; min-height: 2em; }
}
@media (max-width: 480px) {
  .grid { grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; }
  .card { padding: 10px; }
  .card .box { width: 60px; height: 60px; }
  .card .pname { font-size: 12px; }
  .card .count { font-size: 11px; padding: 3px 10px; }
}

.games { list-style: none; padding: 0; margin: 0; }
.games li { display: flex; align-items: center; gap: 12px; padding: 10px 0; border-bottom: 1px solid #eee; }
.games li .row { flex: 1; display: flex; justify-content: space-between; gap: 12px; min-width: 0; }
.name { font-size: 13px; color: #333; overflow-wrap: break-word; min-width: 0; }
.size { color: #666; font-size: 12px; white-space: nowrap; }
.dlbtn { background: #28a745; color: #fff; border: 0; border-radius: 6px; padding: 6px 10px; font-size: 13px; cursor: pointer; }
.dlbtn:hover { background: #218838; }
.dlbtn:disabled { background: #c6c6c6; color: #fff; cursor: not-allowed; }
.dlbtn.q { background: #6c757d; }
.dlgrp { display: flex; gap: 4px; flex-shrink: 0; }

.back { font-size: 11px; color: #667eea; cursor: pointer; margin-left: 8px; }

/* Filters */
.filters { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin: 8px 0 12px; background: #f5f5f5; border-radius: 8px; padding: 10px; }
.filters .gfilt { flex: 1; min-width: 160px; background: #fff; border: 2px solid #ccc; border-radius: 5px; padding: 6px 10px; color: #333; font-size: 13px; }
.regions { display: flex; flex-wrap: wrap; gap: 4px; }
.rbtn { font-size: 11px; background: #e0e0e0; border: 2px solid #999; color: #333; border-radius: 6px; padding: 4px 8px; cursor: pointer; }
.rbtn.include { background: #28a745; color: #fff; border-color: #28a745; }
.rbtn.exclude { background: #dc3545; color: #fff; border-color: #dc3545; }
.chk { font-size: 12px; color: #333; display: flex; align-items: center; gap: 4px; }
.filters select { background: #fff; border: 2px solid #ccc; border-radius: 5px; padding: 6px; color: #333; font-size: 12px; }
.reset { background: #e0e0e0; border: 2px solid #999; color: #333; border-radius: 6px; padding: 6px 10px; cursor: pointer; font-size: 12px; }
.dlall { background: #2f8f46; color: #fff; border: 0; border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 12px; }

/* Status badges — exact colors from old Python app.js (1966-1989) */
.badge { display: inline-block; color: #fff; padding: 2px 10px; border-radius: 5px; font-size: 0.8em; font-weight: bold; white-space: nowrap; }
.badge.sm { padding: 1px 7px; font-size: 0.72em; }

/* Queue / downloads */
.dl { list-style: none; padding: 0; margin: 0; }
.dl li { padding: 10px 0; border-bottom: 1px solid #eee; }
.dl .row { display: flex; justify-content: space-between; align-items: center; gap: 8px; font-size: 13px; }
.pct { color: #007bff; font-variant-numeric: tabular-nums; font-weight: bold; }
.bar { height: 8px; background: #e0e0e0; border-radius: 6px; margin-top: 6px; overflow: hidden; }
.fill { height: 100%; background: linear-gradient(90deg, #667eea, #764ba2); transition: width .3s ease; }
.qmeta { display: flex; gap: 10px; align-items: center; margin-top: 4px; }
.qacts { margin-left: auto; display: flex; gap: 6px; }
.danger { background: #dc3545; color: #fff; border: 0; border-radius: 6px; padding: 4px 10px; cursor: pointer; font-size: 12px; }
.danger:hover { background: #c82333; }

/* History */
.hist { list-style: none; padding: 0; margin: 0; }
.hist li { padding: 10px; margin-bottom: 8px; background: #f9f9f9; border-radius: 8px; border-left: 4px solid #28a745; }
.hist li.st-err { border-left-color: #dc3545; }
.hist li.st-cancel { border-left-color: #ffc107; }
.hist li.st-info { border-left-color: #17a2b8; }
.hist li.st-queue { border-left-color: #6c757d; }
.hist li.st-run { border-left-color: #007bff; }

/* Settings */
.field { margin: 12px 0; }
.field label { display: block; font-size: 12px; color: #333; font-weight: bold; margin-bottom: 4px; }
.field select, .field input[type=text], .field input[type=number] { background: #f8f8f8; border: 2px solid #ccc; border-radius: 5px; padding: 8px 10px; color: #000; font-size: 13px; }
.field select:focus, .field input:focus { outline: none; border-color: #667eea; }
.saved { color: #28a745; font-size: 13px; }
.browse-row { display: flex; gap: 8px; }
.browse-row input { flex: 1; background: #f8f8f8; border: 2px solid #ccc; border-radius: 5px; padding: 8px 10px; color: #000; font-size: 13px; }
.keyrows { display: flex; flex-direction: column; gap: 6px; }
.keyrows input { background: #f8f8f8; border: 2px solid #ccc; border-radius: 5px; padding: 8px 10px; color: #000; font-size: 13px; }
.sysinfo { background: #f0f8ff; border: 2px solid #007bff; border-radius: 8px; padding: 10px 12px; font-size: 13px; }
.sysrow { display: flex; justify-content: space-between; gap: 12px; padding: 3px 0; border-bottom: 1px solid #e0eaff; }
.sysrow:last-child { border-bottom: 0; }
.sysrow span:first-child { font-weight: bold; color: #0056b3; }
.sysrow span:last-child { text-align: right; word-break: break-all; }
.browse-btn { background: #007bff; color: #fff; border: 0; border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 13px; white-space: nowrap; }
.browse-btn:hover { background: #0069d9; }

/* ===== Accessibility (mirrors Python static/css/accessibility.css) ===== */
.sr-only {
  position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px;
  overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0;
}
:focus-visible { outline: 3px solid #007bff; outline-offset: 2px; border-radius: 4px; }
button, a[role="button"], input[type="button"], input[type="submit"] {
  cursor: pointer; font-family: inherit; min-height: 44px; min-width: 44px;
}
input:focus-visible, select:focus-visible, textarea:focus-visible { border-color: #007bff; outline: none; }
@media (prefers-contrast: more) {
  button, input, select, textarea { border: 3px solid currentColor; font-weight: bold; }
  :focus-visible { outline-width: 4px; }
}
@media (prefers-reduced-motion: reduce) {
  * { animation: none !important; transition: none !important; }
}
/* Font scale — driven by --font-scale (settings.accessibility.font_scale) */
.app { font-size: calc(14px * var(--font-scale, 1)); }
header h1 { font-size: calc(20px * var(--font-scale, 1)); }
.panel h2 { font-size: calc(15px * var(--font-scale, 1)); }
h3 { font-size: calc(13px * var(--font-scale, 1)); }
.name { font-size: calc(13px * var(--font-scale, 1)); }
.field label { font-size: calc(12px * var(--font-scale, 1)); }
.tabs button { font-size: calc(13px * var(--font-scale, 1)); }
.muted, .err { font-size: calc(13px * var(--font-scale, 1)); }
.searchbar input, .searchbar button { font-size: calc(13px * var(--font-scale, 1)); }
.games li .size { font-size: calc(12px * var(--font-scale, 1)); }

/* ===== Toasts ===== */
.toasts {
  position: fixed; top: 16px; right: 16px; z-index: 1000;
  display: flex; flex-direction: column; gap: 8px; max-width: 320px;
}
.toast {
  background: #333; color: #fff; padding: 10px 14px; border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.3); font-size: 13px; word-break: break-word;
  border-left: 4px solid #6c757d; cursor: default;
}
.toast-info { border-left-color: #007bff; }
.toast-success { border-left-color: #28a745; }
.toast-error { border-left-color: #dc3545; }

/* ===== Confirm modal ===== */
.modal-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.45);
  display: flex; align-items: center; justify-content: center; z-index: 1100;
}
.modal {
  background: #fff; color: #333; border-radius: 12px; padding: 20px 24px;
  max-width: 90%; width: 380px; box-shadow: 0 20px 60px rgba(0,0,0,0.4);
}
.modal-title { margin: 0 0 8px; font-size: 16px; }
.modal-msg { margin: 0 0 16px; font-size: 14px; line-height: 1.4; }
.modal-actions { display: flex; gap: 10px; justify-content: flex-end; }
.modal-actions .btn { min-height: 40px; padding: 8px 16px; border: 1px solid #ccc; background: #f1f1f1; border-radius: 8px; }
.modal-actions .btn.primary { background: #007bff; border-color: #007bff; color: #fff; }
.modal-actions .btn:focus-visible { outline: 3px solid #007bff; outline-offset: 2px; }
</style>

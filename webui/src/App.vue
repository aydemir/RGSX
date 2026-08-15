<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { connectSSE, apiGet, apiPost } from './api.js'
import { t as _t, getLocale, setLocale, STRINGS } from './i18n.js'
import QBittorrent from './components/QBittorrent.vue'
import Support from './components/Support.vue'
import BrowseDirectories from './components/BrowseDirectories.vue'

const connected = ref(false)
const lastEvent = ref('')
const snapshot = reactive({ history: [], queue: [], active: false, progress: {}, downloaded: {} })
const progress = reactive({})

const tt = (k) => (STRINGS[locale.value] && STRINGS[locale.value][k]) || STRINGS.tr[k] || k
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
  display: { grid: '3x4', light_mode: false, font_family: 'Arial', monitor: '', fullscreen: false },
  show_unsupported_platforms: false,
  global_sort_option: 'name',
  max_simultaneous_downloads: 3,
  sources: { mode: 'rgsx', custom_url: '' },
  symlink: { enabled: false, target_directory: '' },
  accessibility: false,
  roms_folder: ''
}
function normalizeSettings(s) {
  s = s || {}
  const d = JSON.parse(JSON.stringify(DEFAULT_SETTINGS))
  Object.keys(d).forEach(k => { if (k !== 'display' && k !== 'sources' && k !== 'symlink' && s[k] !== undefined) d[k] = s[k] })
  d.display = Object.assign({}, DEFAULT_SETTINGS.display, s.display || {})
  d.sources = Object.assign({}, DEFAULT_SETTINGS.sources, s.sources || {})
  d.symlink = Object.assign({}, DEFAULT_SETTINGS.symlink, s.symlink || {})
  return d
}
const settings = ref(normalizeSettings(null))
const systemInfo = ref(null)
const languages = ref([])
const dataLang = ref('')
const saveMsg = ref('')
const openBrowse = ref(false)

// ===================== SSE =====================
let es = null
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
      snapshot.history = Array.isArray(data) ? data : (data && data.history) || snapshot.history
    },
    downloaded: (data) => {
      lastEvent.value = 'downloaded'
      snapshot.downloaded = (data && data.downloaded) || data || snapshot.downloaded
    },
  })
  await loadPlatforms()
  loadFiltersFromSettings()
  loadSettings()
})
onUnmounted(() => { es && es.close() })

// ===================== Catalog =====================
async function loadPlatforms() {
  catalogError.value = ''
  try {
    const p = await apiGet('/api/platforms')
    platforms.value = (p.platforms || []).slice(0, 200)
  } catch (e) { catalogError.value = tt('catalog_error') }
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
  } catch (e) { /* kuyruk SSE ile güncellenir */ }
}
async function downloadAll() {
  const names = filteredGames.value.map((g) => g.name)
  if (!names.length) return
  if (!confirm(`"${selectedPlatform.value}" için görünen ${names.length} oyun indirilsin mi?`)) return
  try {
    const r = await apiPost('/api/download/batch', { platform: selectedPlatform.value, game_names: names })
    saveMsg.value = (r && r.success) ? `Kuyruğa eklendi: ${r.queued || names.length}` : (r && r.error) || 'Hata'
    setTimeout(() => (saveMsg.value = ''), 4000)
  } catch (e) { saveMsg.value = 'Toplu indirme başarısız' }
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
  if (!confirm('Kuyruk tamamen temizlensin mi?')) return
  try { await apiPost('/api/queue/clear', {}) } catch (e) {}
}
async function cancelDownload(item) {
  try { await apiPost('/api/cancel', item.task_id ? { task_id: item.task_id } : { url: item.url }) } catch (e) {}
}
async function pauseAll() { try { await apiPost('/api/pause', {}) } catch (e) {} }
async function resumeAll() { try { await apiPost('/api/resume', {}) } catch (e) {} }

// ===================== History =====================
const historyItems = computed(() => snapshot.history || [])
function historyStatusClass(s) {
  s = String(s || '').toLowerCase()
  if (s === 'erreur' || s === 'error' || s === 'failed') return 'st-err'
  if (s === 'canceled' || s === 'cancelled') return 'st-cancel'
  if (s === 'already_present') return 'st-info'
  if (['queued', 'downloading', 'connecting', 'extracting'].includes(s) || s.startsWith('try ')) return 'st-run'
  return 'st-ok'
}
function historyStatusText(s) {
  s = String(s || '')
  if (s === 'Erreur' || s === 'error' || s === 'failed') return 'Hata'
  if (s === 'Canceled') return 'İptal'
  if (s === 'Already_Present') return 'Zaten var'
  if (s === 'Queued') return 'Kuyrukta'
  if (['Downloading', 'Connecting', 'Extracting'].includes(s) || s.startsWith('Try ')) return 'İndiriliyor'
  return 'Tamamlandı'
}
async function clearHistory() {
  if (!confirm('Geçmiş temizlensin mi? (geri alınamaz)')) return
  try { await apiPost('/api/clear-history', {}) } catch (e) {}
}

// ===================== Downloaded (İndirilenler) =====================
const downloadedItems = computed(() => {
  const map = {}
  const st = gameStatuses.value || {}
  for (const [k, v] of Object.entries(st)) {
    if (v && v.status === 'downloaded') map[k] = { name: k, platform: v.platform || '' }
  }
  const dl = snapshot.downloaded || {}
  for (const [plat, names] of Object.entries(dl)) {
    for (const n of (names || [])) if (!map[n]) map[n] = { name: n, platform: plat }
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
  apiPost('/api/download', { url: g.url, platform: g.platform || '', game_name: g.game_name || g.name || '', mode: 'queue' }).catch(() => {})
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
  saveMsg.value = ''
  try {
    await apiPost('/api/settings', { settings: settings.value })
    saveMsg.value = 'Ayarlar kaydedildi'
    await loadSettings()
  } catch (e) { saveMsg.value = 'Kaydetme başarısız' }
  setTimeout(() => (saveMsg.value = ''), 3000)
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
  saveMsg.value = tt('browse_restart_note')
  setTimeout(() => { if (saveMsg.value === tt('browse_restart_note')) saveMsg.value = '' }, 6000)
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
  <div class="app">
    <header>
      <h1>{{ tt('app_title') }}</h1>
      <span class="status" :class="{ on: connected }">{{ connected ? tt('status_connected') : tt('status_connecting') }}</span>
      <span class="active" v-if="snapshot.active">● aktif indirme</span>
      <button class="gear" :class="{ on: tab === 'settings' }" @click="switchTab('settings')" title="Ayarlar">⚙</button>
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
            <span class="st" :class="'s-' + (gameStatusOf(g) ? gameStatusOf(g).status : '')">
              {{ gameStatusOf(g) ? (gameStatusOf(g).status === 'downloaded' ? '✓' : gameStatusOf(g).status === 'downloading' ? '~' + (gameStatusOf(g).progress || 0) + '%' : gameStatusOf(g).status === 'failed' ? '✗' : '') : '' }}
            </span>
            <div class="row"><span class="name">{{ g.name }}</span><span class="size">{{ g.size || '' }}</span></div>
            <div class="dlgrp">
              <button class="dlbtn" :disabled="!g.url" @click="downloadGame(g, 'now')" title="Şimdi indir">⬇️</button>
              <button class="dlbtn q" :disabled="!g.url" @click="downloadGame(g, 'queue')" title="Kuyruğa ekle">➕</button>
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
            <span class="pct">{{ queuePct(item) != null ? queuePct(item) + '%' : (item.status || '') }}</span>
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
        <li v-for="(item, i) in historyItems" :key="item.task_id || i" :class="historyStatusClass(item.status)">
          <div class="row">
            <span class="name">{{ item.game_name || item.name }}</span>
            <span class="pct">{{ historyStatusText(item.status) }}</span>
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
        <div class="field">
          <label>{{ tt('roms_folder') }}</label>
          <div class="browse-row">
            <input type="text" v-model="settings.roms_folder" @change="saveSettings()" placeholder="varsayılan" />
            <button class="browse-btn" @click="openBrowse = true">📂 {{ tt('browse') }}</button>
          </div>
        </div>
        <BrowseDirectories v-if="openBrowse" :current-path="settings.roms_folder" @select="onBrowseSelect" @close="openBrowse = false" />
        <QBittorrent />
        <p v-if="saveMsg" class="saved">{{ saveMsg }}</p>
      </template>
      <p v-else class="muted">{{ tt('no_settings') }}</p>
    </section>

    <footer class="muted">{{ tt('last_event') }}: {{ lastEvent }}</footer>
  </div>
</template>

<style>
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: #0e1116; color: #e6edf3; }
.app { max-width: 960px; margin: 0 auto; padding: 24px; }
header { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
h1 { font-size: 20px; margin: 0; }
.status { font-size: 12px; padding: 2px 8px; border-radius: 999px; background: #30363d; }
.status.on { background: #1f6f3f; }
.active { color: #58a6ff; font-size: 12px; }
.gear { margin-left: auto; background: #21262d; border: 1px solid #30363d; border-radius: 6px; color: inherit; font-size: 16px; width: 32px; height: 32px; cursor: pointer; }
.gear.on { background: #1f6feb; border-color: #1f6feb; }

.searchbar { display: flex; gap: 8px; margin: 16px 0; }
.searchbar input { flex: 1; background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 8px 10px; color: inherit; font-size: 13px; }
.searchbar button { background: #21262d; border: 1px solid #30363d; border-radius: 6px; padding: 6px 14px; color: inherit; cursor: pointer; }

.tabs { display: flex; gap: 6px; flex-wrap: wrap; border-bottom: 1px solid #21262d; padding-bottom: 8px; }
.tabs button { background: #161b22; border: 1px solid #21262d; color: #8b949e; border-radius: 8px 8px 0 0; padding: 8px 14px; cursor: pointer; font-size: 13px; }
.tabs button.active { background: #1f6feb; border-color: #1f6feb; color: #fff; }

.panel { margin-top: 12px; }
h2 { font-size: 15px; margin: 16px 0 8px; display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
h3 { font-size: 13px; color: #8b949e; margin: 12px 0 4px; }
small { color: #8b949e; font-weight: normal; }
.muted { color: #8b949e; font-size: 13px; }
.err { color: #ff7b72; background: #2d1418; padding: 8px 12px; border-radius: 8px; font-size: 13px; }

.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; }
.card { display: flex; flex-direction: column; align-items: center; gap: 6px; background: #161b22; border: 1px solid #21262d; border-radius: 10px; padding: 10px; cursor: pointer; color: inherit; }
.card:hover { border-color: #1f6feb; background: #15233b; }
.card .box { width: 64px; height: 64px; object-fit: contain; border-radius: 6px; background: #0e1116; }
.card .pname { font-size: 12px; text-align: center; }
.card .count { font-size: 10px; color: #8b949e; }

.games { list-style: none; padding: 0; margin: 0; }
.games li { display: flex; align-items: center; gap: 12px; padding: 10px 0; border-bottom: 1px solid #21262d; }
.games li .row { flex: 1; display: flex; justify-content: space-between; gap: 12px; }
.name { font-size: 13px; }
.size { color: #8b949e; font-size: 12px; white-space: nowrap; }
.dlbtn { background: #1f6feb; color: #fff; border: 0; border-radius: 6px; padding: 6px 10px; font-size: 13px; cursor: pointer; }
.dlbtn:disabled { background: #30363d; color: #8b949e; cursor: not-allowed; }
.dlbtn.q { background: #30363d; }
.dlgrp { display: flex; gap: 4px; }

.back { font-size: 11px; color: #58a6ff; cursor: pointer; margin-left: 8px; }

/* Filters */
.filters { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin: 8px 0 12px; background: #161b22; border: 1px solid #21262d; border-radius: 8px; padding: 10px; }
.filters .gfilt { flex: 1; min-width: 160px; background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 6px 10px; color: inherit; font-size: 13px; }
.regions { display: flex; flex-wrap: wrap; gap: 4px; }
.rbtn { font-size: 11px; background: #21262d; border: 1px solid #30363d; color: #8b949e; border-radius: 6px; padding: 4px 8px; cursor: pointer; }
.rbtn.include { background: #1f6f3f; color: #fff; border-color: #1f6f3f; }
.rbtn.exclude { background: #6e2b2b; color: #fff; border-color: #6e2b2b; }
.chk { font-size: 12px; color: #c9d1d9; display: flex; align-items: center; gap: 4px; }
.filters select { background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 6px; color: inherit; font-size: 12px; }
.reset { background: #21262d; border: 1px solid #30363d; color: inherit; border-radius: 6px; padding: 6px 10px; cursor: pointer; font-size: 12px; }
.dlall { background: #2f8f46; color: #fff; border: 0; border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 12px; }

/* Status badges */
.st { font-weight: bold; margin-right: 6px; min-width: 28px; display: inline-block; text-align: center; }
.s-downloaded { color: #66ff66; }
.s-downloading { color: #ffcc00; }
.s-failed { color: #ff5555; }

/* Queue / downloads */
.dl { list-style: none; padding: 0; margin: 0; }
.dl li { padding: 10px 0; border-bottom: 1px solid #21262d; }
.dl .row { display: flex; justify-content: space-between; font-size: 13px; }
.pct { color: #58a6ff; font-variant-numeric: tabular-nums; }
.bar { height: 8px; background: #21262d; border-radius: 6px; margin-top: 6px; overflow: hidden; }
.fill { height: 100%; background: linear-gradient(90deg, #1f6feb, #58a6ff); transition: width .3s ease; }
.qmeta { display: flex; gap: 10px; align-items: center; margin-top: 4px; }
.qacts { margin-left: auto; display: flex; gap: 6px; }
.danger { background: #6e2b2b; color: #fff; border: 0; border-radius: 6px; padding: 4px 10px; cursor: pointer; font-size: 12px; }
.danger:hover { background: #8a3535; }

/* History */
.hist { list-style: none; padding: 0; margin: 0; }
.hist li { padding: 10px 0; border-bottom: 1px solid #21262d; }
.hist li.st-ok { border-left: 3px solid #2f8f46; padding-left: 8px; }
.hist li.st-err { border-left: 3px solid #ff5555; padding-left: 8px; }
.hist li.st-cancel { border-left: 3px solid #d29922; padding-left: 8px; }
.hist li.st-info { border-left: 3px solid #58a6ff; padding-left: 8px; }
.hist li.st-run { border-left: 3px solid #1f6feb; padding-left: 8px; }

/* Settings */
.field { margin: 12px 0; }
.field label { display: block; font-size: 12px; color: #8b949e; margin-bottom: 4px; }
.field select, .field input[type=text], .field input[type=number] { background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 6px 10px; color: inherit; font-size: 13px; }
.saved { color: #66ff66; font-size: 13px; }
.browse-row { display: flex; gap: 8px; }
.browse-row input { flex: 1; background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 6px 10px; color: inherit; font-size: 13px; }
.browse-btn { background: #007bff; color: #fff; border: 0; border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 13px; white-space: nowrap; }
.browse-btn:hover { background: #0069d9; }
</style>

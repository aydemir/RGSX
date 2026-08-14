<script setup>
import { ref, reactive, onMounted, onUnmounted, computed } from 'vue'
import { connectSSE, apiGet, apiPost } from './api.js'

const connected = ref(false)
const snapshot = reactive({ history: [], queue: [], active: false, progress: {}, downloaded: {} })
const progress = reactive({})
const lastEvent = ref('')
const tv = ref(new URLSearchParams(location.search).get('mode') === 'tv')
const selected = ref(0)        // kuyruk seçimi (TV)
const selPlatform = ref(0)     // platform ızgarası seçimi (TV)
const selGame = ref(0)         // oyun listesi seçimi (TV)

let es = null

// --- Katalog tarama durumu (native backend uçları) ---
const platforms = ref([])
const selectedPlatform = ref(null)   // platform_name
const games = ref([])
const searchTerm = ref('')
const searchResults = ref(null)      // { platforms:[], games:[] }
const catalogLoading = ref(false)
const catalogError = ref('')

async function loadPlatforms() {
  catalogError.value = ''
  try {
    const p = await apiGet('/api/platforms')
    platforms.value = (p.platforms || []).slice(0, 80)
    selPlatform.value = 0
  } catch (e) {
    catalogError.value = 'Katalog yüklenemedi (RGSX_NATIVE_CATALOG=1 ve veri gerekli)'
  }
}

async function selectPlatform(name) {
  selectedPlatform.value = name
  searchResults.value = null
  selGame.value = 0
  games.value = []
  catalogLoading.value = true
  try {
    const g = await apiGet('/api/games/' + encodeURIComponent(name))
    games.value = g.games || []
  } catch (e) {
    catalogError.value = 'Oyun listesi alınamadı'
  } finally {
    catalogLoading.value = false
  }
}

async function doSearch() {
  const q = searchTerm.value.trim()
  if (!q) { searchResults.value = null; return }
  catalogLoading.value = true
  try {
    const r = await apiGet('/api/search?q=' + encodeURIComponent(q))
    searchResults.value = r.results || { platforms: [], games: [] }
  } catch (e) {
    catalogError.value = 'Arama başarısız'
  } finally {
    catalogLoading.value = false
  }
}

async function downloadGame(g) {
  if (!g.url) return
  try {
    await apiPost('/api/download', {
      url: g.url,
      platform: selectedPlatform.value || g.platform || '',
      game_name: g.name || g.game_name || '',
    })
  } catch (e) { /* kuyruk zaten SSE ile güncellenir */ }
}

onMounted(async () => {
  es = connectSSE({
    snapshot: (data) => {
      connected.value = true
      snapshot.history = data.history || []
      snapshot.queue = data.queue || []
      snapshot.active = data.active || false
      snapshot.progress = data.progress || {}
      snapshot.downloaded = data.downloaded || {}
      Object.assign(progress, data.progress || {})
      lastEvent.value = 'snapshot'
    },
    progress: (data) => {
      lastEvent.value = 'progress'
      Object.assign(progress, data)
    },
    queue: (data) => { lastEvent.value = 'queue'; snapshot.queue = data.queue || data || [] },
    history: (data) => { lastEvent.value = 'history'; snapshot.history = data.history || data || [] },
    downloaded: (data) => { lastEvent.value = 'downloaded' },
  })
  await loadPlatforms()
  if (tv.value) {
    window.addEventListener('keydown', onKey)
    gamepadTimer = setInterval(pollGamepad, 100)
  }
})

onUnmounted(() => {
  es && es.close()
  window.removeEventListener('keydown', onKey)
  if (gamepadTimer) clearInterval(gamepadTimer)
})

// TV modu: ok tuşları / gamepad ile katalog + kuyruk gezinmesi.
let gamepadTimer = null
function activeKind() {
  if (searchResults.value) return 'search'
  if (selectedPlatform.value) return 'games'
  return 'platforms'
}
function activeList() {
  const k = activeKind()
  if (k === 'platforms') return platforms.value
  if (k === 'games') return games.value
  if (k === 'search') return searchResults.value?.games || []
  return queueItems.value
}
function move(dir) {
  const list = activeList()
  const n = list.length
  if (!n) return
  if (activeKind() === 'platforms') selPlatform.value = (selPlatform.value + dir + n) % n
  else if (activeKind() === 'games' || activeKind() === 'search') selGame.value = (selGame.value + dir + n) % n
  else selected.value = (selected.value + dir + n) % n
}
function activate() {
  const k = activeKind()
  if (k === 'platforms') {
    const p = platforms.value[selPlatform.value]
    if (p) selectPlatform(p.platform_name || p.name)
  } else if (k === 'games') {
    const g = games.value[selGame.value]
    if (g) downloadGame(g)
  } else if (k === 'search') {
    const g = (searchResults.value?.games || [])[selGame.value]
    if (g) downloadGame(g)
  }
}
function onKey(e) {
  if (e.key === 'ArrowDown') { move(1); e.preventDefault() }
  else if (e.key === 'ArrowUp') { move(-1); e.preventDefault() }
  else if (e.key === 'Enter') { activate(); e.preventDefault() }
}
function pollGamepad() {
  const pads = navigator.getGamepads ? navigator.getGamepads() : []
  for (const p of pads) {
    if (!p) continue
    const [ax] = p.axes
    if (ax !== undefined && Math.abs(ax) > 0.6) { move(ax > 0 ? 1 : -1); break }
    for (let i = 0; i < p.buttons.length; i++) {
      if (p.buttons[i].pressed) {
        if (i === 12) move(-1)
        else if (i === 13) move(1)
        else if (i === 0) activate()   // gamepad A = seç
        break
      }
    }
  }
}

const queueItems = computed(() => snapshot.queue || [])
const pct = (id) => {
  const v = progress[id] || progress[String(id)]
  if (!v) return null
  if (typeof v.percent === 'number') return v.percent
  if (typeof v === 'number') return v
  return null
}
</script>

<template>
  <div class="app" :class="{ tv: tv }">
    <header>
      <h1>RGSX Manager</h1>
      <span class="status" :class="{ on: connected }">
        {{ connected ? 'SSE bağlı' : 'bağlanıyor…' }}
      </span>
      <span class="active" v-if="snapshot.active">● aktif indirme</span>
    </header>

    <p v-if="catalogError" class="err">{{ catalogError }}</p>

    <section v-if="!searchResults">
      <h2>Platformlar <small>({{ platforms.length }})</small></h2>
      <div class="grid">
        <button v-for="(p, i) in platforms" :key="p.platform_name || p.name"
                class="card" :class="{ sel: tv && selPlatform === i }"
                @click="selPlatform = i; selectPlatform(p.platform_name || p.name)">
          <img v-if="p.platform_image" :src="'/api/image/' + encodeURIComponent(p.platform_name || p.name)" class="box" alt="" />
          <span class="pname">{{ p.platform_name || p.name }}</span>
          <span class="count" v-if="p.games_count != null">{{ p.games_count }} oyun</span>
        </button>
      </div>
    </section>

    <section v-if="selectedPlatform && !searchResults">
      <h2>
        {{ selectedPlatform }}
        <small>({{ games.length }} oyun)</small>
        <a class="back" @click="selectedPlatform = null; games = []">← geri</a>
      </h2>
      <p v-if="catalogLoading" class="muted">yükleniyor…</p>
      <ul class="games">
        <li v-for="(g, i) in games" :key="g.name || g.url || i" :class="{ sel: tv && selGame === i }" @click="selGame = i">
          <div class="row">
            <span class="name">{{ g.name || g.game_name }}</span>
            <span class="size">{{ g.size || '' }}</span>
          </div>
          <button class="dlbtn" :disabled="!g.url" @click="downloadGame(g)">İndir</button>
        </li>
      </ul>
    </section>

    <section>
      <h2>Arama</h2>
      <div class="search">
        <input v-model="searchTerm" @keyup.enter="doSearch" placeholder="oyun / platform ara…" />
        <button @click="doSearch">Ara</button>
        <button v-if="searchResults" @click="searchResults = null; searchTerm = ''">Temizle</button>
      </div>
      <div v-if="searchResults" class="results">
        <h3>Oyunlar</h3>
        <ul class="games">
          <li v-for="(g, i) in searchResults.games" :key="g.game_name || g.url || i" :class="{ sel: tv && selGame === i }" @click="selGame = i">
            <div class="row">
              <span class="name">{{ g.game_name }} <small>({{ g.platform }})</small></span>
              <span class="size">{{ g.size || '' }}</span>
            </div>
            <button class="dlbtn" :disabled="!g.url" @click="downloadGame(g)">İndir</button>
          </li>
        </ul>
        <p v-if="!searchResults.games.length" class="muted">sonuç yok</p>
      </div>
    </section>

    <section>
      <h2>İndirmeler <small>(canlı)</small></h2>
      <p v-if="!queueItems.length" class="muted">Kuyruk boş</p>
      <ul class="dl">
        <li v-for="(item, i) in queueItems" :key="item.id || item.url || i"
            :class="{ sel: tv && i === selected }">
          <div class="row">
            <span class="name">{{ item.name || item.game_name || item.url || ('#' + i) }}</span>
            <span class="pct">{{ pct(item.id ?? item.game_index) != null ? pct(item.id ?? item.game_index) + '%' : '—' }}</span>
          </div>
          <div class="bar">
            <div class="fill" :style="{ width: (pct(item.id ?? item.game_index) ?? 0) + '%' }"></div>
          </div>
        </li>
      </ul>
    </section>

    <section>
      <h2>İlerleme haritası <small>(ham SSE)</small></h2>
      <pre class="mono">{{ JSON.stringify(progress, null, 2) }}</pre>
    </section>

    <footer class="muted">son olay: {{ lastEvent }} · refresh gerektirmez</footer>
  </div>
</template>

<style>
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: #0e1116; color: #e6edf3; }
.app { max-width: 880px; margin: 0 auto; padding: 24px; }
header { display: flex; align-items: center; gap: 12px; }
h1 { font-size: 20px; margin: 0; }
.status { font-size: 12px; padding: 2px 8px; border-radius: 999px; background: #30363d; }
.status.on { background: #1f6f3f; }
.active { color: #58a6ff; font-size: 12px; }
nav { display: flex; flex-wrap: wrap; gap: 6px; margin: 16px 0; }
.chip { font-size: 11px; background: #21262d; padding: 3px 8px; border-radius: 6px; }
h2 { font-size: 15px; margin: 20px 0 8px; }
small { color: #8b949e; font-weight: normal; }
.muted { color: #8b949e; font-size: 13px; }
.dl { list-style: none; padding: 0; margin: 0; }
.dl li { padding: 10px 0; border-bottom: 1px solid #21262d; }
.row { display: flex; justify-content: space-between; font-size: 13px; }
.pct { color: #58a6ff; font-variant-numeric: tabular-nums; }
.bar { height: 8px; background: #21262d; border-radius: 6px; margin-top: 6px; overflow: hidden; }
.fill { height: 100%; background: linear-gradient(90deg, #1f6feb, #58a6ff); transition: width .3s ease; }
.mono { background: #161b22; padding: 12px; border-radius: 8px; font-size: 12px; max-height: 240px; overflow: auto; }
.err { color: #ff7b72; background: #2d1418; padding: 8px 12px; border-radius: 8px; font-size: 13px; }

/* Katalog tarayıcı */
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; }
.card { display: flex; flex-direction: column; align-items: center; gap: 6px; background: #161b22; border: 1px solid #21262d; border-radius: 10px; padding: 10px; cursor: pointer; color: inherit; }
.card:hover, .card.sel { border-color: #1f6feb; background: #15233b; }
.card .box { width: 64px; height: 64px; object-fit: contain; border-radius: 6px; background: #0e1116; }
.card .pname { font-size: 12px; text-align: center; }
.card .count { font-size: 10px; color: #8b949e; }
.games { list-style: none; padding: 0; margin: 0; }
.games li { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; border-bottom: 1px solid #21262d; }
.games li.sel { background: #15233b; border-radius: 8px; padding-left: 8px; padding-right: 8px; }
.games .size { color: #8b949e; font-size: 12px; }
.dlbtn { background: #1f6feb; color: #fff; border: 0; border-radius: 6px; padding: 6px 12px; font-size: 12px; cursor: pointer; }
.dlbtn:disabled { background: #30363d; color: #8b949e; cursor: not-allowed; }
.search { display: flex; gap: 8px; }
.search input { flex: 1; background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 8px 10px; color: inherit; font-size: 13px; }
.search button { background: #21262d; border: 1px solid #30363d; border-radius: 6px; padding: 6px 12px; color: inherit; cursor: pointer; }
.results h3 { font-size: 13px; color: #8b949e; margin: 12px 0 4px; }
.back { font-size: 11px; color: #58a6ff; cursor: pointer; margin-left: 8px; }

/* TV modu: 10-foot UI — büyük font, ölçekli layout, seçili vurgu */
.app.tv { max-width: none; padding: 4vh 6vw; }
.app.tv h1 { font-size: 3.2vh; }
.app.tv h2 { font-size: 3vh; margin: 4vh 0 2vh; }
.app.tv .dl li { padding: 2.4vh 0; }
.app.tv .row { font-size: 2.6vh; }
.app.tv .pct { font-size: 2.6vh; }
.app.tv .bar { height: 2.2vh; border-radius: 1vh; }
.app.tv .dl li.sel { background: #15233b; border-radius: 1vh; padding-left: 2vh; padding-right: 2vh; }
.app.tv .chip { font-size: 2vh; padding: 0.6vh 1.4vh; }
</style>

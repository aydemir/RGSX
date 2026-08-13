<script setup>
import { ref, reactive, onMounted, onUnmounted, computed } from 'vue'
import { connectSSE, apiGet } from './api.js'

const connected = ref(false)
const snapshot = reactive({ history: [], queue: [], active: false, progress: {}, downloaded: {} })
const progress = reactive({})
const lastEvent = ref('')
const platforms = ref([])
const tv = ref(new URLSearchParams(location.search).get('mode') === 'tv')
const selected = ref(0)

let es = null

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
  try {
    const p = await apiGet('/api/platforms')
    platforms.value = (p.platforms || p.result?.platforms || []).slice(0, 12)
  } catch (e) { /* katalog kapalı olabilir */ }
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

// TV modu: ok tuşları / gamepad ile seçim gezinmesi.
let gamepadTimer = null
function move(dir) {
  const n = queueItems.value.length
  if (!n) return
  selected.value = (selected.value + dir + n) % n
}
function onKey(e) {
  if (e.key === 'ArrowDown') { move(1); e.preventDefault() }
  else if (e.key === 'ArrowUp') { move(-1); e.preventDefault() }
  else if (e.key === 'Enter') { e.preventDefault() }
}
function pollGamepad() {
  const pads = navigator.getGamepads ? navigator.getGamepads() : []
  for (const p of pads) {
    if (!p) continue
    const [ax] = p.axes
    if (ax !== undefined && Math.abs(ax) > 0.6) { move(ax > 0 ? 1 : -1); break }
    for (let i = 0; i < p.buttons.length; i++) {
      if (p.buttons[i].pressed) { if (i === 12) move(-1); else if (i === 13) move(1); break }
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

    <nav v-if="platforms.length">
      <span v-for="p in platforms" :key="p.name || p" class="chip">{{ p.name || p }}</span>
    </nav>

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

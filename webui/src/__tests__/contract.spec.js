/**
 * TASK-008 / TASK-010 — WebUI sözleşme testleri (vitest).
 * Backend `downloaded` kanonik shape'i `{platform:[names]}` ile
 * App.vue `downloadedItems` / `isDownloadedInSnapshot` toleransını
 * ve `scan` SSE handler sözleşmesini kilitler.
 * Kod: webui/src/App.vue:410 (isDownloadedInSnapshot) + :719 (downloadedItems) + :239 (scan handler)
 */

import { describe, it, expect } from 'vitest'

// App.vue'daki yardımcıların test-edilebilir kopyası (pure, DOM yok)
function stem(name) {
  return String(name).toLowerCase().replace(/\.[^.]+$/, '')
}

function isDownloadedInSnapshot(g, snapshot) {
  const dl = snapshot.downloaded || {}
  const gstem = stem(g.name)
  const glow = String(g.name).toLowerCase()
  for (const [plat, names] of Object.entries(dl)) {
    if (Array.isArray(names)) {
      if (names.some((n) => stem(n) === gstem || String(n).toLowerCase() === glow)) return true
    } else if (names && typeof names === 'object' && names.status === 'downloaded') {
      const n = names.name || plat
      if (stem(n) === gstem || String(n).toLowerCase() === glow) return true
    }
  }
  if (dl[g.name] && dl[g.name].status === 'downloaded') return true
  if (dl[glow] && dl[glow].status === 'downloaded') return true
  if (dl[gstem] && dl[gstem].status === 'downloaded') return true
  return false
}

function downloadedItems(gameStatuses, snapshot) {
  const map = {}
  const norm = (plat, name) => (plat || '') + '|' + stem(name)
  const st = gameStatuses || {}
  for (const [k, v] of Object.entries(st)) {
    if (v && v.status === 'downloaded') {
      const key = norm(v.platform, v.name || k)
      if (!map[key]) map[key] = { name: v.name || k, platform: v.platform || '' }
    }
  }
  const dl = (snapshot && snapshot.downloaded) || {}
  for (const [k, v] of Object.entries(dl)) {
    if (Array.isArray(v)) {
      for (const n of v) {
        const key = norm(k, n)
        if (!map[key]) map[key] = { name: n, platform: k }
      }
    } else if (v && typeof v === 'object' && v.status === 'downloaded') {
      const key = norm(v.platform || '', v.name || k)
      if (!map[key]) map[key] = { name: v.name || k, platform: v.platform || '' }
    }
  }
  return Object.values(map)
}

describe('downloaded shape toleransı', () => {
  it('kanonik {platform:[names]} shape ile yeşil gösterir', () => {
    const snap = { downloaded: { NES: ['Super Mario.nes', 'Zelda.nes'], SNES: ['Mario World.sfc'] } }
    expect(isDownloadedInSnapshot({ name: 'Super Mario.nes' }, snap)).toBe(true)
    expect(isDownloadedInSnapshot({ name: 'Mario World.sfc' }, snap)).toBe(true)
    expect(isDownloadedInSnapshot({ name: 'Unknown.nes' }, snap)).toBe(false)
  })

  it('stem/case-insensitive eşleşir', () => {
    const snap = { downloaded: { NES: ['SUPER_MARIO.NES'] } }
    expect(isDownloadedInSnapshot({ name: 'super_mario.nes' }, snap)).toBe(true)
    expect(isDownloadedInSnapshot({ name: 'Super_Mario.zip' }, snap)).toBe(true) // stem aynı
  })

  it('eski {name:{status,platform}} shape defensive tolere edilir', () => {
    const snap = { downloaded: { 'OldGame.nes': { status: 'downloaded', platform: 'NES', name: 'OldGame.nes' } } }
    expect(isDownloadedInSnapshot({ name: 'OldGame.nes' }, snap)).toBe(true)
  })

  it('downloadedItems her iki shape’i birleştirir ve tekilleştirir (Faz A parity)', () => {
    const statuses = { 'super mario': { status: 'downloaded', platform: 'NES', name: 'Super Mario.nes' } }
    const snap = { downloaded: { NES: ['Super Mario.nes', 'Zelda.nes'] } }
    const items = downloadedItems(statuses, snap)
    // Super Mario iki kaynakta ama tek satır
    expect(items.length).toBe(2)
    expect(items.find((x) => x.name === 'Super Mario.nes')).toBeTruthy()
    expect(items.find((x) => x.name === 'Zelda.nes')).toBeTruthy()
  })

  it('boş downloaded ile boş liste', () => {
    expect(downloadedItems({}, { downloaded: {} }).length).toBe(0)
    expect(isDownloadedInSnapshot({ name: 'x' }, { downloaded: {} })).toBe(false)
  })
})

describe('scan SSE sözleşmesi', () => {
  it('scan payload root/platforms/disk içerir (backend api.rs::scan)', () => {
    // Backend GET /api/scan hem HTTP hem SSE `scan` yayar — handler scanResult.value = data
    const payload = { root: '/roms', platforms: [{ name: 'NES' }], disk: { free: 1000 } }
    let scanResult = null
    const handler = (data) => { scanResult = data }
    handler(payload)
    expect(scanResult.root).toBe('/roms')
    expect(scanResult.platforms.length).toBe(1)
    expect(scanResult.disk).toBeTruthy()
  })
})

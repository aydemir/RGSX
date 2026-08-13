// RGSX WebUI — SSE ile canlı durum. Backend `/api/events` ham SSE yayınlar:
//   event: snapshot\ndata: {history,queue,active,progress,downloaded}\n
//   event: progress\ndata: {<id>: {percent,bytes,total,...}}\n
// Tarayıcı EventSource yalnız sunucu→istemci akış için; komutlar REST POST ile.

export function connectSSE(handlers) {
  const es = new EventSource('/api/events')
  for (const [type, cb] of Object.entries(handlers)) {
    es.addEventListener(type, (e) => {
      try {
        cb(JSON.parse(e.data))
      } catch (err) {
        console.warn('SSE parse hatası', type, err)
      }
    })
  }
  es.onerror = () => console.warn('SSE bağlantı hatası (yeniden denenecek)')
  return es
}

export async function apiGet(path) {
  const r = await fetch(path)
  if (!r.ok) throw new Error(`${path} -> ${r.status}`)
  return r.json()
}

export async function apiPost(path, body) {
  const r = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body || {}),
  })
  return r.json()
}

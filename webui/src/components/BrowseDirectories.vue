<script setup>
import { ref } from 'vue'
import { t, STRINGS, getLocale } from '../i18n.js'

const props = defineProps({
  currentPath: { type: String, default: '' },
})
const emit = defineEmits(['select', 'close'])

const locale = ref(getLocale())
const open = ref(true)
const loading = ref(false)
const errorMsg = ref('')
const current = ref('')
const dirs = ref([])
const parentPath = ref(null)

function loc(key) {
  const table = STRINGS[locale.value] || STRINGS.tr
  return (table && table[key]) || (STRINGS.en && STRINGS.en[key]) || STRINGS.tr[key] || key
}

function deriveParent(p) {
  if (!p) return null
  const idx = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  if (idx <= 0) return null
  return p.slice(0, idx)
}

async function load(path) {
  loading.value = true
  errorMsg.value = ''
  try {
    const url = '/api/browse-directories' + (path ? ('?path=' + encodeURIComponent(path)) : '')
    const res = await fetch(url)
    const data = await res.json().catch(() => ({}))
    if (!res.ok || data.success === false) {
      errorMsg.value = loc('browse_error') + (data.error || ('HTTP ' + res.status))
      return
    }
    current.value = data.current_path || path || ''
    dirs.value = Array.isArray(data.directories) ? data.directories : []
    parentPath.value =
      typeof data.parent_path === 'string' ? data.parent_path
        : data.parent_path === null ? null
          : deriveParent(current.value)
  } catch (e) {
    errorMsg.value = loc('browse_error') + (e && e.message ? e.message : String(e))
  } finally {
    loading.value = false
  }
}

function gotoDir(d) { load(d.path) }
function gotoParent() { load(parentPath.value || '') }
function selectCurrent() {
  emit('select', current.value)
  closeModal()
}
function closeModal() {
  open.value = false
  emit('close')
}

load(props.currentPath || '')
</script>

<template>
  <div v-if="open" class="browse-overlay" @click.self="closeModal">
    <div class="browse-modal">
      <h3 class="browse-title">📂 {{ loc('browse_title') }}</h3>
      <div class="browse-path">{{ current || loc('browse_drives') }}</div>

      <p v-if="errorMsg" class="browse-err">{{ errorMsg }}</p>
      <p v-else-if="loading" class="browse-muted">{{ loc('loading') }}</p>

      <div class="browse-list">
        <p v-if="!loading && !errorMsg && dirs.length === 0" class="browse-empty">{{ loc('browse_empty') }}</p>
        <div v-for="d in dirs" :key="d.path" class="browse-item" @click="gotoDir(d)">
          <span class="browse-icon">{{ d.is_drive ? '💾' : '📁' }}</span>
          <span class="browse-name">{{ d.name }}</span>
        </div>
      </div>

      <div class="browse-actions">
        <button v-if="parentPath !== null" class="b-btn parent" @click="gotoParent">
          {{ parentPath === '' ? '💾 ' + loc('browse_drives') : '⬆️ ' + loc('browse_parent') }}
        </button>
        <button v-if="current" class="b-btn select" @click="selectCurrent">✅ {{ loc('browse_select') }}</button>
        <button class="b-btn cancel" @click="closeModal">❌ {{ loc('browse_cancel') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.browse-overlay {
  position: fixed; inset: 0; background: rgba(0, 0, 0, 0.8);
  display: flex; align-items: center; justify-content: center; padding: 20px; z-index: 9999;
}
.browse-modal {
  background: #161b22; border: 1px solid #30363d; border-radius: 10px;
  padding: 16px; max-width: 520px; width: 100%; max-height: 80vh; overflow-y: auto; color: #e6edf3;
}
.browse-title { margin: 0 0 10px; font-size: 15px; }
.browse-path {
  background: #0e1116; border: 1px solid #30363d; border-radius: 6px; padding: 8px 10px;
  font-family: ui-monospace, monospace; font-size: 13px; color: #58a6ff;
  word-break: break-all; margin-bottom: 10px;
}
.browse-list {
  border: 1px solid #30363d; border-radius: 6px; max-height: 320px; overflow-y: auto;
  margin-bottom: 12px;
}
.browse-item {
  display: flex; align-items: center; gap: 8px; padding: 10px 12px;
  border-bottom: 1px solid #21262d; cursor: pointer;
}
.browse-item:last-child { border-bottom: 0; }
.browse-item:hover { background: #21262d; }
.browse-icon { font-size: 18px; }
.browse-name { flex: 1; font-size: 13px; }
.browse-err { color: #dc3545; background: rgba(220, 53, 69, 0.12); border: 1px solid #dc3545; padding: 8px 10px; border-radius: 6px; font-size: 13px; margin: 0 0 10px; }
.browse-muted { color: #8b949e; font-size: 13px; margin: 0 0 10px; }
.browse-empty { color: #8b949e; font-size: 13px; text-align: center; padding: 20px; margin: 0; }
.browse-actions { display: flex; gap: 8px; justify-content: flex-end; }
.b-btn { padding: 8px 12px; border: 0; border-radius: 6px; color: #fff; cursor: pointer; font-weight: bold; font-size: 13px; }
.b-btn.parent { background: #6c757d; }
.b-btn.select { background: #28a745; }
.b-btn.cancel { background: #dc3545; }
</style>

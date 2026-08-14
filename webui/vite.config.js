import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Build çıktısı `dist/` — Rust `RGSX_WEBUI_DIR=webui/dist` ile sunulur.
// `base: '/static/'` — Rust router statik dosyaları yalnız `/static/*` altında
// sunduğundan asset referansları `/static/assets/...` olmalıdır (PROJECT_MAP).
export default defineConfig({
  plugins: [vue()],
  base: '/static/',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:5000',
    },
  },
})

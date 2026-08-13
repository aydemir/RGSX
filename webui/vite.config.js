import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Build çıktısı `dist/` — Rust `RGSX_WEBUI_DIR=webui/dist` ile sunulur.
export default defineConfig({
  plugins: [vue()],
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

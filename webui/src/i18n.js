// webui/src/i18n.js
// STRINGS artık ports/RGSX/languages/*.json'dan üretilen i18n.strings.js'ten gelir.
// Tek çeviri kaynağı Python RGSX languages/ JSON dosyalarıdır (bkz. scripts/gen-i18n.mjs).
// ELLE DÜZENLEMEYİN: bir çeviri değişecekse ports/RGSX/languages/*.json düzenlenir,
// ardından `npm run gen:i18n` çalıştırılır.
import { STRINGS } from './i18n.strings.js'

const FALLBACK = 'tr';
let current =
  localStorage.getItem('rgsx_locale') ||
  new URLSearchParams(location.search).get('lang') ||
  FALLBACK;

export function getLocale() {
  return current;
}
export function setLocale(l) {
  current = l;
  localStorage.setItem('rgsx_locale', l);
}
export { STRINGS };

// current -> en -> tr -> key fallback zinciri
export function t(key) {
  const table = STRINGS[current] || STRINGS[FALLBACK];
  return (table && table[key]) || (STRINGS.en && STRINGS.en[key]) || (STRINGS[FALLBACK] && STRINGS[FALLBACK][key]) || key;
}

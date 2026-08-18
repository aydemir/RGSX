// webui/src/i18n.js
// STRINGS artık ports/RGSX/languages/*.json'dan üretilen i18n.strings.js'ten gelir.
// Tek çeviri kaynağı Python RGSX languages/ JSON dosyalarıdır (bkz. scripts/gen-i18n.mjs).
// ELLE DÜZENLEMEYİN: bir çeviri değişecekse ports/RGSX/languages/*.json düzenlenir,
// ardından `npm run gen:i18n` çalıştırılır.
import { STRINGS } from './i18n.strings.js'

const FALLBACK = 'tr';
const explicitLocale =
  localStorage.getItem('rgsx_locale') ||
  new URLSearchParams(location.search).get('lang') ||
  '';
let current = explicitLocale || FALLBACK;

export function getLocale() {
  return current;
}
export function setLocale(l) {
  current = l;
  localStorage.setItem('rgsx_locale', l);
}
// Sunucu veri-dili bind'i: kullanici acikca secmediyse sunucu dilini uygular
// (localStorage'a KALICI YAZMAZ — yalniz oturum icin gecerli).
export function applyLocale(l) {
  if (l && STRINGS[l]) current = l;
}
export function hasExplicitLocale() {
  return !!explicitLocale;
}
export { STRINGS };

// current -> en -> tr -> key fallback zinciri
export function t(key) {
  const table = STRINGS[current] || STRINGS[FALLBACK];
  return (table && table[key]) || (STRINGS.en && STRINGS.en[key]) || (STRINGS[FALLBACK] && STRINGS[FALLBACK][key]) || key;
}

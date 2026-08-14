// Basit istemci-taraflı i18n — RGSX WebUI arayüz dizgeleri (TASK-003 Adım 3).
// Sunucu veri dili (/api/translations) ile ayrı; bu tablo SPA UI etiketlerindendir.

const STRINGS = {
  tr: {
    app_title: "RGSX Yöneticisi",
    status_connected: "SSE bağlı",
    status_connecting: "bağlanıyor…",
    active_download: "● aktif indirme",
    platforms: "Platformlar",
    search: "Arama",
    search_placeholder: "oyun / platform ara…",
    search_button: "Ara",
    clear: "Temizle",
    loading: "yükleniyor…",
    games: "oyun",
    back: "← geri",
    download: "İndir",
    downloads: "İndirmeler",
    progress_map: "İlerleme haritası",
    raw_sse: "ham SSE",
    last_event: "son olay",
    no_results: "sonuç yok",
    queue_empty: "Kuyruk boş",
    search_results: "Oyunlar",
    settings: "Ayarlar",
    close: "Kapat",
    ui_language: "Arayüz Dili",
    data_language: "Veri Dili (sunucu)",
    server_settings: "Sunucu Ayarları",
    no_settings: "Ayar mevcut değil",
    catalog_error: "Katalog yüklenemedi (RGSX_NATIVE_CATALOG=1 ve veri gerekli)",
    search_failed: "Arama başarısız",
    game_list_failed: "Oyun listesi alınamadı",
    refresh_note: "refresh gerektirmez",
  },
  en: {
    app_title: "RGSX Manager",
    status_connected: "SSE connected",
    status_connecting: "connecting…",
    active_download: "● active download",
    platforms: "Platforms",
    search: "Search",
    search_placeholder: "search game / platform…",
    search_button: "Search",
    clear: "Clear",
    loading: "loading…",
    games: "games",
    back: "← back",
    download: "Download",
    downloads: "Downloads",
    progress_map: "Progress map",
    raw_sse: "raw SSE",
    last_event: "last event",
    no_results: "no results",
    queue_empty: "Queue empty",
    search_results: "Games",
    settings: "Settings",
    close: "Close",
    ui_language: "UI Language",
    data_language: "Data Language (server)",
    server_settings: "Server Settings",
    no_settings: "No settings available",
    catalog_error: "Catalog failed to load (RGSX_NATIVE_CATALOG=1 and data required)",
    search_failed: "Search failed",
    game_list_failed: "Game list could not be fetched",
    refresh_note: "no refresh needed",
  },
};

const FALLBACK = "tr";
let current =
  localStorage.getItem("rgsx_locale") ||
  new URLSearchParams(location.search).get("lang") ||
  FALLBACK;

export function getLocale() {
  return current;
}
export function setLocale(l) {
  current = l;
  localStorage.setItem("rgsx_locale", l);
}
export { STRINGS };
export function t(key) {
  const table = STRINGS[current] || STRINGS[FALLBACK];
  return (table && table[key]) || (STRINGS[FALLBACK] && STRINGS[FALLBACK][key]) || key;
}

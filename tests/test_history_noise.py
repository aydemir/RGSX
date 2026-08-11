# -*- coding: utf-8 -*-
"""_strip_history_error_noise birim testleri.

Fonksiyon TVUI durum sütunu ve WebUI /api/history yanıtı tarafından ortak kullanılır;
ham archive.org hata mesajını kısa tutar. history.json'da tam metin korunur.
"""
from history import _strip_history_error_noise


class TestStripHistoryErrorNoise:
    def test_english_marker_prefix_and_list_block_removed(self):
        noisy = ("Download error Crazy Cars ++.zip: Accès refusé (HTTP 500). "
                 "Fichiers disponibles exemples: ['Addams Family.zip', 'After Burner II.zip', "
                 "'Aladdin.zip', 'Amiga 500 Tutorial.mp4']")
        assert _strip_history_error_noise(noisy) == "Accès refusé (HTTP 500)"

    def test_french_marker(self):
        msg = "Erreur téléchargement Chase HQ.zip: Accès refusé (HTTP 500)"
        assert _strip_history_error_noise(msg) == "Accès refusé (HTTP 500)"

    def test_turkish_marker(self):
        msg = "İndirme hatası Oyun.zip: Bağlantı reddedildi (HTTP 500)"
        assert _strip_history_error_noise(msg) == "Bağlantı reddedildi (HTTP 500)"

    def test_english_available_files_marker(self):
        msg = "Download error X.zip: some error. Available files examples: ['A.zip', 'B.zip']"
        assert _strip_history_error_noise(msg) == "some error"

    def test_marker_without_colon_keeps_rest(self):
        msg = "Download failed for X"  # ":" yoksa ":" ayırıcısı aranmadan kalan alınır
        assert _strip_history_error_noise(msg) == "X"

    def test_trailing_punctuation_cleaned(self):
        msg = "Download error X.zip: err."
        assert _strip_history_error_noise(msg) == "err"

    def test_no_marker_unchanged(self):
        msg = "just a plain status"
        assert _strip_history_error_noise(msg) == "just a plain status"

    def test_empty_and_none_safe(self):
        assert _strip_history_error_noise("") == ""
        assert _strip_history_error_noise(None) is None
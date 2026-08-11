"""Faz 11 — ilk açılışta sistem dili otomatik algılama testleri (TASK-003).

Senaryolar (tasks/in-progress/TASK-003-faz11-language-detect.md):
- yeni kurulum + desteklenen OS dili (tr) -> auto tr
- yeni kurulum + desteklenmeyen OS dili (ru) -> in-memory en, key YOK, mode auto
- manuel seçim -> manual yazılır, sonraki boot'larda korunur
- eski settings regression: language=="tr" (mode yok) -> manual kalır
- language=="en" (mode yok) -> mode auto'ya migrate; algılanan da en ise
  hiçbir şey yazılmaz / bildirim gösterilmez; farklı dil ise settings güncellenir
  + tek seferlik bildirim
- Termux/RetroBat env mirası düşük öncelik
- önkoşul: mevcut suite baseline değişmez
"""

import json

import pytest

import config
import language
import rgsx_settings
from rgsx_settings import (
    get_language_fallback_notified,
    get_language_mode,
    set_language_fallback_notified,
    set_language_mode,
)

LANGS = {
    "en": {"x": "yes", "language_fallback_notice": "EN fallback", "language_auto_detected": "EN auto {lang}"},
    "tr": {"x": "evet", "language_fallback_notice": "TR fallback", "language_auto_detected": "TR auto {lang}"},
    "fr": {"x": "oui", "language_fallback_notice": "FR fallback", "language_auto_detected": "FR auto {lang}"},
}


@pytest.fixture
def lang_env(tmp_path, monkeypatch):
    """config yollarını tmp_path'e yönlendirir + minimal çeviri dosyaları kurar."""
    app = tmp_path / "app"
    langs = app / "languages"
    langs.mkdir(parents=True)
    for code, data in LANGS.items():
        (langs / f"{code}.json").write_text(json.dumps(data), encoding="utf-8")

    monkeypatch.setattr(config, "SAVE_FOLDER", str(tmp_path))
    monkeypatch.setattr(config, "APP_FOLDER", str(app))
    monkeypatch.setattr(config, "LANGUAGES_FOLDER", str(langs))
    monkeypatch.setattr(config, "RGSX_SETTINGS_PATH", str(tmp_path / "rgsx_settings.json"))

    # language() modülünün global durumunu testler arası temizle
    saved_current = language.current_language
    saved_translations = language.translations
    yield tmp_path
    language.current_language = saved_current
    language.translations = saved_translations


def _write_settings(path, data):
    path.write_text(json.dumps(data), encoding="utf-8")


def _read_settings(path):
    return json.loads(path.read_text(encoding="utf-8"))


def _translate_with(code):
    assert language.load_language(code)


def test_normalize_lang_code():
    assert language._normalize_lang_code("tr_TR.UTF-8") == "tr"
    assert language._normalize_lang_code("tr_TR") == "tr"
    assert language._normalize_lang_code("pt_BR") == "pt"
    assert language._normalize_lang_code("en_US.UTF-8") == "en"
    assert language._normalize_lang_code("fr") == "fr"
    assert language._normalize_lang_code("C") is None
    assert language._normalize_lang_code("POSIX") is None
    assert language._normalize_lang_code(None) is None
    assert language._normalize_lang_code("") is None


def test_normalize_windows_locale_full_names():
    # locale.getlocale() Windows'ta "tr_TR" yerine İngilizce tam ad döndürür
    assert language._normalize_lang_code("Turkish_Turkey") == "tr"
    assert language._normalize_lang_code("English_United States") == "en"
    assert language._normalize_lang_code("French_France") == "fr"
    assert language._normalize_lang_code("German_Germany") == "de"
    assert language._normalize_lang_code("Italian_Italy") == "it"
    assert language._normalize_lang_code("Portuguese_Brazil") == "pt"
    assert language._normalize_lang_code("Spanish_Spain") == "es"


def test_translation_exists(lang_env):
    assert language._translation_exists("en")
    assert language._translation_exists("tr")
    assert not language._translation_exists("ru")


def test_set_language_writes_manual_mode(lang_env, monkeypatch):
    tmp_path = lang_env
    monkeypatch.setattr(language, "detect_system_language", lambda: (_ for _ in ()).throw(AssertionError("detect çağrılmamalı")))
    assert language.set_language("fr")
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "fr"
    assert settings["language_mode"] == "manual"
    assert language.current_language == "fr"


def test_save_language_preference_auto_mode(lang_env):
    tmp_path = lang_env
    language.save_language_preference("tr", manual=False)
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "tr"
    assert settings["language_mode"] == "auto"


def test_initialize_new_install_supported(lang_env, monkeypatch):
    """Yeni kurulum + desteklenen OS dili (tr) -> language=tr, mode=auto."""
    tmp_path = lang_env
    monkeypatch.setattr(language, "detect_system_language", lambda: "tr")
    assert language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "tr"
    assert settings["language_mode"] == "auto"
    assert language.current_language == "tr"
    assert config.language_fallback_notify is False


def test_initialize_new_install_unsupported(lang_env, monkeypatch):
    """Yeni kurulum + desteklenmeyen OS dili (ru) -> in-memory en, key YOK, mode auto."""
    tmp_path = lang_env
    monkeypatch.setattr(language, "detect_system_language", lambda: "ru")
    assert language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert "language" not in settings
    assert settings["language_mode"] == "auto"
    assert language.current_language == "en"
    assert get_language_fallback_notified() is True
    assert config.language_fallback_notify is True
    assert config.language_notify_message == "EN fallback"


def test_initialize_fallback_notify_only_once(lang_env, monkeypatch):
    """2. boot: marker var -> bildirim tekrarlamaz."""
    tmp_path = lang_env
    monkeypatch.setattr(language, "detect_system_language", lambda: "ru")
    language.initialize_language()
    assert config.language_fallback_notify is True

    monkeypatch.setattr(language, "detect_system_language", lambda: "ru")
    language.initialize_language()
    assert config.language_fallback_notify is False
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language_mode"] == "auto"


def test_initialize_manual_preserved_no_detect(lang_env, monkeypatch):
    """Manuel dil korunur; algılama çağrılmaz."""
    tmp_path = lang_env
    _write_settings(tmp_path / "rgsx_settings.json", {"language": "tr", "language_mode": "manual"})
    monkeypatch.setattr(language, "detect_system_language", lambda: (_ for _ in ()).throw(AssertionError("detect çağrılmamalı")))
    language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "tr"
    assert settings["language_mode"] == "manual"
    assert language.current_language == "tr"
    assert config.language_fallback_notify is False


def test_initialize_legacy_tr_is_manual(lang_env, monkeypatch):
    """Eski settings `language=="tr"` (mode yok) -> manual kalır, detect YOK."""
    tmp_path = lang_env
    _write_settings(tmp_path / "rgsx_settings.json", {"language": "tr"})
    monkeypatch.setattr(language, "detect_system_language", lambda: (_ for _ in ()).throw(AssertionError("detect çağrılmamalı")))
    language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "tr"
    assert settings["language_mode"] == "manual"
    assert config.language_fallback_notify is False


def test_initialize_legacy_en_detected_en_noop(lang_env, monkeypatch):
    """Eski `language=="en"` -> auto; algılanan da en -> dil en kalır, bildirim YOK."""
    tmp_path = lang_env
    _write_settings(tmp_path / "rgsx_settings.json", {"language": "en"})
    monkeypatch.setattr(language, "detect_system_language", lambda: "en")
    language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "en"
    assert settings["language_mode"] == "auto"
    assert config.language_fallback_notify is False


def test_initialize_legacy_en_detected_supported_notifies(lang_env, monkeypatch):
    """Eski `language=="en"` -> auto; sistem dili desteklenen fr -> settings güncellenir + tek seferlik bildirim."""
    tmp_path = lang_env
    _write_settings(tmp_path / "rgsx_settings.json", {"language": "en"})
    monkeypatch.setattr(language, "detect_system_language", lambda: "fr")
    language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "fr"
    assert settings["language_mode"] == "auto"
    assert language.current_language == "fr"
    assert get_language_fallback_notified() is True
    assert config.language_fallback_notify is True
    assert config.language_notify_message == "FR auto Français"

    # 2. boot: bildirim tekrarlamaz, fr korunur
    language.initialize_language()
    assert config.language_fallback_notify is False


def test_initialize_manual_broken_repairs_to_auto(lang_env, monkeypatch):
    """manual ama çeviri dosyası yok -> onarım: auto-detect'e düşer."""
    tmp_path = lang_env
    _write_settings(tmp_path / "rgsx_settings.json", {"language": "ru", "language_mode": "manual"})
    monkeypatch.setattr(language, "detect_system_language", lambda: "tr")
    language.initialize_language()
    settings = _read_settings(tmp_path / "rgsx_settings.json")
    assert settings["language"] == "tr"
    assert settings["language_mode"] == "auto"


def test_classify_environment_termux(monkeypatch):
    monkeypatch.setenv("TERMUX_VERSION", "1.0")
    assert language._classify_environment() == "termux"
    monkeypatch.delenv("TERMUX_VERSION")
    assert language._classify_environment() == "desktop"


def test_classify_environment_prefix_termux(monkeypatch):
    monkeypatch.setenv("PREFIX", "/data/data/com.termux/files/usr")
    assert language._classify_environment() == "termux"
    monkeypatch.delenv("PREFIX")


def test_classify_environment_retrobat(monkeypatch):
    monkeypatch.setenv("RETROBAT", "1")
    assert language._classify_environment() == "retrobat"
    monkeypatch.delenv("RETROBAT")


def test_detect_os_locale_env_only(monkeypatch):
    """Locale yok ama env var var -> env değeri döner (düşük öncelik zinciri)."""
    monkeypatch.delenv("LANG", raising=False)
    monkeypatch.delenv("LC_ALL", raising=False)
    monkeypatch.delenv("LC_MESSAGES", raising=False)
    monkeypatch.setattr("locale.getdefaultlocale", lambda: (None, None))
    monkeypatch.setattr("locale.getlocale", lambda: (None, None))
    monkeypatch.setenv("LC_ALL", "tr_TR.UTF-8")
    assert language._detect_os_locale() == "tr"


def test_detect_os_locale_none(monkeypatch):
    monkeypatch.delenv("LANG", raising=False)
    monkeypatch.delenv("LC_ALL", raising=False)
    monkeypatch.delenv("LC_MESSAGES", raising=False)
    monkeypatch.setattr("locale.getdefaultlocale", lambda: (None, None))
    monkeypatch.setattr("locale.getlocale", lambda: (None, None))
    assert language._detect_os_locale() is None


def test_detect_os_locale_getdefaultlocale_path(monkeypatch):
    """getlocale/env boş, son çare getdefaultlocale -> 'tr_TR' -> 'tr'."""
    monkeypatch.delenv("LANG", raising=False)
    monkeypatch.delenv("LC_ALL", raising=False)
    monkeypatch.delenv("LC_MESSAGES", raising=False)
    monkeypatch.setattr("locale.getlocale", lambda: (None, None))
    monkeypatch.setattr("locale.getdefaultlocale", lambda: ("tr_TR", "cp1254"))
    assert language._detect_os_locale() == "tr"


def test_detect_system_language_batocera(monkeypatch):
    monkeypatch.setattr(language, "_classify_environment", lambda: "batocera")
    monkeypatch.setattr(language, "detect_batocera_language", lambda: "fr")
    assert language.detect_system_language() == "fr"


def test_detect_system_language_batocera_fallback_os(monkeypatch):
    monkeypatch.setattr(language, "_classify_environment", lambda: "batocera")
    monkeypatch.setattr(language, "detect_batocera_language", lambda: None)
    monkeypatch.setattr(language, "_detect_os_locale", lambda: "tr")
    assert language.detect_system_language() == "tr"


def test_detect_system_language_termux_low_priority(monkeypatch):
    """Termux'ta env mirası gerçek dil sayılmaz ama düşük öncelikli fallback yine kullanılır."""
    monkeypatch.setattr(language, "_classify_environment", lambda: "termux")
    monkeypatch.setattr(language, "_detect_os_locale", lambda: "tr")
    assert language.detect_system_language() == "tr"


def test_detect_system_language_retrobat_low_priority(monkeypatch):
    monkeypatch.setattr(language, "_classify_environment", lambda: "retrobat")
    monkeypatch.setattr(language, "_detect_os_locale", lambda: "fr")
    assert language.detect_system_language() == "fr"


def test_language_mode_persisters(lang_env):
    tmp_path = lang_env
    assert get_language_mode() == "auto"
    set_language_mode("manual")
    assert get_language_mode() == "manual"
    set_language_mode("invalid")
    assert get_language_mode() == "auto"
    set_language_fallback_notified(True)
    assert get_language_fallback_notified() is True
    set_language_fallback_notified(False)
    assert get_language_fallback_notified() is False
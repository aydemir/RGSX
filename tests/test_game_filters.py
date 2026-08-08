"""Tests for GameFilters business logic (pure, no SDL needed)."""

import sys
import types

import pytest

from game_filters import GameFilters
from config import Game


def make_game(name, display_name=None):
    return Game(
        name=name,
        url=None,
        size="100 MB",
        display_name=display_name or name,
        regions=None,
        is_non_release=None,
        base_name=None,
    )


class TestDefaults:
    def test_init_defaults(self):
        gf = GameFilters()
        assert all(state == "include" for state in gf.region_filters.values())
        assert gf.hide_non_release is False
        assert gf.one_rom_per_game is False
        assert gf.hide_downloaded is False
        assert gf.regex_mode is False
        assert gf.region_priority[0] == "USA"

    def test_is_active_default_false(self):
        assert GameFilters().is_active() is False

    def test_is_active_when_region_excluded(self):
        gf = GameFilters()
        gf.region_filters["USA"] = "exclude"
        assert gf.is_active() is True

    def test_is_active_when_option_enabled(self):
        gf = GameFilters()
        gf.hide_non_release = True
        assert gf.is_active() is True


class TestLoadAndToDict:
    def test_load_from_dict_merges_regions(self):
        gf = GameFilters()
        gf.load_from_dict({"region_filters": {"USA": "exclude"}, "hide_non_release": True})
        assert gf.region_filters["USA"] == "exclude"
        assert gf.region_filters["Japan"] == "include"
        assert gf.hide_non_release is True

    def test_load_from_dict_empty_regions_defaults_include(self):
        gf = GameFilters()
        gf.region_filters["USA"] = "exclude"
        gf.load_from_dict({})
        assert gf.region_filters["USA"] == "include"

    def test_load_from_dict_custom_priority(self):
        gf = GameFilters()
        gf.load_from_dict({"region_priority": ["Japan", "USA"]})
        assert gf.region_priority == ["Japan", "USA"]

    def test_to_dict_roundtrip(self):
        gf = GameFilters()
        gf.load_from_dict({"region_filters": {"Europe": "exclude"}, "one_rom_per_game": True, "hide_downloaded": True})
        data = gf.to_dict()
        assert data["region_filters"]["Europe"] == "exclude"
        assert data["one_rom_per_game"] is True
        assert data["hide_downloaded"] is True
        assert "region_priority" in data

    def test_reset(self):
        gf = GameFilters()
        gf.load_from_dict({"region_filters": {"USA": "exclude"}, "hide_non_release": True, "one_rom_per_game": True, "regex_mode": True})
        gf.reset()
        assert all(state == "include" for state in gf.region_filters.values())
        assert gf.hide_non_release is False
        assert gf.one_rom_per_game is False
        assert gf.hide_downloaded is False
        assert gf.regex_mode is False


class TestGetGameRegions:
    @pytest.mark.parametrize("name,expected", [
        ("Super Mario (USA)", ["USA"]),
        ("Sonic (Europe)", ["Europe"]),
        ("Mega Man (Japan)", ["Japan"]),
        ("Zelda (World)", ["World"]),
        ("Streets of Rage (Europe) (En,Fr)", ["France", "Europe"]),
        ("Virtua Fighter (Germany)", ["Germany"]),
        ("Street Fighter (Korea)", ["Korea"]),
        ("Pokemon (Canada)", ["Canada"]),
        ("Game (Fr,De)", ["France", "Germany"]),
        ("Mystery Game", ["Other"]),
        ("International Game (En,Nl)", ["Europe"]),
        ("Game (FRA)", ["France"]),
        ("Game (ES)", ["Other"]),
        ("Game (IT)", ["Other"]),
        ("Game (NL)", ["Europe"]),
        ("Game (PT)", ["Other"]),
        ("Game (Spain)", ["Other"]),
        ("Game (Italy)", ["Other"]),
    ])
    def test_region_detection(self, name, expected):
        assert GameFilters.get_game_regions(name) == expected

    def test_iso_codes_fr(self):
        assert GameFilters.get_game_regions("Game (FR)") == ["France"]


class TestIsNonReleaseGame:
    @pytest.mark.parametrize("name", [
        "Game (Beta)",
        "Game (Demo)",
        "Game (Proto)",
        "Game (Sample)",
        "Game (Kiosk)",
        "Game (Preview)",
        "Game (Test)",
        "Game (Debug)",
        "Game (Alpha)",
        "Game (Pre-Release)",
        "Game (Unfinished)",
        "Game (WIP)",
        "Game (Bootleg)",
    ])
    def test_non_release_detected(self, name):
        assert GameFilters.is_non_release_game(name) is True

    def test_regular_game_not_non_release(self):
        assert GameFilters.is_non_release_game("Super Mario (USA)") is False


class TestBaseGameName:
    def test_removes_extension(self):
        assert GameFilters.get_base_game_name("Sonic.zip") == "Sonic"

    def test_removes_parens_and_brackets(self):
        assert GameFilters.get_base_game_name("Mega Man (USA) [Rev 1]") == "Mega Man"

    def test_keeps_disc_info(self):
        assert GameFilters.get_base_game_name("Final Fantasy VII (Disc 1)") == "Final Fantasy VII (Disc 1)"

    def test_cd_disc_format(self):
        assert GameFilters.get_base_game_name("Resident Evil (USA) (CD 2)") == "Resident Evil (Disc 2)"

    def test_normalizes_spaces(self):
        assert GameFilters.get_base_game_name("Super   Mario (USA)") == "Super Mario"


class TestCaching:
    def test_get_cached_regions(self):
        game = make_game("Sonic (USA).zip", "Sonic (USA)")
        regions = GameFilters.get_cached_regions(game)
        assert regions == ["USA"]
        assert game.regions == regions

    def test_get_cached_non_release(self):
        game = make_game("Game (Demo).zip", "Game (Demo)")
        assert GameFilters.get_cached_non_release(game) is True
        assert game.is_non_release is True

    def test_get_cached_base_name(self):
        game = make_game("Sonic (USA).zip", "Sonic (USA)")
        assert GameFilters.get_cached_base_name(game) == "Sonic"

    def test_get_region_priority_default(self):
        game = make_game("Game (Japan).zip", "Game (Japan)")
        gf = GameFilters()
        assert gf.get_region_priority(game) == gf.region_priority.index("Japan")

    def test_get_region_priority_unknown_region_skipped(self):
        game = make_game("Game (Atlantis).zip", "Game (Atlantis)")
        gf = GameFilters()
        gf.region_priority = ["USA"]
        # 'Atlantis' matches nothing in priority list -> lowest priority (len)
        assert gf.get_region_priority(game) == 1


class TestResolvePlatformName:
    def test_resolves_from_config(self, monkeypatch):
        import config
        import game_filters

        monkeypatch.setattr(config, "platforms", {"NES": "nintendo_nes"})
        monkeypatch.setattr(config, "current_platform", "NES")
        monkeypatch.setattr(config, "platform_names", {"nintendo_nes": "Nintendo NES"})
        assert game_filters._resolve_platform_name() == "Nintendo NES"

    def test_missing_platform_returns_none(self, monkeypatch):
        import config
        import game_filters

        monkeypatch.setattr(config, "platforms", {})
        monkeypatch.setattr(config, "current_platform", "NOPE")
        monkeypatch.setattr(config, "platform_names", {})
        assert game_filters._resolve_platform_name() is None


class TestApplyFilters:
    def test_inactive_returns_same_list(self):
        gf = GameFilters()
        games = [make_game("a.zip"), make_game("b.zip")]
        assert gf.apply_filters(games, "NES") is games

    def test_region_exclude(self):
        gf = GameFilters()
        gf.region_filters["USA"] = "exclude"
        usa = make_game("Mario (USA).zip", "Mario (USA)")
        jp = make_game("Mario (Japan).zip", "Mario (Japan)")
        result = gf.apply_filters([usa, jp], "NES")
        assert usa not in result
        assert jp in result

    def test_hide_non_release(self):
        gf = GameFilters()
        gf.hide_non_release = True
        demo = make_game("Game (Demo).zip", "Game (Demo)")
        full = make_game("Game.zip", "Game")
        result = gf.apply_filters([demo, full], "NES")
        assert demo not in result
        assert full in result

    def test_hide_downloaded(self, monkeypatch):
        gf = GameFilters()
        gf.hide_downloaded = True
        fake_history = types.SimpleNamespace(is_game_downloaded=lambda p, g: g == "downloaded.zip")
        monkeypatch.setitem(sys.modules, "history", fake_history)
        downloaded = make_game("downloaded.zip")
        other = make_game("other.zip")
        result = gf.apply_filters([downloaded, other], "NES")
        assert downloaded not in result
        assert other in result

    def test_hide_downloaded_without_platform(self, monkeypatch):
        import config

        gf = GameFilters()
        gf.hide_downloaded = True
        monkeypatch.setattr(config, "platforms", {})
        monkeypatch.setattr(config, "current_platform", "NOPE")
        monkeypatch.setattr(config, "platform_names", {})
        games = [make_game("any.zip")]
        result = gf.apply_filters(games, None)
        assert result == games

    def test_one_rom_per_game(self):
        gf = GameFilters()
        gf.one_rom_per_game = True
        usa = make_game("Mario (USA).zip", "Mario (USA)")
        jp = make_game("Mario (Japan).zip", "Mario (Japan)")
        result = gf.apply_filters([usa, jp], "NES")
        assert len(result) == 1

    def test_complex_combination(self):
        gf = GameFilters()
        gf.hide_non_release = True
        gf.one_rom_per_game = True
        games = [
            make_game("Sonic (USA).zip", "Sonic (USA)"),
            make_game("Sonic (Japan).zip", "Sonic (Japan)"),
            make_game("Sonic (Demo).zip", "Sonic (Demo)"),
            make_game("Other Game (USA).zip", "Other Game (USA)"),
        ]
        result = gf.apply_filters(games, "Genesis")
        assert all("Demo" not in g.display_name for g in result)
        names = [g.display_name for g in result]
        assert "Sonic" in "".join(names)
        assert len(result) <= 3

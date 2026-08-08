"""Regression tests: config.game_filter_obj=None must not crash draw_filter_* (bug fix)."""

import pytest


@pytest.mark.usefixtures("display_env")
class TestFilterNoneSafety:
    def test_draw_filter_advanced_initializes_when_none(self):
        import config
        from display import draw_filter_advanced

        config.game_filter_obj = None
        draw_filter_advanced(config.screen)
        assert config.game_filter_obj is not None

    def test_draw_filter_advanced_keeps_existing_object(self):
        import config
        from display import draw_filter_advanced
        from game_filters import GameFilters

        config.game_filter_obj = GameFilters()
        draw_filter_advanced(config.screen)
        assert isinstance(config.game_filter_obj, GameFilters)

    def test_draw_filter_priority_config_initializes_when_none(self):
        import config
        from display import draw_filter_priority_config

        config.game_filter_obj = None
        draw_filter_priority_config(config.screen)
        assert config.game_filter_obj is not None

    def test_draw_filter_priority_config_keeps_existing_object(self):
        import config
        from display import draw_filter_priority_config
        from game_filters import GameFilters

        config.game_filter_obj = GameFilters()
        draw_filter_priority_config(config.screen)
        assert isinstance(config.game_filter_obj, GameFilters)

    def test_draw_filter_menu_choice_smoke(self):
        import config
        from display import draw_filter_menu_choice

        config.menu_state = "platform"
        config.filter_menu_entries = [{"label": "Recherche"}, {"label": "Avancé"}]
        config.selected_filter_choice = 1
        draw_filter_menu_choice(config.screen)

    def test_draw_filter_menu_choice_no_entries(self):
        import config
        from display import draw_filter_menu_choice

        config.filter_menu_entries = []
        draw_filter_menu_choice(config.screen)

    def test_draw_global_sort_menu_smoke(self):
        import config
        from display import draw_global_sort_menu

        draw_global_sort_menu(config.screen)


@pytest.mark.usefixtures("display_env")
class TestFilterAdvancedBranches:
    def test_region_exclude_status(self):
        import config
        from display import draw_filter_advanced
        from game_filters import GameFilters

        gf = GameFilters()
        gf.region_filters["USA"] = "exclude"
        config.game_filter_obj = gf
        draw_filter_advanced(config.screen)

    def test_selected_filter_option_clamped(self):
        import config
        from display import draw_filter_advanced

        config.game_filter_obj = None
        config.selected_filter_option = 999
        draw_filter_advanced(config.screen)
        assert config.selected_filter_option >= 0

    def test_selected_filter_option_attr_absent(self):
        import config
        from display import draw_filter_advanced

        config.game_filter_obj = None
        if hasattr(config, "selected_filter_option"):
            delattr(config, "selected_filter_option")
        draw_filter_advanced(config.screen)
        assert config.selected_filter_option == 0

    def test_load_filters_from_settings(self, monkeypatch):
        import config
        from display import draw_filter_advanced
        import rgsx_settings

        config.game_filter_obj = None
        monkeypatch.setattr(
            rgsx_settings, "load_game_filters",
            lambda: {"region_filters": {"Japan": "exclude"}, "hide_non_release": True},
        )
        draw_filter_advanced(config.screen)
        assert config.game_filter_obj.region_filters["Japan"] == "exclude"
        assert config.game_filter_obj.hide_non_release is True

    def test_toggle_selected_branch(self):
        import config
        from display import draw_filter_advanced

        config.game_filter_obj = None
        config.selected_filter_option = 9  # ilk toggle (region sayısı)
        draw_filter_advanced(config.screen)

    def test_button_selected_branch(self):
        import config
        from display import draw_filter_advanced

        config.game_filter_obj = None
        config.selected_filter_option = 13  # ilk buton (9 region + 4 option)
        draw_filter_advanced(config.screen)

    def test_priority_config_loads_settings(self, monkeypatch):
        import config
        from display import draw_filter_priority_config
        import rgsx_settings

        config.game_filter_obj = None
        monkeypatch.setattr(
            rgsx_settings, "load_game_filters",
            lambda: {"region_priority": ["Japan", "USA"]},
        )
        draw_filter_priority_config(config.screen)
        assert config.game_filter_obj.region_priority == ["Japan", "USA"]

    def test_priority_config_existing_index(self):
        import config
        from display import draw_filter_priority_config
        from game_filters import GameFilters

        config.game_filter_obj = GameFilters()
        config.selected_priority_index = 0
        draw_filter_priority_config(config.screen)

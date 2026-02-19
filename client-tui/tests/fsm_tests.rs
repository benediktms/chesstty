use chesstty_tui::prelude::*;

fn mock_human_vs_engine_session() -> UiState {
    UiState::GameBoard(GameBoardState::new(GameMode::HumanVsEngine {
        human_side: PlayerColor::White,
    }))
}

fn mock_human_vs_engine_session_black() -> UiState {
    UiState::GameBoard(GameBoardState::new(GameMode::HumanVsEngine {
        human_side: PlayerColor::Black,
    }))
}

fn mock_human_vs_human_session() -> UiState {
    UiState::GameBoard(GameBoardState::new(GameMode::HumanVsHuman))
}

fn mock_engine_vs_engine_session() -> UiState {
    UiState::GameBoard(GameBoardState::new(GameMode::EngineVsEngine))
}

fn mock_review_session(total_plies: u32) -> UiState {
    UiState::ReviewBoard(ReviewBoardState::new(total_plies))
}

fn mock_match_summary() -> UiState {
    UiState::MatchSummary(MatchSummaryState::new(
        Some((1, "Black wins by checkmate".to_string())),
        40,
        GameMode::HumanVsHuman,
    ))
}

fn mock_start_screen() -> UiState {
    UiState::StartScreen(StartScreenState::new())
}

fn get_control_keys(state: &UiState) -> Vec<&'static str> {
    state.controls().iter().map(|c| c.key).collect()
}

fn get_control_labels(state: &UiState) -> Vec<&'static str> {
    state.controls().iter().map(|c| c.label).collect()
}

mod controls_tests {
    use super::*;

    #[test]
    fn human_vs_engine_shows_pause_control() {
        let state = mock_human_vs_engine_session();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"p"),
            "HumanVsEngine should show pause control, got: {:?}",
            keys
        );
    }

    #[test]
    fn human_vs_engine_white_shows_undo_control() {
        let mut state = mock_human_vs_engine_session();
        if let UiState::GameBoard(ref mut gs) = state {
            gs.move_count = 5;
            gs.controls = gs.derive_controls();
        }
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"u"),
            "HumanVsEngine with moves should show undo control, got: {:?}",
            keys
        );
    }

    #[test]
    fn human_vs_engine_black_shows_pause_control() {
        let state = mock_human_vs_engine_session_black();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"p"),
            "HumanVsEngine (black) should show pause control, got: {:?}",
            keys
        );
    }

    #[test]
    fn human_vs_human_hides_pause_control() {
        let state = mock_human_vs_human_session();
        let keys = get_control_keys(&state);
        assert!(
            !keys.contains(&"p"),
            "HumanVsHuman should NOT show pause control, got: {:?}",
            keys
        );
    }

    // Pause state now comes from GameSession, not FSM
    // This test is no longer applicable - controls are derived from game_mode
    // and actual pause state is read from the game session at render time

    #[test]
    fn engine_vs_engine_shows_pause_control() {
        let state = mock_engine_vs_engine_session();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"p"),
            "EngineVsEngine should show pause control, got: {:?}",
            keys
        );
    }

    #[test]
    fn review_mode_shows_jump_snap_controls() {
        let state = mock_review_session(20);
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"Home/End"),
            "Review mode should show jump control, got: {:?}",
            keys
        );
        assert!(
            keys.contains(&"s"),
            "Review mode should show snap control, got: {:?}",
            keys
        );
    }

    #[test]
    fn review_mode_shows_auto_control() {
        let state = mock_review_session(20);
        let labels = get_control_labels(&state);
        assert!(
            labels.contains(&"Auto"),
            "Review mode should show auto control, got: {:?}",
            labels
        );
    }

    #[test]
    fn review_mode_toggle_auto_play_changes_label() {
        let mut state = mock_review_session(20);
        if let UiState::ReviewBoard(ref mut rs) = state {
            rs.toggle_auto_play();
        }
        let labels = get_control_labels(&state);
        assert!(
            labels.contains(&"Stop"),
            "Review mode after toggle should show Stop, got: {:?}",
            labels
        );
    }

    #[test]
    fn match_summary_shows_new_game_control() {
        let state = mock_match_summary();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"n"),
            "MatchSummary should show new game control, got: {:?}",
            keys
        );
    }

    #[test]
    fn match_summary_shows_quit_control() {
        let state = mock_match_summary();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"q"),
            "MatchSummary should show quit control, got: {:?}",
            keys
        );
    }

    #[test]
    fn start_screen_shows_select_control() {
        let state = mock_start_screen();
        let keys = get_control_keys(&state);
        assert!(
            keys.contains(&"Enter"),
            "StartScreen should show Enter control, got: {:?}",
            keys
        );
    }

    #[test]
    fn all_game_modes_show_input_control() {
        let modes = vec![
            mock_human_vs_engine_session(),
            mock_human_vs_human_session(),
            mock_engine_vs_engine_session(),
        ];

        for state in modes {
            let keys = get_control_keys(&state);
            assert!(
                keys.contains(&"i"),
                "All game modes should show input control, got: {:?}",
                keys
            );
        }
    }

    #[test]
    fn all_game_modes_show_menu_control() {
        let modes = vec![
            mock_human_vs_engine_session(),
            mock_human_vs_human_session(),
            mock_engine_vs_engine_session(),
            mock_review_session(10),
        ];

        for state in modes {
            let keys = get_control_keys(&state);
            assert!(
                keys.contains(&"Esc"),
                "All modes should show menu control, got: {:?}",
                keys
            );
        }
    }
}

mod render_spec_tests {
    use super::*;

    #[test]
    fn game_board_has_board_component() {
        let state = mock_human_vs_engine_session();
        let spec = state.render_spec();
        assert_eq!(spec.view, View::GameBoard);
    }

    #[test]
    fn review_board_has_board_component() {
        let state = mock_review_session(20);
        let spec = state.render_spec();
        assert_eq!(spec.view, View::ReviewBoard);
    }

    #[test]
    fn match_summary_view_is_match_summary() {
        let state = mock_match_summary();
        let spec = state.render_spec();
        assert_eq!(spec.view, View::MatchSummary);
    }

    #[test]
    fn start_screen_view_is_start_screen() {
        let state = mock_start_screen();
        let spec = state.render_spec();
        assert_eq!(spec.view, View::StartScreen);
    }
}

mod tab_order_tests {
    use super::*;

    #[test]
    fn component_manager_game_board_first_component() {
        let cm = ComponentManager::game_board();
        let layout = Layout::game_board();

        // Should return InfoPanel as first selectable component
        assert_eq!(cm.first_component(&layout), Some(Component::InfoPanel));
    }

    #[test]
    fn component_manager_game_board_tab_order() {
        let cm = ComponentManager::game_board();
        let layout = Layout::game_board();

        // Tab order should be left-to-right: InfoPanel -> EnginePanel -> HistoryPanel
        assert_eq!(
            cm.tab_order(&layout),
            vec![
                Component::InfoPanel,
                Component::EnginePanel,
                Component::HistoryPanel,
            ]
        );
    }

    #[test]
    fn component_manager_tab_wraps_around() {
        let cm = ComponentManager::game_board();
        let layout = Layout::game_board();

        // After HistoryPanel, should wrap to InfoPanel
        assert_eq!(
            cm.next_component(Component::HistoryPanel, &layout),
            Some(Component::InfoPanel)
        );

        // Before InfoPanel, should wrap to HistoryPanel
        assert_eq!(
            cm.prev_component(Component::InfoPanel, &layout),
            Some(Component::HistoryPanel)
        );
    }

    #[test]
    fn component_manager_hidden_panels_excluded_from_tab_order() {
        let mut cm = ComponentManager::new();
        cm.set_visible(Component::InfoPanel, true);
        cm.set_visible(Component::EnginePanel, false); // hidden
        cm.set_visible(Component::HistoryPanel, true);

        let layout = Layout::game_board();

        // EnginePanel should be excluded
        assert_eq!(
            cm.tab_order(&layout),
            vec![Component::InfoPanel, Component::HistoryPanel,]
        );
    }

    #[test]
    fn component_manager_next_navigates_forward() {
        let cm = ComponentManager::game_board();
        let layout = Layout::game_board();

        assert_eq!(
            cm.next_component(Component::InfoPanel, &layout),
            Some(Component::EnginePanel)
        );
        assert_eq!(
            cm.next_component(Component::EnginePanel, &layout),
            Some(Component::HistoryPanel)
        );
    }

    #[test]
    fn component_manager_prev_navigates_backward() {
        let cm = ComponentManager::game_board();
        let layout = Layout::game_board();

        assert_eq!(
            cm.prev_component(Component::HistoryPanel, &layout),
            Some(Component::EnginePanel)
        );
        assert_eq!(
            cm.prev_component(Component::EnginePanel, &layout),
            Some(Component::InfoPanel)
        );
    }
}

mod review_tab_order_tests {
    use super::*;

    #[test]
    fn review_board_tab_order() {
        let cm = ComponentManager::review_board();
        let layout = Layout::review_board();

        // Tab order: AdvancedAnalysis -> ReviewSummary -> InfoPanel -> HistoryPanel
        assert_eq!(
            cm.tab_order(&layout),
            vec![
                Component::AdvancedAnalysis,
                Component::ReviewSummary,
                Component::InfoPanel,
                Component::HistoryPanel,
            ]
        );
    }

    #[test]
    fn review_board_next_navigates_correctly() {
        let cm = ComponentManager::review_board();
        let layout = Layout::review_board();

        assert_eq!(
            cm.next_component(Component::AdvancedAnalysis, &layout),
            Some(Component::ReviewSummary)
        );
        assert_eq!(
            cm.next_component(Component::ReviewSummary, &layout),
            Some(Component::InfoPanel)
        );
        assert_eq!(
            cm.next_component(Component::HistoryPanel, &layout),
            Some(Component::AdvancedAnalysis) // wraps
        );
    }
}

mod focus_mode_tests {
    use super::*;

    #[test]
    fn default_focus_is_board() {
        let cm = ComponentManager::new();
        assert!(matches!(cm.focus_mode, FocusMode::Board));
    }

    #[test]
    fn select_component_changes_focus() {
        let mut cm = ComponentManager::new();
        cm.select_component(Component::InfoPanel);
        assert!(matches!(
            cm.focus_mode,
            FocusMode::ComponentSelected {
                component: Component::InfoPanel
            }
        ));
    }

    #[test]
    fn expand_component_changes_focus() {
        let mut cm = ComponentManager::new();
        cm.expand_component(Component::HistoryPanel);
        assert!(matches!(
            cm.focus_mode,
            FocusMode::ComponentExpanded {
                component: Component::HistoryPanel
            }
        ));
    }

    #[test]
    fn clear_focus_returns_to_board() {
        let mut cm = ComponentManager::new();
        cm.select_component(Component::InfoPanel);
        cm.clear_focus();
        assert!(matches!(cm.focus_mode, FocusMode::Board));
    }

    #[test]
    fn selected_component_returns_correct_component() {
        let mut cm = ComponentManager::new();
        assert_eq!(cm.selected_component(), None);

        cm.select_component(Component::EnginePanel);
        assert_eq!(cm.selected_component(), Some(Component::EnginePanel));
    }

    #[test]
    fn expanded_component_returns_correct_component() {
        let mut cm = ComponentManager::new();
        assert_eq!(cm.expanded_component(), None);

        cm.expand_component(Component::HistoryPanel);
        assert_eq!(cm.expanded_component(), Some(Component::HistoryPanel));
    }
}

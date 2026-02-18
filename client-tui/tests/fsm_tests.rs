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

mod tab_selection_tests {
    use super::*;

    #[test]
    fn pane_manager_next_selectable_wraps_around() {
        let pm = PaneManager::new();

        let next = pm.next_selectable(PaneId::MoveHistory);
        assert_eq!(next, Some(PaneId::GameInfo));
    }

    #[test]
    fn pane_manager_prev_selectable_wraps_around() {
        let pm = PaneManager::new();

        let prev = pm.prev_selectable(PaneId::GameInfo);
        assert_eq!(prev, Some(PaneId::MoveHistory));
    }

    #[test]
    fn pane_manager_cycles_through_all_selectable() {
        let pm = PaneManager::new();

        let mut current = Some(PaneId::GameInfo);
        let mut visited = Vec::new();

        for _ in 0..4 {
            let next = current.and_then(|p| pm.next_selectable(p));
            if let Some(p) = next {
                visited.push(p);
                current = Some(p);
            }
        }

        assert!(
            visited.contains(&PaneId::GameInfo),
            "Should cycle back to GameInfo, visited: {:?}",
            visited
        );
        assert!(
            visited.contains(&PaneId::EngineAnalysis),
            "Should visit EngineAnalysis, visited: {:?}",
            visited
        );
        assert!(
            visited.contains(&PaneId::MoveHistory),
            "Should visit MoveHistory, visited: {:?}",
            visited
        );
    }
}

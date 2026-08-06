use client_tui::prelude::*;

/// Tests for flat focus model navigation on UiStateMachine
mod tab_order_tests {
    use super::*;
    use client_tui::ui::fsm::UiStateMachine;

    fn game_board_fsm() -> UiStateMachine {
        let mut fsm = UiStateMachine::default();
        fsm.transition_to(UiMode::GameBoard);
        fsm
    }

    #[test]
    fn in_section_navigation() {
        let fsm = game_board_fsm();
        let layout = GameBoardState.layout(&fsm);

        // In the nested layout, InfoPanel/EnginePanel/HistoryPanel share the
        // right column section. next/prev_in_section navigates within it.
        assert_eq!(
            fsm.next_in_section(Component::InfoPanel, &layout),
            Some(Component::EnginePanel)
        );
        assert_eq!(
            fsm.prev_in_section(Component::InfoPanel, &layout),
            Some(Component::HistoryPanel)
        );

        // The center section (Board) has no selectable components,
        // so cross-section navigation returns None.
        assert_eq!(fsm.next_section(Component::InfoPanel, &layout), None);
    }
}

/// Tests for flat focus model state management
mod focus_mode_tests {
    use super::*;
    use client_tui::ui::fsm::UiStateMachine;

    #[test]
    fn default_focus_is_board() {
        let fsm = UiStateMachine::default();
        assert!(fsm.focused_component.is_none());
        assert_eq!(fsm.focused_component, None);
        assert!(!fsm.expanded);
    }

    #[test]
    fn select_component_sets_focus() {
        let mut fsm = UiStateMachine::default();
        fsm.select_component(Component::InfoPanel);

        assert!(fsm.focused_component.is_some());
        assert_eq!(fsm.focused_component, Some(Component::InfoPanel));
        assert!(!fsm.expanded);
        assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));
    }

    #[test]
    fn expand_component_sets_expanded() {
        let mut fsm = UiStateMachine::default();
        fsm.expand_component(Component::HistoryPanel);

        assert_eq!(fsm.focused_component, Some(Component::HistoryPanel));
        assert!(fsm.expanded);
        assert_eq!(fsm.expanded_component(), Some(Component::HistoryPanel));
        assert_eq!(fsm.selected_component(), None);
    }

    #[test]
    fn clear_focus_returns_to_board() {
        let mut fsm = UiStateMachine::default();
        fsm.select_component(Component::InfoPanel);
        fsm.clear_focus();

        assert!(fsm.focused_component.is_none());
        assert_eq!(fsm.focused_component, None);
        assert!(!fsm.expanded);
    }

    #[test]
    fn selected_component_returns_correct_value() {
        let mut fsm = UiStateMachine::default();
        assert_eq!(fsm.selected_component(), None);

        fsm.select_component(Component::EnginePanel);
        assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));
    }

    #[test]
    fn expanded_component_returns_correct_value() {
        let mut fsm = UiStateMachine::default();
        assert_eq!(fsm.expanded_component(), None);

        fsm.expand_component(Component::HistoryPanel);
        assert_eq!(fsm.expanded_component(), Some(Component::HistoryPanel));
    }

    #[test]
    fn multiple_select_calls_persist() {
        let mut fsm = UiStateMachine::default();
        fsm.transition_to(UiMode::GameBoard);

        fsm.select_component(Component::InfoPanel);
        assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));

        fsm.select_component(Component::EnginePanel);
        assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));

        fsm.select_component(Component::HistoryPanel);
        assert_eq!(fsm.selected_component(), Some(Component::HistoryPanel));
    }
}

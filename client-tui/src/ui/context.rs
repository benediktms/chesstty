use crate::ui::pane::PaneId;

/// The focus contexts, forming a stack.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusContext {
    /// Board is focused. User types moves.
    Board,
    /// A pane is selected (highlighted border). Arrow keys navigate/scroll.
    PaneSelected { pane_id: PaneId },
    /// A pane is expanded to fill the board area.
    PaneExpanded { pane_id: PaneId },
}

/// Manages the context stack. The bottom is always Board.
#[derive(Clone, Debug)]
pub struct FocusStack {
    stack: Vec<FocusContext>,
}

impl FocusStack {
    pub fn new() -> Self {
        Self {
            stack: vec![FocusContext::Board],
        }
    }

    /// Current active context (top of stack).
    pub fn current(&self) -> &FocusContext {
        self.stack.last().expect("FocusStack should never be empty")
    }

    /// Push a new context onto the stack.
    pub fn push(&mut self, ctx: FocusContext) {
        self.stack.push(ctx);
    }

    /// Pop back to previous context. Returns false if already at Board (bottom).
    pub fn pop(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    /// Check if we're at the base (Board) context.
    pub fn is_board_focused(&self) -> bool {
        matches!(self.current(), FocusContext::Board)
    }

    /// Get the currently selected pane (only in PaneSelected context).
    pub fn selected_pane(&self) -> Option<PaneId> {
        match self.current() {
            FocusContext::PaneSelected { pane_id } => Some(*pane_id),
            _ => None,
        }
    }

    /// Get the currently expanded pane (only in PaneExpanded context).
    pub fn expanded_pane(&self) -> Option<PaneId> {
        match self.current() {
            FocusContext::PaneExpanded { pane_id } => Some(*pane_id),
            _ => None,
        }
    }
}

impl Default for FocusStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let stack = FocusStack::new();
        assert!(stack.is_board_focused());
        assert_eq!(stack.selected_pane(), None);
        assert_eq!(stack.expanded_pane(), None);
    }

    #[test]
    fn test_push_pane_selected() {
        let mut stack = FocusStack::new();
        stack.push(FocusContext::PaneSelected {
            pane_id: PaneId::MoveHistory,
        });
        assert!(!stack.is_board_focused());
        assert_eq!(stack.selected_pane(), Some(PaneId::MoveHistory));
        assert_eq!(stack.expanded_pane(), None);
    }

    #[test]
    fn test_push_pane_expanded() {
        let mut stack = FocusStack::new();
        stack.push(FocusContext::PaneSelected {
            pane_id: PaneId::MoveHistory,
        });
        stack.push(FocusContext::PaneExpanded {
            pane_id: PaneId::MoveHistory,
        });
        assert_eq!(stack.expanded_pane(), Some(PaneId::MoveHistory));
        assert_eq!(stack.selected_pane(), None); // Expanded, not Selected
    }

    #[test]
    fn test_pop_returns_to_previous() {
        let mut stack = FocusStack::new();
        stack.push(FocusContext::PaneSelected {
            pane_id: PaneId::MoveHistory,
        });
        assert!(stack.pop());
        assert!(stack.is_board_focused());
    }

    #[test]
    fn test_pop_at_board_returns_false() {
        let mut stack = FocusStack::new();
        assert!(!stack.pop());
        assert!(stack.is_board_focused());
    }

    #[test]
    fn test_double_push_and_double_pop() {
        let mut stack = FocusStack::new();
        stack.push(FocusContext::PaneSelected {
            pane_id: PaneId::EngineAnalysis,
        });
        stack.push(FocusContext::PaneExpanded {
            pane_id: PaneId::EngineAnalysis,
        });

        assert!(stack.pop()); // Back to PaneSelected
        assert_eq!(stack.selected_pane(), Some(PaneId::EngineAnalysis));

        assert!(stack.pop()); // Back to Board
        assert!(stack.is_board_focused());
    }

    #[test]
    fn test_selected_pane_from_expanded() {
        let mut stack = FocusStack::new();
        stack.push(FocusContext::PaneExpanded {
            pane_id: PaneId::UciDebug,
        });
        // In Expanded context, selected_pane() returns None
        assert_eq!(stack.selected_pane(), None);
        assert_eq!(stack.expanded_pane(), Some(PaneId::UciDebug));
    }
}

use ratatui::style::Color;
use std::collections::HashMap;

/// Identifies a pane by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    GameInfo,
    MoveHistory,
    EngineAnalysis,
    UciDebug,
    ReviewSummary,
}

/// Static properties describing a pane's capabilities.
#[allow(dead_code)]
pub struct PaneProperties {
    pub id: PaneId,
    pub title: &'static str,
    pub is_selectable: bool,
    pub is_expandable: bool,
    pub border_color: Color,
    pub preferred_height: u16,
}

/// Returns the static properties for a given pane.
pub fn pane_properties(id: PaneId) -> PaneProperties {
    match id {
        PaneId::GameInfo => PaneProperties {
            id,
            title: "Game Info",
            is_selectable: false,
            is_expandable: false,
            border_color: Color::Cyan,
            preferred_height: 10,
        },
        PaneId::MoveHistory => PaneProperties {
            id,
            title: "Move History",
            is_selectable: true,
            is_expandable: true,
            border_color: Color::Cyan,
            preferred_height: 15,
        },
        PaneId::EngineAnalysis => PaneProperties {
            id,
            title: "Engine Analysis",
            is_selectable: true,
            is_expandable: true,
            border_color: Color::Cyan,
            preferred_height: 12,
        },
        PaneId::UciDebug => PaneProperties {
            id,
            title: "UCI Debug",
            is_selectable: true,
            is_expandable: true,
            border_color: Color::Magenta,
            preferred_height: 15,
        },
        PaneId::ReviewSummary => PaneProperties {
            id,
            title: "Review Summary",
            is_selectable: true,
            is_expandable: true,
            border_color: Color::Green,
            preferred_height: 15,
        },
    }
}

/// Returns true if a scrollbar should be rendered for the given content/visible heights.
#[allow(dead_code)]
pub fn needs_scrollbar(content_height: u16, visible_height: u16) -> bool {
    content_height > visible_height
}

/// Manages the collection of panes, their visibility, scroll positions, and ordering.
pub struct PaneManager {
    /// Determines the render and tab order of panes.
    pane_order: Vec<PaneId>,
    /// Which panes are currently visible.
    visibility: HashMap<PaneId, bool>,
    /// Scroll position per pane.
    scroll_positions: HashMap<PaneId, u16>,
}

impl PaneManager {
    /// Create a new PaneManager with default configuration.
    pub fn new() -> Self {
        let pane_order = vec![
            PaneId::GameInfo,
            PaneId::EngineAnalysis,
            PaneId::MoveHistory,
            PaneId::ReviewSummary,
            PaneId::UciDebug,
        ];

        let mut visibility = HashMap::new();
        visibility.insert(PaneId::GameInfo, true);
        visibility.insert(PaneId::MoveHistory, true);
        visibility.insert(PaneId::EngineAnalysis, true);
        visibility.insert(PaneId::ReviewSummary, false);
        visibility.insert(PaneId::UciDebug, false);

        let mut scroll_positions = HashMap::new();
        for &id in &pane_order {
            scroll_positions.insert(id, 0);
        }

        Self {
            pane_order,
            visibility,
            scroll_positions,
        }
    }

    /// Returns all currently visible panes in render order.
    pub fn visible_panes(&self) -> Vec<PaneId> {
        self.pane_order
            .iter()
            .filter(|id| self.is_visible(**id))
            .copied()
            .collect()
    }

    /// Returns only visible panes that are selectable, in order.
    pub fn visible_selectable_panes(&self) -> Vec<PaneId> {
        self.pane_order
            .iter()
            .filter(|id| self.is_visible(**id) && pane_properties(**id).is_selectable)
            .copied()
            .collect()
    }

    /// Check if a pane is visible.
    pub fn is_visible(&self, id: PaneId) -> bool {
        self.visibility.get(&id).copied().unwrap_or(false)
    }

    /// Toggle visibility of a pane.
    pub fn toggle_visibility(&mut self, id: PaneId) {
        let entry = self.visibility.entry(id).or_insert(false);
        *entry = !*entry;
    }

    /// Get the scroll position for a pane.
    pub fn scroll(&self, id: PaneId) -> u16 {
        self.scroll_positions.get(&id).copied().unwrap_or(0)
    }

    /// Get a mutable reference to the scroll position for a pane.
    pub fn scroll_mut(&mut self, id: PaneId) -> &mut u16 {
        self.scroll_positions.entry(id).or_insert(0)
    }

    /// Get the next selectable pane after `current`, wrapping around.
    /// Returns None if no selectable panes are visible.
    pub fn next_selectable(&self, current: PaneId) -> Option<PaneId> {
        let selectable = self.visible_selectable_panes();
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|&id| id == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + 1) % selectable.len()]),
            None => Some(selectable[0]),
        }
    }

    /// Get the previous selectable pane before `current`, wrapping around.
    /// Returns None if no selectable panes are visible.
    pub fn prev_selectable(&self, current: PaneId) -> Option<PaneId> {
        let selectable = self.visible_selectable_panes();
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|&id| id == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + selectable.len() - 1) % selectable.len()]),
            None => Some(selectable[selectable.len() - 1]),
        }
    }

    /// Get the first visible selectable pane, if any.
    pub fn first_selectable(&self) -> Option<PaneId> {
        self.visible_selectable_panes().into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_visibility() {
        let pm = PaneManager::new();
        assert!(pm.is_visible(PaneId::GameInfo));
        assert!(pm.is_visible(PaneId::MoveHistory));
        assert!(pm.is_visible(PaneId::EngineAnalysis));
        assert!(!pm.is_visible(PaneId::UciDebug));
    }

    #[test]
    fn test_toggle_visibility() {
        let mut pm = PaneManager::new();
        assert!(!pm.is_visible(PaneId::UciDebug));
        pm.toggle_visibility(PaneId::UciDebug);
        assert!(pm.is_visible(PaneId::UciDebug));
        pm.toggle_visibility(PaneId::UciDebug);
        assert!(!pm.is_visible(PaneId::UciDebug));
    }

    #[test]
    fn test_visible_selectable_panes() {
        let pm = PaneManager::new();
        let selectable = pm.visible_selectable_panes();
        // GameInfo is not selectable, UciDebug is hidden
        assert_eq!(
            selectable,
            vec![PaneId::EngineAnalysis, PaneId::MoveHistory]
        );
    }

    #[test]
    fn test_scroll_positions() {
        let mut pm = PaneManager::new();
        assert_eq!(pm.scroll(PaneId::MoveHistory), 0);
        *pm.scroll_mut(PaneId::MoveHistory) = 10;
        assert_eq!(pm.scroll(PaneId::MoveHistory), 10);
    }

    #[test]
    fn test_next_selectable() {
        let pm = PaneManager::new();
        // Order: EngineAnalysis, MoveHistory (both visible + selectable)
        assert_eq!(
            pm.next_selectable(PaneId::EngineAnalysis),
            Some(PaneId::MoveHistory)
        );
        assert_eq!(
            pm.next_selectable(PaneId::MoveHistory),
            Some(PaneId::EngineAnalysis)
        );
    }

    #[test]
    fn test_prev_selectable() {
        let pm = PaneManager::new();
        assert_eq!(
            pm.prev_selectable(PaneId::EngineAnalysis),
            Some(PaneId::MoveHistory)
        );
        assert_eq!(
            pm.prev_selectable(PaneId::MoveHistory),
            Some(PaneId::EngineAnalysis)
        );
    }

    #[test]
    fn test_next_selectable_skips_hidden() {
        let mut pm = PaneManager::new();
        pm.toggle_visibility(PaneId::UciDebug); // Now visible + selectable
                                                // Order: EngineAnalysis, MoveHistory, UciDebug
        assert_eq!(
            pm.next_selectable(PaneId::EngineAnalysis),
            Some(PaneId::MoveHistory)
        );
        assert_eq!(
            pm.next_selectable(PaneId::MoveHistory),
            Some(PaneId::UciDebug)
        );
        assert_eq!(
            pm.next_selectable(PaneId::UciDebug),
            Some(PaneId::EngineAnalysis)
        );

        // Now hide EngineAnalysis
        pm.toggle_visibility(PaneId::EngineAnalysis);
        assert_eq!(
            pm.next_selectable(PaneId::MoveHistory),
            Some(PaneId::UciDebug)
        );
        assert_eq!(
            pm.next_selectable(PaneId::UciDebug),
            Some(PaneId::MoveHistory)
        );
    }

    #[test]
    fn test_next_selectable_none_visible() {
        let mut pm = PaneManager::new();
        pm.toggle_visibility(PaneId::EngineAnalysis); // hide
        pm.toggle_visibility(PaneId::MoveHistory); // hide
                                                   // UciDebug is already hidden, GameInfo is not selectable
        assert_eq!(pm.next_selectable(PaneId::MoveHistory), None);
    }

    #[test]
    fn test_scrollbar_not_needed() {
        assert!(!needs_scrollbar(5, 10));
    }

    #[test]
    fn test_scrollbar_needed() {
        assert!(needs_scrollbar(20, 10));
    }

    #[test]
    fn test_scrollbar_exact_fit() {
        assert!(!needs_scrollbar(10, 10));
    }
}

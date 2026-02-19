use std::collections::HashMap;

use super::component::Component;
use super::render_spec::{Column, ColumnContent, Layout, Row};

#[derive(Debug, Clone, PartialEq)]
pub enum FocusMode {
    Board,
    ComponentSelected { component: Component },
    ComponentExpanded { component: Component },
}

impl Default for FocusMode {
    fn default() -> Self {
        FocusMode::Board
    }
}

pub struct ComponentManager {
    pub visibility: HashMap<Component, bool>,
    pub scroll: HashMap<Component, u16>,
    pub focus_mode: FocusMode,
}

impl ComponentManager {
    pub fn new() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert(Component::InfoPanel, true);
        visibility.insert(Component::HistoryPanel, true);
        visibility.insert(Component::EnginePanel, true);
        visibility.insert(Component::ReviewSummary, false);
        visibility.insert(Component::AdvancedAnalysis, false);
        visibility.insert(Component::DebugPanel, false);

        let mut scroll = HashMap::new();
        scroll.insert(Component::InfoPanel, 0);
        scroll.insert(Component::HistoryPanel, 0);
        scroll.insert(Component::EnginePanel, 0);
        scroll.insert(Component::DebugPanel, 0);
        scroll.insert(Component::ReviewSummary, 0);
        scroll.insert(Component::AdvancedAnalysis, 0);

        Self {
            visibility,
            scroll,
            focus_mode: FocusMode::default(),
        }
    }

    pub fn game_board() -> Self {
        let mut manager = Self::new();
        manager.visibility.insert(Component::InfoPanel, true);
        manager.visibility.insert(Component::HistoryPanel, true);
        manager.visibility.insert(Component::EnginePanel, true);
        manager.visibility.insert(Component::ReviewSummary, false);
        manager
            .visibility
            .insert(Component::AdvancedAnalysis, false);
        manager
    }

    pub fn review_board() -> Self {
        let mut manager = Self::new();
        manager.visibility.insert(Component::InfoPanel, true);
        manager.visibility.insert(Component::HistoryPanel, true);
        manager.visibility.insert(Component::EnginePanel, false);
        manager.visibility.insert(Component::ReviewSummary, true);
        manager.visibility.insert(Component::AdvancedAnalysis, true);
        manager
    }

    pub fn is_visible(&self, component: &Component) -> bool {
        self.visibility.get(component).copied().unwrap_or(false)
    }

    pub fn set_visible(&mut self, component: Component, visible: bool) {
        self.visibility.insert(component, visible);
    }

    pub fn toggle_visibility(&mut self, component: Component) {
        let current = self.visibility.get(&component).copied().unwrap_or(false);
        self.visibility.insert(component, !current);
    }

    pub fn scroll(&self, component: &Component) -> u16 {
        self.scroll.get(component).copied().unwrap_or(0)
    }

    pub fn scroll_mut(&mut self, component: &Component) -> &mut u16 {
        self.scroll.entry(component.clone()).or_insert(0)
    }

    pub fn is_board_focused(&self) -> bool {
        matches!(self.focus_mode, FocusMode::Board)
    }

    pub fn selected_component(&self) -> Option<Component> {
        match &self.focus_mode {
            FocusMode::ComponentSelected { component } => Some(*component),
            _ => None,
        }
    }

    pub fn expanded_component(&self) -> Option<Component> {
        match &self.focus_mode {
            FocusMode::ComponentExpanded { component } => Some(*component),
            _ => None,
        }
    }

    pub fn select_component(&mut self, component: Component) {
        self.focus_mode = FocusMode::ComponentSelected { component };
    }

    pub fn expand_component(&mut self, component: Component) {
        self.focus_mode = FocusMode::ComponentExpanded { component };
    }

    pub fn clear_focus(&mut self) -> bool {
        if !matches!(self.focus_mode, FocusMode::Board) {
            self.focus_mode = FocusMode::Board;
            true
        } else {
            false
        }
    }

    fn flatten_columns(&self, columns: &[Column]) -> Vec<Component> {
        let mut result = Vec::new();
        for column in columns {
            match &column.content {
                ColumnContent::Component(component) => {
                    if component.is_selectable() && self.is_visible(component) {
                        result.push(*component);
                    }
                }
                ColumnContent::Nested(nested_columns) => {
                    result.extend(self.flatten_columns(nested_columns));
                }
            }
        }
        result
    }

    pub fn tab_order(&self, layout: &Layout) -> Vec<Component> {
        let mut result = Vec::new();
        for row in &layout.rows {
            result.extend(self.flatten_columns(&row.columns));
        }
        result
    }

    fn visible_selectable_components(&self, layout: &Layout) -> Vec<Component> {
        self.tab_order(layout)
    }

    pub fn next_component(&self, current: Component, layout: &Layout) -> Option<Component> {
        let selectable = self.visible_selectable_components(layout);
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + 1) % selectable.len()]),
            None => selectable.first().copied(),
        }
    }

    pub fn prev_component(&self, current: Component, layout: &Layout) -> Option<Component> {
        let selectable = self.visible_selectable_components(layout);
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + selectable.len() - 1) % selectable.len()]),
            None => selectable.last().copied(),
        }
    }

    pub fn first_component(&self, layout: &Layout) -> Option<Component> {
        self.visible_selectable_components(layout)
            .into_iter()
            .next()
    }
}

impl Default for ComponentManager {
    fn default() -> Self {
        Self::new()
    }
}

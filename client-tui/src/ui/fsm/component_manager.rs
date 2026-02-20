use std::collections::HashMap;

use super::component::Component;
use super::render_spec::{Layout, Section, SectionContent};

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

        tracing::debug!(
            "ComponentManager::new() created at {:p}",
            &visibility as *const _
        );

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
        let result = match &self.focus_mode {
            FocusMode::ComponentSelected { component } => Some(*component),
            _ => None,
        };
        tracing::debug!(
            "ComponentManager::selected_component() -> {:?} (focus_mode: {:?})",
            result,
            self.focus_mode
        );
        result
    }

    pub fn expanded_component(&self) -> Option<Component> {
        match &self.focus_mode {
            FocusMode::ComponentExpanded { component } => Some(*component),
            _ => None,
        }
    }

    pub fn select_component(&mut self, component: Component) {
        tracing::debug!("ComponentManager::select_component({:?}) called", component);
        self.focus_mode = FocusMode::ComponentSelected { component };
        tracing::debug!("  focus_mode is now: {:?}", self.focus_mode);
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

    fn flatten_sections(&self, sections: &[Section]) -> Vec<Component> {
        let mut result = Vec::new();
        for section in sections {
            match &section.content {
                SectionContent::Component(component) => {
                    if component.is_selectable() && self.is_visible(component) {
                        result.push(*component);
                    }
                }
                SectionContent::Nested(nested_sections) => {
                    result.extend(self.flatten_sections(nested_sections));
                }
            }
        }
        result
    }

    pub fn tab_order(&self, layout: &Layout) -> Vec<Component> {
        let mut result = Vec::new();
        for row in &layout.rows {
            result.extend(self.flatten_sections(&row.sections));
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

    pub fn section_index(&self, component: Component, layout: &Layout) -> Option<usize> {
        let sections_by_pos = self.sections_by_position(layout);
        for (section_idx, sections) in sections_by_pos.into_iter().enumerate() {
            for section in sections {
                if self.section_contains_component(component, &section) {
                    return Some(section_idx);
                }
            }
        }
        None
    }

    pub fn components_in_section(&self, section_index: usize, layout: &Layout) -> Vec<Component> {
        let sections_by_pos = self.sections_by_position(layout);
        sections_by_pos
            .get(section_index)
            .map(|sections| self.flatten_sections(sections.as_slice()))
            .unwrap_or_default()
    }

    pub fn next_in_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let components = self.components_in_section(section_idx, layout);
        if components.is_empty() {
            return None;
        }
        let current_idx = components.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(components[(idx + 1) % components.len()]),
            None => components.first().copied(),
        }
    }

    pub fn prev_in_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let components = self.components_in_section(section_idx, layout);
        if components.is_empty() {
            return None;
        }
        let current_idx = components.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(components[(idx + components.len() - 1) % components.len()]),
            None => components.last().copied(),
        }
    }

    pub fn next_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let sections_by_pos = self.sections_by_position(layout);
        let num_sections = sections_by_pos.len();
        if num_sections == 0 {
            return None;
        }
        let next_section_idx = (section_idx + 1) % num_sections;
        self.components_in_section(next_section_idx, layout)
            .into_iter()
            .next()
    }

    pub fn prev_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let sections_by_pos = self.sections_by_position(layout);
        let num_sections = sections_by_pos.len();
        if num_sections == 0 {
            return None;
        }
        let prev_section_idx = if section_idx == 0 {
            num_sections - 1
        } else {
            section_idx - 1
        };
        self.components_in_section(prev_section_idx, layout)
            .into_iter()
            .next()
    }

    fn sections_by_position(&self, layout: &Layout) -> Vec<Vec<Section>> {
        let max_sections = layout
            .rows
            .iter()
            .map(|r| r.sections.len())
            .max()
            .unwrap_or(0);
        let mut result: Vec<Vec<Section>> = vec![Vec::new(); max_sections];
        let content_rows = layout.rows.len().saturating_sub(1);
        for row in layout.rows.iter().take(content_rows) {
            for (section_idx, section) in row.sections.iter().enumerate() {
                if section_idx < result.len() {
                    result[section_idx].push(section.clone());
                }
            }
        }
        result
    }

    fn section_contains_component(&self, component: Component, section: &Section) -> bool {
        match &section.content {
            SectionContent::Component(c) => *c == component,
            SectionContent::Nested(nested) => nested
                .iter()
                .any(|section| self.section_contains_component(component, section)),
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

use crate::state::GameMode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Items available in the in-game popup menu.
#[derive(Debug, Clone, PartialEq)]
pub enum PopupMenuItem {
    Restart,
    AdjustDifficulty,
    SuspendSession,
    Quit,
}

impl PopupMenuItem {
    pub fn label(&self) -> &'static str {
        match self {
            PopupMenuItem::Restart => "Restart Game",
            PopupMenuItem::AdjustDifficulty => "Adjust Difficulty",
            PopupMenuItem::SuspendSession => "Suspend Session",
            PopupMenuItem::Quit => "Quit to Menu",
        }
    }
}

/// State for the in-game popup menu.
#[derive(Debug, Clone)]
pub struct PopupMenuState {
    pub selected_index: usize,
    pub items: Vec<PopupMenuItem>,
}

impl PopupMenuState {
    /// Create a new popup menu state based on the current game mode.
    pub fn new(mode: &GameMode) -> Self {
        let mut items = vec![PopupMenuItem::Restart];

        // Only show difficulty adjustment if an engine is involved
        if matches!(
            mode,
            GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
        ) {
            items.push(PopupMenuItem::AdjustDifficulty);
        }

        items.push(PopupMenuItem::SuspendSession);
        items.push(PopupMenuItem::Quit);

        Self {
            selected_index: 0,
            items,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if self.selected_index < self.items.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected item.
    pub fn selected_item(&self) -> &PopupMenuItem {
        &self.items[self.selected_index]
    }
}

/// Widget for rendering the popup menu as a centered overlay.
pub struct PopupMenuWidget<'a> {
    pub state: &'a PopupMenuState,
}

impl Widget for PopupMenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate centered popup area
        let popup_width = 30u16;
        let popup_height = (self.state.items.len() as u16) + 4; // items + border + title + hint
        let popup_area = centered_rect(popup_width, popup_height, area);

        // Clear the background
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Menu ")
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let mut lines = vec![];

        for (i, item) in self.state.items.iter().enumerate() {
            let is_selected = i == self.state.selected_index;
            let prefix = if is_selected { " \u{25b6} " } else { "   " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, item.label()),
                style,
            )));
        }

        // Add hint at bottom
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Esc: Close  Enter: Select",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

/// Helper to create a centered Rect within an area.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);

    horizontal[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_items_with_engine() {
        let state = PopupMenuState::new(&GameMode::HumanVsEngine {
            human_side: crate::state::PlayerColor::White,
        });
        assert!(state.items.contains(&PopupMenuItem::Restart));
        assert!(state.items.contains(&PopupMenuItem::AdjustDifficulty));
        assert!(state.items.contains(&PopupMenuItem::SuspendSession));
        assert!(state.items.contains(&PopupMenuItem::Quit));
    }

    #[test]
    fn test_menu_items_without_engine() {
        let state = PopupMenuState::new(&GameMode::HumanVsHuman);
        assert!(state.items.contains(&PopupMenuItem::Restart));
        assert!(!state.items.contains(&PopupMenuItem::AdjustDifficulty));
        assert!(state.items.contains(&PopupMenuItem::SuspendSession));
        assert!(state.items.contains(&PopupMenuItem::Quit));
    }

    #[test]
    fn test_menu_items_engine_vs_engine() {
        let state = PopupMenuState::new(&GameMode::EngineVsEngine);
        assert!(state.items.contains(&PopupMenuItem::AdjustDifficulty));
    }

    #[test]
    fn test_navigate_down() {
        let mut state = PopupMenuState::new(&GameMode::HumanVsHuman);
        assert_eq!(state.selected_index, 0);
        state.move_down();
        assert_eq!(state.selected_index, 1);
        // Move down past end should clamp
        for _ in 0..10 {
            state.move_down();
        }
        assert_eq!(state.selected_index, state.items.len() - 1);
    }

    #[test]
    fn test_navigate_up() {
        let mut state = PopupMenuState::new(&GameMode::HumanVsHuman);
        state.move_up(); // At 0, should stay at 0
        assert_eq!(state.selected_index, 0);
        state.move_down();
        state.move_down();
        state.move_up();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_selected_item() {
        let state = PopupMenuState::new(&GameMode::HumanVsHuman);
        assert_eq!(*state.selected_item(), PopupMenuItem::Restart);
    }
}

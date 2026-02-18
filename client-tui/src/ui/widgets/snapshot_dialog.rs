use chess_client::PositionReview;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Which field in the snapshot dialog currently has focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapshotDialogFocus {
    MovesBack,
    Name,
    PlayNow,
}

/// State for the snapshot creation dialog.
#[derive(Debug, Clone)]
pub struct SnapshotDialogState {
    /// How many moves before the current ply to rewind.
    pub moves_back: u32,
    /// Maximum value for moves_back (equals current_ply).
    pub max_moves_back: u32,
    /// Editable name for the snapshot.
    pub name_buffer: String,
    /// Whether to play immediately or save for later.
    pub play_immediately: bool,
    /// Which field is focused.
    pub focus: SnapshotDialogFocus,
    /// Current ply for display.
    pub current_ply: u32,
    /// Game ID for default name generation.
    pub game_id: String,
    /// Whether the target position is terminal (checkmate/stalemate).
    pub is_target_terminal: bool,
}

/// Convert a ply number to a human-readable move number (round).
/// Ply 0 → move 0, ply 1 → move 1, ply 2 → move 1, ply 3 → move 2, etc.
pub fn move_number_from_ply(ply: u32) -> u32 {
    ply.div_ceil(2)
}

/// Determine the side to move after a given ply.
/// Ply 0 (start) → White to move; ply 1 (after White moved) → Black to move; etc.
pub fn side_to_move_at_ply(ply: u32) -> &'static str {
    if ply.is_multiple_of(2) {
        "White"
    } else {
        "Black"
    }
}

/// Check if a FEN represents a terminal position (checkmate or stalemate).
fn is_terminal_fen(fen: &str) -> bool {
    if let Ok(board) = fen.parse::<cozy_chess::Board>() {
        board.status() != cozy_chess::GameStatus::Ongoing
    } else {
        false
    }
}

impl SnapshotDialogState {
    /// Create a new snapshot dialog with defaults populated from review state.
    pub fn new(current_ply: u32, game_id: &str, positions: &[PositionReview]) -> Self {
        let moves_back = if current_ply > 0 { 1 } else { 0 };
        let target_ply = current_ply.saturating_sub(moves_back);
        let is_target_terminal = check_terminal_at_ply(target_ply, positions);
        Self {
            moves_back,
            max_moves_back: current_ply,
            name_buffer: String::new(),
            play_immediately: true,
            focus: SnapshotDialogFocus::MovesBack,
            current_ply,
            game_id: game_id.to_string(),
            is_target_terminal,
        }
    }

    /// Returns the default name for the current target ply.
    pub fn default_name(&self) -> String {
        format_default_name(self.target_ply(), &self.game_id)
    }

    /// Returns the effective name: the user's input, or the default if empty.
    pub fn effective_name(&self) -> String {
        if self.name_buffer.is_empty() {
            self.default_name()
        } else {
            self.name_buffer.clone()
        }
    }

    /// Computed target ply.
    pub fn target_ply(&self) -> u32 {
        self.current_ply.saturating_sub(self.moves_back)
    }

    /// Increment moves_back (clamped to max).
    pub fn increment_moves_back(&mut self, positions: &[PositionReview]) {
        if self.moves_back < self.max_moves_back {
            self.moves_back += 1;
            self.is_target_terminal = check_terminal_at_ply(self.target_ply(), positions);
        }
    }

    /// Decrement moves_back (clamped to 0).
    pub fn decrement_moves_back(&mut self, positions: &[PositionReview]) {
        if self.moves_back > 0 {
            self.moves_back -= 1;
            self.is_target_terminal = check_terminal_at_ply(self.target_ply(), positions);
        }
    }

    /// Cycle focus to the next field.
    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            SnapshotDialogFocus::MovesBack => SnapshotDialogFocus::Name,
            SnapshotDialogFocus::Name => SnapshotDialogFocus::PlayNow,
            SnapshotDialogFocus::PlayNow => SnapshotDialogFocus::MovesBack,
        };
    }

    /// Cycle focus to the previous field.
    pub fn prev_focus(&mut self) {
        self.focus = match self.focus {
            SnapshotDialogFocus::MovesBack => SnapshotDialogFocus::PlayNow,
            SnapshotDialogFocus::Name => SnapshotDialogFocus::MovesBack,
            SnapshotDialogFocus::PlayNow => SnapshotDialogFocus::Name,
        };
    }
}

/// Check whether the position at the given ply is terminal.
fn check_terminal_at_ply(target_ply: u32, positions: &[PositionReview]) -> bool {
    if target_ply == 0 {
        return false; // Starting position is never terminal
    }
    positions
        .iter()
        .find(|p| p.ply == target_ply)
        .map(|p| is_terminal_fen(&p.fen))
        .unwrap_or(false)
}

/// Format the default snapshot name with move number and side to move.
fn format_default_name(target_ply: u32, game_id: &str) -> String {
    let move_num = move_number_from_ply(target_ply);
    let side = side_to_move_at_ply(target_ply);
    format!("Move {} {} - {}", move_num, side, truncate_id(game_id))
}

fn truncate_id(id: &str) -> &str {
    if id.len() > 16 {
        &id[..16]
    } else {
        id
    }
}

/// Widget for rendering the snapshot dialog as a centered overlay.
pub struct SnapshotDialogWidget<'a> {
    pub state: &'a SnapshotDialogState,
}

impl Widget for SnapshotDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_width = 56u16;
        let has_warning = self.state.is_target_terminal;
        let popup_height = if has_warning { 16u16 } else { 14u16 };
        let popup_area = centered_rect(popup_width, popup_height, area);

        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Create Snapshot ")
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let mut constraints = vec![
            Constraint::Length(1), // blank
            Constraint::Length(1), // moves back
            Constraint::Length(1), // target info
            Constraint::Length(1), // hint or warning
            Constraint::Length(1), // blank
            Constraint::Length(1), // name
            Constraint::Length(1), // blank
            Constraint::Length(1), // play immediately
            Constraint::Length(1), // blank
            Constraint::Length(1), // footer
        ];
        if has_warning {
            // Insert warning line after hint (index 3) and extra blank
            constraints.insert(4, Constraint::Length(1)); // warning
            constraints.insert(5, Constraint::Length(1)); // extra blank
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        let active_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let normal_style = Style::default().fg(Color::White);
        let dim_style = Style::default().fg(Color::DarkGray);

        // Moves back
        let mb_style = if self.state.focus == SnapshotDialogFocus::MovesBack {
            active_style
        } else {
            normal_style
        };
        let moves_back_line = Line::from(vec![
            Span::styled("  Moves before current ply: ", normal_style),
            Span::styled(format!("[ {} ]", self.state.moves_back), mb_style),
        ]);
        Paragraph::new(moves_back_line).render(chunks[1], buf);

        // Target info: "Target: Move 10, White to move (ply 19)"
        let target_ply = self.state.target_ply();
        let move_num = move_number_from_ply(target_ply);
        let side = side_to_move_at_ply(target_ply);
        let target_line = Line::from(vec![
            Span::styled("  Target: ", normal_style),
            Span::styled(
                format!("Move {}, {} to move", move_num, side),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" (ply {})", target_ply), dim_style),
        ]);
        Paragraph::new(target_line).render(chunks[2], buf);

        // Hint for moves back
        if self.state.focus == SnapshotDialogFocus::MovesBack {
            Paragraph::new(Line::from(Span::styled(
                "                        (h/l or \u{2190}/\u{2192})",
                dim_style,
            )))
            .render(chunks[3], buf);
        }

        // Warning + offset for remaining elements
        let offset = if has_warning {
            let warning_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
            let warning_line = Line::from(Span::styled(
                "  \u{26a0} Terminal position \u{2014} cannot start here",
                warning_style,
            ));
            Paragraph::new(warning_line).render(chunks[4], buf);
            2
        } else {
            0
        };

        // Name
        let name_idx = 5 + offset;
        let name_style = if self.state.focus == SnapshotDialogFocus::Name {
            active_style
        } else {
            normal_style
        };
        let cursor = if self.state.focus == SnapshotDialogFocus::Name {
            "_"
        } else {
            ""
        };
        let name_line = if self.state.name_buffer.is_empty() {
            // Show placeholder (default name) in dim style
            let placeholder = self.state.default_name();
            let display = if placeholder.len() > 28 {
                format!("{}...", &placeholder[..25])
            } else {
                placeholder
            };
            Line::from(vec![
                Span::styled("  Name: ", normal_style),
                Span::styled(format!("[{}{}]", display, cursor), dim_style),
            ])
        } else {
            let display = if self.state.name_buffer.len() > 28 {
                format!("{}...", &self.state.name_buffer[..25])
            } else {
                self.state.name_buffer.clone()
            };
            Line::from(vec![
                Span::styled("  Name: ", normal_style),
                Span::styled(format!("[{}{}]", display, cursor), name_style),
            ])
        };
        Paragraph::new(name_line).render(chunks[name_idx], buf);

        // Play immediately toggle
        let play_idx = name_idx + 2;
        let pn_style = if self.state.focus == SnapshotDialogFocus::PlayNow {
            active_style
        } else {
            normal_style
        };
        let (yes_style, no_style) = if self.state.play_immediately {
            (
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                dim_style,
            )
        } else {
            (
                dim_style,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        };
        let play_line = Line::from(vec![
            Span::styled("  Play immediately?   ", pn_style),
            Span::styled(
                if self.state.play_immediately {
                    "[Yes]"
                } else {
                    " Yes "
                },
                yes_style,
            ),
            Span::styled(" / ", normal_style),
            Span::styled(
                if self.state.play_immediately {
                    " No "
                } else {
                    "[No]"
                },
                no_style,
            ),
        ]);
        Paragraph::new(play_line).render(chunks[play_idx], buf);

        // Footer
        let footer_idx = play_idx + 2;
        let footer = Line::from(Span::styled("  Enter: Confirm  Esc: Cancel", dim_style));
        Paragraph::new(footer).render(chunks[footer_idx], buf);
    }
}

/// Helper to create a centered Rect within an area.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
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

    fn empty_positions() -> Vec<PositionReview> {
        vec![]
    }

    fn sample_positions() -> Vec<PositionReview> {
        vec![
            PositionReview {
                ply: 1,
                fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".into(),
                ..Default::default()
            },
            PositionReview {
                ply: 2,
                fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".into(),
                ..Default::default()
            },
            PositionReview {
                ply: 3,
                fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".into(),
                ..Default::default()
            },
        ]
    }

    /// Fool's mate positions (4 plies, ply 4 is checkmate).
    fn fools_mate_positions() -> Vec<PositionReview> {
        vec![
            PositionReview {
                ply: 1,
                fen: "rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq - 0 1".into(),
                ..Default::default()
            },
            PositionReview {
                ply: 2,
                fen: "rnbqkbnr/pppp1ppp/8/4p3/8/5P2/PPPPP1PP/RNBQKBNR w KQkq - 0 2".into(),
                ..Default::default()
            },
            PositionReview {
                ply: 3,
                fen: "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2".into(),
                ..Default::default()
            },
            PositionReview {
                ply: 4,
                // After Qh4# — checkmate
                fen: "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3".into(),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn test_new_defaults() {
        let dialog = SnapshotDialogState::new(10, "game_abc", &sample_positions());
        assert_eq!(dialog.moves_back, 1);
        assert_eq!(dialog.target_ply(), 9);
        assert!(dialog.play_immediately);
        assert_eq!(dialog.focus, SnapshotDialogFocus::MovesBack);
    }

    #[test]
    fn test_new_defaults_at_zero() {
        let dialog = SnapshotDialogState::new(0, "game_abc", &empty_positions());
        assert_eq!(dialog.moves_back, 0);
        assert_eq!(dialog.target_ply(), 0);
    }

    #[test]
    fn test_target_ply_computation() {
        let dialog = SnapshotDialogState::new(20, "test", &empty_positions());
        assert_eq!(dialog.target_ply(), 19); // moves_back=1

        let mut dialog2 = SnapshotDialogState::new(20, "test", &empty_positions());
        dialog2.moves_back = 0;
        assert_eq!(dialog2.target_ply(), 20);
        dialog2.moves_back = 5;
        assert_eq!(dialog2.target_ply(), 15);
        dialog2.moves_back = 20;
        assert_eq!(dialog2.target_ply(), 0);
    }

    #[test]
    fn test_increment_decrement_clamping() {
        let positions = empty_positions();
        let mut dialog = SnapshotDialogState::new(3, "test", &positions);
        assert_eq!(dialog.moves_back, 1);

        // Increment to max
        dialog.increment_moves_back(&positions);
        assert_eq!(dialog.moves_back, 2);
        dialog.increment_moves_back(&positions);
        assert_eq!(dialog.moves_back, 3);
        dialog.increment_moves_back(&positions); // clamped
        assert_eq!(dialog.moves_back, 3);

        // Decrement to 0
        dialog.decrement_moves_back(&positions);
        assert_eq!(dialog.moves_back, 2);
        dialog.decrement_moves_back(&positions);
        assert_eq!(dialog.moves_back, 1);
        dialog.decrement_moves_back(&positions);
        assert_eq!(dialog.moves_back, 0);
        dialog.decrement_moves_back(&positions); // clamped
        assert_eq!(dialog.moves_back, 0);
    }

    #[test]
    fn test_focus_cycling() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        assert_eq!(dialog.focus, SnapshotDialogFocus::MovesBack);

        dialog.next_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
        dialog.next_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::PlayNow);
        dialog.next_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::MovesBack);

        // Reverse
        dialog.prev_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::PlayNow);
        dialog.prev_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
        dialog.prev_focus();
        assert_eq!(dialog.focus, SnapshotDialogFocus::MovesBack);
    }

    #[test]
    fn test_move_number_from_ply() {
        assert_eq!(move_number_from_ply(0), 0);
        assert_eq!(move_number_from_ply(1), 1);
        assert_eq!(move_number_from_ply(2), 1);
        assert_eq!(move_number_from_ply(3), 2);
        assert_eq!(move_number_from_ply(4), 2);
        assert_eq!(move_number_from_ply(19), 10);
        assert_eq!(move_number_from_ply(20), 10);
    }

    #[test]
    fn test_side_to_move_at_ply() {
        assert_eq!(side_to_move_at_ply(0), "White");
        assert_eq!(side_to_move_at_ply(1), "Black");
        assert_eq!(side_to_move_at_ply(2), "White");
        assert_eq!(side_to_move_at_ply(3), "Black");
    }

    #[test]
    fn test_name_buffer_starts_empty() {
        let dialog = SnapshotDialogState::new(19, "game_abc", &empty_positions());
        assert!(dialog.name_buffer.is_empty());
    }

    #[test]
    fn test_default_name_includes_move_number() {
        let dialog = SnapshotDialogState::new(19, "game_abc", &empty_positions());
        // target_ply = 18, move_number = 9, side = White (even ply)
        let name = dialog.default_name();
        assert!(name.contains("Move 9"), "got: {}", name);
        assert!(name.contains("White"), "got: {}", name);
    }

    #[test]
    fn test_effective_name_uses_default_when_empty() {
        let dialog = SnapshotDialogState::new(10, "game_abc", &empty_positions());
        assert!(dialog.name_buffer.is_empty());
        let effective = dialog.effective_name();
        assert_eq!(effective, dialog.default_name());
    }

    #[test]
    fn test_effective_name_uses_user_input_when_set() {
        let mut dialog = SnapshotDialogState::new(10, "game_abc", &empty_positions());
        dialog.name_buffer = "my custom name".to_string();
        assert_eq!(dialog.effective_name(), "my custom name");
    }

    #[test]
    fn test_terminal_ply_detected() {
        let positions = fools_mate_positions();
        // current_ply=4, moves_back=0 → target_ply=4 (the checkmate)
        let mut dialog = SnapshotDialogState::new(4, "test", &positions);
        dialog.moves_back = 0;
        dialog.is_target_terminal = check_terminal_at_ply(dialog.target_ply(), &positions);
        assert!(dialog.is_target_terminal);
    }

    #[test]
    fn test_terminal_ply_not_detected_for_non_terminal() {
        let positions = fools_mate_positions();
        // current_ply=4, moves_back=1 → target_ply=3 (not terminal)
        let dialog = SnapshotDialogState::new(4, "test", &positions);
        assert!(!dialog.is_target_terminal);
    }

    #[test]
    fn test_terminal_detection_updates_on_navigate() {
        let positions = fools_mate_positions();
        let mut dialog = SnapshotDialogState::new(4, "test", &positions);
        assert!(!dialog.is_target_terminal); // target_ply=3

        // Decrement moves_back to 0 → target_ply=4 (checkmate)
        dialog.decrement_moves_back(&positions);
        assert_eq!(dialog.moves_back, 0);
        assert!(dialog.is_target_terminal);

        // Increment back → target_ply=3 (not terminal)
        dialog.increment_moves_back(&positions);
        assert!(!dialog.is_target_terminal);
    }

    #[test]
    fn test_starting_position_not_terminal() {
        let positions = sample_positions();
        let mut dialog = SnapshotDialogState::new(3, "test", &positions);
        // Go all the way to target_ply=0
        dialog.moves_back = 3;
        dialog.is_target_terminal = check_terminal_at_ply(dialog.target_ply(), &positions);
        assert!(!dialog.is_target_terminal);
    }

    #[test]
    fn test_name_editable_when_focused() {
        let mut dialog = SnapshotDialogState::new(10, "game_abc", &empty_positions());
        dialog.focus = SnapshotDialogFocus::Name;

        // Clear the default name and type a custom one
        dialog.name_buffer.clear();
        for c in "my snapshot".chars() {
            dialog.name_buffer.push(c);
        }
        assert_eq!(dialog.name_buffer, "my snapshot");

        // Backspace removes the last character
        dialog.name_buffer.pop();
        assert_eq!(dialog.name_buffer, "my snapsho");
    }

    #[test]
    fn test_name_preserved_after_moves_back_change() {
        let positions = sample_positions();
        let mut dialog = SnapshotDialogState::new(3, "test", &positions);

        // Simulate user editing the name
        dialog.name_buffer = "custom name".to_string();

        // Changing moves_back should NOT overwrite the name
        dialog.increment_moves_back(&positions);
        assert_eq!(dialog.name_buffer, "custom name");

        dialog.decrement_moves_back(&positions);
        assert_eq!(dialog.name_buffer, "custom name");
    }

    #[test]
    fn test_name_preserved_through_full_moves_back_sweep() {
        let positions = empty_positions();
        let mut dialog = SnapshotDialogState::new(5, "test", &positions);
        dialog.name_buffer = "my game".to_string();

        // Sweep moves_back from 1 to max and back to 0
        for _ in 0..5 {
            dialog.increment_moves_back(&positions);
        }
        assert_eq!(dialog.moves_back, 5);
        assert_eq!(dialog.name_buffer, "my game");

        for _ in 0..5 {
            dialog.decrement_moves_back(&positions);
        }
        assert_eq!(dialog.moves_back, 0);
        assert_eq!(dialog.name_buffer, "my game");
    }

    /// Simulate the input dispatch: when focus is Name, 'j' should be typed
    /// into name_buffer (not trigger next_focus). This mirrors the guard in
    /// handle_snapshot_dialog_input.
    #[test]
    fn test_j_types_into_name_when_focused() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        dialog.name_buffer.clear();
        dialog.focus = SnapshotDialogFocus::Name;

        // Simulate: KeyCode::Char('j') when focus == Name → push to buffer
        let c = 'j';
        if dialog.focus == SnapshotDialogFocus::Name {
            dialog.name_buffer.push(c);
        } else {
            dialog.next_focus();
        }

        assert_eq!(dialog.name_buffer, "j");
        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
    }

    /// When focus is NOT Name, 'j' should cycle focus (not type into buffer).
    #[test]
    fn test_j_cycles_focus_when_not_on_name() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        dialog.name_buffer.clear();
        dialog.focus = SnapshotDialogFocus::MovesBack;

        let c = 'j';
        if dialog.focus == SnapshotDialogFocus::Name {
            dialog.name_buffer.push(c);
        } else {
            dialog.next_focus();
        }

        assert!(dialog.name_buffer.is_empty());
        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
    }

    /// Same guard logic for 'k': types into name when focused, cycles otherwise.
    #[test]
    fn test_k_types_into_name_when_focused() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        dialog.name_buffer.clear();
        dialog.focus = SnapshotDialogFocus::Name;

        let c = 'k';
        if dialog.focus == SnapshotDialogFocus::Name {
            dialog.name_buffer.push(c);
        } else {
            dialog.prev_focus();
        }

        assert_eq!(dialog.name_buffer, "k");
        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
    }

    #[test]
    fn test_k_cycles_focus_when_not_on_name() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        dialog.focus = SnapshotDialogFocus::PlayNow;

        let c = 'k';
        if dialog.focus == SnapshotDialogFocus::Name {
            dialog.name_buffer.push(c);
        } else {
            dialog.prev_focus();
        }

        assert_eq!(dialog.focus, SnapshotDialogFocus::Name);
    }

    /// All printable characters should be typeable into the name field.
    #[test]
    fn test_all_nav_keys_type_into_name_when_focused() {
        let mut dialog = SnapshotDialogState::new(5, "test", &empty_positions());
        dialog.name_buffer.clear();
        dialog.focus = SnapshotDialogFocus::Name;

        for c in ['h', 'j', 'k', 'l', 'a', 'z', '0', '9', ' ', '-'] {
            dialog.name_buffer.push(c);
        }

        assert_eq!(dialog.name_buffer, "hjklaz09 -");
    }
}

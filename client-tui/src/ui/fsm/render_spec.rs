use serde::{Deserialize, Serialize};

pub use super::component::Component;

use chess_client::MoveRecord;
use cozy_chess::{Board, Square};

// ============================================================================
// Input Phase - tracks move input state
// ============================================================================

#[derive(Clone, Debug, Copy, PartialEq, Default)]
pub enum InputPhase {
    #[default]
    SelectPiece,
    SelectDestination,
    SelectPromotion {
        from: Square,
        to: Square,
    },
}

// ============================================================================
// Tab Input State - for tab-based move input with typeahead
// ============================================================================

#[derive(Clone, Debug)]
pub struct TabInputState {
    pub active: bool,
    pub current_tab: usize,
    pub typeahead_buffer: String,
    pub from_square: Option<Square>,
}

impl Default for TabInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TabInputState {
    pub fn new() -> Self {
        Self {
            active: false,
            current_tab: 0,
            typeahead_buffer: String::new(),
            from_square: None,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.current_tab = 0;
        self.typeahead_buffer.clear();
        self.from_square = None;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.typeahead_buffer.clear();
        self.from_square = None;
    }

    pub fn advance_to_destination(&mut self, from: Square) {
        self.current_tab = 1;
        self.from_square = Some(from);
        self.typeahead_buffer.clear();
    }
}

/// A control displayed to the user (key + label)
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub key: &'static str,
    pub label: &'static str,
}

impl Control {
    pub fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

/// Overlay types - dialogs
/// Note: Dialog state is managed in GameSession, this just tracks what's active
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    None,
    PopupMenu,
    SnapshotDialog,
    PromotionDialog { from: Square, to: Square },
}

impl Default for Overlay {
    fn default() -> Self {
        Overlay::None
    }
}

// ============================================================================
// Review UI State - UI navigation state for review mode
// ============================================================================

#[derive(Clone, Debug)]
pub struct ReviewUIState {
    pub current_ply: u32,
    pub board_at_ply: Board,
    pub fen_at_ply: String,
    pub auto_play: bool,
    pub move_history: Vec<MoveRecord>,
}

impl Default for ReviewUIState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewUIState {
    pub fn new() -> Self {
        Self {
            current_ply: 0,
            board_at_ply: Board::default(),
            fen_at_ply: Board::default().to_string(),
            auto_play: false,
            move_history: Vec::new(),
        }
    }

    pub fn with_review(review: &chess_client::GameReviewProto) -> Self {
        let board = Board::default();
        let fen = board.to_string();
        Self {
            current_ply: 0,
            board_at_ply: board,
            fen_at_ply: fen,
            auto_play: false,
            move_history: Vec::new(), // Will be built from review positions
        }
    }

    pub fn go_to_ply(&mut self, ply: u32, review: &chess_client::GameReviewProto) {
        let max_ply = review.total_plies;
        let target = ply.min(max_ply);

        if target == 0 {
            self.board_at_ply = Board::default();
            self.fen_at_ply = self.board_at_ply.to_string();
        } else if let Some(position) = review.positions.get(target as usize) {
            self.board_at_ply = position.fen.parse().unwrap_or_default();
            self.fen_at_ply = position.fen.clone();
        }

        self.current_ply = target;
    }

    pub fn next_ply(&mut self, review: &chess_client::GameReviewProto) -> bool {
        if self.current_ply < review.total_plies {
            self.go_to_ply(self.current_ply + 1, review);
            true
        } else {
            false
        }
    }

    pub fn prev_ply(&mut self) {
        if self.current_ply > 0 {
            self.current_ply -= 1;
            // Note: In a full implementation, we'd rebuild the board here
        }
    }
}

/// Layout constraint types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    Percentage(u16),
    Min(u16),
    Length(u16),
    Ratio(u16, u16),
}

impl Default for Constraint {
    fn default() -> Self {
        Constraint::Min(10)
    }
}

/// Section content - either a component or nested sections
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SectionContent {
    Component(Component),
    Nested(Vec<Section>),
}

impl Default for SectionContent {
    fn default() -> Self {
        SectionContent::Component(Component::Board)
    }
}

/// A section in a layout row
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Section {
    pub constraint: Constraint,
    pub content: SectionContent,
}

impl Section {
    pub fn new(constraint: Constraint, content: SectionContent) -> Self {
        Self {
            constraint,
            content,
        }
    }

    pub fn component(constraint: Constraint, component: Component) -> Self {
        Self {
            constraint,
            content: SectionContent::Component(component),
        }
    }

    pub fn nested(constraint: Constraint, sections: Vec<Section>) -> Self {
        Self {
            constraint,
            content: SectionContent::Nested(sections),
        }
    }
}

/// A row in a layout
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Row {
    pub height: Constraint,
    pub sections: Vec<Section>,
}

impl Row {
    pub fn new(height: Constraint, sections: Vec<Section>) -> Self {
        Self { height, sections }
    }
}

/// The complete layout specification for a view
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub rows: Vec<Row>,
    pub overlay: Overlay,
}

impl Layout {
    pub fn new(rows: Vec<Row>, overlay: Overlay) -> Self {
        Self { rows, overlay }
    }

    /// Start screen - just the menu, no special layout needed
    pub fn start_screen() -> Self {
        Self::default()
    }

    /// Match summary layout - just controls at bottom
    pub fn match_summary() -> Self {
        Self {
            rows: vec![Row::new(
                Constraint::Length(1),
                vec![Section::component(
                    Constraint::Percentage(100),
                    Component::Controls,
                )],
            )],
            overlay: Overlay::None,
        }
    }
}


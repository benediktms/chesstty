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

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum View {
    #[default]
    StartScreen,
    GameBoard,
    ReviewBoard,
    MatchSummary,
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

/// Column content - either a component or nested columns
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ColumnContent {
    Component(Component),
    Nested(Vec<Column>),
}

impl Default for ColumnContent {
    fn default() -> Self {
        ColumnContent::Component(Component::Board)
    }
}

/// A column in a layout row
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Column {
    pub constraint: Constraint,
    pub content: ColumnContent,
}

impl Column {
    pub fn new(constraint: Constraint, content: ColumnContent) -> Self {
        Self {
            constraint,
            content,
        }
    }

    pub fn component(constraint: Constraint, component: Component) -> Self {
        Self {
            constraint,
            content: ColumnContent::Component(component),
        }
    }

    pub fn nested(constraint: Constraint, columns: Vec<Column>) -> Self {
        Self {
            constraint,
            content: ColumnContent::Nested(columns),
        }
    }
}

/// A row in a layout
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Row {
    pub height: Constraint,
    pub columns: Vec<Column>,
}

impl Row {
    pub fn new(height: Constraint, columns: Vec<Column>) -> Self {
        Self { height, columns }
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

    /// Game board layout:
    /// Row 1: [Board 75%] [Info/Engine/History 25%]
    /// Row 2: [Controls 100%]
    pub fn game_board() -> Self {
        Self {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left column empty (space redistributed)
                        Column::nested(
                            Constraint::Percentage(75),
                            vec![Column::component(Constraint::Min(10), Component::Board)],
                        ),
                        Column::nested(
                            Constraint::Percentage(25),
                            vec![
                                Column::component(Constraint::Length(8), Component::InfoPanel),
                                Column::component(Constraint::Length(12), Component::EnginePanel),
                                Column::component(Constraint::Min(10), Component::HistoryPanel),
                            ],
                        ),
                    ],
                ),
                Row::new(
                    Constraint::Length(1),
                    vec![Column::component(
                        Constraint::Percentage(100),
                        Component::Controls,
                    )],
                ),
            ],
            overlay: Overlay::None,
        }
    }

    /// Review board layout:
    /// Row 1: [Advanced/ReviewSummary 20%] [Board 55%] [Info/History 25%]
    /// Row 2: [Controls 100%]
    pub fn review_board() -> Self {
        Self {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left: Advanced Analysis (35%) + Review Summary
                        Column::nested(
                            Constraint::Percentage(20),
                            vec![
                                Column::component(
                                    Constraint::Percentage(35),
                                    Component::AdvancedAnalysis,
                                ),
                                Column::component(Constraint::Min(10), Component::ReviewSummary),
                            ],
                        ),
                        // Center: Board
                        Column::component(Constraint::Percentage(55), Component::Board),
                        // Right: Game Info + Move History
                        Column::nested(
                            Constraint::Percentage(25),
                            vec![
                                Column::component(Constraint::Length(8), Component::InfoPanel),
                                Column::component(Constraint::Min(10), Component::HistoryPanel),
                            ],
                        ),
                    ],
                ),
                Row::new(
                    Constraint::Length(1),
                    vec![Column::component(
                        Constraint::Percentage(100),
                        Component::Controls,
                    )],
                ),
            ],
            overlay: Overlay::None,
        }
    }

    /// Game board layout with expanded pane replacing board:
    /// Row 1: [Expanded Pane 75%] [Info/Engine/History 25%]
    /// Row 2: [Controls 100%]
    pub fn game_board_with_pane(component: Component) -> Self {
        Self {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left column: expanded pane (takes 75% that board normally occupies)
                        Column::component(Constraint::Percentage(75), component),
                        // Right panel stays in same position (25%)
                        Column::nested(
                            Constraint::Percentage(25),
                            vec![
                                Column::component(Constraint::Length(8), Component::InfoPanel),
                                Column::component(Constraint::Length(12), Component::EnginePanel),
                                Column::component(Constraint::Min(10), Component::HistoryPanel),
                            ],
                        ),
                    ],
                ),
                Row::new(
                    Constraint::Length(1),
                    vec![Column::component(
                        Constraint::Percentage(100),
                        Component::Controls,
                    )],
                ),
            ],
            overlay: Overlay::None,
        }
    }

    /// Review board layout with expanded pane replacing board:
    /// Row 1: [Advanced/ReviewSummary 20%] [Expanded Pane 55%] [Info/History 25%]
    /// Row 2: [Controls 100%]
    pub fn review_board_with_pane(component: Component) -> Self {
        Self {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left: Advanced Analysis (35%) + Review Summary
                        Column::nested(
                            Constraint::Percentage(20),
                            vec![
                                Column::component(
                                    Constraint::Percentage(35),
                                    Component::AdvancedAnalysis,
                                ),
                                Column::component(Constraint::Min(10), Component::ReviewSummary),
                            ],
                        ),
                        // Center: expanded pane (takes 55% that board normally occupies)
                        Column::component(Constraint::Percentage(55), component),
                        // Right: Game Info + Move History
                        Column::nested(
                            Constraint::Percentage(25),
                            vec![
                                Column::component(Constraint::Length(8), Component::InfoPanel),
                                Column::component(Constraint::Min(10), Component::HistoryPanel),
                            ],
                        ),
                    ],
                ),
                Row::new(
                    Constraint::Length(1),
                    vec![Column::component(
                        Constraint::Percentage(100),
                        Component::Controls,
                    )],
                ),
            ],
            overlay: Overlay::None,
        }
    }

    /// Match summary layout - just controls at bottom
    pub fn match_summary() -> Self {
        Self {
            rows: vec![Row::new(
                Constraint::Length(1),
                vec![Column::component(
                    Constraint::Percentage(100),
                    Component::Controls,
                )],
            )],
            overlay: Overlay::None,
        }
    }
}

/// Render spec - the complete specification for rendering
#[derive(Clone, Debug, Default)]
pub struct RenderSpec {
    pub view: View,
    pub layout: Layout,
    pub expanded_panel: Option<Component>,
}

impl RenderSpec {
    pub fn start_screen() -> Self {
        Self {
            view: View::StartScreen,
            layout: Layout::start_screen(),
            expanded_panel: None,
        }
    }

    pub fn game_board() -> Self {
        Self {
            view: View::GameBoard,
            layout: Layout::game_board(),
            expanded_panel: None,
        }
    }

    pub fn review_board() -> Self {
        Self {
            view: View::ReviewBoard,
            layout: Layout::review_board(),
            expanded_panel: None,
        }
    }

    pub fn match_summary() -> Self {
        Self {
            view: View::MatchSummary,
            layout: Layout::match_summary(),
            expanded_panel: None,
        }
    }

    /// Create RenderSpec based on game mode
    pub fn from_game_mode(
        mode: &crate::state::GameMode,
        expanded_panel: Option<Component>,
    ) -> Self {
        let mut spec = match mode {
            crate::state::GameMode::ReviewMode => Self::review_board(),
            _ => Self::game_board(),
        };
        spec.expanded_panel = expanded_panel;
        spec
    }

    /// Game board layout with expanded pane (pane replaces board in center column)
    pub fn game_board_with_pane(component: Component) -> Self {
        Self {
            view: View::GameBoard,
            layout: Layout::game_board_with_pane(component),
            expanded_panel: Some(component),
        }
    }

    /// Review board layout with expanded pane (pane replaces board in center column)
    pub fn review_board_with_pane(component: Component) -> Self {
        Self {
            view: View::ReviewBoard,
            layout: Layout::review_board_with_pane(component),
            expanded_panel: Some(component),
        }
    }
}

use chess_client::{
    AdvancedGameAnalysisProto, GameModeProto, GameReviewProto, MoveClassification, MoveRecord,
    PositionReview,
};
use cozy_chess::{Board, Square};

/// Local review navigation state. All data is fetched once from the server
/// and then navigated entirely client-side (no further server calls).
pub struct ReviewState {
    /// The full game review data from the server.
    pub review: GameReviewProto,
    /// Current ply being displayed (0 = starting position, max = total_plies).
    pub current_ply: u32,
    /// Board at the current ply position.
    pub board_at_ply: Board,
    /// FEN string at the current ply position (kept in sync with board_at_ply).
    pub fen_at_ply: String,
    /// Whether auto-play is active.
    pub auto_play: bool,
    /// Move history reconstructed from review positions for the MoveHistoryPanel.
    pub move_history: Vec<MoveRecord>,
    /// Original game mode (for creating snapshots that preserve engine settings).
    pub game_mode: Option<GameModeProto>,
    /// Original skill level (for creating snapshots that preserve difficulty).
    pub skill_level: u8,
    /// Advanced analysis data (tactical patterns, king safety, tension, psychological profiles).
    pub advanced: Option<AdvancedGameAnalysisProto>,
}

impl ReviewState {
    /// Navigate to a specific ply. Rebuilds the board from the position FENs
    /// stored in the review data.
    pub fn go_to_ply(&mut self, ply: u32) {
        let max_ply = self.review.total_plies;
        let target = ply.min(max_ply);

        if target == 0 {
            self.board_at_ply = Board::default();
            self.fen_at_ply = self.board_at_ply.to_string();
            self.current_ply = 0;
            return;
        }

        // PositionReview.ply corresponds to the move that was played,
        // and fen is the FEN *after* that move.
        if let Some(pos) = self.review.positions.iter().find(|p| p.ply == target) {
            if let Ok(board) = pos.fen.parse::<Board>() {
                self.board_at_ply = board;
                self.fen_at_ply = pos.fen.clone();
                self.current_ply = target;
            }
        }
        // If position not found or FEN invalid: do not update state, so
        // current_ply and board_at_ply stay in sync and the UI remains consistent.
    }

    /// Advance to the next ply.
    pub fn next_ply(&mut self) {
        if self.current_ply < self.review.total_plies {
            self.go_to_ply(self.current_ply + 1);
        }
    }

    /// Go back to the previous ply.
    pub fn prev_ply(&mut self) {
        if self.current_ply > 0 {
            self.go_to_ply(self.current_ply - 1);
        }
    }

    /// Jump to the starting position (ply 0).
    pub fn go_to_start(&mut self) {
        self.go_to_ply(0);
    }

    /// Jump to the final position (last ply).
    pub fn go_to_end(&mut self) {
        self.go_to_ply(self.review.total_plies);
    }

    /// Get the side to move at the current review position.
    pub fn side_to_move(&self) -> &str {
        match self.board_at_ply.side_to_move() {
            cozy_chess::Color::White => "white",
            cozy_chess::Color::Black => "black",
        }
    }

    /// Build a ReviewState from review data, constructing move_history from positions.
    pub fn new(review: GameReviewProto) -> Self {
        Self::with_metadata(review, None, 0, None)
    }

    /// Build a ReviewState with original game metadata preserved for snapshot creation.
    pub fn with_metadata(
        review: GameReviewProto,
        game_mode: Option<GameModeProto>,
        skill_level: u8,
        advanced: Option<AdvancedGameAnalysisProto>,
    ) -> Self {
        let move_history = review
            .positions
            .iter()
            .map(|pos| MoveRecord {
                san: pos.played_san.clone(),
                fen_after: pos.fen.clone(),
                from: String::new(),
                to: String::new(),
                piece: String::new(),
                captured: None,
                promotion: None,
                clock_ms: pos.clock_ms,
            })
            .collect();

        Self {
            current_ply: 0,
            board_at_ply: Board::default(),
            fen_at_ply: Board::default().to_string(),
            auto_play: false,
            move_history,
            review,
            game_mode,
            skill_level,
            advanced,
        }
    }

    /// Get the PositionReview for the current ply (None at ply 0).
    pub fn current_position(&self) -> Option<&PositionReview> {
        if self.current_ply == 0 {
            return None;
        }
        self.review
            .positions
            .iter()
            .find(|p| p.ply == self.current_ply)
    }

    /// Get the AdvancedPositionAnalysis for the current ply (None if no advanced data or at ply 0).
    pub fn advanced_position(&self) -> Option<&chess_client::AdvancedPositionAnalysisProto> {
        self.advanced
            .as_ref()?
            .positions
            .iter()
            .find(|p| p.ply == self.current_ply)
    }

    /// Get (from, to) squares of the played move at the current ply by diffing boards.
    pub fn played_move_squares(&self) -> Option<(Square, Square)> {
        if self.current_ply == 0 {
            return None;
        }
        // Get board before and after the current move
        let prev_board = if self.current_ply == 1 {
            Board::default()
        } else {
            let prev_pos = self
                .review
                .positions
                .iter()
                .find(|p| p.ply == self.current_ply - 1)?;
            prev_pos.fen.parse::<Board>().ok()?
        };
        let curr_board = &self.board_at_ply;

        // Determine the side that moved (side to move on prev_board)
        let moving_side = prev_board.side_to_move();

        // Find the square that lost a piece of the moving side (from)
        // and the square that gained a piece of the moving side (to)
        let mut from_sq = None;
        let mut to_sq = None;

        for sq_idx in 0..64u8 {
            let sq = Square::index(sq_idx as usize);
            let prev_piece = prev_board.color_on(sq);
            let curr_piece = curr_board.color_on(sq);

            match (prev_piece, curr_piece) {
                (Some(c), None) if c == moving_side => {
                    from_sq = Some(sq);
                }
                (Some(c), Some(d)) if c == moving_side && d != moving_side => {
                    // Piece was replaced by opponent (shouldn't normally happen for from)
                    from_sq = Some(sq);
                }
                (None, Some(c)) if c == moving_side => {
                    to_sq = Some(sq);
                }
                (Some(c), Some(d)) if c != moving_side && d == moving_side => {
                    // Capture: opponent piece replaced by our piece
                    to_sq = Some(sq);
                }
                _ => {}
            }
        }

        Some((from_sq?, to_sq?))
    }

    /// Parse best move UCI notation (e.g., "e2e4") into (from, to) squares.
    pub fn best_move_squares(&self) -> Option<(Square, Square)> {
        let pos = self.current_position()?;
        let uci = &pos.best_move_uci;
        if uci.len() < 4 {
            return None;
        }
        let from = parse_uci_square(&uci[0..2])?;
        let to = parse_uci_square(&uci[2..4])?;
        Some((from, to))
    }

    /// Get plies of critical moments (blunders and mistakes) sorted by ply.
    pub fn critical_moments(&self) -> Vec<u32> {
        self.review
            .positions
            .iter()
            .filter(|p| {
                let class = MoveClassification::try_from(p.classification);
                matches!(
                    class,
                    Ok(MoveClassification::ClassificationBlunder)
                        | Ok(MoveClassification::ClassificationMistake)
                )
            })
            .map(|p| p.ply)
            .collect()
    }
}

/// Parse a UCI square string like "e2" into a cozy_chess Square.
fn parse_uci_square(s: &str) -> Option<Square> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let file = match bytes[0] {
        b'a'..=b'h' => cozy_chess::File::index((bytes[0] - b'a') as usize),
        _ => return None,
    };
    let rank = match bytes[1] {
        b'1'..=b'8' => cozy_chess::Rank::index((bytes[1] - b'1') as usize),
        _ => return None,
    };
    Some(Square::new(file, rank))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `GameReviewProto` with positions for the first few moves
    /// of a game (1. e4 e5 2. Nf3).
    fn sample_review() -> GameReviewProto {
        GameReviewProto {
            game_id: "test".into(),
            total_plies: 3,
            analyzed_plies: 3,
            analysis_depth: 20,
            positions: vec![
                PositionReview {
                    ply: 1,
                    // After 1. e4 — black to move
                    fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".into(),
                    ..Default::default()
                },
                PositionReview {
                    ply: 2,
                    // After 1... e5 — white to move
                    fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".into(),
                    ..Default::default()
                },
                PositionReview {
                    ply: 3,
                    // After 2. Nf3 — black to move
                    fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn new_review_state(review: GameReviewProto) -> ReviewState {
        ReviewState::new(review)
    }

    #[test]
    fn review_side_to_move_at_starting_position() {
        let rs = new_review_state(sample_review());
        assert_eq!(rs.side_to_move(), "white");
    }

    #[test]
    fn review_side_to_move_alternates_with_navigation() {
        let mut rs = new_review_state(sample_review());

        // After 1. e4 → black to move
        rs.next_ply();
        assert_eq!(rs.current_ply, 1);
        assert_eq!(rs.side_to_move(), "black");

        // After 1... e5 → white to move
        rs.next_ply();
        assert_eq!(rs.current_ply, 2);
        assert_eq!(rs.side_to_move(), "white");

        // After 2. Nf3 → black to move
        rs.next_ply();
        assert_eq!(rs.current_ply, 3);
        assert_eq!(rs.side_to_move(), "black");
    }

    #[test]
    fn review_side_to_move_correct_after_backward_navigation() {
        let mut rs = new_review_state(sample_review());

        // Go to end then navigate back
        rs.go_to_end();
        assert_eq!(rs.side_to_move(), "black");

        rs.prev_ply();
        assert_eq!(rs.current_ply, 2);
        assert_eq!(rs.side_to_move(), "white");

        rs.prev_ply();
        assert_eq!(rs.current_ply, 1);
        assert_eq!(rs.side_to_move(), "black");

        // Back to start
        rs.go_to_start();
        assert_eq!(rs.current_ply, 0);
        assert_eq!(rs.side_to_move(), "white");
    }

    #[test]
    fn review_side_to_move_correct_after_jump_to_ply() {
        let mut rs = new_review_state(sample_review());

        // Jump directly to ply 2 (white to move)
        rs.go_to_ply(2);
        assert_eq!(rs.side_to_move(), "white");

        // Jump back to ply 1 (black to move)
        rs.go_to_ply(1);
        assert_eq!(rs.side_to_move(), "black");
    }

    #[test]
    fn review_fen_at_ply_updates_with_navigation() {
        let mut rs = new_review_state(sample_review());

        // Ply 0: starting position FEN
        let start_fen = Board::default().to_string();
        assert_eq!(rs.fen_at_ply, start_fen);

        // After 1. e4
        rs.next_ply();
        assert_eq!(
            rs.fen_at_ply,
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        );

        // Back to start
        rs.go_to_start();
        assert_eq!(rs.fen_at_ply, start_fen);
    }

    #[test]
    fn review_fen_unchanged_for_missing_ply() {
        let mut rs = new_review_state(GameReviewProto {
            game_id: "test".into(),
            total_plies: 5,
            analyzed_plies: 3,
            positions: vec![
                PositionReview {
                    ply: 1,
                    fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".into(),
                    ..Default::default()
                },
                // ply 2 is missing
                PositionReview {
                    ply: 3,
                    fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });

        // Navigate to ply 1 (exists)
        rs.go_to_ply(1);
        assert_eq!(rs.current_ply, 1);
        let fen_at_1 = rs.fen_at_ply.clone();

        // Navigate to ply 2 (missing) — state should not change
        rs.go_to_ply(2);
        assert_eq!(rs.current_ply, 1); // unchanged
        assert_eq!(rs.fen_at_ply, fen_at_1); // unchanged
    }

    #[test]
    fn test_with_metadata_preserves_game_mode() {
        use chess_client::{GameModeProto, GameModeType, PlayerSideProto};

        let review = sample_review();
        let game_mode = Some(GameModeProto {
            mode: GameModeType::HumanVsEngine.into(),
            human_side: Some(PlayerSideProto::White.into()),
        });
        let rs = ReviewState::with_metadata(review, game_mode.clone(), 12, None);

        assert_eq!(rs.skill_level, 12);
        assert!(rs.game_mode.is_some());
        let gm = rs.game_mode.unwrap();
        assert_eq!(gm.mode, i32::from(GameModeType::HumanVsEngine));
        assert_eq!(gm.human_side, Some(i32::from(PlayerSideProto::White)));
    }

    #[test]
    fn test_fen_at_ply_for_snapshot() {
        let mut rs = new_review_state(sample_review());

        // At ply 0: starting position
        assert_eq!(rs.fen_at_ply, Board::default().to_string());

        // Navigate to ply 1 (after 1. e4)
        rs.go_to_ply(1);
        assert_eq!(
            rs.fen_at_ply,
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        );

        // Navigate to ply 2 (after 1... e5)
        rs.go_to_ply(2);
        assert_eq!(
            rs.fen_at_ply,
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"
        );

        // Navigate to ply 3 (after 2. Nf3)
        rs.go_to_ply(3);
        assert_eq!(
            rs.fen_at_ply,
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );

        // Back to start
        rs.go_to_ply(0);
        assert_eq!(rs.fen_at_ply, Board::default().to_string());
    }
}

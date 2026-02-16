use chess::{
    format_color, format_piece_upper, format_square, EngineAnalysis, Game, GameMode, GamePhase,
    GameResult, HistoryEntry, PlayerSide,
};
use cozy_chess::{Color, Move};
use engine::{EngineCommand, EngineEvent, GoParams, StockfishEngine};
use std::time::Instant;

use super::commands::{EngineConfig, SessionError};
use super::snapshot::{MoveRecord, SessionSnapshot, TimerSnapshot};

/// Internal mutable state, owned entirely by the session actor. No locks.
pub(crate) struct SessionState {
    pub session_id: String,
    pub game: Game,
    pub phase: GamePhase,
    pub game_mode: GameMode,
    pub engine: Option<StockfishEngine>,
    pub engine_config: Option<EngineConfig>,
    pub analysis: Option<EngineAnalysis>,
    pub engine_thinking: bool,
    pub timer: Option<TimerState>,
}

/// Server-owned timer state.
pub(crate) struct TimerState {
    pub white_remaining_ms: u64,
    pub black_remaining_ms: u64,
    pub active_side: Option<PlayerSide>,
    pub last_tick: Instant,
}

impl TimerState {
    pub fn new(white_ms: u64, black_ms: u64) -> Self {
        Self {
            white_remaining_ms: white_ms,
            black_remaining_ms: black_ms,
            active_side: None,
            last_tick: Instant::now(),
        }
    }

    /// Tick the timer, decrementing the active side's remaining time.
    /// Returns true if a flag has fallen (time expired).
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_tick).as_millis() as u64;
        self.last_tick = now;

        match self.active_side {
            Some(PlayerSide::White) => {
                self.white_remaining_ms = self.white_remaining_ms.saturating_sub(elapsed_ms);
                self.white_remaining_ms == 0
            }
            Some(PlayerSide::Black) => {
                self.black_remaining_ms = self.black_remaining_ms.saturating_sub(elapsed_ms);
                self.black_remaining_ms == 0
            }
            None => false,
        }
    }

    pub fn start(&mut self, side: PlayerSide) {
        self.last_tick = Instant::now();
        self.active_side = Some(side);
    }

    pub fn stop(&mut self) {
        // Flush any remaining elapsed time before stopping
        self.tick();
        self.active_side = None;
    }

    pub fn switch_to(&mut self, side: PlayerSide) {
        // Flush elapsed time for current side, then switch
        self.tick();
        self.active_side = Some(side);
        self.last_tick = Instant::now();
    }

    pub fn to_snapshot(&self) -> TimerSnapshot {
        TimerSnapshot {
            white_remaining_ms: self.white_remaining_ms,
            black_remaining_ms: self.black_remaining_ms,
            active_side: self.active_side.map(|s| match s {
                PlayerSide::White => "white".to_string(),
                PlayerSide::Black => "black".to_string(),
            }),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_side.is_some()
    }
}

impl SessionState {
    pub fn new(session_id: String, game: Game, game_mode: GameMode) -> Self {
        let phase = GamePhase::from_game(&game);
        Self {
            session_id,
            game,
            phase,
            game_mode,
            engine: None,
            engine_config: None,
            analysis: None,
            engine_thinking: false,
            timer: None,
        }
    }

    /// Build a full snapshot of the current state.
    pub fn snapshot(&self) -> SessionSnapshot {
        let history: Vec<MoveRecord> = self
            .game
            .history()
            .iter()
            .map(history_entry_to_record)
            .collect();

        let last_move = self
            .game
            .history()
            .last()
            .map(|e| (format_square(e.from), format_square(e.to)));

        SessionSnapshot {
            session_id: self.session_id.clone(),
            fen: self.game.to_fen(),
            side_to_move: format_color(self.game.side_to_move()),
            phase: self.phase.clone(),
            game_mode: self.game_mode.clone(),
            status: self.game.status(),
            move_count: self.game.history().len(),
            history,
            last_move,
            engine_config: self.engine_config.clone(),
            analysis: self.analysis.clone(),
            engine_thinking: self.engine_thinking,
            timer: self.timer.as_ref().map(|t| t.to_snapshot()),
        }
    }

    /// Try to receive the next engine event.
    pub async fn next_engine_event(&mut self) -> Option<EngineEvent> {
        match self.engine.as_mut() {
            Some(engine) => engine.recv_event().await,
            None => std::future::pending().await,
        }
    }

    pub fn timer_active(&self) -> bool {
        self.timer.as_ref().map_or(false, |t| t.is_active())
    }

    pub fn apply_move(&mut self, mv: Move) -> Result<SessionSnapshot, SessionError> {
        self.game
            .make_move(mv)
            .map_err(|e| SessionError::IllegalMove(e.to_string()))?;
        self.phase = GamePhase::from_game(&self.game);
        self.analysis = None;

        // Switch timer to the new side
        if let Some(ref mut timer) = self.timer {
            if matches!(self.phase, GamePhase::Playing { .. }) {
                timer.switch_to(PlayerSide::from(self.game.side_to_move()));
            } else {
                timer.stop();
            }
        }

        Ok(self.snapshot())
    }

    pub fn apply_undo(&mut self) -> Result<SessionSnapshot, SessionError> {
        self.game.undo().map_err(|_| SessionError::NothingToUndo)?;
        self.phase = GamePhase::from_game(&self.game);
        self.analysis = None;
        self.engine_thinking = false;
        Ok(self.snapshot())
    }

    pub fn apply_redo(&mut self) -> Result<SessionSnapshot, SessionError> {
        self.game.redo().map_err(|_| SessionError::NothingToRedo)?;
        self.phase = GamePhase::from_game(&self.game);
        self.analysis = None;
        Ok(self.snapshot())
    }

    pub fn apply_reset(&mut self, fen: Option<String>) -> Result<SessionSnapshot, SessionError> {
        let new_game = match fen {
            Some(ref f) => {
                Game::from_fen(f).map_err(|e| SessionError::InvalidFen(e.to_string()))?
            }
            None => Game::new(),
        };
        self.game = new_game;
        self.phase = GamePhase::from_game(&self.game);
        self.analysis = None;
        self.engine_thinking = false;
        Ok(self.snapshot())
    }

    /// Check if the engine should auto-trigger for the current position.
    /// Called after every state mutation.
    pub fn should_auto_trigger_engine(&self) -> bool {
        if self.engine_thinking {
            return false;
        }
        if !matches!(self.phase, GamePhase::Playing { .. }) {
            return false;
        }
        if self.engine.is_none() {
            return false;
        }
        if !matches!(self.game.status(), cozy_chess::GameStatus::Ongoing) {
            return false;
        }

        match &self.game_mode {
            GameMode::EngineVsEngine => true,
            GameMode::HumanVsEngine { human_side } => {
                let current = PlayerSide::from(self.game.side_to_move());
                current != *human_side
            }
            _ => false,
        }
    }

    /// Trigger engine move calculation. Called internally by the actor.
    pub async fn trigger_engine(&mut self) -> Result<(), SessionError> {
        let engine = self
            .engine
            .as_ref()
            .ok_or(SessionError::EngineNotConfigured)?;

        let fen = self.game.to_fen();
        let skill = self
            .engine_config
            .as_ref()
            .map(|c| c.skill_level)
            .unwrap_or(10);

        engine
            .send_command(EngineCommand::SetPosition { fen, moves: vec![] })
            .await
            .map_err(|e| SessionError::Internal(e.to_string()))?;

        let go_params = match skill {
            0..=3 => GoParams {
                depth: Some(4),
                ..Default::default()
            },
            4..=7 => GoParams {
                depth: Some(8),
                ..Default::default()
            },
            8..=12 => GoParams {
                movetime: Some(500),
                ..Default::default()
            },
            13..=17 => GoParams {
                movetime: Some(1000),
                ..Default::default()
            },
            _ => GoParams {
                movetime: Some(2000),
                ..Default::default()
            },
        };

        engine
            .send_command(EngineCommand::Go(go_params))
            .await
            .map_err(|e| SessionError::Internal(e.to_string()))?;

        self.engine_thinking = true;
        Ok(())
    }

    /// Tick timer and return true if a flag fell (time expired).
    pub fn tick_timer(&mut self) -> bool {
        if let Some(ref mut timer) = self.timer {
            if timer.tick() {
                // Time expired — end the game
                let loser = timer.active_side.unwrap();
                let result = match loser {
                    PlayerSide::White => GameResult::BlackWins,
                    PlayerSide::Black => GameResult::WhiteWins,
                };
                self.phase = GamePhase::Ended {
                    result,
                    reason: "Time expired".to_string(),
                };
                timer.stop();
                return true;
            }
        }
        false
    }
}

fn history_entry_to_record(entry: &HistoryEntry) -> MoveRecord {
    MoveRecord {
        from: format_square(entry.from),
        to: format_square(entry.to),
        piece: format_piece_upper(entry.piece).to_string(),
        piece_color: format_color(entry.piece_color),
        captured: entry.captured.map(|p| format_piece_upper(p).to_string()),
        promotion: entry.promotion.map(|p| format_piece_upper(p).to_string()),
        san: entry.san.clone(),
        fen_after: entry.fen.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::{File, Rank, Square};

    fn test_state() -> SessionState {
        SessionState::new("test".to_string(), Game::new(), GameMode::HumanVsHuman)
    }

    #[test]
    fn test_snapshot_initial() {
        let state = test_state();
        let snap = state.snapshot();
        assert_eq!(snap.move_count, 0);
        assert_eq!(snap.side_to_move, "white");
        assert!(!snap.engine_thinking);
    }

    #[test]
    fn test_apply_move() {
        let mut state = test_state();
        let mv = Move {
            from: Square::new(File::E, Rank::Second),
            to: Square::new(File::E, Rank::Fourth),
            promotion: None,
        };
        let snap = state.apply_move(mv).unwrap();
        assert_eq!(snap.move_count, 1);
        assert_eq!(snap.side_to_move, "black");
        assert_eq!(snap.last_move, Some(("e2".into(), "e4".into())));
    }

    #[test]
    fn test_auto_trigger_human_vs_human() {
        let state = test_state();
        assert!(!state.should_auto_trigger_engine());
    }

    #[test]
    fn test_timer_tick() {
        let mut state = test_state();
        state.timer = Some(TimerState::new(100, 100));
        state.timer.as_mut().unwrap().start(PlayerSide::White);

        // Simulate time passing
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!state.tick_timer());

        let snap = state.snapshot();
        assert!(snap.timer.unwrap().white_remaining_ms < 100);
    }
}

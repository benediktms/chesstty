pub mod actor;
pub mod commands;
pub mod events;
pub mod handle;
pub mod snapshot;
pub mod state;

use std::collections::HashMap;
use std::sync::Arc;

use chess::{Game, GameMode, GamePhase, PlayerSide};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::persistence::{
    self, FinishedGameData, FinishedGameStore, PositionStore, SavedPositionData, SessionStore,
    StoredMoveRecord, SuspendedSessionData,
};
use actor::run_session_actor;
pub use events::{SessionEvent, UciDirection};
pub use handle::SessionHandle;
pub use snapshot::{SessionSnapshot, TimerSnapshot};
use state::SessionState;

/// Manages all active sessions. Spawns an actor task per session.
pub struct SessionManager {
    sessions: RwLock<HashMap<String, SessionHandle>>,
    store: SessionStore,
    position_store: PositionStore,
    finished_game_store: Arc<FinishedGameStore>,
}

impl SessionManager {
    pub fn new(
        store: SessionStore,
        position_store: PositionStore,
        finished_game_store: Arc<FinishedGameStore>,
    ) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            store,
            position_store,
            finished_game_store,
        }
    }

    pub async fn create_session(
        &self,
        fen: Option<String>,
        game_mode: GameMode,
    ) -> Result<SessionSnapshot, String> {
        let session_id = Uuid::new_v4().to_string();
        let game = match fen {
            Some(ref f) => Game::from_fen(f).map_err(|e| format!("Invalid FEN: {}", e))?,
            None => Game::new(),
        };

        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, _) = broadcast::channel(100);

        let state = SessionState::new(session_id.clone(), game, game_mode);
        let initial_snapshot = state.snapshot();

        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            run_session_actor(state, cmd_rx, event_tx_clone).await;
        });

        let handle = SessionHandle::new(cmd_tx);
        self.sessions.write().await.insert(session_id, handle);

        Ok(initial_snapshot)
    }

    pub async fn get_handle(&self, session_id: &str) -> Result<SessionHandle, String> {
        self.sessions
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Session not found: {}", session_id))
    }

    /// Close a session. If the game ended, saves it to the finished game store
    /// and returns the game_id so the caller can enqueue it for review.
    pub async fn close_session(&self, session_id: &str) -> Result<Option<String>, String> {
        let handle = self
            .sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        // Save finished game data if the game reached the Ended phase.
        // GamePhase::Ended is the source of truth — the starting position is irrelevant.
        // The review worker handles any starting position (it analyzes using per-ply FENs).
        let saved_game_id = if let Ok(snapshot) = handle.get_snapshot().await {
            if let GamePhase::Ended {
                ref result,
                ref reason,
            } = snapshot.phase
            {
                self.save_finished_game(&snapshot, result, reason)
            } else {
                None
            }
        } else {
            None
        };

        handle.shutdown().await;
        Ok(saved_game_id)
    }

    /// Persist a finished game's move history for post-game review.
    /// Returns the game_id if saved successfully.
    fn save_finished_game(
        &self,
        snapshot: &SessionSnapshot,
        result: &chess::GameResult,
        reason: &str,
    ) -> Option<String> {
        let result_str = match result {
            chess::GameResult::WhiteWins => "WhiteWins",
            chess::GameResult::BlackWins => "BlackWins",
            chess::GameResult::Draw => "Draw",
        };

        let game_mode_str = match &snapshot.game_mode {
            GameMode::HumanVsHuman => "HumanVsHuman".to_string(),
            GameMode::HumanVsEngine { human_side } => format!("HumanVsEngine:{:?}", human_side),
            GameMode::EngineVsEngine => "EngineVsEngine".to_string(),
            GameMode::Analysis => "Analysis".to_string(),
            GameMode::Review => "Review".to_string(),
        };

        let human_side = match &snapshot.game_mode {
            GameMode::HumanVsEngine { human_side } => Some(match human_side {
                PlayerSide::White => "white".to_string(),
                PlayerSide::Black => "black".to_string(),
            }),
            _ => None,
        };

        let skill_level = snapshot
            .engine_config
            .as_ref()
            .map(|c| c.skill_level)
            .unwrap_or(0);

        let moves: Vec<StoredMoveRecord> = snapshot
            .history
            .iter()
            .map(|m| StoredMoveRecord {
                from: m.from.clone(),
                to: m.to.clone(),
                piece: m.piece.clone(),
                captured: m.captured.clone(),
                promotion: m.promotion.clone(),
                san: m.san.clone(),
                fen_after: m.fen_after.clone(),
                clock_ms: m.clock_ms,
            })
            .collect();

        let data = FinishedGameData {
            game_id: persistence::generate_finished_game_id(),
            start_fen: snapshot.start_fen.clone(),
            result: result_str.to_string(),
            result_reason: reason.to_string(),
            game_mode: game_mode_str,
            human_side,
            skill_level,
            move_count: snapshot.move_count as u32,
            moves,
            created_at: persistence::now_timestamp(),
        };

        match self.finished_game_store.save(&data) {
            Ok(_) => {
                tracing::info!(game_id = %data.game_id, "Saved finished game for review");
                Some(data.game_id)
            }
            Err(e) => {
                tracing::warn!("Failed to save finished game: {}", e);
                None
            }
        }
    }

    /// Suspend a session — server owns all state, client just passes session_id.
    pub async fn suspend_session(&self, session_id: &str) -> Result<String, String> {
        let handle = self.get_handle(session_id).await?;
        let snapshot = handle.get_snapshot().await.map_err(|e| e.to_string())?;

        let game_mode_str = match &snapshot.game_mode {
            GameMode::HumanVsHuman => "HumanVsHuman".to_string(),
            GameMode::HumanVsEngine { human_side } => format!("HumanVsEngine:{:?}", human_side),
            GameMode::EngineVsEngine => "EngineVsEngine".to_string(),
            GameMode::Analysis => "Analysis".to_string(),
            GameMode::Review => "Review".to_string(),
        };

        let human_side = match &snapshot.game_mode {
            GameMode::HumanVsEngine { human_side } => Some(match human_side {
                PlayerSide::White => "white".to_string(),
                PlayerSide::Black => "black".to_string(),
            }),
            _ => None,
        };

        let skill_level = snapshot
            .engine_config
            .as_ref()
            .map(|c| c.skill_level)
            .unwrap_or(0);

        let data = SuspendedSessionData {
            suspended_id: persistence::generate_suspended_id(),
            fen: snapshot.fen,
            side_to_move: snapshot.side_to_move,
            move_count: snapshot.move_count as u32,
            game_mode: game_mode_str,
            human_side,
            skill_level,
            created_at: persistence::now_timestamp(),
        };

        let suspended_id = self.store.save(&data).map_err(|e| e.to_string())?;
        self.close_session(session_id).await?;
        Ok(suspended_id)
    }

    pub async fn resume_suspended(&self, suspended_id: &str) -> Result<SessionSnapshot, String> {
        let data = self
            .store
            .load(suspended_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Suspended session not found: {}", suspended_id))?;

        let game_mode = if data.game_mode.starts_with("HumanVsEngine") {
            let human_side = if data.human_side.as_deref() == Some("black") {
                PlayerSide::Black
            } else {
                PlayerSide::White
            };
            GameMode::HumanVsEngine { human_side }
        } else {
            match data.game_mode.as_str() {
                "EngineVsEngine" => GameMode::EngineVsEngine,
                "Analysis" => GameMode::Analysis,
                "Review" => GameMode::Review,
                _ => GameMode::HumanVsHuman,
            }
        };

        let snapshot = self.create_session(Some(data.fen), game_mode).await?;
        self.store.delete(suspended_id).map_err(|e| e.to_string())?;
        Ok(snapshot)
    }

    pub fn list_suspended(&self) -> Result<Vec<SuspendedSessionData>, String> {
        self.store.list().map_err(|e| e.to_string())
    }

    pub fn delete_suspended(&self, suspended_id: &str) -> Result<(), String> {
        self.store.delete(suspended_id).map_err(|e| e.to_string())
    }

    /// Save a snapshot directly as a suspended session (from review mode, no active session).
    pub fn save_snapshot(
        &self,
        fen: &str,
        _name: &str,
        game_mode: &str,
        human_side: Option<String>,
        move_count: u32,
        skill_level: u8,
    ) -> Result<String, String> {
        // Validate the FEN
        let board: cozy_chess::Board = fen.parse().map_err(|_| format!("Invalid FEN: {}", fen))?;
        let side_to_move = match board.side_to_move() {
            cozy_chess::Color::White => "white".to_string(),
            cozy_chess::Color::Black => "black".to_string(),
        };

        let data = SuspendedSessionData {
            suspended_id: persistence::generate_suspended_id(),
            fen: fen.to_string(),
            side_to_move,
            move_count,
            game_mode: game_mode.to_string(),
            human_side,
            skill_level,
            created_at: persistence::now_timestamp(),
        };

        self.store.save(&data).map_err(|e| e.to_string())
    }

    pub fn save_position(&self, name: &str, fen: &str) -> Result<String, String> {
        let _board: cozy_chess::Board = fen.parse().map_err(|_| format!("Invalid FEN: {}", fen))?;
        let data = SavedPositionData {
            position_id: persistence::generate_position_id(),
            name: name.to_string(),
            fen: fen.to_string(),
            is_default: false,
            created_at: persistence::now_timestamp(),
        };
        self.position_store.save(&data).map_err(|e| e.to_string())
    }

    pub fn list_positions(&self) -> Result<Vec<SavedPositionData>, String> {
        self.position_store.list().map_err(|e| e.to_string())
    }

    pub fn delete_position(&self, position_id: &str) -> Result<(), String> {
        self.position_store
            .delete(position_id)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_manager() -> SessionManager {
        let (mgr, _) = test_manager_with_store();
        mgr
    }

    fn test_manager_with_store() -> (SessionManager, Arc<FinishedGameStore>) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf());
        let position_store = PositionStore::new(dir.path().to_path_buf(), None);
        let finished_game_store = Arc::new(FinishedGameStore::new(dir.path().to_path_buf()));
        // Leak the TempDir so it lives for the test duration.
        // (Tests are short-lived so this is fine.)
        std::mem::forget(dir);
        let mgr = SessionManager::new(store, position_store, finished_game_store.clone());
        (mgr, finished_game_store)
    }

    #[tokio::test]
    async fn test_create_and_close_session() {
        let mgr = test_manager();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();

        // Session should be reachable
        let handle = mgr.get_handle(&session_id).await.unwrap();
        let snap = handle.get_snapshot().await.unwrap();
        assert_eq!(snap.move_count, 0);

        // Close session
        mgr.close_session(&session_id).await.unwrap();

        // Session should no longer be reachable
        assert!(mgr.get_handle(&session_id).await.is_err());
    }

    #[tokio::test]
    async fn test_close_session_twice_returns_error() {
        let mgr = test_manager();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();

        mgr.close_session(&session_id).await.unwrap();

        // Second close should fail
        let result = mgr.close_session(&session_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_close_session_shuts_down_actor() {
        let mgr = test_manager();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();

        // Get a handle before closing
        let handle = mgr.get_handle(&session_id).await.unwrap();
        mgr.close_session(&session_id).await.unwrap();

        // Give the actor a moment to process the shutdown
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // The handle's channel should be closed — commands should fail
        let result = handle.get_snapshot().await;
        assert!(result.is_err());
    }

    /// Test the cleanup guard pattern: dropping a session handle via close_session
    /// from an Arc<SessionManager> (simulates what the CleanupGuard does).
    #[tokio::test]
    async fn test_cleanup_guard_pattern() {
        let mgr = Arc::new(test_manager());
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();

        // Simulate what CleanupGuard::drop does
        let mgr_clone = mgr.clone();
        let id_clone = session_id.clone();
        tokio::spawn(async move {
            let _ = mgr_clone.close_session(&id_clone).await;
        });

        // Wait for the spawned task
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Session should be gone
        assert!(mgr.get_handle(&session_id).await.is_err());
    }

    /// After fool's mate via SessionManager, the session should still be usable
    /// for reads but the game should be in Ended phase.
    #[tokio::test]
    async fn test_game_completion_via_session_manager() {
        let mgr = test_manager();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();
        let handle = mgr.get_handle(&session_id).await.unwrap();

        // Play fool's mate
        let moves = [
            (
                cozy_chess::File::F,
                cozy_chess::Rank::Second,
                cozy_chess::File::F,
                cozy_chess::Rank::Third,
            ),
            (
                cozy_chess::File::E,
                cozy_chess::Rank::Seventh,
                cozy_chess::File::E,
                cozy_chess::Rank::Fifth,
            ),
            (
                cozy_chess::File::G,
                cozy_chess::Rank::Second,
                cozy_chess::File::G,
                cozy_chess::Rank::Fourth,
            ),
            (
                cozy_chess::File::D,
                cozy_chess::Rank::Eighth,
                cozy_chess::File::H,
                cozy_chess::Rank::Fourth,
            ),
        ];
        for (ff, fr, tf, tr) in moves {
            handle
                .make_move(cozy_chess::Move {
                    from: cozy_chess::Square::new(ff, fr),
                    to: cozy_chess::Square::new(tf, tr),
                    promotion: None,
                })
                .await
                .unwrap();
        }

        let snap = handle.get_snapshot().await.unwrap();
        assert!(matches!(snap.phase, chess::GamePhase::Ended { .. }));
        assert!(!snap.engine_thinking);

        // Session should still be in the manager (game ended != session closed)
        assert!(mgr.get_handle(&session_id).await.is_ok());

        // Explicitly close the session
        mgr.close_session(&session_id).await.unwrap();
        assert!(mgr.get_handle(&session_id).await.is_err());
    }

    /// Helper: plays fool's mate on a session handle.
    async fn play_fools_mate(handle: &SessionHandle) {
        let moves = [
            (
                cozy_chess::File::F,
                cozy_chess::Rank::Second,
                cozy_chess::File::F,
                cozy_chess::Rank::Third,
            ),
            (
                cozy_chess::File::E,
                cozy_chess::Rank::Seventh,
                cozy_chess::File::E,
                cozy_chess::Rank::Fifth,
            ),
            (
                cozy_chess::File::G,
                cozy_chess::Rank::Second,
                cozy_chess::File::G,
                cozy_chess::Rank::Fourth,
            ),
            (
                cozy_chess::File::D,
                cozy_chess::Rank::Eighth,
                cozy_chess::File::H,
                cozy_chess::Rank::Fourth,
            ),
        ];
        for (ff, fr, tf, tr) in moves {
            handle
                .make_move(cozy_chess::Move {
                    from: cozy_chess::Square::new(ff, fr),
                    to: cozy_chess::Square::new(tf, tr),
                    promotion: None,
                })
                .await
                .unwrap();
        }
    }

    /// Closing a finished game should persist it to the FinishedGameStore
    /// with correct result, move history, and start FEN.
    #[tokio::test]
    async fn test_close_finished_game_saves_to_store() {
        let (mgr, finished_store) = test_manager_with_store();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();
        let handle = mgr.get_handle(&session_id).await.unwrap();

        // No finished games before
        assert!(finished_store.list().unwrap().is_empty());

        play_fools_mate(&handle).await;
        mgr.close_session(&session_id).await.unwrap();

        // Should have exactly one finished game
        let games = finished_store.list().unwrap();
        assert_eq!(games.len(), 1);

        let game = &games[0];
        assert_eq!(game.result, "BlackWins");
        assert_eq!(game.result_reason, "Checkmate");
        assert_eq!(game.move_count, 4);
        assert_eq!(game.moves.len(), 4);
        assert_eq!(game.game_mode, "HumanVsHuman");
        assert_eq!(
            game.start_fen,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    /// Closing a non-finished game should NOT save to the finished game store.
    #[tokio::test]
    async fn test_close_ongoing_game_does_not_save() {
        let (mgr, finished_store) = test_manager_with_store();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();

        // Close without playing any moves
        mgr.close_session(&session_id).await.unwrap();

        assert!(finished_store.list().unwrap().is_empty());
    }

    /// Finished game preserves individual move SAN and FEN snapshots.
    #[tokio::test]
    async fn test_finished_game_move_history_integrity() {
        let (mgr, finished_store) = test_manager_with_store();
        let snap = mgr
            .create_session(None, GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();
        let handle = mgr.get_handle(&session_id).await.unwrap();

        play_fools_mate(&handle).await;
        mgr.close_session(&session_id).await.unwrap();

        let game = &finished_store.list().unwrap()[0];

        // First move: f3 (White pawn)
        assert_eq!(game.moves[0].san, "f3");
        assert_eq!(game.moves[0].piece, "P");
        assert!(game.moves[0].captured.is_none());

        // Second move: e5 (Black pawn)
        assert_eq!(game.moves[1].san, "e5");

        // Fourth move: Qh4# (Black queen checkmate)
        // SAN may or may not include the '#' suffix depending on the notation implementation
        assert!(
            game.moves[3].san.starts_with("Qh4"),
            "Expected Qh4 or Qh4#, got: {}",
            game.moves[3].san
        );
        assert_eq!(game.moves[3].piece, "Q");

        // Each move should have a valid FEN
        for m in &game.moves {
            assert!(!m.fen_after.is_empty());
            assert!(m.fen_after.parse::<cozy_chess::Board>().is_ok());
        }
    }

    /// Games started from a custom FEN (e.g. snapshots) that reach checkmate
    /// ARE saved to the finished game store for review analysis.
    #[tokio::test]
    async fn test_custom_start_fen_game_saved_for_review() {
        let (mgr, finished_store) = test_manager_with_store();
        // A position where black can mate in one with Qh4#
        let custom_fen = "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2";
        let snap = mgr
            .create_session(Some(custom_fen.to_string()), GameMode::HumanVsHuman)
            .await
            .unwrap();
        let session_id = snap.session_id.clone();
        let handle = mgr.get_handle(&session_id).await.unwrap();

        // Qh4# — black mates immediately
        handle
            .make_move(cozy_chess::Move {
                from: cozy_chess::Square::new(cozy_chess::File::D, cozy_chess::Rank::Eighth),
                to: cozy_chess::Square::new(cozy_chess::File::H, cozy_chess::Rank::Fourth),
                promotion: None,
            })
            .await
            .unwrap();

        mgr.close_session(&session_id).await.unwrap();

        // Custom FEN game that ended should be saved for review
        let games = finished_store.list().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].result, "BlackWins");
        assert_eq!(games[0].start_fen, custom_fen);
        assert_eq!(games[0].move_count, 1);
    }

    #[test]
    fn test_save_snapshot_creates_suspended_session() {
        let mgr = test_manager();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        let result = mgr.save_snapshot(
            fen,
            "Test Snapshot",
            "HumanVsEngine:White",
            Some("white".to_string()),
            10,
            5,
        );
        assert!(result.is_ok());

        let suspended = mgr.list_suspended().unwrap();
        assert_eq!(suspended.len(), 1);
        assert_eq!(suspended[0].fen, fen);
        assert_eq!(suspended[0].game_mode, "HumanVsEngine:White");
        assert_eq!(suspended[0].move_count, 10);
        assert_eq!(suspended[0].skill_level, 5);
        assert_eq!(suspended[0].side_to_move, "white");
    }

    #[test]
    fn test_save_snapshot_invalid_fen_fails() {
        let mgr = test_manager();
        let result = mgr.save_snapshot("not a valid fen", "Bad", "HumanVsHuman", None, 0, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid FEN"));
    }

    #[tokio::test]
    async fn test_save_and_resume_snapshot_preserves_position() {
        let mgr = test_manager();
        // A normal (non-terminal) position after 1. e4 e5
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        let suspended_id = mgr
            .save_snapshot(fen, "Test", "HumanVsHuman", None, 2, 0)
            .unwrap();

        let snap = mgr.resume_suspended(&suspended_id).await.unwrap();
        assert_eq!(snap.fen, fen);
        assert!(matches!(snap.phase, GamePhase::Playing { .. }));

        // Suspended session should be deleted after resume
        let suspended = mgr.list_suspended().unwrap();
        assert!(suspended.is_empty());
    }

    #[tokio::test]
    async fn test_resume_snapshot_with_terminal_fen_results_in_ended() {
        let mgr = test_manager();
        // Fool's mate final position — checkmate, White to move but no legal moves
        let checkmate_fen = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
        let suspended_id = mgr
            .save_snapshot(checkmate_fen, "Checkmate", "HumanVsHuman", None, 4, 0)
            .unwrap();

        // Resuming a terminal FEN creates a session that immediately detects checkmate.
        // This documents why client-side validation matters.
        let snap = mgr.resume_suspended(&suspended_id).await.unwrap();
        assert!(
            matches!(snap.phase, GamePhase::Ended { .. }),
            "Expected Ended phase for checkmate FEN, got {:?}",
            snap.phase
        );
    }
}

pub mod actor;
pub mod commands;
pub mod events;
pub mod handle;
pub mod snapshot;
pub mod state;

use std::collections::HashMap;

use chess::{Game, GameMode, PlayerSide};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::persistence::{
    self, PositionStore, SavedPositionData, SessionStore, SuspendedSessionData,
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
}

impl SessionManager {
    pub fn new(store: SessionStore, position_store: PositionStore) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            store,
            position_store,
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

    pub async fn close_session(&self, session_id: &str) -> Result<(), String> {
        let handle = self
            .sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        handle.shutdown().await;
        Ok(())
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
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf());
        let position_store = PositionStore::new(dir.path().to_path_buf(), None);
        // Leak the TempDir so it lives for the test duration.
        // (Tests are short-lived so this is fine.)
        std::mem::forget(dir);
        SessionManager::new(store, position_store)
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
            (cozy_chess::File::F, cozy_chess::Rank::Second, cozy_chess::File::F, cozy_chess::Rank::Third),
            (cozy_chess::File::E, cozy_chess::Rank::Seventh, cozy_chess::File::E, cozy_chess::Rank::Fifth),
            (cozy_chess::File::G, cozy_chess::Rank::Second, cozy_chess::File::G, cozy_chess::Rank::Fourth),
            (cozy_chess::File::D, cozy_chess::Rank::Eighth, cozy_chess::File::H, cozy_chess::Rank::Fourth),
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
}

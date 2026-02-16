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

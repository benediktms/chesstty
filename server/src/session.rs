use chess::{Game, GameError, HistoryEntry};
use chess_common::uci::convert_uci_castling_to_cozy;
use cozy_chess::{Color, GameStatus as CozyGameStatus, Move, Piece, Square};
use engine::{EngineCommand, EngineEvent, EngineHandle, GoParams, StockfishEngine};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Manages multiple chess game sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<GameSession>>>>>,
}

/// A single game session with associated game state and engine
pub struct GameSession {
    id: String,
    game: Arc<RwLock<Game>>,  // Shared for event handler
    engine_cmd_tx: Option<mpsc::Sender<EngineCommand>>,  // Channel to send commands to engine task
    engine_task: Option<JoinHandle<()>>,  // Per-session event handler task
    skill_level: u8,
    engine_enabled: bool,
    event_tx: broadcast::Sender<SessionEvent>,
}

/// Events that can occur during a game session
#[derive(Clone, Debug)]
pub enum SessionEvent {
    MoveMade {
        from: Square,
        to: Square,
        san: String,
        fen: String,
        status: CozyGameStatus,
    },
    EngineMoveReady {
        best_move: Move,
        evaluation: Option<String>,
    },
    EngineThinking {
        info: engine::EngineInfo,
    },
    GameEnded {
        result: String,
        reason: String,
    },
    Error {
        message: String,
    },
    UciMessage {
        direction: UciMessageDirection,
        message: String,
        context: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub enum UciMessageDirection {
    ToEngine,
    FromEngine,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
        // No more global polling loop! Each session has its own event handler.
    }

    /// Create a new game session
    pub async fn create_session(&self, fen: Option<String>) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();

        let game = if let Some(fen_str) = fen {
            Game::from_fen(&fen_str).map_err(|e| format!("Invalid FEN: {}", e))?
        } else {
            Game::new()
        };

        let (event_tx, _) = broadcast::channel(100);
        let session = GameSession {
            id: session_id.clone(),
            game: Arc::new(RwLock::new(game)),
            engine_cmd_tx: None,
            engine_task: None,
            skill_level: 10,
            engine_enabled: false,
            event_tx,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), Arc::new(RwLock::new(session)));

        tracing::info!("Created session: {}", session_id);
        Ok(session_id)
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Arc<RwLock<GameSession>>, String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Session not found: {}", session_id))
    }

    /// Close a session and clean up resources
    pub async fn close_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        if let Some(session_arc) = sessions.remove(session_id) {
            let mut session = session_arc.write().await;

            // Abort the engine task if running (this will clean up the engine)
            if let Some(task) = session.engine_task.take() {
                task.abort();
            }

            tracing::info!("Closed session: {}", session_id);
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Make a move in a session
    pub async fn make_move(
        &self,
        session_id: &str,
        mv: Move,
    ) -> Result<(HistoryEntry, CozyGameStatus), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        // Note: make_move will validate legality internally

        session.make_move(mv).await
    }

    /// Get legal moves for a session
    pub async fn get_legal_moves(
        &self,
        session_id: &str,
        from_square: Option<Square>,
    ) -> Result<Vec<Move>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.read().await;

        let legal_moves = {
            let game = session.game.read().await;
            game.legal_moves()
        };

        if let Some(from) = from_square {
            Ok(legal_moves
                .into_iter()
                .filter(|mv| mv.from == from)
                .collect())
        } else {
            Ok(legal_moves)
        }
    }

    /// Undo the last move
    pub async fn undo_move(&self, session_id: &str) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        session.undo_move().await
    }

    /// Redo a previously undone move
    pub async fn redo_move(&self, session_id: &str) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        session.redo_move().await
    }

    /// Reset the game to a new position
    pub async fn reset_game(&self, session_id: &str, fen: Option<String>) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        session.reset_game(fen).await
    }

    /// Configure the engine for a session
    pub async fn set_engine(
        &self,
        session_id: &str,
        enabled: bool,
        skill_level: u8,
    ) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        session.set_engine(enabled, skill_level).await
    }

    /// Trigger the engine to make a move
    pub async fn trigger_engine_move(
        &self,
        session_id: &str,
        movetime_ms: Option<u64>,
    ) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let mut session = session_arc.write().await;

        session.trigger_engine_move(movetime_ms).await
    }

    /// Stop the engine calculation
    pub async fn stop_engine(&self, session_id: &str) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.read().await;

        session.stop_engine().await
    }

    /// Subscribe to session events
    pub async fn subscribe_events(
        &self,
        session_id: &str,
    ) -> Result<broadcast::Receiver<SessionEvent>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.read().await;

        Ok(session.event_tx.subscribe())
    }

    /// Get session info (for status queries)
    pub async fn get_session_info(&self, session_id: &str) -> Result<SessionInfo, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.read().await;
        let game = session.game.read().await;

        Ok(SessionInfo {
            id: session.id.clone(),
            fen: game.to_fen(),
            side_to_move: game.side_to_move(),
            status: game.status(),
            move_count: game.history().len(),
            history: game.history().to_vec(),
            engine_enabled: session.engine_enabled,
            skill_level: session.skill_level,
        })
    }
}

/// Spawns a background task to handle engine commands and events
/// This replaces the old polling loop with an event-driven architecture
fn spawn_engine_event_handler(
    session_id: String,
    mut engine: StockfishEngine,
    mut cmd_rx: mpsc::Receiver<EngineCommand>,
    event_tx: broadcast::Sender<SessionEvent>,
    game: Arc<RwLock<Game>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Engine event handler started for session {}", session_id);

        loop {
            tokio::select! {
                // Handle commands from the GameSession
                Some(cmd) = cmd_rx.recv() => {
                    tracing::debug!("Sending command to engine: {:?}", cmd);
                    if let Err(e) = engine.send_command(cmd).await {
                        tracing::error!("Failed to send command to engine: {}", e);
                        let _ = event_tx.send(SessionEvent::Error {
                            message: format!("Engine command failed: {}", e),
                        });
                    }
                }

                // Handle events from the engine (blocking wait - instant response!)
                Some(event) = engine.recv_event() => {
                    match event {
                        EngineEvent::BestMove(mv) => {
                            tracing::info!("Engine found best move: {:?}", mv);

                            // Get legal moves and convert UCI castling notation
                            let legal_moves = {
                                let game = game.read().await;
                                game.legal_moves()
                            };

                            let converted_mv = convert_uci_castling_to_cozy(mv, &legal_moves);

                            if !legal_moves.contains(&converted_mv) {
                                let fen = {
                                    let game = game.read().await;
                                    game.to_fen()
                                };

                                tracing::error!(
                                    "Engine suggested illegal move {:?} (converted: {:?}). Legal moves: {:?}. Current FEN: {}",
                                    mv, converted_mv, legal_moves, fen
                                );

                                let _ = event_tx.send(SessionEvent::Error {
                                    message: format!("Engine suggested illegal move: {:?}", mv),
                                });
                                continue;
                            }

                            // Execute the move
                            let result = {
                                let mut game = game.write().await;
                                game.make_move(converted_mv)
                            };

                            match result {
                                Ok(entry) => {
                                    let status = {
                                        let game = game.read().await;
                                        game.status()
                                    };

                                    tracing::info!("Engine move executed: {}", entry.san);

                                    // Broadcast move made event
                                    let _ = event_tx.send(SessionEvent::MoveMade {
                                        from: entry.from,
                                        to: entry.to,
                                        san: entry.san.clone(),
                                        fen: entry.fen.clone(),
                                        status,
                                    });

                                    // Check if game ended
                                    if !matches!(status, CozyGameStatus::Ongoing) {
                                        let winner = {
                                            let game = game.read().await;
                                            if game.side_to_move() == Color::White {
                                                "0-1"
                                            } else {
                                                "1-0"
                                            }
                                        };

                                        let (result, reason) = match status {
                                            CozyGameStatus::Won => {
                                                (winner.to_string(), "Checkmate".to_string())
                                            }
                                            CozyGameStatus::Drawn => {
                                                ("1/2-1/2".to_string(), "Draw".to_string())
                                            }
                                            CozyGameStatus::Ongoing => unreachable!(),
                                        };

                                        let _ = event_tx.send(SessionEvent::GameEnded { result, reason });
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to execute engine move: {}", e);
                                    let _ = event_tx.send(SessionEvent::Error {
                                        message: format!("Engine move failed: {}", e),
                                    });
                                }
                            }
                        }
                        EngineEvent::Info(info) => {
                            let _ = event_tx.send(SessionEvent::EngineThinking { info: info.clone() });
                        }
                        EngineEvent::RawUciMessage { direction, message } => {
                            let _ = event_tx.send(SessionEvent::UciMessage {
                                direction: match direction {
                                    engine::UciMessageDirection::ToEngine => {
                                        UciMessageDirection::ToEngine
                                    }
                                    engine::UciMessageDirection::FromEngine => {
                                        UciMessageDirection::FromEngine
                                    }
                                },
                                message,
                                context: None,
                            });
                        }
                        EngineEvent::Ready => {
                            tracing::debug!("Engine ready");
                        }
                        EngineEvent::Error(err) => {
                            tracing::error!("Engine error: {}", err);
                            let _ = event_tx.send(SessionEvent::Error {
                                message: format!("Engine error: {}", err),
                            });
                        }
                    }
                }

                // Channel closed - clean up
                else => {
                    tracing::info!("Engine event handler stopping for session {}", session_id);
                    let _ = engine.shutdown().await;
                    break;
                }
            }
        }
    })
}

impl GameSession {
    /// Make a move in this session
    async fn make_move(&mut self, mv: Move) -> Result<(HistoryEntry, CozyGameStatus), String> {
        // Log the move attempt for debugging
        tracing::info!("Attempting move: {:?}", mv);

        let current_fen = {
            let game = self.game.read().await;
            game.to_fen()
        };
        tracing::debug!("Current FEN: {}", current_fen);

        // Validate and execute the move
        let (entry, status) = {
            let mut game = self.game.write().await;
            let entry = game.make_move(mv).map_err(|e| {
                let legal_moves = game.legal_moves();
                tracing::error!("Move {:?} rejected: {}. Legal move count: {}", mv, e, legal_moves.len());
                format!("Illegal move: {}", e)
            })?;
            let status = game.status();
            (entry, status)
        };

        // Broadcast the move event
        let _ = self.event_tx.send(SessionEvent::MoveMade {
            from: entry.from,
            to: entry.to,
            san: entry.san.clone(),
            fen: entry.fen.clone(),
            status,
        });

        // Check if game ended
        if !matches!(status, CozyGameStatus::Ongoing) {
            let winner = {
                let game = self.game.read().await;
                if game.side_to_move() == Color::White {
                    "0-1" // Black won (white to move but lost)
                } else {
                    "1-0" // White won (black to move but lost)
                }
            };

            let (result, reason) = match status {
                CozyGameStatus::Won => {
                    (winner.to_string(), "Checkmate".to_string())
                }
                CozyGameStatus::Drawn => ("1/2-1/2".to_string(), "Draw".to_string()),
                CozyGameStatus::Ongoing => unreachable!(),
            };

            let _ = self
                .event_tx
                .send(SessionEvent::GameEnded { result, reason });
        }

        Ok((entry, status))
    }

    /// Undo the last move
    async fn undo_move(&mut self) -> Result<(), String> {
        let mut game = self.game.write().await;
        game.undo().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Redo a previously undone move
    async fn redo_move(&mut self) -> Result<(), String> {
        let mut game = self.game.write().await;
        game.redo()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Reset the game
    async fn reset_game(&mut self, fen: Option<String>) -> Result<(), String> {
        let new_game = if let Some(fen_str) = fen {
            Game::from_fen(&fen_str).map_err(|e| format!("Invalid FEN: {}", e))?
        } else {
            Game::new()
        };

        let mut game = self.game.write().await;
        *game = new_game;
        Ok(())
    }

    /// Configure the engine
    async fn set_engine(&mut self, enabled: bool, skill_level: u8) -> Result<(), String> {
        if skill_level > 20 {
            return Err("Skill level must be between 0 and 20".to_string());
        }

        self.skill_level = skill_level;
        self.engine_enabled = enabled;

        if enabled && self.engine_task.is_none() {
            // Initialize engine
            tracing::info!("Initializing Stockfish engine for session {}", self.id);
            let mut engine = StockfishEngine::spawn(Some(skill_level))
                .await
                .map_err(|e| format!("Failed to initialize engine: {}", e))?;

            // Set skill level
            engine
                .send_command(EngineCommand::SetOption {
                    name: "Skill Level".to_string(),
                    value: Some(skill_level.to_string()),
                })
                .await
                .map_err(|e| format!("Failed to set skill level: {}", e))?;

            // Create command channel
            let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>(32);

            // Spawn event handler task
            let task = spawn_engine_event_handler(
                self.id.clone(),
                engine,
                cmd_rx,
                self.event_tx.clone(),
                self.game.clone(),
            );

            self.engine_cmd_tx = Some(cmd_tx);
            self.engine_task = Some(task);
        } else if !enabled {
            // Abort engine task if running (this will clean up the engine)
            if let Some(task) = self.engine_task.take() {
                task.abort();
            }
            self.engine_cmd_tx = None;
        }

        Ok(())
    }

    /// Trigger engine to calculate a move
    async fn trigger_engine_move(&mut self, movetime_ms: Option<u64>) -> Result<(), String> {
        let cmd_tx = self
            .engine_cmd_tx
            .as_ref()
            .ok_or_else(|| "Engine not initialized".to_string())?;

        if !self.engine_enabled {
            return Err("Engine not enabled".to_string());
        }

        // Check if game is ongoing and get FEN
        let (status, fen) = {
            let game = self.game.read().await;
            (game.status(), game.to_fen())
        };

        if !matches!(status, CozyGameStatus::Ongoing) {
            return Err("Game is not ongoing".to_string());
        }

        // Send position to engine via channel
        cmd_tx
            .send(EngineCommand::SetPosition { fen, moves: vec![] })
            .await
            .map_err(|e| format!("Failed to send position: {}", e))?;

        // Calculate move time based on skill level
        let movetime = movetime_ms.unwrap_or_else(|| match self.skill_level {
            0..=5 => 200,
            6..=10 => 500,
            11..=15 => 1000,
            _ => 2000,
        });

        // Start calculation via channel
        cmd_tx
            .send(EngineCommand::Go(GoParams {
                movetime: Some(movetime),
                depth: None,
                infinite: false,
            }))
            .await
            .map_err(|e| format!("Failed to start engine calculation: {}", e))?;

        Ok(())
    }

    /// Stop engine calculation
    async fn stop_engine(&self) -> Result<(), String> {
        if let Some(cmd_tx) = &self.engine_cmd_tx {
            cmd_tx
                .send(EngineCommand::Stop)
                .await
                .map_err(|e| format!("Failed to stop engine: {}", e))?;
        }
        Ok(())
    }

}

/// Information about a game session
#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub id: String,
    pub fen: String,
    pub side_to_move: Color,
    pub status: CozyGameStatus,
    pub move_count: usize,
    pub history: Vec<HistoryEntry>,
    pub engine_enabled: bool,
    pub skill_level: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_session() {
        let manager = SessionManager::new();
        let session_id = manager.create_session(None).await.unwrap();
        assert!(!session_id.is_empty());

        // Verify session can be retrieved
        let info = manager.get_session_info(&session_id).await.unwrap();
        assert_eq!(info.move_count, 0);
    }

    #[tokio::test]
    async fn test_make_move() {
        let manager = SessionManager::new();
        let session_id = manager.create_session(None).await.unwrap();

        // Make e2-e4
        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Second);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Fourth);
        let mv = Move {
            from,
            to,
            promotion: None,
        };

        let result = manager.make_move(&session_id, mv).await;
        assert!(result.is_ok());

        // Verify move was made
        let info = manager.get_session_info(&session_id).await.unwrap();
        assert_eq!(info.move_count, 1);
    }

    #[tokio::test]
    async fn test_undo_redo() {
        let manager = SessionManager::new();
        let session_id = manager.create_session(None).await.unwrap();

        // Make a move
        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Second);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Fourth);
        let mv = Move {
            from,
            to,
            promotion: None,
        };
        manager.make_move(&session_id, mv).await.unwrap();

        // Undo
        manager.undo_move(&session_id).await.unwrap();
        let info = manager.get_session_info(&session_id).await.unwrap();
        assert_eq!(info.move_count, 0);

        // Redo
        manager.redo_move(&session_id).await.unwrap();
        let info = manager.get_session_info(&session_id).await.unwrap();
        assert_eq!(info.move_count, 1);
    }

    #[tokio::test]
    async fn test_close_session() {
        let manager = SessionManager::new();
        let session_id = manager.create_session(None).await.unwrap();

        // Close session
        let result = manager.close_session(&session_id).await;
        assert!(result.is_ok());

        // Verify session is gone
        let result = manager.get_session_info(&session_id).await;
        assert!(result.is_err());
    }
}

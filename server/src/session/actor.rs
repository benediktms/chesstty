use chess::{
    convert_uci_castling_to_cozy, format_uci_move, AnalysisScore, EngineAnalysis, PlayerSide,
};
use engine::{EngineCommand, EngineEvent, StockfishConfig, StockfishEngine};
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use tracing::Instrument;

use super::commands::*;
use super::events::*;
use super::state::{SessionState, TimerState};

/// The main session actor loop.
/// Owns all mutable state. Processes commands and engine events sequentially.
pub(crate) async fn run_session_actor(
    state: SessionState,
    cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: broadcast::Sender<SessionEvent>,
) {
    let session_id = state.session_id.clone();
    run_session_actor_inner(state, cmd_rx, event_tx)
        .instrument(tracing::info_span!("session", id = %session_id))
        .await;
}

async fn run_session_actor_inner(
    mut state: SessionState,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: broadcast::Sender<SessionEvent>,
) {
    tracing::info!("Session actor started");

    let mut timer_interval = time::interval(time::Duration::from_millis(100));
    timer_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    // Auto-trigger engine if needed on startup (e.g., after resume)
    maybe_auto_trigger(&mut state, &event_tx).await;

    loop {
        tokio::select! {
            biased;

            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SessionCommand::Shutdown) | None => {
                        tracing::info!("Session actor shutting down");
                        if let Some(engine) = state.engine.take() {
                            let _ = engine.shutdown().await;
                        }
                        break;
                    }
                    Some(cmd) => {
                        handle_command(&mut state, cmd, &event_tx).await;
                    }
                }
            }

            Some(engine_event) = state.next_engine_event() => {
                handle_engine_event(&mut state, engine_event, &event_tx).await;
            }

            _ = timer_interval.tick(), if state.timer_active() => {
                if state.tick_timer() {
                    // Flag fell — broadcast state change
                    let _ = event_tx.send(SessionEvent::StateChanged(state.snapshot()));
                }
            }
        }
    }

    tracing::info!("Session actor exited");
}

async fn handle_command(
    state: &mut SessionState,
    cmd: SessionCommand,
    event_tx: &broadcast::Sender<SessionEvent>,
) {
    match cmd {
        SessionCommand::MakeMove { mv, reply } => {
            let result = state.apply_move(mv);
            if let Ok(ref snap) = result {
                let _ = event_tx.send(SessionEvent::StateChanged(snap.clone()));
            }
            let _ = reply.send(result);
            maybe_auto_trigger(state, event_tx).await;
        }
        SessionCommand::Undo { reply } => {
            let result = state.apply_undo();
            if let Ok(ref snap) = result {
                let _ = event_tx.send(SessionEvent::StateChanged(snap.clone()));
            }
            let _ = reply.send(result);
            maybe_auto_trigger(state, event_tx).await;
        }
        SessionCommand::Redo { reply } => {
            let result = state.apply_redo();
            if let Ok(ref snap) = result {
                let _ = event_tx.send(SessionEvent::StateChanged(snap.clone()));
            }
            let _ = reply.send(result);
            maybe_auto_trigger(state, event_tx).await;
        }
        SessionCommand::Reset { fen, reply } => {
            let result = state.apply_reset(fen);
            if let Ok(ref snap) = result {
                let _ = event_tx.send(SessionEvent::StateChanged(snap.clone()));
            }
            let _ = reply.send(result);
            maybe_auto_trigger(state, event_tx).await;
        }
        SessionCommand::ConfigureEngine { config, reply } => {
            let result = configure_engine(state, config).await;
            if result.is_ok() {
                let _ = event_tx.send(SessionEvent::StateChanged(state.snapshot()));
                maybe_auto_trigger(state, event_tx).await;
            }
            let _ = reply.send(result);
        }
        SessionCommand::StopEngine { reply } => {
            let result = stop_engine(state).await;
            let _ = reply.send(result);
        }
        SessionCommand::Pause { reply } => {
            if let chess::GamePhase::Playing { turn } = &state.phase {
                state.phase = chess::GamePhase::Paused { resume_turn: *turn };
                // Stop engine if thinking
                if state.engine_thinking {
                    let _ = stop_engine(state).await;
                    state.engine_thinking = false;
                }
                // Pause timer
                if let Some(ref mut timer) = state.timer {
                    timer.stop();
                }
                let _ = event_tx.send(SessionEvent::StateChanged(state.snapshot()));
                let _ = reply.send(Ok(()));
            } else {
                let _ = reply.send(Err(SessionError::InvalidPhaseTransition(format!(
                    "Cannot pause from {:?}",
                    state.phase
                ))));
            }
        }
        SessionCommand::Resume { reply } => {
            if let chess::GamePhase::Paused { resume_turn } = &state.phase {
                let turn = *resume_turn;
                state.phase = chess::GamePhase::Playing { turn };
                // Resume timer
                if let Some(ref mut timer) = state.timer {
                    timer.start(PlayerSide::from(state.game.side_to_move()));
                }
                let _ = event_tx.send(SessionEvent::StateChanged(state.snapshot()));
                let _ = reply.send(Ok(()));
                maybe_auto_trigger(state, event_tx).await;
            } else {
                let _ = reply.send(Err(SessionError::InvalidPhaseTransition(format!(
                    "Cannot resume from {:?}",
                    state.phase
                ))));
            }
        }
        SessionCommand::SetTimer {
            white_ms,
            black_ms,
            reply,
        } => {
            state.timer = Some(TimerState::new(white_ms, black_ms));
            // Start clock for current side if game is playing
            if matches!(state.phase, chess::GamePhase::Playing { .. }) {
                state
                    .timer
                    .as_mut()
                    .unwrap()
                    .start(PlayerSide::from(state.game.side_to_move()));
            }
            let _ = event_tx.send(SessionEvent::StateChanged(state.snapshot()));
            let _ = reply.send(Ok(()));
        }
        SessionCommand::GetSnapshot { reply } => {
            let _ = reply.send(state.snapshot());
        }
        SessionCommand::GetLegalMoves { from, reply } => {
            let moves = compute_legal_moves(state, from);
            let _ = reply.send(moves);
        }
        SessionCommand::Subscribe { reply } => {
            let snapshot = state.snapshot();
            let rx = event_tx.subscribe();
            let _ = reply.send((snapshot, rx));
        }
        SessionCommand::Shutdown => unreachable!(),
    }
}

fn compute_legal_moves(state: &SessionState, from: Option<cozy_chess::Square>) -> Vec<LegalMove> {
    let legal = state.game.legal_moves();
    legal
        .into_iter()
        .filter(|mv| from.is_none_or(|sq| mv.from == sq))
        .map(|mv| {
            let is_capture = state.game.position().piece_on(mv.to).is_some();
            let mut board_position = state.game.position().clone();
            board_position.play(mv);
            let is_check = !board_position.checkers().is_empty();
            let is_checkmate = is_check && board_position.status() == cozy_chess::GameStatus::Won;

            LegalMove {
                from: chess::format_square(mv.from),
                to: chess::format_square(mv.to),
                promotion: mv.promotion.map(|p| chess::format_piece(p).to_string()),
                san: String::new(),
                is_capture,
                is_check,
                is_checkmate,
            }
        })
        .collect()
}

async fn configure_engine(
    state: &mut SessionState,
    config: EngineConfig,
) -> Result<(), SessionError> {
    if config.skill_level > 20 {
        return Err(SessionError::Internal(
            "Skill level must be 0-20".to_string(),
        ));
    }

    if config.enabled && state.engine.is_none() {
        let sf_config = StockfishConfig {
            skill_level: Some(config.skill_level),
            threads: config.threads,
            hash_mb: config.hash_mb,
            label: Some(state.session_id.clone()),
        };
        let engine = StockfishEngine::spawn_with_config(sf_config)
            .await
            .map_err(|e| SessionError::Internal(format!("Failed to spawn engine: {}", e)))?;

        engine
            .send_command(EngineCommand::SetOption {
                name: "Skill Level".to_string(),
                value: Some(config.skill_level.to_string()),
            })
            .await
            .map_err(|e| SessionError::Internal(e.to_string()))?;

        state.engine = Some(engine);
    } else if !config.enabled {
        if let Some(engine) = state.engine.take() {
            let _ = engine.shutdown().await;
        }
        state.engine_thinking = false;
    }

    state.engine_config = Some(config);
    Ok(())
}

async fn stop_engine(state: &mut SessionState) -> Result<(), SessionError> {
    if let Some(ref engine) = state.engine {
        engine
            .send_command(EngineCommand::Stop)
            .await
            .map_err(|e| SessionError::Internal(e.to_string()))?;
    }
    state.engine_thinking = false;
    Ok(())
}

/// Auto-trigger engine if it's the engine's turn and game is ongoing.
async fn maybe_auto_trigger(state: &mut SessionState, event_tx: &broadcast::Sender<SessionEvent>) {
    if state.should_auto_trigger_engine() {
        if let Err(e) = state.trigger_engine().await {
            tracing::error!("Failed to auto-trigger engine: {}", e);
            let _ = event_tx.send(SessionEvent::Error(format!("Engine trigger failed: {}", e)));
        }
    }
}

async fn handle_engine_event(
    state: &mut SessionState,
    event: EngineEvent,
    event_tx: &broadcast::Sender<SessionEvent>,
) {
    match event {
        EngineEvent::BestMove(mv) => {
            state.engine_thinking = false;

            // Discard bestmove if we're paused — it's a leftover from a stop command
            if matches!(state.phase, chess::GamePhase::Paused { .. }) {
                tracing::debug!("Discarding bestmove while paused: {:?}", mv);
                return;
            }

            let legal_moves = state.game.legal_moves();
            let converted = convert_uci_castling_to_cozy(mv, &legal_moves);

            if !legal_moves.contains(&converted) {
                tracing::error!("Engine suggested illegal move: {:?}", mv);
                let _ = event_tx.send(SessionEvent::Error(format!(
                    "Engine suggested illegal move: {:?}",
                    mv
                )));
                return;
            }

            match state.apply_move(converted) {
                Ok(snapshot) => {
                    let _ = event_tx.send(SessionEvent::StateChanged(snapshot));
                    maybe_auto_trigger(state, event_tx).await;
                }
                Err(e) => {
                    tracing::error!("Failed to apply engine move: {}", e);
                    let _ = event_tx.send(SessionEvent::Error(e.to_string()));
                }
            }
        }
        EngineEvent::Info(info) => {
            let analysis = EngineAnalysis {
                depth: info.depth.map(|d| d as u32),
                seldepth: info.seldepth.map(|d| d as u32),
                time_ms: info.time_ms,
                nodes: info.nodes,
                score: info.score.map(|s| match s {
                    engine::Score::Centipawns(cp) => AnalysisScore::Centipawns(cp),
                    engine::Score::Mate(m) => AnalysisScore::Mate(m as i32),
                }),
                pv: info.pv.iter().map(|mv| format_uci_move(*mv)).collect(),
                nps: info.nps,
            };
            state.analysis = Some(analysis.clone());
            let _ = event_tx.send(SessionEvent::EngineThinking(analysis));
        }
        EngineEvent::RawUciMessage { direction, message } => {
            let _ = event_tx.send(SessionEvent::UciMessage(UciLogEntry {
                direction: match direction {
                    engine::UciMessageDirection::ToEngine => UciDirection::ToEngine,
                    engine::UciMessageDirection::FromEngine => UciDirection::FromEngine,
                },
                message,
                context: None,
            }));
        }
        EngineEvent::Ready => {
            tracing::debug!("Engine ready");
        }
        EngineEvent::Error(err) => {
            tracing::error!("Engine error: {}", err);
            let _ = event_tx.send(SessionEvent::Error(format!("Engine error: {}", err)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess::{Game, GameMode};

    async fn spawn_test_actor() -> (
        super::super::handle::SessionHandle,
        broadcast::Receiver<SessionEvent>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = broadcast::channel(100);
        let state = SessionState::new("test".to_string(), Game::new(), GameMode::HumanVsHuman);
        tokio::spawn(run_session_actor(state, cmd_rx, event_tx));
        let handle = super::super::handle::SessionHandle::new(cmd_tx);
        (handle, event_rx)
    }

    #[tokio::test]
    async fn test_make_move_via_actor() {
        let (handle, mut events) = spawn_test_actor().await;
        let mv = cozy_chess::Move {
            from: cozy_chess::Square::new(cozy_chess::File::E, cozy_chess::Rank::Second),
            to: cozy_chess::Square::new(cozy_chess::File::E, cozy_chess::Rank::Fourth),
            promotion: None,
        };
        let snap = handle.make_move(mv).await.unwrap();
        assert_eq!(snap.move_count, 1);
        assert_eq!(snap.side_to_move, "black");

        let event = events.recv().await.unwrap();
        assert!(matches!(event, SessionEvent::StateChanged(_)));
    }

    #[tokio::test]
    async fn test_subscribe_gets_initial_snapshot() {
        let (handle, _) = spawn_test_actor().await;
        let (snapshot, _rx) = handle.subscribe().await.unwrap();
        assert_eq!(snapshot.move_count, 0);
        assert_eq!(snapshot.side_to_move, "white");
        assert!(!snapshot.engine_thinking);
    }

    #[tokio::test]
    async fn test_pause_resume() {
        let (handle, _) = spawn_test_actor().await;

        // Pause should fail from Playing in HvH (it works because Playing is the phase)
        let result = handle.pause().await;
        assert!(result.is_ok());

        let snap = handle.get_snapshot().await.unwrap();
        assert!(matches!(snap.phase, chess::GamePhase::Paused { .. }));

        // Resume
        let result = handle.resume().await;
        assert!(result.is_ok());

        let snap = handle.get_snapshot().await.unwrap();
        assert!(matches!(snap.phase, chess::GamePhase::Playing { .. }));
    }

    #[tokio::test]
    async fn test_shutdown() {
        let (handle, _) = spawn_test_actor().await;
        handle.shutdown().await;
        let mv = cozy_chess::Move {
            from: cozy_chess::Square::new(cozy_chess::File::E, cozy_chess::Rank::Second),
            to: cozy_chess::Square::new(cozy_chess::File::E, cozy_chess::Rank::Fourth),
            promotion: None,
        };
        assert!(handle.make_move(mv).await.is_err());
    }
}

//! Session persistence endpoints

use crate::persistence::{FinishedGameRepository, PositionRepository, SessionRepository};
use crate::service::converters::convert_snapshot_to_proto;
use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct PersistenceEndpoints<
    S: SessionRepository,
    P: PositionRepository,
    F: FinishedGameRepository,
> {
    session_manager: Arc<SessionManager<S, P, F>>,
}

impl<S, P, F> PersistenceEndpoints<S, P, F>
where
    S: SessionRepository + Send + Sync + 'static,
    P: PositionRepository + Send + Sync + 'static,
    F: FinishedGameRepository + Send + Sync + 'static,
{
    pub fn new(session_manager: Arc<SessionManager<S, P, F>>) -> Self {
        Self { session_manager }
    }

    /// Suspend a session. The server owns all state -- the client only needs to
    /// provide the session_id.
    pub async fn suspend_session(
        &self,
        request: Request<SuspendSessionRequest>,
    ) -> Result<Response<SuspendSessionResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC suspend_session");

        let suspended_id = self
            .session_manager
            .suspend_session(&req.session_id)
            .await
            .map_err(Status::internal)?;

        Ok(Response::new(SuspendSessionResponse { suspended_id }))
    }

    pub async fn list_suspended_sessions(
        &self,
        _request: Request<ListSuspendedSessionsRequest>,
    ) -> Result<Response<ListSuspendedSessionsResponse>, Status> {
        tracing::info!("RPC list_suspended_sessions");

        let sessions = self
            .session_manager
            .list_suspended()
            .await
            .map_err(Status::internal)?;

        let proto_sessions: Vec<SuspendedSessionInfo> = sessions
            .into_iter()
            .map(|s| {
                // Parse the stored game_mode string into a GameModeProto
                let game_mode_proto = parse_stored_game_mode(&s.game_mode, &s.human_side);
                SuspendedSessionInfo {
                    suspended_id: s.suspended_id,
                    fen: s.fen,
                    game_mode: Some(game_mode_proto),
                    move_count: s.move_count,
                    side_to_move: s.side_to_move,
                    created_at: s.created_at,
                }
            })
            .collect();

        Ok(Response::new(ListSuspendedSessionsResponse {
            sessions: proto_sessions,
        }))
    }

    pub async fn resume_suspended_session(
        &self,
        request: Request<ResumeSuspendedSessionRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(suspended_id = %req.suspended_id, "RPC resume_suspended_session");

        let snapshot = self
            .session_manager
            .resume_suspended(&req.suspended_id)
            .await
            .map_err(Status::not_found)?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }

    pub async fn save_snapshot(
        &self,
        request: Request<SaveSnapshotRequest>,
    ) -> Result<Response<SaveSnapshotResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(name = %req.name, fen = %req.fen, "RPC save_snapshot");

        // Convert proto game mode to storage strings
        let (game_mode_str, human_side) = if let Some(ref gm) = req.game_mode {
            let mode = GameModeType::try_from(gm.mode).unwrap_or(GameModeType::HumanVsHuman);
            match mode {
                GameModeType::HumanVsEngine => {
                    let side = gm
                        .human_side
                        .and_then(|v| PlayerSideProto::try_from(v).ok())
                        .map(|s| match s {
                            PlayerSideProto::Black => "black".to_string(),
                            _ => "white".to_string(),
                        });
                    let mode_str = format!(
                        "HumanVsEngine:{:?}",
                        if side.as_deref() == Some("black") {
                            "Black"
                        } else {
                            "White"
                        }
                    );
                    (mode_str, side)
                }
                GameModeType::EngineVsEngine => ("EngineVsEngine".to_string(), None),
                GameModeType::Analysis => ("Analysis".to_string(), None),
                GameModeType::Review => ("Review".to_string(), None),
                _ => ("HumanVsHuman".to_string(), None),
            }
        } else {
            ("HumanVsHuman".to_string(), None)
        };

        let suspended_id = self
            .session_manager
            .save_snapshot(
                &req.fen,
                &req.name,
                &game_mode_str,
                human_side,
                req.move_count,
                req.skill_level as u8,
            )
            .await
            .map_err(Status::internal)?;

        Ok(Response::new(SaveSnapshotResponse { suspended_id }))
    }

    pub async fn delete_suspended_session(
        &self,
        request: Request<DeleteSuspendedSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(suspended_id = %req.suspended_id, "RPC delete_suspended_session");

        self.session_manager
            .delete_suspended(&req.suspended_id)
            .await
            .map_err(Status::not_found)?;

        Ok(Response::new(Empty {}))
    }
}

/// Parse the stored game_mode string (e.g., "HumanVsEngine") and optional
/// human_side into a GameModeProto for the client.
fn parse_stored_game_mode(game_mode: &str, human_side: &Option<String>) -> GameModeProto {
    match game_mode {
        s if s.starts_with("HumanVsEngine") => {
            let side = match human_side.as_deref() {
                Some("black") => PlayerSideProto::Black as i32,
                _ => PlayerSideProto::White as i32,
            };
            GameModeProto {
                mode: GameModeType::HumanVsEngine as i32,
                human_side: Some(side),
            }
        }
        "EngineVsEngine" => GameModeProto {
            mode: GameModeType::EngineVsEngine as i32,
            human_side: None,
        },
        "Analysis" => GameModeProto {
            mode: GameModeType::Analysis as i32,
            human_side: None,
        },
        "Review" => GameModeProto {
            mode: GameModeType::Review as i32,
            human_side: None,
        },
        _ => GameModeProto {
            mode: GameModeType::HumanVsHuman as i32,
            human_side: None,
        },
    }
}

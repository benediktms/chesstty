//! Session management endpoints

use crate::persistence::{FinishedGameRepository, PositionRepository, SessionRepository};
use crate::service::converters::{convert_snapshot_to_proto, parse_game_mode_from_proto};
use crate::session::SessionManager;
use ::chess::GameMode;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SessionEndpoints<
    S: SessionRepository,
    P: PositionRepository,
    F: FinishedGameRepository,
> {
    session_manager: Arc<SessionManager<S, P, F>>,
}

impl<S, P, F> SessionEndpoints<S, P, F>
where
    S: SessionRepository + Send + Sync + 'static,
    P: PositionRepository + Send + Sync + 'static,
    F: FinishedGameRepository + Send + Sync + 'static,
{
    pub fn new(session_manager: Arc<SessionManager<S, P, F>>) -> Self {
        Self { session_manager }
    }

    pub async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(fen = ?req.fen, game_mode = ?req.game_mode, "RPC create_session");

        // Parse game_mode from the request, defaulting to HumanVsHuman
        let game_mode = req
            .game_mode
            .as_ref()
            .map(parse_game_mode_from_proto)
            .unwrap_or(GameMode::HumanVsHuman);

        let snapshot = self
            .session_manager
            .create_session(req.fen, game_mode)
            .await
            .map_err(Status::invalid_argument)?;

        // If a timer was provided, configure it on the session
        if let Some(timer) = req.timer {
            let handle = self
                .session_manager
                .get_handle(&snapshot.session_id)
                .await
                .map_err(Status::internal)?;

            handle
                .set_timer(timer.white_remaining_ms, timer.black_remaining_ms)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            // Re-fetch snapshot after timer is set
            let updated = handle
                .get_snapshot()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            return Ok(Response::new(convert_snapshot_to_proto(updated)));
        }

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }

    pub async fn get_session(
        &self,
        request: Request<GetSessionRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::debug!(session_id = %req.session_id, "RPC get_session");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let snapshot = handle
            .get_snapshot()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }
}

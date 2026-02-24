//! Engine control and pause/resume endpoints

use crate::persistence::{FinishedGameRepository, PositionRepository, SessionRepository};
use crate::session::commands::EngineConfig;
use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct EngineEndpoints<S: SessionRepository, P: PositionRepository, F: FinishedGameRepository> {
    session_manager: Arc<SessionManager<S, P, F>>,
}

impl<S, P, F> EngineEndpoints<S, P, F>
where
    S: SessionRepository + Send + Sync + 'static,
    P: PositionRepository + Send + Sync + 'static,
    F: FinishedGameRepository + Send + Sync + 'static,
{
    pub fn new(session_manager: Arc<SessionManager<S, P, F>>) -> Self {
        Self { session_manager }
    }

    pub async fn set_engine(
        &self,
        request: Request<SetEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(
            session_id = %req.session_id,
            enabled = req.enabled,
            skill = req.skill_level,
            threads = ?req.threads,
            hash = ?req.hash_mb,
            "RPC set_engine"
        );

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let config = EngineConfig {
            enabled: req.enabled,
            skill_level: req.skill_level as u8,
            threads: req.threads,
            hash_mb: req.hash_mb,
        };

        handle
            .configure_engine(config)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    pub async fn stop_engine(
        &self,
        request: Request<StopEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC stop_engine");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        handle
            .stop_engine()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    pub async fn pause_session(
        &self,
        request: Request<PauseSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC pause_session");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        handle
            .pause()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    pub async fn resume_session(
        &self,
        request: Request<ResumeSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC resume_session");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        handle
            .resume()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }
}

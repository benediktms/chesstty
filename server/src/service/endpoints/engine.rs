//! Engine control endpoints

use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct EngineEndpoints {
    session_manager: Arc<SessionManager>,
}

impl EngineEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn set_engine(
        &self,
        request: Request<SetEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, enabled = req.enabled, skill = req.skill_level, threads = ?req.threads, hash = ?req.hash_mb, "RPC set_engine");
        self.session_manager
            .set_engine(
                &req.session_id,
                req.enabled,
                req.skill_level as u8,
                req.threads,
                req.hash_mb,
            )
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        Ok(Response::new(Empty {}))
    }

    pub async fn trigger_engine_move(
        &self,
        request: Request<TriggerEngineMoveRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, movetime = ?req.movetime_ms, "RPC trigger_engine_move");
        self.session_manager
            .trigger_engine_move(&req.session_id, req.movetime_ms)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        Ok(Response::new(Empty {}))
    }

    pub async fn stop_engine(
        &self,
        request: Request<StopEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC stop_engine");
        self.session_manager
            .stop_engine(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        Ok(Response::new(Empty {}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> Arc<SessionManager> {
        let temp_dir = std::env::temp_dir().join(format!("chesstty_test_{}", uuid::Uuid::new_v4()));
        Arc::new(SessionManager::new(temp_dir, None))
    }

    #[tokio::test]
    async fn test_set_engine() {
        let manager = test_manager();
        let endpoints = EngineEndpoints::new(manager.clone());

        let session_id = manager.create_session(None).await.unwrap();

        let request = Request::new(SetEngineRequest {
            session_id,
            enabled: true,
            skill_level: 10,
            threads: Some(2),
            hash_mb: Some(128),
        });

        let response = endpoints.set_engine(request).await;
        assert!(response.is_ok());
    }
}

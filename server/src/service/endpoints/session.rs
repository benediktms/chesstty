//! Session management endpoints

use crate::session::SessionManager;
use crate::service::converters::convert_session_info_to_proto;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SessionEndpoints {
    session_manager: Arc<SessionManager>,
}

impl SessionEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::info!(fen = ?req.fen, "RPC create_session");
        let session_id = self
            .session_manager
            .create_session(req.fen)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        let info = self
            .session_manager
            .get_session_info(&session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
    }

    pub async fn get_session(
        &self,
        request: Request<GetSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::debug!(session_id = %req.session_id, "RPC get_session");
        let info = self
            .session_manager
            .get_session_info(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
    }

    pub async fn close_session(
        &self,
        request: Request<CloseSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC close_session");
        self.session_manager
            .close_session(&req.session_id)
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
    async fn test_create_and_get_session() {
        let manager = test_manager();
        let endpoints = SessionEndpoints::new(manager);

        let request = Request::new(CreateSessionRequest { fen: None });
        let response = endpoints.create_session(request).await.unwrap();
        let info = response.into_inner();

        assert!(!info.session_id.is_empty());
        assert_eq!(info.move_count, 0);

        let get_req = Request::new(GetSessionRequest {
            session_id: info.session_id.clone(),
        });
        let get_resp = endpoints.get_session(get_req).await.unwrap();
        assert_eq!(get_resp.into_inner().session_id, info.session_id);
    }

    #[tokio::test]
    async fn test_close_session() {
        let manager = test_manager();
        let endpoints = SessionEndpoints::new(manager);

        let request = Request::new(CreateSessionRequest { fen: None });
        let response = endpoints.create_session(request).await.unwrap();
        let session_id = response.into_inner().session_id;

        let close_req = Request::new(CloseSessionRequest { session_id: session_id.clone() });
        endpoints.close_session(close_req).await.unwrap();

        // Verify session is gone
        let get_req = Request::new(GetSessionRequest { session_id });
        assert!(endpoints.get_session(get_req).await.is_err());
    }
}

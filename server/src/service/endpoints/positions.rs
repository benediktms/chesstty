//! Saved positions endpoints

use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct PositionsEndpoints {
    session_manager: Arc<SessionManager>,
}

impl PositionsEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn save_position(
        &self,
        request: Request<SavePositionRequest>,
    ) -> Result<Response<SavePositionResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(name = %req.name, fen = %req.fen, "RPC save_position");
        let position_id = self
            .session_manager
            .save_position(req.name, req.fen)
            .map_err(|e| Status::invalid_argument(e))?;

        Ok(Response::new(SavePositionResponse { position_id }))
    }

    pub async fn list_positions(
        &self,
        _request: Request<ListPositionsRequest>,
    ) -> Result<Response<ListPositionsResponse>, Status> {
        tracing::info!("RPC list_positions");
        let positions = self
            .session_manager
            .list_positions()
            .map_err(|e| Status::internal(e))?;

        let proto_positions: Vec<SavedPosition> = positions
            .into_iter()
            .map(|p| SavedPosition {
                position_id: p.position_id,
                name: p.name,
                fen: p.fen,
                is_default: p.is_default,
                created_at: p.created_at,
            })
            .collect();

        Ok(Response::new(ListPositionsResponse {
            positions: proto_positions,
        }))
    }

    pub async fn delete_position(
        &self,
        request: Request<DeletePositionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(position_id = %req.position_id, "RPC delete_position");
        self.session_manager
            .delete_position(&req.position_id)
            .map_err(|e| Status::invalid_argument(e))?;

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
    async fn test_save_and_list_positions() {
        let manager = test_manager();
        let endpoints = PositionsEndpoints::new(manager.clone());

        let save_req = Request::new(SavePositionRequest {
            name: "Test Position".to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        });

        let save_resp = endpoints.save_position(save_req).await.unwrap();
        let position_id = save_resp.into_inner().position_id;

        let list_req = Request::new(ListPositionsRequest {});
        let list_resp = endpoints.list_positions(list_req).await.unwrap();
        let positions = list_resp.into_inner().positions;

        assert!(positions.iter().any(|p| p.position_id == position_id));
    }
}

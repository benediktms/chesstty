//! Saved positions endpoints

use crate::persistence::Persistence;
use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct PositionsEndpoints<D: Persistence> {
    session_manager: Arc<SessionManager<D>>,
}

impl<D: Persistence> PositionsEndpoints<D> {
    pub fn new(session_manager: Arc<SessionManager<D>>) -> Self {
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
            .save_position(&req.name, &req.fen)
            .await
            .map_err(Status::invalid_argument)?;

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
            .await
            .map_err(Status::internal)?;

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
            .await
            .map_err(Status::invalid_argument)?;

        Ok(Response::new(Empty {}))
    }
}

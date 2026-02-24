//! Game action endpoints

use crate::persistence::{FinishedGameRepository, PositionRepository, SessionRepository};
use crate::service::converters::convert_snapshot_to_proto;
use crate::service::parsers::{parse_move_repr, parse_square_grpc};
use crate::session::SessionManager;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct GameEndpoints<S: SessionRepository, P: PositionRepository, F: FinishedGameRepository> {
    session_manager: Arc<SessionManager<S, P, F>>,
}

impl<S, P, F> GameEndpoints<S, P, F>
where
    S: SessionRepository + Send + Sync + 'static,
    P: PositionRepository + Send + Sync + 'static,
    F: FinishedGameRepository + Send + Sync + 'static,
{
    pub fn new(session_manager: Arc<SessionManager<S, P, F>>) -> Self {
        Self { session_manager }
    }

    pub async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, mv = ?req.r#move, "RPC make_move");

        let mv_repr = req
            .r#move
            .ok_or_else(|| Status::invalid_argument("Move is required"))?;

        let mv = parse_move_repr(&mv_repr).map_err(|e| *e)?;

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let snapshot = handle
            .make_move(mv)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }

    pub async fn get_legal_moves(
        &self,
        request: Request<GetLegalMovesRequest>,
    ) -> Result<Response<LegalMovesResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!(session_id = %req.session_id, from = ?req.from_square, "RPC get_legal_moves");

        let from_square = if let Some(ref sq_str) = req.from_square {
            Some(parse_square_grpc(sq_str).map_err(|e| *e)?)
        } else {
            None
        };

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let moves = handle
            .get_legal_moves(from_square)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let move_details: Vec<MoveDetail> = moves
            .into_iter()
            .map(|mv| MoveDetail {
                from: mv.from,
                to: mv.to,
                promotion: mv.promotion,
                san: mv.san,
                is_capture: mv.is_capture,
                is_check: mv.is_check,
                is_checkmate: mv.is_checkmate,
            })
            .collect();

        Ok(Response::new(LegalMovesResponse {
            moves: move_details,
        }))
    }

    pub async fn undo_move(
        &self,
        request: Request<UndoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC undo_move");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let snapshot = handle
            .undo()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }

    pub async fn redo_move(
        &self,
        request: Request<RedoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC redo_move");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let snapshot = handle
            .redo()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }

    pub async fn reset_game(
        &self,
        request: Request<ResetGameRequest>,
    ) -> Result<Response<chess_proto::SessionSnapshot>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, fen = ?req.fen, "RPC reset_game");

        let handle = self
            .session_manager
            .get_handle(&req.session_id)
            .await
            .map_err(Status::not_found)?;

        let snapshot = handle
            .reset(req.fen)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(convert_snapshot_to_proto(snapshot)))
    }
}

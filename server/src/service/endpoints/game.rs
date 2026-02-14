//! Game action endpoints

use crate::session::SessionManager;
use crate::service::converters::{convert_history_entry_to_proto, convert_session_info_to_proto};
use crate::service::parsers::{format_move_san, parse_move_repr, parse_square_grpc};
use chess_common::format_square;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct GameEndpoints {
    session_manager: Arc<SessionManager>,
}

impl GameEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, mv = ?req.r#move, "RPC make_move");
        let mv_repr = req
            .r#move
            .ok_or_else(|| Status::invalid_argument("Move is required"))?;

        let mv = parse_move_repr(&mv_repr)?;

        let (entry, _status) = self
            .session_manager
            .make_move(&req.session_id, mv)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        let info = self
            .session_manager
            .get_session_info(&req.session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        let response = MakeMoveResponse {
            session_info: Some(convert_session_info_to_proto(info)),
            move_record: Some(convert_history_entry_to_proto(&entry)),
        };

        Ok(Response::new(response))
    }

    pub async fn get_legal_moves(
        &self,
        request: Request<GetLegalMovesRequest>,
    ) -> Result<Response<LegalMovesResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!(session_id = %req.session_id, from = ?req.from_square, "RPC get_legal_moves");

        let from_square = if let Some(ref sq_str) = req.from_square {
            Some(parse_square_grpc(sq_str)?)
        } else {
            None
        };

        let moves = self
            .session_manager
            .get_legal_moves(&req.session_id, from_square)
            .await
            .map_err(|e| Status::not_found(e))?;

        let move_details: Vec<MoveDetail> = moves
            .into_iter()
            .map(|mv| {
                MoveDetail {
                    from: format_square(mv.from),
                    to: format_square(mv.to),
                    promotion: mv.promotion.map(|p| chess_common::format_piece(p).to_string()),
                    san: format_move_san(&mv),
                    is_capture: false,
                    is_check: false,
                    is_checkmate: false,
                }
            })
            .collect();

        Ok(Response::new(LegalMovesResponse {
            moves: move_details,
        }))
    }

    pub async fn undo_move(
        &self,
        request: Request<UndoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC undo_move");
        self.session_manager
            .undo_move(&req.session_id)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        let info = self
            .session_manager
            .get_session_info(&req.session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
    }

    pub async fn redo_move(
        &self,
        request: Request<RedoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC redo_move");
        self.session_manager
            .redo_move(&req.session_id)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        let info = self
            .session_manager
            .get_session_info(&req.session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
    }

    pub async fn reset_game(
        &self,
        request: Request<ResetGameRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, fen = ?req.fen, "RPC reset_game");
        self.session_manager
            .reset_game(&req.session_id, req.fen)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        let info = self
            .session_manager
            .get_session_info(&req.session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
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
    async fn test_make_move() {
        let manager = test_manager();
        let endpoints = GameEndpoints::new(manager.clone());

        // Create session first
        let session_id = manager.create_session(None).await.unwrap();

        let request = Request::new(MakeMoveRequest {
            session_id: session_id.clone(),
            r#move: Some(MoveRepr {
                from: "e2".to_string(),
                to: "e4".to_string(),
                promotion: None,
            }),
        });

        let response = endpoints.make_move(request).await.unwrap();
        let result = response.into_inner();

        assert!(result.session_info.is_some());
        assert_eq!(result.session_info.unwrap().move_count, 1);
    }

    #[tokio::test]
    async fn test_undo_redo() {
        let manager = test_manager();
        let endpoints = GameEndpoints::new(manager.clone());

        let session_id = manager.create_session(None).await.unwrap();

        // Make a move
        let mv_req = Request::new(MakeMoveRequest {
            session_id: session_id.clone(),
            r#move: Some(MoveRepr {
                from: "e2".to_string(),
                to: "e4".to_string(),
                promotion: None,
            }),
        });
        endpoints.make_move(mv_req).await.unwrap();

        // Undo
        let undo_req = Request::new(UndoMoveRequest { session_id: session_id.clone() });
        let undo_resp = endpoints.undo_move(undo_req).await.unwrap();
        assert_eq!(undo_resp.into_inner().move_count, 0);

        // Redo
        let redo_req = Request::new(RedoMoveRequest { session_id });
        let redo_resp = endpoints.redo_move(redo_req).await.unwrap();
        assert_eq!(redo_resp.into_inner().move_count, 1);
    }
}

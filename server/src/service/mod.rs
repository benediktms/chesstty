//! gRPC service implementation with modular organization
//!
//! This module contains the ChessService gRPC implementation split into:
//! - converters: Domain model → Proto conversions
//! - parsers: Proto → Domain model parsing
//! - Service implementation with clear endpoint organization

mod converters;
mod parsers;

use crate::session::SessionManager;
use chess_common::format_square;
use chess_proto::chess_service_server::ChessService;
use chess_proto::*;
use cozy_chess::Square;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

pub use converters::{
    convert_game_status, convert_history_entry_to_proto, convert_session_event_to_proto,
    convert_session_info_to_proto,
};
pub use parsers::{
    format_move_san, parse_file_grpc, parse_move_repr, parse_piece_grpc, parse_rank_grpc,
    parse_square_grpc,
};

/// Implementation of the ChessService gRPC service
pub struct ChessServiceImpl {
    session_manager: Arc<SessionManager>,
}

impl ChessServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[tonic::async_trait]
impl ChessService for ChessServiceImpl {
    // =========================================================================
    // Session Management Endpoints
    // =========================================================================

    async fn create_session(
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

    async fn get_session(
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

    async fn close_session(
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

    // =========================================================================
    // Game Action Endpoints
    // =========================================================================

    async fn make_move(
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

    async fn get_legal_moves(
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

    async fn undo_move(
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

    async fn redo_move(
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

    async fn reset_game(
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

    // =========================================================================
    // Engine Control Endpoints
    // =========================================================================

    async fn set_engine(
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

    async fn trigger_engine_move(
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

    async fn stop_engine(
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

    // =========================================================================
    // Event Streaming Endpoint
    // =========================================================================

    type StreamEventsStream = Pin<Box<dyn Stream<Item = Result<GameEvent, Status>> + Send>>;

    async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, "RPC stream_events");

        let mut event_rx = self
            .session_manager
            .subscribe_events(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        let session_id = req.session_id.clone();
        let stream = async_stream::stream! {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if let Some(proto_event) = convert_session_event_to_proto(event, &session_id) {
                            yield Ok(proto_event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("Client lagged, skipped {} events", skipped);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Event stream closed for session {}", session_id);
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    // =========================================================================
    // Session Persistence Endpoints
    // =========================================================================

    async fn suspend_session(
        &self,
        request: Request<SuspendSessionRequest>,
    ) -> Result<Response<SuspendSessionResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(session_id = %req.session_id, game_mode = %req.game_mode, "RPC suspend_session");
        let suspended_id = self
            .session_manager
            .suspend_session(
                &req.session_id,
                req.game_mode,
                req.human_side,
                req.skill_level as u8,
            )
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(SuspendSessionResponse { suspended_id }))
    }

    async fn list_suspended_sessions(
        &self,
        _request: Request<ListSuspendedSessionsRequest>,
    ) -> Result<Response<ListSuspendedSessionsResponse>, Status> {
        tracing::info!("RPC list_suspended_sessions");
        let sessions = self
            .session_manager
            .list_suspended()
            .map_err(|e| Status::internal(e))?;

        let proto_sessions: Vec<SuspendedSessionInfo> = sessions
            .into_iter()
            .map(|s| SuspendedSessionInfo {
                suspended_id: s.suspended_id,
                fen: s.fen,
                game_mode: s.game_mode,
                human_side: s.human_side,
                skill_level: s.skill_level as u32,
                move_count: s.move_count,
                side_to_move: s.side_to_move,
                created_at: s.created_at,
            })
            .collect();

        Ok(Response::new(ListSuspendedSessionsResponse {
            sessions: proto_sessions,
        }))
    }

    async fn resume_suspended_session(
        &self,
        request: Request<ResumeSuspendedSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
        tracing::info!(suspended_id = %req.suspended_id, "RPC resume_suspended_session");
        let (session_id, _data) = self
            .session_manager
            .resume_suspended(&req.suspended_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        let info = self
            .session_manager
            .get_session_info(&session_id)
            .await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(convert_session_info_to_proto(info)))
    }

    async fn delete_suspended_session(
        &self,
        request: Request<DeleteSuspendedSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        tracing::info!(suspended_id = %req.suspended_id, "RPC delete_suspended_session");
        self.session_manager
            .delete_suspended(&req.suspended_id)
            .map_err(|e| Status::not_found(e))?;

        Ok(Response::new(Empty {}))
    }

    // =========================================================================
    // Saved Positions Endpoints
    // =========================================================================

    async fn save_position(
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

    async fn list_positions(
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

    async fn delete_position(
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

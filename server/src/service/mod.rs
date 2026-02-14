//! gRPC service implementation with modular organization
//!
//! This module contains the ChessService gRPC implementation split into:
//! - converters: Domain model → Proto conversions
//! - parsers: Proto → Domain model parsing
//! - endpoints: Individual endpoint handlers organized by domain

mod converters;
mod endpoints;
mod parsers;

use crate::session::SessionManager;
use chess_proto::chess_service_server::ChessService;
use chess_proto::*;
use endpoints::*;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

/// Implementation of the ChessService gRPC service
///
/// This service delegates to specialized endpoint handlers for better modularity and testability.
pub struct ChessServiceImpl {
    session_endpoints: SessionEndpoints,
    game_endpoints: GameEndpoints,
    engine_endpoints: EngineEndpoints,
    events_endpoints: EventsEndpoints,
    persistence_endpoints: PersistenceEndpoints,
    positions_endpoints: PositionsEndpoints,
}

impl ChessServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_endpoints: SessionEndpoints::new(session_manager.clone()),
            game_endpoints: GameEndpoints::new(session_manager.clone()),
            engine_endpoints: EngineEndpoints::new(session_manager.clone()),
            events_endpoints: EventsEndpoints::new(session_manager.clone()),
            persistence_endpoints: PersistenceEndpoints::new(session_manager.clone()),
            positions_endpoints: PositionsEndpoints::new(session_manager),
        }
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
        self.session_endpoints.create_session(request).await
    }

    async fn get_session(
        &self,
        request: Request<GetSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        self.session_endpoints.get_session(request).await
    }

    async fn close_session(
        &self,
        request: Request<CloseSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.session_endpoints.close_session(request).await
    }

    // =========================================================================
    // Game Action Endpoints
    // =========================================================================

    async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        self.game_endpoints.make_move(request).await
    }

    async fn get_legal_moves(
        &self,
        request: Request<GetLegalMovesRequest>,
    ) -> Result<Response<LegalMovesResponse>, Status> {
        self.game_endpoints.get_legal_moves(request).await
    }

    async fn undo_move(
        &self,
        request: Request<UndoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        self.game_endpoints.undo_move(request).await
    }

    async fn redo_move(
        &self,
        request: Request<RedoMoveRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        self.game_endpoints.redo_move(request).await
    }

    async fn reset_game(
        &self,
        request: Request<ResetGameRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        self.game_endpoints.reset_game(request).await
    }

    // =========================================================================
    // Engine Control Endpoints
    // =========================================================================

    async fn set_engine(
        &self,
        request: Request<SetEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.engine_endpoints.set_engine(request).await
    }

    async fn trigger_engine_move(
        &self,
        request: Request<TriggerEngineMoveRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.engine_endpoints.trigger_engine_move(request).await
    }

    async fn stop_engine(
        &self,
        request: Request<StopEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.engine_endpoints.stop_engine(request).await
    }

    // =========================================================================
    // Event Streaming Endpoint
    // =========================================================================

    type StreamEventsStream = Pin<Box<dyn Stream<Item = Result<GameEvent, Status>> + Send>>;

    async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        self.events_endpoints.stream_events(request).await
    }

    // =========================================================================
    // Session Persistence Endpoints
    // =========================================================================

    async fn suspend_session(
        &self,
        request: Request<SuspendSessionRequest>,
    ) -> Result<Response<SuspendSessionResponse>, Status> {
        self.persistence_endpoints.suspend_session(request).await
    }

    async fn list_suspended_sessions(
        &self,
        request: Request<ListSuspendedSessionsRequest>,
    ) -> Result<Response<ListSuspendedSessionsResponse>, Status> {
        self.persistence_endpoints
            .list_suspended_sessions(request)
            .await
    }

    async fn resume_suspended_session(
        &self,
        request: Request<ResumeSuspendedSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        self.persistence_endpoints
            .resume_suspended_session(request)
            .await
    }

    async fn delete_suspended_session(
        &self,
        request: Request<DeleteSuspendedSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.persistence_endpoints
            .delete_suspended_session(request)
            .await
    }

    // =========================================================================
    // Saved Positions Endpoints
    // =========================================================================

    async fn save_position(
        &self,
        request: Request<SavePositionRequest>,
    ) -> Result<Response<SavePositionResponse>, Status> {
        self.positions_endpoints.save_position(request).await
    }

    async fn list_positions(
        &self,
        request: Request<ListPositionsRequest>,
    ) -> Result<Response<ListPositionsResponse>, Status> {
        self.positions_endpoints.list_positions(request).await
    }

    async fn delete_position(
        &self,
        request: Request<DeletePositionRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.positions_endpoints.delete_position(request).await
    }
}

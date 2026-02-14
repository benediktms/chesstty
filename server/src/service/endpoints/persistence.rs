//! Session persistence endpoints

use crate::session::SessionManager;
use crate::service::converters::convert_session_info_to_proto;
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct PersistenceEndpoints {
    session_manager: Arc<SessionManager>,
}

impl PersistenceEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn suspend_session(
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

    pub async fn list_suspended_sessions(
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

    pub async fn resume_suspended_session(
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

    pub async fn delete_suspended_session(
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> Arc<SessionManager> {
        let temp_dir = std::env::temp_dir().join(format!("chesstty_test_{}", uuid::Uuid::new_v4()));
        Arc::new(SessionManager::new(temp_dir, None))
    }

    #[tokio::test]
    async fn test_suspend_and_resume() {
        let manager = test_manager();
        let endpoints = PersistenceEndpoints::new(manager.clone());

        let session_id = manager.create_session(None).await.unwrap();

        // Suspend
        let suspend_req = Request::new(SuspendSessionRequest {
            session_id: session_id.clone(),
            game_mode: "HumanVsEngine".to_string(),
            human_side: Some("white".to_string()),
            skill_level: 10,
        });

        let suspend_resp = endpoints.suspend_session(suspend_req).await.unwrap();
        let suspended_id = suspend_resp.into_inner().suspended_id;

        // Resume
        let resume_req = Request::new(ResumeSuspendedSessionRequest { suspended_id: suspended_id.clone() });
        let resume_resp = endpoints.resume_suspended_session(resume_req).await.unwrap();
        let new_session = resume_resp.into_inner();

        assert!(!new_session.session_id.is_empty());
        assert_ne!(new_session.session_id, session_id);
    }
}

//! Event streaming endpoint

use crate::session::SessionManager;
use crate::service::converters::convert_session_event_to_proto;
use chess_proto::*;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

pub struct EventsEndpoints {
    session_manager: Arc<SessionManager>,
}

impl EventsEndpoints {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    pub async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Pin<Box<dyn Stream<Item = Result<GameEvent, Status>> + Send>>>, Status> {
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
}

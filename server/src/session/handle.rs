use cozy_chess::{Move, Square};
use tokio::sync::{broadcast, mpsc, oneshot};

use super::commands::*;
use super::events::SessionEvent;
use super::snapshot::SessionSnapshot;

/// Cheap, cloneable handle to a session actor.
#[derive(Clone)]
pub struct SessionHandle {
    id: String,
    cmd_tx: mpsc::Sender<SessionCommand>,
}

impl SessionHandle {
    pub(crate) fn new(id: String, cmd_tx: mpsc::Sender<SessionCommand>) -> Self {
        Self { id, cmd_tx }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn make_move(&self, mv: Move) -> Result<SessionSnapshot, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::MakeMove { mv, reply: tx })
            .await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn undo(&self) -> Result<SessionSnapshot, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Undo { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn redo(&self) -> Result<SessionSnapshot, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Redo { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn reset(&self, fen: Option<String>) -> Result<SessionSnapshot, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Reset { fen, reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn configure_engine(&self, config: EngineConfig) -> Result<(), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::ConfigureEngine { config, reply: tx })
            .await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn stop_engine(&self) -> Result<(), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::StopEngine { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn pause(&self) -> Result<(), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Pause { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn resume(&self) -> Result<(), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Resume { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn set_timer(&self, white_ms: u64, black_ms: u64) -> Result<(), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::SetTimer {
            white_ms,
            black_ms,
            reply: tx,
        })
        .await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))?
    }

    pub async fn get_snapshot(&self) -> Result<SessionSnapshot, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::GetSnapshot { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))
    }

    pub async fn get_legal_moves(
        &self,
        from: Option<Square>,
    ) -> Result<Vec<LegalMove>, SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::GetLegalMoves { from, reply: tx })
            .await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))
    }

    pub async fn subscribe(
        &self,
    ) -> Result<(SessionSnapshot, broadcast::Receiver<SessionEvent>), SessionError> {
        let (tx, rx) = oneshot::channel();
        self.send(SessionCommand::Subscribe { reply: tx }).await?;
        rx.await
            .map_err(|_| SessionError::Internal("Reply dropped".into()))
    }

    pub async fn shutdown(&self) {
        let _ = self.cmd_tx.send(SessionCommand::Shutdown).await;
    }

    async fn send(&self, cmd: SessionCommand) -> Result<(), SessionError> {
        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|_| SessionError::Internal("Session actor closed".into()))
    }
}

//! Mock ChessService implementation for testing

use crate::error::{ClientError, ClientResult};
use crate::traits::ChessService;
use async_trait::async_trait;
use chess_proto::*;
use std::sync::{Arc, Mutex};

/// Mock service for testing - only compiled in test mode or with mock feature
#[cfg(any(test, feature = "mock"))]
pub struct MockChessService {
    responses: Arc<Mutex<MockResponses>>,
    call_log: Arc<Mutex<Vec<MockCall>>>,
    session_id: Arc<Mutex<Option<String>>>,
}

#[cfg(any(test, feature = "mock"))]
#[derive(Default)]
struct MockResponses {
    create_session: Option<Box<dyn Fn() -> ClientResult<SessionSnapshot> + Send>>,
    get_session: Option<Box<dyn Fn() -> ClientResult<SessionSnapshot> + Send>>,
    make_move: Option<Box<dyn Fn() -> ClientResult<SessionSnapshot> + Send>>,
    get_legal_moves: Option<Box<dyn Fn() -> ClientResult<Vec<MoveDetail>> + Send>>,
    close_session: Option<Box<dyn Fn() -> ClientResult<()> + Send>>,
    pause_session: Option<Box<dyn Fn() -> ClientResult<()> + Send>>,
    resume_session: Option<Box<dyn Fn() -> ClientResult<()> + Send>>,
    set_engine: Option<Box<dyn Fn() -> ClientResult<()> + Send>>,
}

#[cfg(any(test, feature = "mock"))]
#[derive(Debug, Clone)]
pub enum MockCall {
    CreateSession {
        fen: Option<String>,
        game_mode: Option<GameModeProto>,
        timer: Option<TimerState>,
    },
    GetSession,
    MakeMove {
        from: String,
        to: String,
        promotion: Option<String>,
    },
    GetLegalMoves {
        from_square: Option<String>,
    },
    CloseSession,
    PauseSession,
    ResumeSession,
    SetEngine {
        enabled: bool,
        skill_level: u8,
        threads: u32,
        hash_mb: u32,
    },
}

#[cfg(any(test, feature = "mock"))]
impl Default for MockChessService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "mock"))]
impl MockChessService {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(MockResponses::default())),
            call_log: Arc::new(Mutex::new(Vec::new())),
            session_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Configure create_session response
    pub fn with_create_session_response<F>(self, f: F) -> Self
    where
        F: Fn() -> ClientResult<SessionSnapshot> + Send + 'static,
    {
        self.responses.lock().unwrap().create_session = Some(Box::new(f));
        self
    }

    /// Configure get_session response
    pub fn with_get_session_response<F>(self, f: F) -> Self
    where
        F: Fn() -> ClientResult<SessionSnapshot> + Send + 'static,
    {
        self.responses.lock().unwrap().get_session = Some(Box::new(f));
        self
    }

    /// Configure make_move response
    pub fn with_make_move_response<F>(self, f: F) -> Self
    where
        F: Fn() -> ClientResult<SessionSnapshot> + Send + 'static,
    {
        self.responses.lock().unwrap().make_move = Some(Box::new(f));
        self
    }

    /// Configure get_legal_moves response
    pub fn with_legal_moves_response<F>(self, f: F) -> Self
    where
        F: Fn() -> ClientResult<Vec<MoveDetail>> + Send + 'static,
    {
        self.responses.lock().unwrap().get_legal_moves = Some(Box::new(f));
        self
    }

    /// Pre-configure with a standard game session
    pub fn with_standard_game(self) -> Self {
        let snapshot = SessionSnapshot {
            session_id: "test-session-001".to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            side_to_move: "white".to_string(),
            phase: GamePhase::Playing as i32,
            status: GameStatus::Ongoing as i32,
            move_count: 0,
            history: vec![],
            last_move: None,
            analysis: None,
            engine_config: None,
            game_mode: Some(GameModeProto {
                mode: GameModeType::HumanVsHuman as i32,
                human_side: None,
            }),
            engine_thinking: false,
            timer: None,
            start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        };

        let snapshot2 = snapshot.clone();
        self.with_create_session_response(move || Ok(snapshot.clone()))
            .with_get_session_response(move || Ok(snapshot2.clone()))
    }

    /// Get recorded calls for verification
    pub fn get_calls(&self) -> Vec<MockCall> {
        self.call_log.lock().unwrap().clone()
    }

    /// Clear call history
    pub fn clear_calls(&self) {
        self.call_log.lock().unwrap().clear()
    }
}

#[cfg(any(test, feature = "mock"))]
#[async_trait]
impl ChessService for MockChessService {
    async fn create_session(
        &mut self,
        fen: Option<String>,
        game_mode: Option<GameModeProto>,
        timer: Option<TimerState>,
    ) -> ClientResult<SessionSnapshot> {
        self.call_log.lock().unwrap().push(MockCall::CreateSession {
            fen,
            game_mode,
            timer,
        });

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.create_session {
            let result = f();
            if let Ok(ref snapshot) = result {
                *self.session_id.lock().unwrap() = Some(snapshot.session_id.clone());
            }
            result
        } else {
            Err(ClientError::NotConfigured("create_session".to_string()))
        }
    }

    async fn get_session(&mut self) -> ClientResult<SessionSnapshot> {
        self.call_log.lock().unwrap().push(MockCall::GetSession);

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.get_session {
            f()
        } else {
            Err(ClientError::NoActiveSession)
        }
    }

    async fn close_session(&mut self) -> ClientResult<()> {
        self.call_log.lock().unwrap().push(MockCall::CloseSession);
        self.session_id.lock().unwrap().take();

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.close_session {
            f()
        } else {
            Ok(())
        }
    }

    async fn make_move(
        &mut self,
        from: &str,
        to: &str,
        promotion: Option<String>,
    ) -> ClientResult<SessionSnapshot> {
        self.call_log.lock().unwrap().push(MockCall::MakeMove {
            from: from.to_string(),
            to: to.to_string(),
            promotion,
        });

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.make_move {
            f()
        } else {
            Err(ClientError::NotConfigured("make_move".to_string()))
        }
    }

    async fn get_legal_moves(
        &mut self,
        from_square: Option<String>,
    ) -> ClientResult<Vec<MoveDetail>> {
        self.call_log
            .lock()
            .unwrap()
            .push(MockCall::GetLegalMoves { from_square });

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.get_legal_moves {
            f()
        } else {
            Err(ClientError::NotConfigured("get_legal_moves".to_string()))
        }
    }

    async fn pause_session(&mut self) -> ClientResult<()> {
        self.call_log.lock().unwrap().push(MockCall::PauseSession);

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.pause_session {
            f()
        } else {
            Ok(())
        }
    }

    async fn resume_session(&mut self) -> ClientResult<()> {
        self.call_log.lock().unwrap().push(MockCall::ResumeSession);

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.resume_session {
            f()
        } else {
            Ok(())
        }
    }

    async fn set_engine(
        &mut self,
        enabled: bool,
        skill_level: u8,
        threads: u32,
        hash_mb: u32,
    ) -> ClientResult<()> {
        self.call_log.lock().unwrap().push(MockCall::SetEngine {
            enabled,
            skill_level,
            threads,
            hash_mb,
        });

        let responses = self.responses.lock().unwrap();
        if let Some(ref f) = responses.set_engine {
            f()
        } else {
            Ok(())
        }
    }

    async fn stream_session_events(
        &mut self,
    ) -> ClientResult<tonic::Streaming<SessionStreamEvent>> {
        Err(ClientError::NotConfigured(
            "stream_session_events".to_string(),
        ))
    }
}

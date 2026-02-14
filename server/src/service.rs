use crate::session::{SessionEvent, SessionInfo, SessionManager};
use ::chess::HistoryEntry;
use chess_proto::chess_service_server::ChessService;
use chess_proto::*;
use cozy_chess::{File as CozyFile, GameStatus as CozyGameStatus, Move, Piece, Rank, Square};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{Request, Response, Status};

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
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<chess_proto::SessionInfo>, Status> {
        let req = request.into_inner();
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
        self.session_manager
            .close_session(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        Ok(Response::new(Empty {}))
    }

    async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        let req = request.into_inner();
        let mv_repr = req
            .r#move
            .ok_or_else(|| Status::invalid_argument("Move is required"))?;

        let mv = parse_move_repr(&mv_repr)?;

        let (entry, status) = self
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

        let from_square = if let Some(ref sq_str) = req.from_square {
            Some(parse_square(sq_str)?)
        } else {
            None
        };

        let moves = self
            .session_manager
            .get_legal_moves(&req.session_id, from_square)
            .await
            .map_err(|e| Status::not_found(e))?;

        // Get session to access game state for move details
        let session = self
            .session_manager
            .get_session(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;
        let session_guard = session.read().await;

        let move_details: Vec<MoveDetail> = moves
            .into_iter()
            .map(|mv| {
                let san = format_move_san(&mv); // Simplified SAN
                let is_capture = false; // Would need board state to determine
                let is_check = false; // Would need to make move to determine
                let is_checkmate = false; // Would need to make move to determine

                MoveDetail {
                    from: format_square(mv.from),
                    to: format_square(mv.to),
                    promotion: mv.promotion.map(format_piece_lower),
                    san,
                    is_capture,
                    is_check,
                    is_checkmate,
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

    async fn set_engine(
        &self,
        request: Request<SetEngineRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        self.session_manager
            .set_engine(&req.session_id, req.enabled, req.skill_level as u8)
            .await
            .map_err(|e| Status::invalid_argument(e))?;

        Ok(Response::new(Empty {}))
    }

    async fn trigger_engine_move(
        &self,
        request: Request<TriggerEngineMoveRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
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
        self.session_manager
            .stop_engine(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        Ok(Response::new(Empty {}))
    }

    type StreamEventsStream = Pin<Box<dyn Stream<Item = Result<GameEvent, Status>> + Send>>;

    async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let req = request.into_inner();

        let mut event_rx = self
            .session_manager
            .subscribe_events(&req.session_id)
            .await
            .map_err(|e| Status::not_found(e))?;

        let stream = async_stream::stream! {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if let Some(proto_event) = convert_session_event_to_proto(event, &req.session_id) {
                            yield Ok(proto_event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("Client lagged, skipped {} events", skipped);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Event stream closed for session {}", req.session_id);
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

fn convert_session_info_to_proto(info: crate::session::SessionInfo) -> chess_proto::SessionInfo {
    chess_proto::SessionInfo {
        session_id: info.id,
        fen: info.fen,
        side_to_move: format_color(info.side_to_move),
        status: convert_game_status(info.status) as i32,
        move_count: info.move_count as u32,
        history: info
            .history
            .iter()
            .map(convert_history_entry_to_proto)
            .collect(),
        engine_config: if info.engine_enabled {
            Some(EngineConfig {
                enabled: info.engine_enabled,
                skill_level: info.skill_level as u32,
            })
        } else {
            None
        },
    }
}

fn convert_history_entry_to_proto(entry: &HistoryEntry) -> MoveRecord {
    MoveRecord {
        from: format_square(entry.from),
        to: format_square(entry.to),
        piece: format_piece(entry.piece),
        captured: entry.captured.map(format_piece),
        san: entry.san.clone(),
        fen_after: entry.fen.clone(),
        promotion: entry.promotion.map(format_piece_lower),
    }
}

fn convert_game_status(status: CozyGameStatus) -> GameStatus {
    match status {
        CozyGameStatus::Ongoing => GameStatus::Ongoing,
        CozyGameStatus::Won => GameStatus::Checkmate,
        CozyGameStatus::Drawn => GameStatus::Draw,
    }
}

fn convert_session_event_to_proto(event: SessionEvent, session_id: &str) -> Option<GameEvent> {
    match event {
        SessionEvent::MoveMade {
            from,
            to,
            san,
            fen,
            status,
        } => Some(GameEvent {
            event: Some(game_event::Event::MoveMade(MoveMadeEvent {
                session_id: session_id.to_string(),
                r#move: Some(MoveRecord {
                    from: format_square(from),
                    to: format_square(to),
                    piece: String::new(), // Would need to track
                    captured: None,
                    san,
                    fen_after: fen.clone(),
                    promotion: None,
                }),
                new_fen: fen,
                status: convert_game_status(status) as i32,
            })),
        }),
        SessionEvent::EngineMoveReady {
            best_move,
            evaluation,
        } => Some(GameEvent {
            event: Some(game_event::Event::EngineMoveReady(EngineMoveReadyEvent {
                session_id: session_id.to_string(),
                r#move: Some(MoveRepr {
                    from: format_square(best_move.from),
                    to: format_square(best_move.to),
                    promotion: best_move.promotion.map(format_piece_lower),
                }),
                evaluation,
            })),
        }),
        SessionEvent::EngineThinking { info } => Some(GameEvent {
            event: Some(game_event::Event::EngineThinking(EngineThinkingEvent {
                session_id: session_id.to_string(),
                info: Some(EngineInfo {
                    depth: info.depth.map(|d| d as u32),
                    seldepth: info.seldepth.map(|d| d as u32),
                    time_ms: info.time_ms,
                    nodes: info.nodes,
                    score: info.score.map(|s| format!("{:?}", s)),
                    pv: info.pv.iter().map(|mv| format_uci_move_str(*mv)).collect(),
                    nps: info.nps,
                }),
            })),
        }),
        SessionEvent::GameEnded { result, reason } => Some(GameEvent {
            event: Some(game_event::Event::GameEnded(GameEndedEvent {
                session_id: session_id.to_string(),
                result,
                reason,
            })),
        }),
        SessionEvent::Error { message } => Some(GameEvent {
            event: Some(game_event::Event::Error(ErrorEvent {
                session_id: session_id.to_string(),
                error_message: message,
            })),
        }),
        SessionEvent::UciMessage {
            direction,
            message,
            context,
        } => Some(GameEvent {
            event: Some(game_event::Event::UciMessage(UciMessageEvent {
                session_id: session_id.to_string(),
                direction: match direction {
                    crate::session::UciMessageDirection::ToEngine => UciDirection::ToEngine as i32,
                    crate::session::UciMessageDirection::FromEngine => {
                        UciDirection::FromEngine as i32
                    }
                },
                message,
                context,
            })),
        }),
    }
}

// ============================================================================
// Parsing Functions
// ============================================================================

fn parse_move_repr(mv: &MoveRepr) -> Result<Move, Status> {
    let from = parse_square(&mv.from)?;
    let to = parse_square(&mv.to)?;
    let promotion = if let Some(ref p) = mv.promotion {
        Some(parse_piece(p)?)
    } else {
        None
    };

    Ok(Move {
        from,
        to,
        promotion,
    })
}

fn parse_square(s: &str) -> Result<Square, Status> {
    if s.len() != 2 {
        return Err(Status::invalid_argument(format!("Invalid square: {}", s)));
    }

    let chars: Vec<char> = s.chars().collect();
    let file = parse_file(chars[0])?;
    let rank = parse_rank(chars[1])?;

    Ok(Square::new(file, rank))
}

fn parse_file(c: char) -> Result<CozyFile, Status> {
    match c {
        'a' => Ok(CozyFile::A),
        'b' => Ok(CozyFile::B),
        'c' => Ok(CozyFile::C),
        'd' => Ok(CozyFile::D),
        'e' => Ok(CozyFile::E),
        'f' => Ok(CozyFile::F),
        'g' => Ok(CozyFile::G),
        'h' => Ok(CozyFile::H),
        _ => Err(Status::invalid_argument(format!("Invalid file: {}", c))),
    }
}

fn parse_rank(c: char) -> Result<Rank, Status> {
    match c {
        '1' => Ok(Rank::First),
        '2' => Ok(Rank::Second),
        '3' => Ok(Rank::Third),
        '4' => Ok(Rank::Fourth),
        '5' => Ok(Rank::Fifth),
        '6' => Ok(Rank::Sixth),
        '7' => Ok(Rank::Seventh),
        '8' => Ok(Rank::Eighth),
        _ => Err(Status::invalid_argument(format!("Invalid rank: {}", c))),
    }
}

fn parse_piece(s: &str) -> Result<Piece, Status> {
    match s.to_lowercase().as_str() {
        "q" => Ok(Piece::Queen),
        "r" => Ok(Piece::Rook),
        "b" => Ok(Piece::Bishop),
        "n" => Ok(Piece::Knight),
        _ => Err(Status::invalid_argument(format!("Invalid piece: {}", s))),
    }
}

// ============================================================================
// Formatting Functions
// ============================================================================

fn format_square(sq: Square) -> String {
    let file = match sq.file() {
        CozyFile::A => 'a',
        CozyFile::B => 'b',
        CozyFile::C => 'c',
        CozyFile::D => 'd',
        CozyFile::E => 'e',
        CozyFile::F => 'f',
        CozyFile::G => 'g',
        CozyFile::H => 'h',
    };
    let rank = match sq.rank() {
        Rank::First => '1',
        Rank::Second => '2',
        Rank::Third => '3',
        Rank::Fourth => '4',
        Rank::Fifth => '5',
        Rank::Sixth => '6',
        Rank::Seventh => '7',
        Rank::Eighth => '8',
    };
    format!("{}{}", file, rank)
}

fn format_piece(piece: Piece) -> String {
    match piece {
        Piece::Pawn => "P".to_string(),
        Piece::Knight => "N".to_string(),
        Piece::Bishop => "B".to_string(),
        Piece::Rook => "R".to_string(),
        Piece::Queen => "Q".to_string(),
        Piece::King => "K".to_string(),
    }
}

fn format_piece_lower(piece: Piece) -> String {
    match piece {
        Piece::Pawn => "p".to_string(),
        Piece::Knight => "n".to_string(),
        Piece::Bishop => "b".to_string(),
        Piece::Rook => "r".to_string(),
        Piece::Queen => "q".to_string(),
        Piece::King => "k".to_string(),
    }
}

fn format_color(color: cozy_chess::Color) -> String {
    match color {
        cozy_chess::Color::White => "white".to_string(),
        cozy_chess::Color::Black => "black".to_string(),
    }
}

fn format_move_san(mv: &Move) -> String {
    // Simplified SAN format (just from-to notation)
    // A full implementation would need game context
    format!("{}{}", format_square(mv.from), format_square(mv.to))
}

fn format_uci_move_str(mv: Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

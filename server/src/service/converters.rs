//! Conversion functions from domain types to protobuf types

use crate::session::{SessionEvent, SessionInfo, UciMessageDirection};
use ::chess::HistoryEntry;
use chess_common::{format_color, format_piece, format_piece_upper, format_square, format_uci_move};
use chess_proto::*;
use cozy_chess::GameStatus as CozyGameStatus;

pub fn convert_session_info_to_proto(info: SessionInfo) -> chess_proto::SessionInfo {
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
                threads: 0,  // Not tracked per-session currently
                hash_mb: 0,
            })
        } else {
            None
        },
    }
}

pub fn convert_history_entry_to_proto(entry: &HistoryEntry) -> MoveRecord {
    MoveRecord {
        from: format_square(entry.from),
        to: format_square(entry.to),
        piece: format_piece_upper(entry.piece).to_string(),
        captured: entry.captured.map(|p| format_piece_upper(p).to_string()),
        san: entry.san.clone(),
        fen_after: entry.fen.clone(),
        promotion: entry.promotion.map(|p| format_piece(p).to_string()),
    }
}

pub fn convert_game_status(status: CozyGameStatus) -> GameStatus {
    match status {
        CozyGameStatus::Ongoing => GameStatus::Ongoing,
        CozyGameStatus::Won => GameStatus::Won,
        CozyGameStatus::Drawn => GameStatus::Drawn,
    }
}

pub fn convert_session_event_to_proto(event: SessionEvent, session_id: &str) -> Option<GameEvent> {
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
                    promotion: best_move.promotion.map(|p| format_piece(p).to_string()),
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
                    pv: info.pv.iter().map(|mv| format_uci_move(*mv)).collect(),
                    nps: info.nps,
                }),
            })),
        }),
        SessionEvent::GameEnded { result, reason } => {
            // Determine winner from result
            let winner = if result == "1-0" {
                "white".to_string()
            } else if result == "0-1" {
                "black".to_string()
            } else {
                String::new() // Draw
            };

            Some(GameEvent {
                event: Some(game_event::Event::GameEnded(GameEndedEvent {
                    session_id: session_id.to_string(),
                    result,
                    reason,
                    winner,
                })),
            })
        }
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
                    UciMessageDirection::ToEngine => UciDirection::ToEngine as i32,
                    UciMessageDirection::FromEngine => UciDirection::FromEngine as i32,
                },
                message,
                context,
            })),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::{Color, Square, File, Rank};

    #[test]
    fn test_convert_game_status() {
        assert_eq!(convert_game_status(CozyGameStatus::Ongoing), GameStatus::Ongoing);
        assert_eq!(convert_game_status(CozyGameStatus::Won), GameStatus::Won);
        assert_eq!(convert_game_status(CozyGameStatus::Drawn), GameStatus::Drawn);
    }

    #[test]
    fn test_convert_session_info_basic() {
        let info = SessionInfo {
            id: "test123".to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            side_to_move: Color::White,
            status: CozyGameStatus::Ongoing,
            move_count: 0,
            history: vec![],
            engine_enabled: false,
            skill_level: 10,
        };

        let proto = convert_session_info_to_proto(info);
        assert_eq!(proto.session_id, "test123");
        assert_eq!(proto.side_to_move, "white");
        assert_eq!(proto.move_count, 0);
        assert!(proto.engine_config.is_none());
    }
}

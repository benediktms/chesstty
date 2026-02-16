//! Conversion functions from domain types to protobuf types

use crate::session::commands::EngineConfig;
use crate::session::snapshot::MoveRecord;
use crate::session::{SessionEvent, SessionSnapshot, TimerSnapshot, UciDirection};
use ::chess::{AnalysisScore, EngineAnalysis, GameMode, GamePhase, PlayerSide};
use chess_proto::*;
use cozy_chess::GameStatus as CozyGameStatus;

/// Convert a domain SessionSnapshot into the proto SessionSnapshot.
pub fn convert_snapshot_to_proto(snap: SessionSnapshot) -> chess_proto::SessionSnapshot {
    chess_proto::SessionSnapshot {
        session_id: snap.session_id,
        fen: snap.fen,
        side_to_move: snap.side_to_move,
        phase: convert_game_phase_to_proto(&snap.phase) as i32,
        status: convert_game_status(snap.status) as i32,
        move_count: snap.move_count as u32,
        history: snap
            .history
            .iter()
            .map(convert_move_record_to_proto)
            .collect(),
        last_move: snap.last_move.map(|(from, to)| LastMove { from, to }),
        analysis: snap.analysis.as_ref().map(convert_engine_analysis_to_proto),
        engine_config: snap
            .engine_config
            .as_ref()
            .map(convert_engine_config_to_proto),
        game_mode: Some(convert_game_mode_to_proto(&snap.game_mode)),
        engine_thinking: snap.engine_thinking,
        timer: snap.timer.as_ref().map(convert_timer_to_proto),
    }
}

/// Convert a domain MoveRecord to the proto MoveRecord.
pub fn convert_move_record_to_proto(record: &MoveRecord) -> chess_proto::MoveRecord {
    chess_proto::MoveRecord {
        from: record.from.clone(),
        to: record.to.clone(),
        piece: record.piece.clone(),
        captured: record.captured.clone(),
        san: record.san.clone(),
        fen_after: record.fen_after.clone(),
        promotion: record.promotion.clone(),
    }
}

/// Convert a cozy_chess GameStatus to the proto GameStatus enum.
pub fn convert_game_status(status: CozyGameStatus) -> GameStatus {
    match status {
        CozyGameStatus::Ongoing => GameStatus::Ongoing,
        CozyGameStatus::Won => GameStatus::Won,
        CozyGameStatus::Drawn => GameStatus::Drawn,
    }
}

/// Convert the domain GamePhase to the proto GamePhase enum.
pub fn convert_game_phase_to_proto(phase: &GamePhase) -> chess_proto::GamePhase {
    match phase {
        GamePhase::Setup => chess_proto::GamePhase::Setup,
        GamePhase::Playing { .. } => chess_proto::GamePhase::Playing,
        GamePhase::Paused { .. } => chess_proto::GamePhase::Paused,
        GamePhase::Ended { .. } => chess_proto::GamePhase::Ended,
        GamePhase::Analyzing => chess_proto::GamePhase::Analyzing,
    }
}

/// Convert the domain GameMode to the proto GameModeProto message.
pub fn convert_game_mode_to_proto(mode: &GameMode) -> GameModeProto {
    match mode {
        GameMode::HumanVsHuman => GameModeProto {
            mode: GameModeType::HumanVsHuman as i32,
            human_side: None,
        },
        GameMode::HumanVsEngine { human_side } => GameModeProto {
            mode: GameModeType::HumanVsEngine as i32,
            human_side: Some(match human_side {
                PlayerSide::White => PlayerSideProto::White as i32,
                PlayerSide::Black => PlayerSideProto::Black as i32,
            }),
        },
        GameMode::EngineVsEngine => GameModeProto {
            mode: GameModeType::EngineVsEngine as i32,
            human_side: None,
        },
        GameMode::Analysis => GameModeProto {
            mode: GameModeType::Analysis as i32,
            human_side: None,
        },
        GameMode::Review => GameModeProto {
            mode: GameModeType::Review as i32,
            human_side: None,
        },
    }
}

/// Convert the domain EngineAnalysis to the proto EngineAnalysis message.
pub fn convert_engine_analysis_to_proto(analysis: &EngineAnalysis) -> chess_proto::EngineAnalysis {
    chess_proto::EngineAnalysis {
        depth: analysis.depth,
        seldepth: analysis.seldepth,
        time_ms: analysis.time_ms,
        nodes: analysis.nodes,
        score: analysis.score.as_ref().map(|s| match s {
            AnalysisScore::Centipawns(cp) => format!("cp {}", cp),
            AnalysisScore::Mate(m) => format!("mate {}", m),
        }),
        pv: analysis.pv.clone(),
        nps: analysis.nps,
    }
}

/// Convert the domain EngineConfig to the proto EngineConfig message.
pub fn convert_engine_config_to_proto(config: &EngineConfig) -> chess_proto::EngineConfig {
    chess_proto::EngineConfig {
        enabled: config.enabled,
        skill_level: config.skill_level as u32,
        threads: config.threads.unwrap_or(0),
        hash_mb: config.hash_mb.unwrap_or(0),
    }
}

/// Convert the domain TimerSnapshot to the proto TimerState message.
pub fn convert_timer_to_proto(timer: &TimerSnapshot) -> chess_proto::TimerState {
    chess_proto::TimerState {
        white_remaining_ms: timer.white_remaining_ms,
        black_remaining_ms: timer.black_remaining_ms,
        active_side: timer.active_side.clone(),
    }
}

/// Convert a domain SessionEvent into a proto SessionStreamEvent.
pub fn convert_session_event_to_proto(event: SessionEvent, session_id: &str) -> SessionStreamEvent {
    let session_id = session_id.to_string();
    match event {
        SessionEvent::StateChanged(snapshot) => SessionStreamEvent {
            session_id,
            event: Some(session_stream_event::Event::StateChanged(
                convert_snapshot_to_proto(snapshot),
            )),
        },
        SessionEvent::EngineThinking(analysis) => SessionStreamEvent {
            session_id,
            event: Some(session_stream_event::Event::EngineThinking(
                convert_engine_analysis_to_proto(&analysis),
            )),
        },
        SessionEvent::UciMessage(entry) => SessionStreamEvent {
            session_id,
            event: Some(session_stream_event::Event::UciMessage(UciMessageEvent {
                session_id: String::new(),
                direction: match entry.direction {
                    UciDirection::ToEngine => chess_proto::UciDirection::ToEngine as i32,
                    UciDirection::FromEngine => chess_proto::UciDirection::FromEngine as i32,
                },
                message: entry.message,
                context: entry.context,
            })),
        },
        SessionEvent::Error(message) => SessionStreamEvent {
            session_id,
            event: Some(session_stream_event::Event::Error(message)),
        },
    }
}

/// Parse a proto GameModeProto into a domain GameMode.
/// Defaults to HumanVsHuman when the mode value is unrecognized.
pub fn parse_game_mode_from_proto(proto: &GameModeProto) -> GameMode {
    match GameModeType::try_from(proto.mode) {
        Ok(GameModeType::HumanVsEngine) => {
            let human_side = match proto
                .human_side
                .and_then(|v| PlayerSideProto::try_from(v).ok())
            {
                Some(PlayerSideProto::Black) => PlayerSide::Black,
                _ => PlayerSide::White,
            };
            GameMode::HumanVsEngine { human_side }
        }
        Ok(GameModeType::EngineVsEngine) => GameMode::EngineVsEngine,
        Ok(GameModeType::Analysis) => GameMode::Analysis,
        Ok(GameModeType::Review) => GameMode::Review,
        Ok(GameModeType::HumanVsHuman) | Err(_) => GameMode::HumanVsHuman,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::chess::GamePhase;

    #[test]
    fn test_convert_game_status() {
        assert_eq!(
            convert_game_status(CozyGameStatus::Ongoing),
            GameStatus::Ongoing
        );
        assert_eq!(convert_game_status(CozyGameStatus::Won), GameStatus::Won);
        assert_eq!(
            convert_game_status(CozyGameStatus::Drawn),
            GameStatus::Drawn
        );
    }

    #[test]
    fn test_convert_game_phase() {
        assert_eq!(
            convert_game_phase_to_proto(&GamePhase::Setup),
            chess_proto::GamePhase::Setup
        );
        assert_eq!(
            convert_game_phase_to_proto(&GamePhase::Playing {
                turn: cozy_chess::Color::White
            }),
            chess_proto::GamePhase::Playing
        );
        assert_eq!(
            convert_game_phase_to_proto(&GamePhase::Paused {
                resume_turn: cozy_chess::Color::White
            }),
            chess_proto::GamePhase::Paused
        );
        assert_eq!(
            convert_game_phase_to_proto(&GamePhase::Analyzing),
            chess_proto::GamePhase::Analyzing
        );
    }

    #[test]
    fn test_convert_game_mode_human_vs_human() {
        let proto = convert_game_mode_to_proto(&GameMode::HumanVsHuman);
        assert_eq!(proto.mode, GameModeType::HumanVsHuman as i32);
        assert!(proto.human_side.is_none());
    }

    #[test]
    fn test_convert_game_mode_human_vs_engine() {
        let proto = convert_game_mode_to_proto(&GameMode::HumanVsEngine {
            human_side: PlayerSide::Black,
        });
        assert_eq!(proto.mode, GameModeType::HumanVsEngine as i32);
        assert_eq!(proto.human_side, Some(PlayerSideProto::Black as i32));
    }

    #[test]
    fn test_parse_game_mode_roundtrip() {
        let modes = vec![
            GameMode::HumanVsHuman,
            GameMode::HumanVsEngine {
                human_side: PlayerSide::White,
            },
            GameMode::HumanVsEngine {
                human_side: PlayerSide::Black,
            },
            GameMode::EngineVsEngine,
            GameMode::Analysis,
            GameMode::Review,
        ];
        for mode in modes {
            let proto = convert_game_mode_to_proto(&mode);
            let parsed = parse_game_mode_from_proto(&proto);
            assert_eq!(format!("{:?}", mode), format!("{:?}", parsed));
        }
    }

    #[test]
    fn test_parse_game_mode_unknown_defaults() {
        let proto = GameModeProto {
            mode: 99, // unrecognized enum value
            human_side: None,
        };
        let mode = parse_game_mode_from_proto(&proto);
        assert!(matches!(mode, GameMode::HumanVsHuman));
    }
}

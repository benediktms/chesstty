use crate::{EngineInfo, Score};
use cozy_chess::{File, Move, Piece, Rank, Square};

/// Incoming message from UCI engine
#[derive(Debug, Clone)]
pub enum UciMessage {
    Id { name: String, value: String },
    UciOk,
    ReadyOk,
    BestMove { mv: Move, ponder: Option<Move> },
    Info(EngineInfo),
}

/// Parse a UCI message line
pub fn parse_uci_message(line: &str) -> Result<UciMessage, crate::UciError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    match tokens.first() {
        Some(&"uciok") => Ok(UciMessage::UciOk),
        Some(&"readyok") => Ok(UciMessage::ReadyOk),

        Some(&"id") => {
            if tokens.len() < 3 {
                return Err(crate::UciError::MalformedMessage(line.to_string()));
            }
            let name = tokens[1].to_string();
            let value = tokens[2..].join(" ");
            Ok(UciMessage::Id { name, value })
        }

        Some(&"bestmove") => {
            if tokens.len() < 2 {
                return Err(crate::UciError::MalformedMessage(line.to_string()));
            }
            let mv = parse_uci_move(tokens[1])?;
            let ponder = if tokens.len() >= 4 && tokens[2] == "ponder" {
                Some(parse_uci_move(tokens[3])?)
            } else {
                None
            };
            Ok(UciMessage::BestMove { mv, ponder })
        }

        Some(&"info") => Ok(UciMessage::Info(parse_info_line(&tokens[1..])?)),

        _ => Err(crate::UciError::UnknownMessage(line.to_string())),
    }
}

/// Parse an "info" line from the engine
fn parse_info_line(tokens: &[&str]) -> Result<EngineInfo, crate::UciError> {
    let mut info = EngineInfo::default();
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                i += 1;
                info.depth = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "seldepth" => {
                i += 1;
                info.seldepth = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "time" => {
                i += 1;
                info.time_ms = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "nodes" => {
                i += 1;
                info.nodes = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "nps" => {
                i += 1;
                info.nps = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "score" => {
                i += 1;
                if let Some(&score_type) = tokens.get(i) {
                    i += 1;
                    if let Some(value_str) = tokens.get(i) {
                        info.score = match score_type {
                            "cp" => value_str.parse().ok().map(Score::Centipawns),
                            "mate" => value_str.parse().ok().map(Score::Mate),
                            _ => None,
                        };
                    }
                }
            }
            "pv" => {
                // Collect all moves until next keyword
                i += 1;
                while i < tokens.len() && !is_keyword(tokens[i]) {
                    if let Ok(mv) = parse_uci_move(tokens[i]) {
                        info.pv.push(mv);
                    }
                    i += 1;
                }
                continue; // Don't increment i again
            }
            "multipv" => {
                i += 1;
                info.multipv = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "currmove" => {
                i += 1;
                info.currmove = tokens.get(i).and_then(|s| parse_uci_move(s).ok());
            }
            "hashfull" => {
                i += 1;
                info.hashfull = tokens.get(i).and_then(|s| s.parse().ok());
            }
            _ => {
                // Unknown keyword, skip
            }
        }
        i += 1;
    }

    Ok(info)
}

fn is_keyword(token: &str) -> bool {
    matches!(
        token,
        "depth"
            | "seldepth"
            | "time"
            | "nodes"
            | "score"
            | "pv"
            | "multipv"
            | "currmove"
            | "hashfull"
            | "nps"
            | "tbhits"
            | "cpuload"
            | "string"
    )
}

/// Parse UCI move format (e2e4, e7e8q)
pub fn parse_uci_move(s: &str) -> Result<Move, crate::UciError> {
    if s.len() < 4 {
        return Err(crate::UciError::InvalidMove(s.to_string()));
    }

    let from = parse_square(&s[0..2])?;
    let to = parse_square(&s[2..4])?;

    let promotion = if s.len() == 5 {
        Some(match &s[4..5] {
            "q" => Piece::Queen,
            "r" => Piece::Rook,
            "b" => Piece::Bishop,
            "n" => Piece::Knight,
            _ => return Err(crate::UciError::InvalidPromotion(s.to_string())),
        })
    } else {
        None
    };

    Ok(Move {
        from,
        to,
        promotion,
    })
}

fn parse_square(s: &str) -> Result<Square, crate::UciError> {
    if s.len() != 2 {
        return Err(crate::UciError::InvalidSquare(s.to_string()));
    }

    let file = match s.chars().next().unwrap() {
        'a' => File::A,
        'b' => File::B,
        'c' => File::C,
        'd' => File::D,
        'e' => File::E,
        'f' => File::F,
        'g' => File::G,
        'h' => File::H,
        _ => return Err(crate::UciError::InvalidSquare(s.to_string())),
    };

    let rank = match s.chars().nth(1).unwrap() {
        '1' => Rank::First,
        '2' => Rank::Second,
        '3' => Rank::Third,
        '4' => Rank::Fourth,
        '5' => Rank::Fifth,
        '6' => Rank::Sixth,
        '7' => Rank::Seventh,
        '8' => Rank::Eighth,
        _ => return Err(crate::UciError::InvalidSquare(s.to_string())),
    };

    Ok(Square::new(file, rank))
}

/// Format move for UCI (cozy-chess Move → "e2e4")
pub fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => unreachable!(),
        });
    }
    s
}

fn format_square(sq: Square) -> String {
    let file = match sq.file() {
        File::A => 'a',
        File::B => 'b',
        File::C => 'c',
        File::D => 'd',
        File::E => 'e',
        File::F => 'f',
        File::G => 'g',
        File::H => 'h',
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bestmove() {
        let msg = parse_uci_message("bestmove e2e4 ponder e7e5").unwrap();
        match msg {
            UciMessage::BestMove { mv, ponder } => {
                assert_eq!(format_uci_move(&mv), "e2e4");
                assert_eq!(format_uci_move(&ponder.unwrap()), "e7e5");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_parse_info() {
        let msg = parse_uci_message("info depth 12 score cp 35 nodes 15234 pv e2e4 e7e5").unwrap();
        match msg {
            UciMessage::Info(info) => {
                assert_eq!(info.depth, Some(12));
                assert!(matches!(info.score, Some(Score::Centipawns(35))));
                assert_eq!(info.nodes, Some(15234));
                assert_eq!(info.pv.len(), 2);
            }
            _ => panic!("Wrong message type"),
        }
    }
}

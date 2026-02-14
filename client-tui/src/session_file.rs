use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A serializable move record for session persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedMove {
    pub from: String,
    pub to: String,
    pub piece: String,
    pub captured: Option<String>,
    pub san: String,
    pub promotion: Option<String>,
}

/// Represents a saved/suspended game session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedSession {
    pub fen: String,
    pub history: Vec<SavedMove>,
    pub game_mode: String,
    pub skill_level: u8,
    pub human_side: Option<String>,
    pub timestamp: String,
}

/// Get the path to the saved session file.
pub fn session_file_path() -> PathBuf {
    session_file_path_in(default_session_dir())
}

fn default_session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".chesstty")
}

fn session_file_path_in(dir: PathBuf) -> PathBuf {
    dir.join("suspended_session.json")
}

/// Save a session to disk.
pub fn save_session(session: &SavedSession) -> Result<PathBuf, String> {
    save_session_to(session, default_session_dir())
}

fn save_session_to(session: &SavedSession, dir: PathBuf) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create directory: {}", e))?;

    let path = session_file_path_in(dir);
    let json = serde_json::to_string_pretty(session)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;

    std::fs::write(&path, json).map_err(|e| format!("Failed to write session file: {}", e))?;

    Ok(path)
}

/// Load a saved session from disk, if one exists.
pub fn load_session() -> Result<Option<SavedSession>, String> {
    load_session_from(default_session_dir())
}

fn load_session_from(dir: PathBuf) -> Result<Option<SavedSession>, String> {
    let path = session_file_path_in(dir);
    if !path.exists() {
        return Ok(None);
    }

    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read session file: {}", e))?;

    let session: SavedSession = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse session file: {}", e))?;

    Ok(Some(session))
}

/// Delete the saved session file.
pub fn clear_saved_session() -> Result<(), String> {
    clear_saved_session_in(default_session_dir())
}

fn clear_saved_session_in(dir: PathBuf) -> Result<(), String> {
    let path = session_file_path_in(dir);
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove session file: {}", e))?;
    }
    Ok(())
}

/// Build a SavedSession from the current client state.
pub fn build_saved_session(
    fen: &str,
    history: &[chess_proto::MoveRecord],
    mode: &crate::state::GameMode,
    skill_level: u8,
) -> SavedSession {
    use crate::state::{GameMode, PlayerColor};

    let (game_mode_str, human_side) = match mode {
        GameMode::HumanVsHuman => ("HumanVsHuman".to_string(), None),
        GameMode::HumanVsEngine { human_side } => {
            let side = match human_side {
                PlayerColor::White => "white",
                PlayerColor::Black => "black",
            };
            ("HumanVsEngine".to_string(), Some(side.to_string()))
        }
        GameMode::EngineVsEngine => ("EngineVsEngine".to_string(), None),
        GameMode::AnalysisMode => ("AnalysisMode".to_string(), None),
        GameMode::ReviewMode => ("ReviewMode".to_string(), None),
    };

    let saved_history: Vec<SavedMove> = history
        .iter()
        .map(|m| SavedMove {
            from: m.from.clone(),
            to: m.to.clone(),
            piece: m.piece.clone(),
            captured: m.captured.clone().filter(|s| !s.is_empty()),
            san: m.san.clone(),
            promotion: m.promotion.clone().filter(|s| !s.is_empty()),
        })
        .collect();

    let timestamp = {
        use std::time::SystemTime;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}", now.as_secs())
    };

    SavedSession {
        fen: fen.to_string(),
        history: saved_history,
        game_mode: game_mode_str,
        skill_level,
        human_side,
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> SavedSession {
        SavedSession {
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            history: vec![SavedMove {
                from: "e2".to_string(),
                to: "e4".to_string(),
                piece: "P".to_string(),
                captured: None,
                san: "e4".to_string(),
                promotion: None,
            }],
            game_mode: "HumanVsEngine".to_string(),
            skill_level: 10,
            human_side: Some("white".to_string()),
            timestamp: "1234567890".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let session = sample_session();

        let path = save_session_to(&session, dir.path().to_path_buf()).unwrap();
        assert!(path.exists());

        let loaded = load_session_from(dir.path().to_path_buf()).unwrap();
        assert_eq!(loaded, Some(session));
    }

    #[test]
    fn test_load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_session_from(dir.path().to_path_buf()).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_clear_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let session = sample_session();

        save_session_to(&session, dir.path().to_path_buf()).unwrap();
        clear_saved_session_in(dir.path().to_path_buf()).unwrap();

        let loaded = load_session_from(dir.path().to_path_buf()).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_saved_session_serialization() {
        let session = sample_session();
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("HumanVsEngine"));
        assert!(json.contains("e4"));
        assert!(json.contains("white"));
    }

    #[test]
    fn test_save_with_history() {
        let dir = tempfile::tempdir().unwrap();
        let session = SavedSession {
            fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2".to_string(),
            history: vec![
                SavedMove {
                    from: "e2".to_string(),
                    to: "e4".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    san: "e4".to_string(),
                    promotion: None,
                },
                SavedMove {
                    from: "e7".to_string(),
                    to: "e5".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    san: "e5".to_string(),
                    promotion: None,
                },
            ],
            game_mode: "HumanVsHuman".to_string(),
            skill_level: 10,
            human_side: None,
            timestamp: "1234567890".to_string(),
        };

        save_session_to(&session, dir.path().to_path_buf()).unwrap();
        let loaded = load_session_from(dir.path().to_path_buf()).unwrap().unwrap();
        assert_eq!(loaded.history.len(), 2);
        assert_eq!(loaded.history[1].san, "e5");
    }
}

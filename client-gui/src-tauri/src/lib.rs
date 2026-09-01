use std::path::PathBuf;

use chess_client::{
    session_stream_event, ChessClient, ClientError, GameModeProto, GameModeType, MoveDetail,
    MoveRecord, PlayerSideProto, SessionSnapshot,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

struct ActiveGame {
    session_id: String,
    client: ChessClient,
}

#[derive(Default)]
struct AppState {
    active_game: Mutex<Option<ActiveGame>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGameOptions {
    mode: String,
    human_side: String,
    skill_level: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameState {
    snapshot: SnapshotView,
    legal_moves: Vec<LegalMoveView>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotView {
    session_id: String,
    fen: String,
    side_to_move: String,
    phase: i32,
    status: i32,
    move_count: u32,
    history: Vec<MoveView>,
    last_move: Option<LastMoveView>,
    game_mode: i32,
    human_side: Option<String>,
    engine_thinking: bool,
    skill_level: Option<u32>,
    start_fen: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveView {
    from: String,
    to: String,
    piece: String,
    captured: Option<String>,
    san: String,
    promotion: Option<String>,
}

#[derive(Clone, Serialize)]
struct LastMoveView {
    from: String,
    to: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegalMoveView {
    from: String,
    to: String,
    promotion: Option<String>,
    is_capture: bool,
}

impl From<MoveRecord> for MoveView {
    fn from(record: MoveRecord) -> Self {
        Self {
            from: record.from,
            to: record.to,
            piece: record.piece,
            captured: record.captured,
            san: record.san,
            promotion: record.promotion,
        }
    }
}

impl From<MoveDetail> for LegalMoveView {
    fn from(mv: MoveDetail) -> Self {
        Self {
            from: mv.from,
            to: mv.to,
            promotion: mv.promotion,
            is_capture: mv.is_capture,
        }
    }
}

fn snapshot_view(snapshot: SessionSnapshot) -> SnapshotView {
    let game_mode = snapshot.game_mode.as_ref().map_or(0, |mode| mode.mode);
    let human_side = snapshot
        .game_mode
        .as_ref()
        .and_then(|mode| mode.human_side)
        .and_then(|side| PlayerSideProto::try_from(side).ok())
        .map(|side| match side {
            PlayerSideProto::White => "white".to_string(),
            PlayerSideProto::Black => "black".to_string(),
        });
    let skill_level = snapshot
        .engine_config
        .as_ref()
        .map(|config| config.skill_level);

    SnapshotView {
        session_id: snapshot.session_id,
        fen: snapshot.fen,
        side_to_move: snapshot.side_to_move,
        phase: snapshot.phase,
        status: snapshot.status,
        move_count: snapshot.move_count,
        history: snapshot.history.into_iter().map(MoveView::from).collect(),
        last_move: snapshot.last_move.map(|mv| LastMoveView {
            from: mv.from,
            to: mv.to,
        }),
        game_mode,
        human_side,
        engine_thinking: snapshot.engine_thinking,
        skill_level,
        start_fen: snapshot.start_fen,
    }
}

fn game_state(snapshot: SessionSnapshot, legal_moves: Vec<MoveDetail>) -> GameState {
    GameState {
        snapshot: snapshot_view(snapshot),
        legal_moves: legal_moves.into_iter().map(LegalMoveView::from).collect(),
    }
}

fn socket_path() -> PathBuf {
    std::env::var_os("CHESSTTY_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/chesstty.sock"))
}

fn game_mode(options: &NewGameOptions) -> Result<GameModeProto, String> {
    match options.mode.as_str() {
        "human-vs-human" => Ok(GameModeProto {
            mode: GameModeType::HumanVsHuman as i32,
            human_side: None,
        }),
        "human-vs-engine" => {
            let human_side = match options.human_side.as_str() {
                "white" => PlayerSideProto::White,
                "black" => PlayerSideProto::Black,
                _ => return Err("Human side must be white or black".to_string()),
            };
            Ok(GameModeProto {
                mode: GameModeType::HumanVsEngine as i32,
                human_side: Some(human_side as i32),
            })
        }
        _ => Err("Unsupported game mode".to_string()),
    }
}

fn friendly_message(message: &str) -> String {
    if message.starts_with("Session not found") {
        "This game session is no longer available. Start a new game.".to_string()
    } else {
        message.to_string()
    }
}

fn client_error(error: ClientError) -> String {
    match error {
        ClientError::RpcError(status) => friendly_message(status.message()),
        ClientError::ConnectionFailed(_) => {
            "Could not connect to the ChessTTY server. Start the server and try again.".to_string()
        }
        error => error.to_string(),
    }
}

#[tauri::command]
async fn new_game(
    app: AppHandle,
    state: State<'_, AppState>,
    options: NewGameOptions,
) -> Result<GameState, String> {
    if options.skill_level > 20 {
        return Err("Bot strength must be between 0 and 20".to_string());
    }

    if let Some(mut previous) = state.active_game.lock().await.take() {
        let _ = previous.client.close_session().await;
    }

    let mode = game_mode(&options)?;
    let uses_engine = options.mode == "human-vs-engine";
    let mut client = ChessClient::connect_uds(&socket_path())
        .await
        .map_err(client_error)?;
    let snapshot = client
        .create_session(None, Some(mode), None)
        .await
        .map_err(client_error)?;
    let session_id = snapshot.session_id.clone();
    let mut events = client.stream_events().await.map_err(client_error)?;

    if uses_engine {
        client
            .set_engine(true, options.skill_level, None, None)
            .await
            .map_err(client_error)?;
    }

    let legal_moves = client.get_legal_moves(None).await.map_err(client_error)?;
    *state.active_game.lock().await = Some(ActiveGame {
        session_id: session_id.clone(),
        client,
    });

    tauri::async_runtime::spawn(async move {
        loop {
            let event = match events.message().await {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    let state = app.state::<AppState>();
                    let active = state.active_game.lock().await;
                    if active
                        .as_ref()
                        .is_some_and(|game| game.session_id == session_id)
                    {
                        let _ = app.emit("game-error", friendly_message(error.message()));
                    }
                    break;
                }
            };
            let Some(session_stream_event::Event::StateChanged(snapshot)) = event.event else {
                continue;
            };

            let state = app.state::<AppState>();
            let mut active = state.active_game.lock().await;
            let Some(active) = active.as_mut().filter(|game| game.session_id == session_id) else {
                break;
            };
            match active.client.get_legal_moves(None).await {
                Ok(moves) => {
                    let _ = app.emit("game-state", game_state(snapshot, moves));
                }
                Err(error) => {
                    let _ = app.emit("game-error", client_error(error));
                }
            }
        }
    });

    Ok(game_state(snapshot, legal_moves))
}

#[tauri::command]
async fn make_move(
    state: State<'_, AppState>,
    from: String,
    to: String,
    promotion: Option<String>,
) -> Result<GameState, String> {
    let mut active = state.active_game.lock().await;
    let active = active
        .as_mut()
        .ok_or_else(|| "Start a game first".to_string())?;
    let snapshot = active
        .client
        .make_move(&from, &to, promotion)
        .await
        .map_err(client_error)?;
    let legal_moves = active
        .client
        .get_legal_moves(None)
        .await
        .map_err(client_error)?;
    Ok(game_state(snapshot, legal_moves))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![new_game, make_move])
        .run(tauri::generate_context!())
        .expect("error while running ChessTTY");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_game_options() {
        assert!(game_mode(&NewGameOptions {
            mode: "correspondence".to_string(),
            human_side: "white".to_string(),
            skill_level: 10,
        })
        .is_err());
    }

    #[test]
    fn hides_rpc_metadata_for_missing_sessions() {
        assert_eq!(
            friendly_message("Session not found: deadbeef"),
            "This game session is no longer available. Start a new game."
        );
    }
}

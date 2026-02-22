# chess-client - gRPC Client Library

A reusable async Rust client for communicating with the ChessTTY server. Wraps the raw tonic gRPC client into a high-level API with session tracking and typed error handling.

## Usage

```rust
use chess_client::ChessClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ChessClient::connect("http://[::1]:50051").await?;

    // Create a session
    let snapshot = client.create_session(None, None, None).await?;
    println!("Session: {}", snapshot.session_id);

    // Make a move
    let snapshot = client.make_move("e2", "e4", None).await?;
    println!("FEN: {}", snapshot.fen);

    // Subscribe to events
    let mut stream = client.stream_events().await?;
    // stream.next().await for each SessionStreamEvent

    Ok(())
}
```

## API Overview

`ChessClient` tracks the active `session_id` internally. Most methods require an active session.

### Connection

| Method | Description |
|--------|-------------|
| `connect(addr)` | Connect to a server address (e.g., `"http://[::1]:50051"`) |

### Session Lifecycle

| Method | Returns | Description |
|--------|---------|-------------|
| `create_session(fen?, mode?, timer?)` | `SessionSnapshot` | Create a new game session, sets active session |
| `get_session()` | `SessionSnapshot` | Get current session state |
| `close_session()` | `()` | Close the active session |

### Game Actions

| Method | Returns | Description |
|--------|---------|-------------|
| `make_move(from, to, promotion?)` | `SessionSnapshot` | Make a move (e.g., `"e2"`, `"e4"`) |
| `get_legal_moves(from_square?)` | `Vec<MoveDetail>` | Get legal moves, optionally filtered by source square |
| `undo_move()` | `SessionSnapshot` | Undo the last move |
| `redo_move()` | `SessionSnapshot` | Redo a previously undone move |
| `reset_game(fen?)` | `SessionSnapshot` | Reset to start or a custom FEN |

### Engine Control

| Method | Returns | Description |
|--------|---------|-------------|
| `set_engine(enabled, skill, threads?, hash?)` | `()` | Configure or disable the engine |
| `pause()` | `()` | Pause the game (stops engine, pauses timer) |
| `resume()` | `()` | Resume a paused game |

### Event Streaming

| Method | Returns | Description |
|--------|---------|-------------|
| `stream_events()` | `Streaming<SessionStreamEvent>` | Subscribe to server events |

### Persistence

| Method | Returns | Description |
|--------|---------|-------------|
| `suspend_session()` | `String` (suspended_id) | Suspend and save the active session |
| `save_snapshot(fen, name, game_mode?, move_count, skill_level)` | `String` (suspended_id) | Save an arbitrary snapshot (used by review mode) |
| `list_suspended_sessions()` | `Vec<SuspendedSessionInfo>` | List all suspended sessions |
| `resume_suspended_session(id)` | `SessionSnapshot` | Resume a suspended session |
| `delete_suspended_session(id)` | `()` | Delete a suspended session |

### Positions

| Method | Returns | Description |
|--------|---------|-------------|
| `save_position(name, fen)` | `String` (position_id) | Save a named position |
| `list_positions()` | `Vec<SavedPosition>` | List all saved positions |
| `delete_position(id)` | `()` | Delete a saved position |

### Review

| Method | Returns | Description |
|--------|---------|-------------|
| `list_finished_games()` | `Vec<FinishedGameInfo>` | List finished games eligible for review |
| `enqueue_review(game_id)` | `ReviewStatusInfo` | Enqueue a game for background review analysis |
| `get_review_status(game_id)` | `ReviewStatusInfo` | Get the current review status for a game |
| `get_game_review(game_id)` | `GameReviewProto` | Get the full review with per-ply analysis |
| `export_review_pgn(game_id)` | `String` | Export annotated PGN for a reviewed game |
| `delete_finished_game(game_id)` | `()` | Delete a finished game and related review data |

### Advanced Analysis

| Method | Returns | Description |
|--------|---------|-------------|
| `get_advanced_analysis(game_id)` | `AdvancedGameAnalysisProto` | Fetch advanced tactical/psychological analysis for a reviewed game |

## Error Handling

All methods return `ClientResult<T>`, which is `Result<T, ClientError>`:

```rust
pub enum ClientError {
    InvalidAddress(String),             // Malformed server address
    ConnectionFailed(transport::Error), // Can't reach server
    RpcError(tonic::Status),            // Server returned an error
    NoActiveSession,                    // Method called without an active session
    InvalidData(String),                // Server returned unparseable data
}
```

## Re-exports

The crate re-exports all proto types from `chess_proto` for convenience, so consumers don't need to depend on the proto crate directly:

```rust
pub use chess_proto::*;
```

## Module Structure

```
chess-client/src/
├── lib.rs      # Public API: ChessClient, ClientError, proto re-exports
├── client.rs   # ChessClient implementation (all gRPC calls)
├── error.rs    # ClientError enum with thiserror derives
├── traits.rs   # ChessService trait abstraction for client and mock implementations
└── mock.rs     # MockChessService for testing with configurable responses and call logging
```

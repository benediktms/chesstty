# ChessTTY - Terminal Chess with Stockfish Integration

A terminal-based chess application built in Rust, featuring a ratatui TUI client communicating with a gRPC game server that integrates the Stockfish chess engine.

## Architecture Overview

ChessTTY uses a **server-authoritative client-server architecture**. The server owns all game state, move validation, engine management, and timer logic. The client is a thin rendering layer that sends user actions via gRPC and reacts to server-pushed events.

```mermaid
graph TB
    subgraph Client ["TUI Client (client-tui)"]
        UI[Ratatui UI Loop]
        CS[ClientState]
    end

    subgraph ClientLib ["Client Library (chess-client)"]
        GC[ChessClient<br/>gRPC Wrapper]
    end

    subgraph Server ["gRPC Server (server)"]
        SVC[ChessServiceImpl<br/>6 Endpoint Handlers]
        SM[SessionManager<br/>RwLock&lt;HashMap&gt;]
        SA[Session Actor<br/>per-session async task]
        SS[SessionState<br/>Game + Engine + Timer]
    end

    subgraph Engine ["Engine (engine crate)"]
        SF[StockfishEngine<br/>3 async tasks]
        SP[Stockfish Process]
    end

    subgraph Domain ["Chess Logic (chess crate)"]
        G[Game<br/>Board + History]
    end

    subgraph Storage ["Persistence"]
        PS[SessionStore<br/>JSON files]
        PP[PositionStore<br/>JSON files]
    end

    UI --> CS
    CS --> GC
    GC -->|"gRPC (unary + streaming)"| SVC
    SVC --> SM
    SM -->|"mpsc channel"| SA
    SA --> SS
    SS --> G
    SS -->|"EngineCommand"| SF
    SF -->|"UCI protocol"| SP
    SF -->|"EngineEvent"| SA
    SA -->|"broadcast channel"| SVC
    SVC -->|"SessionStreamEvent"| GC
    SM --> PS
    SM --> PP
```

### Design Principles

1. **Server-Authoritative** - All game state, move validation, engine control, and timer management live on the server. The client never validates moves or runs game logic.
2. **Actor Model** - Each game session runs as an isolated async task (actor) that owns all mutable state. No locks on game state; communication happens through channels.
3. **Snapshot-Based State** - Every state change produces a complete `SessionSnapshot`. No deltas or incremental updates. Clients can always reconstruct full state from a single event.
4. **Event-Driven** - The server pushes events to clients via gRPC server streaming. The client reacts to events and re-renders.
5. **Clean Boundaries** - Domain types and proto types are kept completely separate. Conversion happens only at the service layer.

## Stockfish Process Management

The `engine` crate manages Stockfish as a child process with three async tasks:

```
                    EngineCommand                 UCI string
┌────────────┐     (typed enum)     ┌───────────┐  (stdin)   ┌───────────┐
│ Session    │ ──────────────────>  │ Command   │ ────────>  │ Stdin     │ ──> Stockfish
│ Actor      │     mpsc channel     │ Processor │            │ Writer    │     stdin
└────────────┘                      └───────────┘            └───────────┘
      ^                                                            │
      │         EngineEvent                                        │ (emits RawUciMessage)
      │        (typed enum)         ┌───────────┐                  v
      └─────────────────────────── │ Output    │ <────────── Stockfish
               mpsc channel         │ Reader    │   stdout       stdout
                                    └───────────┘
```

1. **Output Reader** - Reads Stockfish stdout line-by-line, parses UCI messages (`bestmove`, `info`, `uciok`, etc.) into typed `EngineEvent` variants, and emits `RawUciMessage` events for the debug panel.
2. **Stdin Writer** - Receives string commands from an internal channel and writes them to Stockfish's stdin. Also emits `RawUciMessage` events for logging.
3. **Command Processor** - Receives typed `EngineCommand` enums (`SetPosition`, `Go`, `Stop`, etc.) and converts them to UCI protocol strings before forwarding to the stdin writer.

**Lifecycle**: On spawn, the engine sends `uci` and waits for `uciok` (10-second timeout), then configures `Skill Level`, `Threads`, and `Hash` via `setoption`. On shutdown, it sends `quit` and waits up to 1 second before killing the process.

**Auto-triggering**: The server automatically triggers engine moves based on game mode. In `HumanVsEngine`, the engine moves when it's the engine's turn. In `EngineVsEngine`, it moves after every position change. Search parameters scale with skill level (depth 4-8 for low skill, movetime 500-2000ms for higher skill).

## Event Architecture

Events flow from the engine process through the session actor to connected clients:

```
Stockfish stdout
    │
    ▼
EngineEvent (mpsc)
    │
    ▼
Session Actor ──── handles BestMove/Info/RawUci ────> SessionEvent (broadcast)
    │                                                       │
    │  (also receives SessionCommands                       ▼
    │   from gRPC endpoints via mpsc)               gRPC stream
    │                                               (SessionStreamEvent)
    ▼                                                       │
Timer tick (100ms interval)                                 ▼
                                                    Client: handle_event()
                                                        │
                                                        ▼
                                                    apply_snapshot()
                                                        │
                                                        ▼
                                                    UI re-render
```

### Session Actor Loop

The actor uses `tokio::select! { biased; }` with priority ordering:

1. **Commands** (highest) - `SessionCommand` from gRPC endpoints via `mpsc`. Each command carries a `oneshot::Sender` for the reply.
2. **Engine events** - `EngineEvent` from the Stockfish process via `mpsc`.
3. **Timer ticks** (lowest) - 100ms interval for decrementing the active player's clock. Only runs when a timer is active.

### Event Types

| Event | Description | Frequency |
|-------|-------------|-----------|
| `StateChanged(SessionSnapshot)` | Full state snapshot after any mutation | On every game action |
| `EngineThinking(EngineAnalysis)` | Transient analysis data (depth, score, PV) | 10+ per second during search |
| `UciMessage(UciLogEntry)` | Raw UCI protocol message for debug panel | Every engine I/O line |
| `Error(String)` | Error notification | On failures |

## gRPC Call Stack

### Protocol Structure

The protocol is defined in 8 `.proto` files organized by domain:

```
proto/proto/
├── chess_service.proto   # Service definition (imports all others)
├── common.proto          # Shared types: MoveRepr, MoveRecord, GamePhase, TimerState
├── session.proto         # SessionSnapshot, CreateSession, GetSession, CloseSession
├── game.proto            # MakeMove, GetLegalMoves, Undo, Redo, Reset
├── engine.proto          # SetEngine, StopEngine, EngineConfig
├── events.proto          # StreamEvents, SessionStreamEvent
├── persistence.proto     # Suspend, Resume, List, Delete sessions
└── positions.proto       # Save, List, Delete positions
```

### RPC Endpoints (20 total)

| Domain | RPCs | Pattern |
|--------|------|---------|
| Session | CreateSession, GetSession, CloseSession | Unary |
| Game | MakeMove, GetLegalMoves, UndoMove, RedoMove, ResetGame | Unary |
| Engine | SetEngine, StopEngine, PauseSession, ResumeSession | Unary |
| Persistence | SuspendSession, ListSuspendedSessions, ResumeSuspendedSession, DeleteSuspendedSession | Unary |
| Positions | SavePosition, ListPositions, DeletePosition | Unary |
| Events | StreamEvents | Server streaming |

**Key design choice**: There is no `TriggerEngineMove` RPC. The server auto-triggers engine moves based on game mode after every state change, keeping the client thin.

### Request/Response Flow

All game mutations follow the same pattern through the actor:

```
Client                gRPC Service          SessionManager       Session Actor
  │                        │                      │                    │
  │── MakeMove(from,to) ──>│                      │                    │
  │                        │── get_handle(id) ───>│                    │
  │                        │<── SessionHandle ────│                    │
  │                        │                      │                    │
  │                        │── handle.make_move(mv) ──────────────────>│
  │                        │   (mpsc + oneshot)                        │
  │                        │                                           │── validate & apply
  │                        │                                           │── broadcast StateChanged
  │                        │                                           │── maybe_auto_trigger()
  │                        │<──────────── SessionSnapshot ─────────────│
  │                        │                                           │
  │                        │── convert to proto ──│                    │
  │<── SessionSnapshot ────│                      │                    │
```

### Event Streaming

The `StreamEvents` RPC returns a gRPC server stream. On subscribe, the client immediately receives the current `SessionSnapshot`, then receives events as they occur:

```protobuf
message SessionStreamEvent {
  string session_id = 1;
  oneof event {
    SessionSnapshot state_changed = 2;
    EngineAnalysis engine_thinking = 3;
    UciMessageEvent uci_message = 4;
    string error = 5;
  }
}
```

The server uses `tokio::broadcast::channel(100)`. If a client falls behind, it skips lagged events and re-syncs on the next `StateChanged`.

## Client-Server Interface

The `chess-client` crate wraps the gRPC client into a high-level async API. The TUI uses it to:

1. **Connect** to the server at `http://[::1]:50051` via `ChessClient::connect()`
2. **Create a session** with game mode, optional FEN, and optional timer
3. **Open an event stream** via `stream_events()` for real-time updates
4. **Send actions** (make_move, undo, set_engine, pause, etc.) as unary RPCs
5. **Handle events** via `poll_event_async()` which dispatches to `apply_snapshot()`, updating the local board and UI state

The client maintains a `ClientState` as a single source of truth for rendering. All updates flow through `apply_snapshot()`, which parses the FEN into a `cozy_chess::Board` for rendering and updates the game mode and pause state.

## Project Structure

```
chesstty/
├── proto/          # gRPC protocol definitions (8 .proto files)
├── server/         # Authoritative game server (actor model, session management)
├── chess-client/   # Reusable gRPC client library
├── client-tui/     # Terminal UI (ratatui + crossterm)
├── chess/          # Core chess logic (cozy-chess wrapper, FEN, SAN, game state)
└── engine/         # Stockfish UCI engine wrapper (async process management)
```

See crate-level READMEs for detailed documentation:
- [server/README.md](server/README.md) - Actor model, session management, service layer
- [client-tui/README.md](client-tui/README.md) - UI render workflow, focus system, widget inventory
- [proto/README.md](proto/README.md) - Protocol definitions, message types, sequence diagrams
- [chess-client/README.md](chess-client/README.md) - Client library API
- [chess/README.md](chess/README.md) - Game logic, move generation, FEN handling
- [engine/README.md](engine/README.md) - Stockfish process management, UCI protocol

## Quick Start

### Prerequisites

- **Rust** (install via [rustup](https://rustup.rs))
- **Stockfish** chess engine ([download](https://stockfishchess.org))
- **just** command runner (optional, `cargo install just`)

### Running

```bash
# Terminal 1: Start server
just server
# or: cargo run -p chesstty-server

# Terminal 2: Start TUI client
just tui
# or: cargo run -p chesstty-tui
```

### Development

```bash
just build          # Build all crates
just test           # Run all tests
just lint           # Run clippy lints
just stockfish      # Check Stockfish installation
```

## Features

### Game Modes
- **Human vs Human** - Two players on the same terminal
- **Human vs Engine** - Play against Stockfish (skill 0-20)
- **Engine vs Engine** - Watch Stockfish play itself

### Capabilities
- **Session Persistence** - Suspend and resume games (JSON file storage)
- **Position Library** - Save and load custom FEN positions (with built-in defaults)
- **Real-time Engine Analysis** - Live depth, score, nodes/sec, and principal variation
- **Move History** - Complete history with undo/redo support
- **Timer Support** - Server-managed chess clocks with flag detection
- **UCI Debug Panel** - View raw Stockfish protocol messages
- **Adaptive Board Rendering** - Auto-sizes to terminal dimensions (Small/Medium/Large)

### Code Quality
- Zero unsafe code (`unsafe_code = "forbid"`)
- Strict lints (`enum_glob_use = "deny"`)
- Structured tracing throughout

## License

MIT License - see LICENSE file for details.

## Acknowledgments

- [cozy-chess](https://github.com/analog-hors/cozy-chess) - Fast chess move generation
- [Stockfish](https://stockfishchess.org) - World's strongest chess engine
- [tonic](https://github.com/hyperium/tonic) - gRPC for Rust
- [ratatui](https://github.com/ratatui/ratatui) - Terminal UI framework

# engine - Stockfish UCI Engine Wrapper

Async wrapper around the Stockfish chess engine process. Handles spawning, UCI protocol communication, and lifecycle management using three dedicated async tasks.

## Architecture

```
                         EngineCommand (mpsc)
┌──────────────┐         (SetPosition, Go,       ┌──────────────────┐
│              │          Stop, Quit, SetOption)   │                  │
│  Consumer    │ ──────────────────────────────>  │  Command         │
│  (Session    │                                   │  Processor Task  │
│   Actor)     │                                   │                  │
│              │         EngineEvent (mpsc)         └────────┬─────────┘
│              │ <──────────────────────────────             │
│              │  (BestMove, Info, Ready,                    │ UCI strings
│              │   Error, RawUciMessage)                     ▼
└──────────────┘                                  ┌──────────────────┐
                                                   │  Stdin Writer    │
      EngineEvent (mpsc)                           │  Task            │ ──> Stockfish stdin
┌──────────────────┐                               └──────────────────┘
│  Output Reader   │
│  Task            │ <── Stockfish stdout          ┌──────────────────┐
│                  │ ──────────────────────────>   │  Event channel   │
└──────────────────┘      (parsed UCI messages)    │  (to consumer)   │
                                                   └──────────────────┘
```

### Three Async Tasks

1. **Output Reader** - Reads Stockfish stdout line-by-line via `BufReader`. Parses each line through the UCI parser into typed `UciMessage` variants, then converts to `EngineEvent` and sends to the consumer via mpsc. Also emits `RawUciMessage` events (direction: `FromEngine`) for debug logging.

2. **Stdin Writer** - Receives raw UCI command strings from an internal channel and writes them to Stockfish's stdin. Emits `RawUciMessage` events (direction: `ToEngine`) for debug logging.

3. **Command Processor** - Receives typed `EngineCommand` enums from the consumer and converts them to UCI protocol strings:

| Command | UCI Output |
|---------|-----------|
| `SetPosition { fen, moves }` | `position fen <fen> moves <moves>` |
| `Go { movetime: 500 }` | `go movetime 500` |
| `Go { depth: 8 }` | `go depth 8` |
| `Go { infinite: true }` | `go infinite` |
| `Stop` | `stop` |
| `SetOption { name, value }` | `setoption name <name> value <value>` |
| `Quit` | `quit` (then exits the task) |

## Spawning and Initialization

```rust
let engine = StockfishEngine::spawn_with_config(StockfishConfig {
    skill_level: Some(10),
    threads: Some(4),
    hash_mb: Some(128),
    label: Some("session-123".to_string()),
}).await?;
```

**Initialization sequence**:
1. Find Stockfish binary (checks `/usr/local/bin`, `/usr/bin`, `/opt/homebrew/bin`, `/usr/games`, then PATH)
2. Spawn process with piped stdin/stdout
3. Send `uci`, wait for `uciok` (10-second timeout)
4. Send `setoption` for Skill Level, Threads (clamped 1-16), Hash (clamped 1-2048 MB)
5. Spawn the three async tasks
6. Send `isready`

## Types

### EngineCommand

```rust
pub enum EngineCommand {
    SetPosition { fen: String, moves: Vec<Move> },
    SetOption { name: String, value: Option<String> },
    Go(GoParams),
    Stop,
    Quit,
}

pub struct GoParams {
    pub movetime: Option<u64>,  // milliseconds
    pub depth: Option<u8>,
    pub infinite: bool,
}
```

### EngineEvent

```rust
pub enum EngineEvent {
    Ready,                    // uciok or readyok
    BestMove(Move),           // Engine's chosen move
    Info(EngineInfo),         // Analysis data during search
    Error(String),
    RawUciMessage {           // Raw protocol line for debug panel
        direction: UciMessageDirection,
        message: String,
    },
}
```

### EngineInfo

```rust
pub struct EngineInfo {
    pub depth: Option<u8>,
    pub seldepth: Option<u8>,
    pub time_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub score: Option<Score>,       // Centipawns(i32) or Mate(i8)
    pub pv: Vec<Move>,              // Principal variation
    pub multipv: Option<u8>,
    pub currmove: Option<Move>,
    pub hashfull: Option<u16>,      // Hash table usage per mille
    pub nps: Option<u64>,           // Nodes per second
}
```

## UCI Parser

The UCI parser (`uci/parser.rs`) tokenizes each line from Stockfish and dispatches on the first token:

| First Token | Parsed Into |
|-------------|-------------|
| `id` | `UciMessage::Id { name?, author? }` |
| `uciok` | `UciMessage::UciOk` |
| `readyok` | `UciMessage::ReadyOk` |
| `bestmove` | `UciMessage::BestMove { mv, ponder? }` |
| `info` | `UciMessage::Info(EngineInfo)` |

The `info` parser extracts all standard UCI info fields: `depth`, `seldepth`, `time`, `nodes`, `nps`, `score` (cp/mate), `pv`, `multipv`, `currmove`, `hashfull`.

## Shutdown

```rust
engine.shutdown().await;
```

1. Sends `EngineCommand::Quit` (becomes UCI `quit`)
2. Waits up to 1 second for the process to exit
3. Force-kills the process if it hasn't exited

## Module Structure

```
engine/src/
├── lib.rs          # Public types: EngineCommand, EngineEvent, EngineInfo, GoParams, Score
├── stockfish.rs    # StockfishEngine: spawn, send_command, recv_event, shutdown
└── uci/
    ├── mod.rs      # UciError, re-exports
    └── parser.rs   # parse_uci_message(), UciMessage enum, info line parsing
```

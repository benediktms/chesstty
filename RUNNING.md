# Running ChessTTY Server/Client Architecture

This document explains how to run the new server/client architecture.

## Architecture Overview

ChessTTY now consists of two separate binaries:

1. **Server** (`chesstty-server`) - Manages game sessions and engine
2. **TUI Client** (`chesstty-tui`) - Connects to server and provides UI

Multiple TUI clients can connect to a single server simultaneously, each getting their own independent game session.

## Prerequisites

- Rust 1.70 or later
- Stockfish chess engine installed (for engine vs human games)
  - macOS: `brew install stockfish`
  - Linux: `apt-get install stockfish`

## Building

Build all components:

```bash
cargo build --release
```

Or build individually:

```bash
# Build server
cargo build --release -p chesstty-server

# Build TUI client
cargo build --release -p chesstty-tui
```

## Running

### 1. Start the Server

In one terminal:

```bash
cargo run --release -p chesstty-server
```

You should see:

```
Starting ChessTTY gRPC server
Server listening on [::1]:50051
```

The server will:
- Listen on `localhost` port `50051`
- Accept multiple concurrent connections
- Manage separate game sessions for each client
- Handle Stockfish engine instances per session

### 2. Start the TUI Client

In another terminal:

```bash
# Full UI with chess board visualization (default)
cargo run --release -p chesstty-tui

# OR: Simple text-based UI
cargo run --release -p chesstty-tui -- --simple
```

**Full UI Features:**
- Visual chess board with piece positions
- Move history panel
- Game status information
- Interactive square selection
- Type square names to make moves (e.g., "e2" then "e4")

**Simple UI Features:**
- Compact FEN and text display
- Same functionality, minimal interface
- Faster for quick testing
- Command: `m e2 e4` for moves

### 3. Start Multiple Clients (Optional)

You can start additional clients in separate terminals to test concurrent sessions:

```bash
# Terminal 3
cargo run --release -p chesstty-tui

# Terminal 4
cargo run --release -p chesstty-tui
```

Each client gets its own independent game session on the server.

## Using the TUI Client

The simplified UI supports these commands:

- `m <from> <to>` - Make a move (e.g., `m e2 e4`)
- `u` - Undo last move
- `r` - Reset game
- `q` - Quit

### Example Game Session

```
> m e2 e4    # Move pawn from e2 to e4
> m e7 e5    # Black responds
> m g1 f3    # White knight
> m b8 c6    # Black knight
> u          # Undo last move
> m d7 d6    # Different black move
> r          # Reset game
> q          # Quit
```

## Testing Multiple Sessions

### Test 1: Two Independent Games

1. Start the server
2. Start Client 1 - play some moves
3. Start Client 2 - play different moves
4. Verify: Moves in Client 1 don't appear in Client 2

### Test 2: Session Isolation

1. Client 1: `m e2 e4`, `m d7 d5`
2. Client 2: `m d2 d4`, `m g8 f6`
3. Verify: Each client shows only their own moves

### Test 3: Undo Independence

1. Both clients make 5 moves
2. Client 1: `u` (undo once)
3. Client 2: `u` `u` (undo twice)
4. Verify: Each has different game state

### Test 4: Engine Support (Future)

Once engine integration is fully wired:

1. Configure engine: Set skill level via server API
2. Trigger engine move
3. Watch for engine response via event stream

## Troubleshooting

### Server won't start

- **Port already in use**: Check if another instance is running
  ```bash
  lsof -i :50051
  kill <PID>
  ```

### Client can't connect

- Verify server is running: `lsof -i :50051`
- Check server logs for errors
- Ensure firewall allows localhost connections

### Stockfish not found

Server will fail to create engine if Stockfish isn't installed:

```bash
# macOS
brew install stockfish

# Ubuntu/Debian
sudo apt-get install stockfish

# Verify installation
which stockfish
```

## Development

### Running with Logging

Enable detailed logging:

```bash
# Server with debug logs
RUST_LOG=debug cargo run -p chesstty-server

# Client with debug logs
RUST_LOG=debug cargo run -p chesstty-tui
```

### Checking Server Health

```bash
# Install grpcurl
brew install grpcurl  # macOS

# List services
grpcurl -plaintext localhost:50051 list

# Create a session (test)
grpcurl -plaintext -d '{}' localhost:50051 chess.ChessService/CreateSession
```

## Current Limitations

1. **Simplified UI**: The current TUI is minimal (text-based commands)
   - Full chess board rendering is in the legacy UI files
   - Can be integrated by adapting the widgets to work with ClientState

2. **Engine Events**: Engine move notifications work server-side but aren't fully wired to client UI
   - Server can trigger engine moves
   - Event streaming is implemented but needs client polling loop

3. **No Persistence**: Games are lost when server restarts
   - Phase 3 will add SQLite persistence

## Next Steps

To complete the implementation:

1. **Wire Engine Events**: Connect server event stream to client UI
2. **Full UI Integration**: Adapt existing board widgets to ClientState
3. **Add Persistence**: Implement game save/load functionality
4. **Web Client**: Build browser-based UI using gRPC-Web

## File Structure

```
chesstty/
├── Cargo.toml              # Workspace root
├── proto/                  # Protocol definitions (gRPC/Protobuf)
│   ├── proto/chess.proto   # Protocol Buffer schema
│   └── src/lib.rs          # Generated Rust code
├── server/                 # Chess server
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── session.rs      # Session management
│   │   └── service.rs      # gRPC service implementation
├── client-tui/            # TUI client
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── client.rs       # gRPC client wrapper
│   │   ├── state.rs        # ClientState adapter
│   │   └── ui/             # UI implementation
├── chess/                  # Chess game logic (shared)
│   └── src/lib.rs
└── engine/                 # Stockfish engine wrapper (shared)
    └── src/lib.rs
```

## Architecture Benefits

✅ **Multiple Clients**: Run many TUI instances against one server
✅ **Session Isolation**: Each client has independent game state
✅ **Centralized Logic**: Game rules enforced server-side
✅ **Future-Proof**: Protocol supports web/mobile clients
✅ **Better Testing**: Can test server logic independently
✅ **Scalability**: Easy to add authentication, persistence, etc.

## Support

For issues or questions:
- Check logs in `chesstty.log` (server) and `chesstty-client.log` (client)
- Review error messages in the terminal
- Verify Stockfish installation for engine features

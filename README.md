# ChessTTY

A terminal-based chess application written in Rust, featuring:
- Full chess game support with legal move generation
- UCI engine support (Stockfish integration)
- PGN import/export
- FEN position support
- Terminal UI with ratatui

## Architecture

### Project Structure

```
src/
├── main.rs           # Entry point
├── app/             # Application orchestration layer
│   ├── state.rs     # Main app state & game modes
│   └── mod.rs
├── chess/           # Chess domain logic
│   ├── game.rs      # Game state wrapper
│   ├── fen.rs       # FEN parser/formatter
│   └── pgn/         # PGN support
│       ├── parser.rs  # PGN import
│       ├── san.rs     # Standard Algebraic Notation
│       └── writer.rs  # PGN export
├── engine/          # UCI engine communication
│   └── uci/
│       ├── parser.rs  # UCI protocol parser
│       └── mod.rs     # Engine wrapper
└── ui/              # Terminal UI (ratatui)
    ├── app.rs       # Main UI loop
    ├── board.rs     # Board rendering
    └── input.rs     # Input handling
```

## Dependencies

- **cozy-chess** (0.3): Chess logic with strong type safety
- **ratatui** (0.28): Terminal UI framework
- **crossterm** (0.28): Cross-platform terminal backend
- **tokio** (1.41): Async runtime for engine I/O
- **thiserror** / **anyhow**: Error handling

## Design Principles

### Type Safety
Uses `cozy-chess` for compile-time guarantees:
- No invalid squares (File::A-H, Rank::First-Eighth)
- Explicit promotion handling
- Separate Piece and Color types

### Clean Architecture
Three distinct layers:
1. **UI Layer**: Rendering and input (no game logic)
2. **App Layer**: Orchestration and state management
3. **Domain Layer**: Chess rules and engine communication

### Custom Parsers
- UCI parser (~300 LOC): Full control over Stockfish communication
- PGN parser (TODO): Custom format handling
- SAN parser (TODO): Move notation

## Current Status

✅ **Enhanced UI Complete**
- Beautiful board rendering with custom ASCII art pieces
- Color-coded squares (light/dark) with RGB colors
- Square selection highlighting (yellow)
- Legal move highlighting (green)
- Last move highlighting (blue)
- File/rank labels (a-h, 1-8)
- Side panel with controls and game status
- Interactive startup menu with:
  - Game mode selection (Human vs Human/Engine, Engine vs Engine)
  - Difficulty settings (Beginner/Intermediate/Advanced/Master)
  - Time control options (None/Blitz/Rapid/Classical)
- Turn indicator showing whose move it is
- Game status display (checkmate/draw detection)

🚧 **In Progress**
- [ ] Square selection via keyboard input
- [ ] Move execution
- [ ] UCI engine integration
- [ ] PGN import/export
- [ ] Time clock display

## Building & Running

```bash
# Check compilation
cargo check

# Build
cargo build

# Run the game
cargo run
```

## Controls

### Main Menu
- `↑/↓`: Navigate menu items
- `←/→`: Change selected option
- `Enter`: Confirm/start game
- `q`: Quit

### In Game
- Type square coordinates (e.g., `e2` then `e4`) to make moves
- `u`: Undo last move
- `n`: Return to menu (new game)
- `q`: Quit game

## Next Steps

### Milestone 1: Human vs Human
- Implement square selection (keyboard: a1-h8 notation)
- Implement move execution
- Display game status (check/checkmate/stalemate)

### Milestone 2: Engine Integration
- Spawn Stockfish process
- Send UCI commands
- Play human vs engine

### Milestone 3: PGN Support
- Implement PGN parser
- Implement PGN writer
- Save/load games

## License

MIT

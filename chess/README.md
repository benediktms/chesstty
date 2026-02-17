# chess - Core Chess Logic

Domain types and game logic for ChessTTY. Wraps the `cozy-chess` library with higher-level abstractions: game state with history, FEN parsing/formatting, SAN notation, move descriptions, and engine analysis types.

## Key Types

### Game

The central game state type, wrapping a `cozy_chess::Board` with undo/redo support:

```rust
pub struct Game {
    position: Board,                  // Current board position (cozy-chess)
    history: Vec<HistoryEntry>,       // Move history stack
    redo_stack: Vec<HistoryEntry>,    // Redo stack (cleared on new move)
}
```

**Operations**:

| Method | Description |
|--------|-------------|
| `Game::new()` | Standard starting position |
| `Game::from_fen(fen)` | Custom position from FEN string |
| `make_move(mv)` | Validate and apply a move, returns `HistoryEntry` |
| `undo()` | O(1) undo using stored `board_before` snapshot |
| `redo()` | O(1) redo from the redo stack |
| `legal_moves()` | All legal moves in current position |
| `status()` | `Ongoing`, `Won`, or `Drawn` |
| `side_to_move()` | `White` or `Black` |
| `to_fen()` | Export current position as FEN string |

**Undo/Redo**: Each `HistoryEntry` stores the full `Board` state before the move (`board_before`), enabling O(1) undo via direct restoration rather than replaying from the start.

### HistoryEntry

Complete record of a single move:

```rust
pub struct HistoryEntry {
    pub mv: Move,                    // The cozy-chess move
    pub from: Square,
    pub to: Square,
    pub piece: Piece,                // Piece that moved
    pub piece_color: Color,          // Color of the piece
    pub captured: Option<Piece>,     // Captured piece (if any)
    pub promotion: Option<Piece>,    // Promotion piece (if any)
    pub san: String,                 // "e4", "Nxd5", "O-O"
    pub fen: String,                 // Position after this move
    pub board_before: Board,         // For O(1) undo
}
```

### GamePhase

State machine for game lifecycle:

```
Setup ──> Playing { turn } ──> Ended { result, reason }
              │       ▲
              ▼       │
         Paused { resume_turn }

Analyzing (standalone mode)
```

### GameMode

Determines who controls each side:

```rust
pub enum GameMode {
    HumanVsHuman,
    HumanVsEngine { human_side: PlayerSide },
    EngineVsEngine,
    Analysis,
    Review,
}
```

### EngineAnalysis

Engine evaluation data shared between server and client:

```rust
pub struct EngineAnalysis {
    pub depth: Option<u32>,
    pub seldepth: Option<u32>,
    pub time_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub score: Option<AnalysisScore>,  // Centipawns(i32) or Mate(i32)
    pub pv: Vec<String>,               // Principal variation
    pub nps: Option<u64>,
}
```

## Modules

```
chess/src/
├── lib.rs            # Public exports
├── game.rs           # Game, HistoryEntry, GamePhase, GameMode, SAN generation
├── types.rs          # PieceKind, PieceColor (domain piece types)
├── fen.rs            # FEN parsing and formatting
├── analysis.rs       # EngineAnalysis, AnalysisScore
├── board_display.rs  # DisplayBoard (8x8 grid for rendering)
├── converters.rs     # format_square, parse_square, format_piece, format_color
└── uci.rs            # UCI castling conversion, format_uci_move
```

### Converter Functions

Utility functions for converting between `cozy_chess` types and string representations:

| Function | Description |
|----------|-------------|
| `format_square(Square) -> String` | `Square::E4` -> `"e4"` |
| `parse_square(&str) -> Option<Square>` | `"e4"` -> `Some(Square::E4)` |
| `format_piece(Piece) -> char` | `Piece::Knight` -> `'n'` |
| `format_piece_upper(Piece) -> char` | `Piece::Knight` -> `'N'` |
| `format_color(Color) -> String` | `Color::White` -> `"white"` |
| `convert_uci_castling_to_cozy(mv, legal)` | Converts UCI castling (e1g1) to cozy-chess format (e1h1) |
| `format_uci_move(Move) -> String` | Format move as UCI string (e.g., `"e2e4"`) |

### DisplayBoard

An 8x8 array representation for rendering, parsed from FEN:

```rust
pub struct DisplayBoard {
    pub squares: [[Option<(PieceKind, PieceColor)>; 8]; 8],
}
```

## Dependencies

- **`cozy-chess`** - Fast legal move generation using magic bitboards. Used for `Board`, `Move`, `Square`, `Piece`, `Color`, and `GameStatus`.
- **`thiserror`** - Error type derivation for `GameError` and `FenError`.

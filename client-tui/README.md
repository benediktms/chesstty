# chesstty-tui - Terminal UI Client

A ratatui-based terminal chess interface that renders the game board, info panels, and overlays. The TUI is a thin rendering layer; all game logic lives on the server.

## UI Render Workflow

### Application Loop

The application runs an outer menu/game loop:

```
┌──────┐     ┌──────────┐     ┌──────────┐
│ Menu │────>│ Game UI  │────>│ Menu     │──> ...
│      │     │ Loop     │     │ (or Quit)│
└──────┘     └──────────┘     └──────────┘
```

1. **Menu phase**: Show main menu (ratatui in raw mode). Pre-fetches suspended sessions and saved positions from the server. User selects game mode, skill, FEN, time control.
2. **Game phase**: Set up terminal (alternate screen, mouse capture), create `ClientState`, configure engine, start event stream, enter the render loop.
3. **Exit**: On quit or return-to-menu, restore terminal and loop back.

### Render Loop (`run_ui_loop`)

The render loop uses `tokio::select! { biased; }` to handle three event sources:

```rust
loop {
    tokio::select! {
        biased;

        // 1. Keyboard events (highest priority)
        event = crossterm_events.next() => { ... }

        // 2. Server events (gRPC stream)
        _ = state.poll_event_async() => { ... }

        // 3. UI tick (~30fps, every 33ms)
        _ = tick_interval.tick() => { /* just re-render */ }
    }

    // After any branch: drain buffered server events, then render
    while state.poll_events().await == Ok(true) { }
    terminal.draw(|f| render(f, state))?;
}
```

- **Keyboard events** wake the loop immediately for responsive input
- **Server events** (engine analysis, state changes) trigger immediate re-renders
- **UI tick** ensures timer displays update even without events

### State Management

`ClientState` is the single source of truth for the TUI:

```
ClientState
├── client: ChessClient              # gRPC connection
├── mode: GameMode                    # HvH, HvE, EvE, Analysis, Review
├── skill_level: u8                   # Stockfish skill (0-20)
├── snapshot: SessionSnapshot         # Latest server snapshot (source of truth)
├── board: Board                      # Parsed from snapshot.fen for rendering
├── legal_moves_cache: HashMap        # Legal moves indexed by source square
├── event_stream: Streaming           # gRPC event stream
└── ui: UiState                       # Ephemeral UI state
    ├── selected_square               # Currently selected piece
    ├── highlighted_squares           # Legal destinations for selected piece
    ├── selectable_squares            # All squares with movable pieces
    ├── last_move                     # Highlight for last move
    ├── engine_info                   # Latest engine analysis
    ├── is_engine_thinking            # Engine status indicator
    ├── input_phase                   # SelectPiece -> SelectDestination -> SelectPromotion
    ├── uci_log                       # UCI message log (max 100 entries)
    ├── pane_manager                  # Pane visibility, order, scroll positions
    ├── focus_stack                   # Focus context stack
    ├── popup_menu                    # Active popup menu state (if any)
    └── paused                        # Pause state
└── review_state: Option<ReviewState>  # Review navigation (when in Review mode)
```

**All updates flow through `apply_snapshot()`**: parses FEN into `cozy_chess::Board`, updates game mode, updates pause state. No local game logic.

## Layout Structure

```
┌─────────────────────────────────────────────┐
│                                             │
│  Board (left)         │  Panels (right)     │
│  [or expanded pane    │  ┌─ GameInfo ─────┐ │
│   + mini board        │  │ Mode, Turn,    │ │
│   in corner]          │  │ Timer, Status  │ │
│                       │  └────────────────┘ │
│                       │  ┌─ Engine ───────┐ │
│                       │  │ Depth, Score,  │ │
│                       │  │ PV, Nodes/sec  │ │
│                       │  └────────────────┘ │
│                       │  ┌─ Move History ─┐ │
│                       │  │ 1. e4   e5     │ │
│                       │  │ 2. Nf3  Nc6    │ │
│                       │  └────────────────┘ │
├─────────────────────────────────────────────┤
│ [UCI Debug Panel - hidden by default]       │
├─────────────────────────────────────────────┤
│ Input: e2 | p Pause | Esc Menu | Tab Panels │
└─────────────────────────────────────────────┘
```

When a pane is expanded (Enter from pane selection), it replaces the board area. A `MiniBoardWidget` (compact 18x10 Unicode board) renders in the bottom-right corner so the position is always visible.

## Focus System

The focus system uses a stack-based model with three contexts:

```
FocusContext::Board (always at bottom of stack)
    │
    │ Tab
    ▼
FocusContext::PaneSelected { pane_id }
    │                    │
    │ Enter              │ Left/Right
    ▼                    ▼
FocusContext::PaneExpanded { pane_id }    (cycle panes)
    │
    │ Esc
    ▼
FocusContext::PaneSelected (pop)
    │
    │ Esc
    ▼
FocusContext::Board (pop)
```

| Context | Keyboard Behavior |
|---------|------------------|
| **Board** | Character input builds algebraic notation, Enter submits move, Esc opens popup menu |
| **PaneSelected** | Left/Right cycle visible panes, Up/Down scroll, Enter expands pane |
| **PaneExpanded** | Up/Down/PageUp/PageDown scroll content, Esc collapses back |

## Review Summary Display

The `ReviewSummaryPanel` widget appears when viewing a completed game review. It displays:

### Accuracy Section
Shows the accuracy percentage for each player with a visual bar chart:
```
Accuracy
  White: 87.3%  ████████████████████
  Black: 72.1%  ███████████████░░░░░
```

Color-coded by performance:
- **Green** (≥90%) - Excellent accuracy
- **Yellow** (70-89%) - Good accuracy
- **Red** (<70%) - Needs improvement

### Evaluation Graph

A **5-row ASCII sparkline chart** showing position evaluation throughout the game:

```
Evaluation
    ▔▔▔▔▔▔▔▔
  ▄▄        ▄▄
▄▄              ▄▄
▁▁                ▁▁░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░
```

**How to read it:**
- **Above the midline** (row 2) = White advantage
- **Below the midline** = Black advantage
- **Height/fullness** = Magnitude of advantage (scaled to ±500 cp)
- **White/Gray** = Normal positions
- **Yellow highlights** = Mistakes made
- **Red highlights** = Blunders made

Each column represents a sampled position across the game, automatically scaled to fit the available width.

### Move Quality Breakdown

Counts moves in each category for each side:
```
Move Quality
  Best         W:5   B:3
  Excellent    W:2   B:4
  Good         W:8   B:10
  Inaccuracy   W:3   B:2
  Mistake      W:1   B:0
  Blunder      W:0   B:1
```

### Critical Moments

Lists blunders and mistakes (up to 10) with move number, side, move, and centipawn loss:
```
Critical Moments
  1. [W] e4?? (52cp)
  3. [B] Nf6? (35cp)
  5. [W] h3?? (280cp)
```

Format: `move_number. [W|B] move_notation (cp_loss)`

### Legend

Reference guide for move quality annotations:
```
Legend
  !! Brilliant
  !  Excellent
     Good / Best
  ?! Inaccuracy
  ?  Mistake
  ?? Blunder
  [] Forced
```

These annotations (NAG - Numeric Annotation Glyphs) appear in exported PGN files.

### Analysis Info

Shows depth of analysis and completion progress:
```
Depth: 18  Plies: 42/42
```

- **Depth** - How many half-moves ahead Stockfish analyzed
- **Plies** - Number of moves analyzed / total moves in game

## Widget Inventory

| Widget | File | Description |
|--------|------|-------------|
| `BoardWidget` | `board.rs` | Chess board with 3 size variants (Small/Medium/Large) auto-selected by terminal size. RGB colored squares, selection/highlight/typeahead overlays. Board flips for Black in HvE. |
| `GameInfoPanel` | `game_info_panel.rs` | Game mode, current turn, timer display (color-coded urgency: green > yellow > red), input phase, status message. Always visible, not selectable. |
| `EngineAnalysisPanel` | `engine_panel.rs` | Depth, score (color-coded: green for advantage, red for disadvantage, yellow for mate), nodes/sec, principal variation line. Toggleable with `#`. |
| `MoveHistoryPanel` | `move_history_panel.rs` | Compact paired format (1. e4 e5) in normal view, detailed expanded mode with piece descriptions. Selectable and expandable. |
| `UciDebugPanel` | `uci_debug_panel.rs` | Syntax-highlighted UCI protocol log. Direction-colored (green for TO_ENGINE, cyan for FROM_ENGINE). Hidden by default, toggle with `@`. |
| `PopupMenuWidget` | `popup_menu.rs` | Modal overlay menu triggered by Esc. Items: Restart, Adjust Difficulty (HvE only), Suspend Session, Quit to Menu. |
| `PromotionWidget` | `promotion_dialog.rs` | Modal promotion piece selector (Q/R/B/N) shown during pawn promotion. |
| `MiniBoardWidget` | `mini_board.rs` | Compact Unicode board (18x10) shown in corner when a pane is expanded. |
| `MenuWidget` | `menu.rs` | Main menu with dynamic items based on game mode, suspended sessions, and saved positions. |
| `FenDialogWidget` | `fen_dialog.rs` | FEN input field with saved positions table overlay. |
| `ReviewSummaryPanel` | `review_summary_panel.rs` | Accuracy scores, classification breakdown, critical moments list. Only visible in Review mode. |
| `SelectableTableState` | `selectable_table.rs` | Reusable table component with keyboard navigation (used by FEN dialog and menu). |

## Pane Management

`PaneManager` controls pane visibility, ordering, and scroll state:

| Pane | Default Visible | Selectable | Expandable |
|------|----------------|------------|------------|
| GameInfo | Yes | No | No |
| EngineAnalysis | Yes | Yes | Yes |
| MoveHistory | Yes | Yes | Yes |
| UciDebug | No | Yes | Yes |
| ReviewSummary | No (review mode only) | Yes | Yes |

Pane order: GameInfo -> EngineAnalysis -> MoveHistory -> UciDebug

Visibility toggles: `@` for UciDebug, `#` for EngineAnalysis

## Input Handling

### Input Priority Chain

1. **Popup menu** (modal) - Up/Down navigate, Enter selects, Esc dismisses
2. **Promotion dialog** (modal) - Character input for piece selection (q/r/b/n)
3. **Ctrl+C** - Always quits
4. **Global toggles** - `@` (UCI panel), `#` (Engine panel)
5. **Context-based** - Dispatched by current `FocusContext`

### Move Input (Board context)

Moves are entered as algebraic square notation:

```
InputPhase::SelectPiece ──(type "e2", Enter)──> InputPhase::SelectDestination
    ──(type "e4", Enter)──> Move submitted to server
    ──(promotion needed)──> InputPhase::SelectPromotion
```

- Typeahead filtering highlights matching squares as the user types
- Backspace clears input buffer
- Esc clears piece selection
- Input is disabled in Engine vs Engine mode

### Special Keys

| Key | Context | Action |
|-----|---------|--------|
| `p` | Board (EvE mode) | Toggle pause |
| `u` | Board (HvE, skill <= 3) | Undo last move |
| `Esc` | Board (no selection) | Open popup menu (auto-pauses engine) |
| `Tab` | Board | Enter pane selection |

### Review Mode

When in Review mode, board context keys are replaced with navigation:

| Key | Action |
|-----|--------|
| Right / `l` | Next ply |
| Left / `h` | Previous ply |
| Home | Go to start |
| End | Go to end |
| `n` | Next critical moment (blunder/mistake) |
| `p` | Previous critical moment |
| Space | Toggle auto-play |

Move input is disabled. All data is loaded once from the server; no further server calls are made during review.

## Module Structure

```
client-tui/src/
├── main.rs              # Entry point, CLI args, tracing setup
├── state.rs             # ClientState, UiState, GameMode, InputPhase
└── ui/
    ├── mod.rs           # UI module exports
    ├── full_ui.rs       # Main app loop, game setup, render function
    ├── simple_ui.rs     # Simplified UI mode (--simple flag)
    ├── input.rs         # Key dispatch, context handlers, AppAction enum
    ├── context.rs       # FocusContext enum, FocusStack
    ├── pane.rs          # PaneId, PaneProperties, PaneManager
    ├── menu_app.rs      # Main menu (GameConfig, mode selection, FEN input)
    └── widgets/
        ├── mod.rs
        ├── board.rs
        ├── engine_panel.rs
        ├── fen_dialog.rs
        ├── game_info_panel.rs
        ├── menu.rs
        ├── mini_board.rs
        ├── move_history_panel.rs
        ├── popup_menu.rs
        ├── promotion_dialog.rs
        ├── review_summary_panel.rs
        ├── selectable_table.rs
        └── uci_debug_panel.rs
```

# client-tui

A ratatui-based terminal chess client. The TUI is a rendering layer over a server-authoritative game model — all game logic lives on the server, the client renders the latest snapshot and forwards user input.

## Architecture Overview

The crate is organized into five layers:

```
┌─────────────────────────────────────────────────────┐
│                   Render Loop                        │
│  tokio::select! { keyboard | server events | tick }  │
├─────────────┬───────────────────────────────────────┤
│  Input      │           Renderer                     │
│  Handling   │  Layout + State → ratatui widgets      │
├─────────────┴───────────┬───────────────────────────┤
│      UI State Machine   │   Declarative Layout       │
│  UiStateMachine (FSM)   │  Layout / Row / Section    │
├─────────────────────────┴───────────────────────────┤
│                    State Layer                        │
│         GameSession (server)  +  ReviewState          │
└─────────────────────────────────────────────────────┘
```

**Data flow:**

```
Keyboard Event
  → input::handle_key()        mutates FSM or GameSession
  → fsm.layout(game_session)   derives Layout from current mode
  → Renderer::render()         converts Layout → ratatui widgets
  → terminal.draw()            paints to screen
```

## Module Structure

```
client-tui/src/
├── main.rs                          # Entry point, tracing setup, calls ui::run_app()
├── lib.rs                           # Library root, public exports
├── prelude.rs                       # Re-exports of common types
├── state.rs                         # GameSession, GameMode, PlayerColor
├── review_state.rs                  # ReviewState (post-game review navigation)
└── ui/
    ├── mod.rs                       # UI module exports
    ├── render_loop.rs               # Main event loop (run_app, run_ui_loop)
    ├── menu_app.rs                  # Menu UI, game configuration
    ├── input.rs                     # Keyboard event dispatch
    ├── fsm/
    │   ├── mod.rs                   # UiStateMachine, UiMode, transitions, navigation
    │   ├── component.rs             # Component enum, properties (selectability, etc.)
    │   ├── render_spec.rs           # Layout, Row, Section, Constraint, Overlay, Control
    │   ├── renderer.rs              # Renderer: Layout → ratatui widgets
    │   ├── hooks.rs                 # Transition hook traits (extension point)
    │   └── states/
    │       ├── mod.rs               # State type exports
    │       ├── start_screen.rs      # StartScreenState
    │       ├── game_board.rs        # GameBoardState (layout builder)
    │       ├── review_board.rs      # ReviewBoardState (layout builder)
    │       └── match_summary.rs     # MatchSummaryState
    └── widgets/
        ├── mod.rs                   # Widget exports
        ├── board.rs                 # BoardWidget (main chess board)
        ├── board_overlay.rs         # BoardOverlay (highlights, arrows, tints)
        ├── mini_board.rs            # MiniBoardWidget (compact Unicode board)
        ├── game_info_panel.rs       # GameInfoPanel (mode, turn, timers)
        ├── move_history_panel.rs    # MoveHistoryPanel (move list)
        ├── engine_panel.rs          # EngineAnalysisPanel (depth, score, PV)
        ├── move_analysis_panel.rs   # MoveAnalysisPanel (move classification)
        ├── advanced_analysis_panel.rs # AdvancedAnalysisPanel (tactics, patterns)
        ├── review_summary_panel.rs  # ReviewSummaryPanel (accuracy, eval graph)
        ├── review_tabs_panel.rs     # ReviewTabsPanel (review navigation tabs)
        ├── uci_debug_panel.rs       # UciDebugPanel (UCI protocol log)
        ├── tab_input.rs             # TabInputWidget (typeahead move entry)
        ├── menu.rs                  # MenuWidget (start screen menu)
        ├── popup_menu.rs            # PopupMenuWidget (in-game pause menu)
        ├── promotion_dialog.rs      # PromotionWidget (pawn promotion selector)
        ├── fen_dialog.rs            # FenDialogWidget (FEN/position input)
        ├── snapshot_dialog.rs       # SnapshotDialogWidget (review snapshot creator)
        └── selectable_table.rs      # SelectableTableState (reusable table navigation)
```

## State Management

Two state structs, each with a distinct responsibility:

### GameSession (`state.rs`)

Server-authoritative game state. The server is the source of truth — the client stores the latest `SessionSnapshot` and derives everything else from it.

```
GameSession
├── client: ChessClient              # gRPC connection to server
├── mode: GameMode                   # HumanVsHuman | HumanVsEngine | EngineVsEngine | AnalysisMode | ReviewMode
├── skill_level: u8                  # Engine difficulty (0-20)
├── snapshot: SessionSnapshot        # Authoritative state from server
├── board: Board                     # Parsed from snapshot.fen (cozy_chess)
├── legal_moves_cache: HashMap       # Legal moves indexed by source square
├── event_stream: Streaming          # gRPC event stream for real-time updates
├── engine_info: Option<EngineInfo>  # Latest engine analysis output
├── is_engine_thinking: bool         # Engine activity indicator
├── uci_log: Vec<UciLogEntry>       # UCI protocol message log (max 100)
├── paused: bool                     # Game pause state
├── selected_square: Option<Square>  # Currently selected piece
├── highlighted_squares: Vec<Square> # Legal destinations for selected piece
├── selectable_squares: Vec<Square>  # Squares with movable pieces
├── last_move: Option<(Square, Square)>      # Previous move (for highlighting)
├── best_move_squares: Option<(Square, Square)> # Engine recommendation
├── review_state: Option<ReviewState>        # Populated in review/analysis modes
└── pre_history: Vec<MoveRecord>             # Moves before current position
```

All updates flow through `apply_snapshot()` — parses FEN, updates board, refreshes game metadata.

### UiStateMachine (`fsm/mod.rs`)

Ephemeral UI state. No game logic, no server communication — purely controls what's on screen and how the user interacts with it.

```
UiStateMachine
├── mode: UiMode                          # Current UI mode (see FSM section)
├── tab_input: TabInputState              # Typeahead move input state
├── input_phase: InputPhase               # SelectPiece | SelectDestination | SelectPromotion
├── popup_menu: Option<PopupMenuState>    # Active popup menu (if any)
├── snapshot_dialog: Option<SnapshotDialogState> # Active snapshot dialog (if any)
├── review_tab: u8                        # Active review analysis tab
├── selected_promotion_piece: Piece       # User's promotion choice
├── focused_component: Option<Component>  # Which panel has focus (None = board)
├── expanded: bool                        # Whether focused panel fills the board area
├── visibility: HashMap<Component, bool>  # Panel show/hide state
└── scroll_state: HashMap<Component, u16> # Per-panel scroll position
```

## UI State Machine

The FSM tracks four mutually exclusive UI modes:

```rust
enum UiMode {
    StartScreen,    // Main menu
    GameBoard,      // Active gameplay
    ReviewBoard,    // Post-game review
    MatchSummary,   // Game result display
}
```

### Transitions

`transition_to(mode)` switches the mode and applies mode-specific setup:

```
StartScreen ──(start game)──> GameBoard
StartScreen ──(load review)──> ReviewBoard
GameBoard ──(game ends)──> MatchSummary
GameBoard ──(escape)──> StartScreen
ReviewBoard ──(escape)──> StartScreen
MatchSummary ──(new game / menu)──> StartScreen
```

Each mode configures panel visibility on entry:

| Panel             | GameBoard | ReviewBoard |
|-------------------|-----------|-------------|
| InfoPanel         | visible   | visible     |
| EnginePanel       | visible   | hidden      |
| HistoryPanel      | visible   | visible     |
| ReviewSummary     | hidden    | visible     |
| AdvancedAnalysis  | hidden    | visible     |

## Declarative Layout System

Layouts are data, not code. Each UI mode produces a `Layout` struct that the `Renderer` interprets.

### Type hierarchy

```
Layout
├── rows: Vec<Row>
│   └── Row
│       ├── height: Constraint
│       └── sections: Vec<Section>
│           └── Section
│               ├── constraint: Constraint
│               └── content: SectionContent
│                   ├── Component(component)     # Leaf: a single widget
│                   └── Nested(Vec<Section>)     # Recursive: vertical stack
└── overlay: Overlay                             # Modal dialog layer
```

**Constraint** types: `Length(u16)`, `Min(u16)`, `Percentage(u16)`, `Ratio(u16, u16)`

### GameBoard Layout

```
┌──────────────────────────────┬─────────────────┐
│                              │   InfoPanel (8)  │
│                              ├─────────────────┤
│      Board (min 10)          │  EnginePanel (12)│  ← if visible
│      [75%]                   ├─────────────────┤
│                              │  HistoryPanel    │
│                              │  (min 10)        │
│                              │  [25%]           │
├──────────────────────────────┴─────────────────┤
│ Controls (1 row)                                │
└─────────────────────────────────────────────────┘
```

When `tab_input` is active, a `TabInput` row (3 cells) appears below the board. When a panel is **expanded**, it replaces the board in the left column:

```
┌──────────────────────────────┬─────────────────┐
│                              │   InfoPanel      │
│   [expanded panel]           ├─────────────────┤
│      [75%]                   │  EnginePanel     │
│                              ├─────────────────┤
│                              │  HistoryPanel    │
│                              │  [25%]           │
├──────────────────────────────┴─────────────────┤
│ Controls                                        │
└─────────────────────────────────────────────────┘
```

### ReviewBoard Layout

```
┌───────────────┬──────────────────────┬─────────────────┐
│ AdvancedAnal. │                      │   InfoPanel (8)  │
│ (35%)         │                      ├─────────────────┤
├───────────────┤     Board (55%)      │  HistoryPanel    │
│ ReviewSummary │                      │  (min 10)        │
│ (min 10)      │                      │                  │
│ [20%]         │                      │  [25%]           │
├───────────────┴──────────────────────┴─────────────────┤
│ Controls                                                │
└─────────────────────────────────────────────────────────┘
```

Layout builders live in `states/game_board.rs` and `states/review_board.rs`. Each checks `fsm.expanded_component()` to decide between normal and expanded variants.

## Component Model

Components are the leaf nodes of the layout tree — each maps to a widget at render time.

```rust
enum Component {
    Board, TabInput, Controls,          // Non-selectable
    InfoPanel,                          // Selectable, not expandable
    HistoryPanel, EnginePanel,          // Selectable + expandable
    DebugPanel,                         // Selectable + expandable (hidden by default)
    ReviewTabs,                         // Not selectable (review mode)
    ReviewSummary, AdvancedAnalysis,    // Selectable + expandable (review mode)
}
```

| Component        | Selectable | Expandable | Default Visible |
|------------------|:----------:|:----------:|:---------------:|
| Board            |     -      |     -      |       yes       |
| TabInput         |     -      |     -      |    on demand    |
| Controls         |     -      |     -      |       yes       |
| InfoPanel        |    yes     |     -      |       yes       |
| HistoryPanel     |    yes     |    yes     |       yes       |
| EnginePanel      |    yes     |    yes     |       yes       |
| DebugPanel       |    yes     |    yes     |       no        |
| ReviewTabs       |     -      |     -      |    review only  |
| ReviewSummary    |    yes     |    yes     |    review only  |
| AdvancedAnalysis |    yes     |    yes     |    review only  |

**Focus mechanics:**

- `focused_component: None` → board context (default)
- `focused_component: Some(c), expanded: false` → panel is selected (highlighted border)
- `focused_component: Some(c), expanded: true` → panel fills the board area

Navigation uses layout-derived tab order: `tab_order(layout)` flattens visible, selectable components from the layout tree. `Tab`/`Shift+Tab` cycles through them. `h`/`l` navigates between sections (columns), `j`/`k` navigates within a section.

## Rendering Pipeline

`Renderer::render(frame, area, layout, game_session, fsm)` walks the layout tree:

1. **Split rows vertically** — each `Row.height` becomes a ratatui constraint
2. **Split sections horizontally** — each `Section.constraint` divides the row
3. **Render content recursively:**
   - `SectionContent::Component(c)` → render the widget for component `c`
   - `SectionContent::Nested(sections)` → split vertically again, recurse
4. **Render overlay** (if active) — painted on top of everything

Each `Component` variant maps to a specific widget (e.g., `Component::Board` → `BoardWidget`, `Component::InfoPanel` → `GameInfoPanel`).

## Controls and Overlays

### Controls

`UiStateMachine::derive_controls(game_session)` is the single source of truth for the controls bar. It returns `Vec<Control>` based on the current mode:

- **StartScreen**: `Enter Select`
- **MatchSummary**: `n New Game | Enter Menu | q Quit`
- **ReviewBoard**: `Tab Tabs | j/k Moves | Space Auto | Home/End Jump | Esc Menu`
- **GameBoard**: `i Input | p Pause | u Undo | Esc Menu | Tab Panels | @ UCI | Ctrl+C Quit` (conditional on game mode and state)

The renderer generically renders `Vec<Control>` as styled spans.

### Overlays

`derive_overlay()` checks FSM state in priority order:

1. `InputPhase::SelectPromotion` → `Overlay::PromotionDialog`
2. `popup_menu.is_some()` → `Overlay::PopupMenu`
3. `snapshot_dialog.is_some()` → `Overlay::SnapshotDialog`
4. Otherwise → `Overlay::None`

Overlays render as modal widgets on top of the full screen area.

## Input Handling

Input dispatch follows a modal priority chain — the topmost active modal consumes the event:

```
TabInput (typeahead move entry)         ← highest priority
  → PopupMenu (in-game pause menu)
    → SnapshotDialog (review snapshot)
      → PromotionDialog (piece selection)
        → Global toggles (@ # $ for panel visibility)
          → Context-based handling          ← lowest priority
```

### Context-based input

When no modal is active, input dispatches by focus state:

**Board context** (`focused_component: None`):
- Game mode: character input builds algebraic notation (`e2` → `e4`), `i` activates TabInput, `Tab` enters panel selection, `p` pauses, `Esc` opens popup menu
- Review mode: `j`/`k` or arrows navigate plies, `Space` toggles auto-play, `Home`/`End` jump to start/end, `n`/`p` jump to critical moments

**Component selected** (`focused_component: Some(_), expanded: false`):
- `h`/`l` or Left/Right — navigate between sections (columns)
- `j`/`k` or Up/Down — navigate within section
- `J`/`K` (Shift) — scroll panel content
- `Tab`/`Shift+Tab` — cycle all selectable components
- `Enter` — expand the panel
- `Esc` — clear focus, return to board

**Component expanded** (`focused_component: Some(_), expanded: true`):
- `j`/`k` or Up/Down — scroll content
- `PageUp`/`PageDown` — jump to top/bottom
- `Esc` — collapse back to selected state

## Render Loop

The application runs an outer menu → game → menu loop. The game phase enters `run_ui_loop`, which uses `tokio::select!` with biased polling:

```rust
loop {
    tokio::select! {
        biased;
        event = crossterm_events.next() => { /* keyboard */ }
        consumed = state.poll_event_async() => { /* server gRPC stream */ }
        _ = tick_interval.tick() => { /* 30fps UI refresh */ }
    }

    // Auto-play: advance review ply every 750ms if active
    // Drain buffered server events
    // Render frame: fsm.layout() → Renderer::render()
    // Handle keyboard → AppAction (Continue | Quit | ReturnToMenu | ...)
}
```

- **Keyboard** (highest priority) — immediate response to user input
- **Server events** — engine analysis updates, state changes from gRPC stream
- **UI tick** (33ms) — ensures timers and animations update even without events

## Widget Inventory

| Widget                 | File                        | Description                                          |
|------------------------|-----------------------------|------------------------------------------------------|
| BoardWidget            | `board.rs`                  | Chess board with adaptive sizing (S/M/L), overlays   |
| BoardOverlay           | `board_overlay.rs`          | Layered square tints, outlines, and arrows            |
| MiniBoardWidget        | `mini_board.rs`             | Compact 18x10 Unicode board for expanded pane mode    |
| GameInfoPanel          | `game_info_panel.rs`        | Game mode, turn, timers, status                       |
| MoveHistoryPanel       | `move_history_panel.rs`     | Scrollable move list with classification markers      |
| EngineAnalysisPanel    | `engine_panel.rs`           | Depth, eval score, nodes/sec, principal variation     |
| MoveAnalysisPanel      | `move_analysis_panel.rs`    | Per-move classification and eval delta                |
| AdvancedAnalysisPanel  | `advanced_analysis_panel.rs`| Tactical patterns, king safety, tension metrics       |
| ReviewSummaryPanel     | `review_summary_panel.rs`   | Accuracy scores, eval graph, move quality breakdown   |
| ReviewTabsPanel        | `review_tabs_panel.rs`      | Tabbed view for review data (Overview / Position)     |
| UciDebugPanel          | `uci_debug_panel.rs`        | Syntax-highlighted UCI protocol log                   |
| TabInputWidget         | `tab_input.rs`              | Two-phase typeahead move entry (piece → destination)  |
| MenuWidget             | `menu.rs`                   | Start screen menu with game configuration             |
| PopupMenuWidget        | `popup_menu.rs`             | In-game modal menu (Restart, Suspend, Quit)           |
| PromotionWidget        | `promotion_dialog.rs`       | Pawn promotion piece selector (Q/R/B/N)               |
| FenDialogWidget        | `fen_dialog.rs`             | FEN input with saved positions table                  |
| SnapshotDialogWidget   | `snapshot_dialog.rs`        | Create playable snapshot from review position         |
| SelectableTableState   | `selectable_table.rs`       | Reusable table with keyboard navigation               |

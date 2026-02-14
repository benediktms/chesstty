# Testing the Chess TUI Client-Server Application

## Quick Start

### 1. Start the Server

In one terminal:
```bash
cargo run --bin chesstty-server
```

You should see:
```
Chess gRPC server listening on [::1]:50051
```

### 2. Start the TUI Client

In another terminal:
```bash
cargo run --bin chesstty-tui
```

### 3. Start in Simple Mode (Optional)

For a text-based interface:
```bash
cargo run --bin chesstty-tui -- --simple
```

## Features Implemented

### ✅ Core Gameplay
- **Board Display**: 128-character wide board with 16x16 ASCII pieces
- **Move Input**: Type square notation (e.g., "e2" then "e4")
- **Legal Move Highlighting**: Selected pieces show legal destinations in green
- **Typeahead Input**: As you type, matching squares are highlighted in cyan
- **Move History**: Displays moves in standard algebraic notation
- **Undo**: Type "undo" or "u"
- **Reset**: Type "reset" or "r"

### ✅ Promotion
- When a pawn reaches the back rank, you'll see a promotion dialog
- Press q/r/b/n to select Queen/Rook/Bishop/Knight
- Dialog shows all available pieces with visual indicators

### ✅ Game Info Panel
- Shows current game mode
- Displays whose turn it is
- Shows selected square and legal moves
- Displays engine evaluation (if available)

### ✅ Controls Panel
- **Dynamic Phase Display**: Shows current input phase (Select Piece/Destination/Promotion)
- **Live Input Buffer**: See what you're typing in real-time
- **Status Messages**: Color-coded feedback (green for success, red for errors)
- **Quick Reference**: Shows available keyboard shortcuts

### ✅ Engine Analysis Panel
- **# Key Toggle**: Press # to show/hide the engine analysis panel
- **Real-time Analysis**: Shows engine thinking information while analyzing
- **Depth Display**: Current search depth and selective depth
- **Evaluation Score**: Position evaluation in pawns (+/- for advantage, M for mate)
- **Node Count**: Number of positions analyzed and nodes per second
- **Principal Variation**: Best line found by the engine
- **Thinking Indicator**: Shows when engine is actively thinking

### ✅ UCI Debug Panel
- **@ Key Toggle**: Press @ to show/hide the UCI debug panel
- **Engine Communication**: See all messages between client and Stockfish
- **Timestamped Logs**: Track when each UCI command was sent/received
- **Color Coding**: Different colors for commands vs responses

### ✅ Menu System
- **Game Mode Selection**: Choose between Human vs Human, Human vs Engine, or Engine vs Engine
- **Difficulty Levels**: Beginner (skill 3), Intermediate (skill 10), Advanced (skill 15), Master (skill 20)
- **Time Controls**: None, Blitz (3 min), Rapid (10 min), Classical (30 min) - displayed but not yet enforced
- **Starting Position**: Standard or Custom FEN
- **Navigation**: Use arrow keys (up/down to select row, left/right to cycle options), Enter to start, Esc/Q to quit

### ✅ FEN Input Dialog
- **Open Dialog**: Press Enter or Space on "Start Position: Custom FEN" row, or press "Start Game" when Custom FEN is selected without a position
- **Input FEN**: Type FEN string directly
- **History Selection**: Press Tab to switch focus to history list, use Up/Down to navigate, Right to select
- **Validation**: Basic FEN format validation (8 ranks, side to move)
- **Confirm**: Press Enter to accept FEN
- **Cancel**: Press Esc to close dialog without saving

### 🚧 Features in Progress
- Time control enforcement (selected in menu but not applied to game clock)
- Enhanced game end UI (detection works, needs dedicated result display panel)

## Input Commands

### During Gameplay
- **Square notation**: `e2` (select piece) → `e4` (move to)
  - Typeahead: Start typing and matching squares will be highlighted
- **#**: Toggle engine analysis panel (see engine thinking)
- **@**: Toggle UCI debug panel (see engine communication)
- **Undo**: `u` or `undo`
- **Reset**: `r` or `reset`
- **Escape**: Clear selection and input buffer
- **Ctrl+C**: Quit

### During Promotion
- **q**: Promote to Queen
- **r**: Promote to Rook
- **b**: Promote to Bishop
- **n**: Promote to Knight
- **Escape**: Cancel promotion

## Testing Checklist

### Menu System
1. Start the TUI client - menu should appear automatically
2. Use Up/Down arrows to navigate between menu items
3. Use Left/Right arrows to cycle through options for each row
4. Test game mode selection (cycles: Human vs Human ↔ Human vs Engine ↔ Engine vs Engine)
5. Test difficulty selection (cycles: Beginner ↔ Intermediate ↔ Advanced ↔ Master)
6. Test time control selection (cycles: None ↔ Blitz ↔ Rapid ↔ Classical)
7. Test start position (toggles: Standard ↔ Custom FEN)
8. Navigate to "Start Game" and press Enter to start
9. Press Esc or Q to quit from menu (should exit cleanly)
10. Verify selected configuration is applied (check game mode in info panel, engine behavior)

### FEN Dialog
1. In menu, set Start Position to "Custom FEN"
2. Press Enter or Space on the Start Position row - FEN dialog should appear
3. Type a FEN string (e.g., "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
4. Press Enter - if valid, dialog closes and FEN is saved
5. Try invalid FEN (e.g., "invalid") - should show error message
6. Press Tab to switch to history list
7. Use Up/Down to navigate FEN history
8. Press Right to select a FEN from history - should copy to input
9. Press Esc to cancel and close dialog
10. With Custom FEN selected, navigate to "Start Game" and press Enter
11. If no FEN selected yet, dialog should open automatically
12. After confirming FEN, game should start with that position

### Basic Movement
1. Start server and client
2. Type `e2` - should select the white pawn (square highlighted in yellow)
3. Type `e4` - pawn should move (legal squares highlighted in green)
4. Check that move history shows "e4" and last move highlighted in blue
5. Type `e7` then `e5` for black's response

### Typeahead Feature
1. During "Select Piece" phase, type `e`
2. All selectable squares starting with 'e' should be highlighted in cyan
3. Type `2` to complete to `e2`
4. Press Enter to select the piece

### Controls Panel
1. Observe the controls panel shows "Phase: Select Piece" initially
2. After selecting a piece, it should change to "Phase: Select Destination"
3. Watch the input buffer update as you type
4. Status messages should appear color-coded (green for success, red for errors)

### Engine Analysis Panel & Engine Play
1. Start server: `cargo run --bin chesstty-server`
2. Start client: `cargo run --bin chesstty-tui`
3. In menu, select "Human vs Engine" or "Engine vs Engine"
4. Choose difficulty level (Beginner to Master)
5. Press Enter to start game
6. **Watch engine play automatically:**
   - Engine analysis panel shows real-time thinking
   - Depth increases as search progresses
   - Score updates (green/red for white/black advantage)
   - Principal variation shows best line
   - Engine automatically makes moves when it's its turn
7. Press `#` to toggle engine analysis panel on/off
8. Make your moves with square notation (e.g., "e2" → "e4")

### UCI Debug Panel
1. Press `@` - UCI debug panel should appear at bottom
2. Make a move against engine (if configured)
3. Watch UCI commands and responses appear in real-time
4. Press `@` again - panel should hide

### Undo Functionality
1. Make a few moves
2. Type `undo` or just `u`
3. Last move should be undone
4. Check move history updated
5. Controls panel should show success message

### Promotion
1. Set up a position where pawn can promote (or play until promotion)
2. Move pawn to 8th rank
3. Promotion dialog should appear with all piece options
4. Press `q` to promote to queen (or r/b/n for other pieces)
5. Dialog should close and piece should be promoted

### Reset
1. Make some moves
2. Type `reset` or just `r`
3. Board should return to starting position
4. Move history should clear

## Troubleshooting

### Server Connection Failed
- Ensure server is running: `cargo run --bin chesstty-server`
- Check server is listening on [::1]:50051
- Server logs: `chesstty-server.log`

### Client Errors
- Client logs: `chesstty-client.log`
- Check for error messages in the status bar (bottom of UI)

### Display Issues
- Terminal should be at least 140 characters wide for full board display
- Use a terminal with good Unicode support for chess pieces

## Known Limitations

1. **Engine Event Streaming**: Engine moves initiated but real-time thinking display not yet implemented
2. **Time Control Enforcement**: Time controls can be selected in menu but are not yet enforced during gameplay
3. **Game Mode Changes**: Cannot change game mode after startup (must restart and use menu)
4. **Advanced FEN Validation**: Basic FEN validation only (checks structure, not board validity)

## Development Notes

### Architecture
- **Server**: Manages game state, validates moves, runs Stockfish engine
- **Client**: Renders UI, handles input, communicates via gRPC
- **Protocol**: gRPC/Protobuf for type-safe communication
- **State**: ClientState maintains cached board position for fast rendering

### Key Files
- `client-tui/src/state.rs`: Client state management
- `client-tui/src/ui/full_ui.rs`: Main UI rendering and input
- `client-tui/src/ui/widgets/`: Individual UI components
- `server/src/session.rs`: Server-side game session management

## Completed UI Features

✅ Menu system for game mode selection
✅ FEN input dialog with history and validation
✅ Engine analysis panel (# key toggle)
✅ Real-time engine event streaming (depth, score, PV, nodes, NPS)
✅ UCI debug panel toggle (@ key)
✅ Typeahead square input
✅ Controls panel with current phase display
✅ Dynamic UI layout based on panel visibility

## Next Steps

To complete remaining features:
1. Implement time control enforcement (game clocks, timers, countdown display)
2. Enhanced game end UI (dedicated result panel for checkmate/stalemate/draw)
3. Add ability to save/load games (PGN export/import)
4. Game notation export (copy PGN to clipboard)
5. Position analysis mode (explore variations, no game state)

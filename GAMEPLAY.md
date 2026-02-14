# ChessTTY - Gameplay Guide

## Starting a Game

1. Run `cargo run`
2. You'll see the startup menu
3. Use **↑/↓** to navigate, **←/→** to change settings
4. Press **Enter** on "Start Game"

## Making Moves

The game uses **algebraic notation** for square input:

### Basic Move Flow

1. **Type the source square** (e.g., `e2`)
   - As you type, you'll see the input at the bottom
   - When you complete 2 characters, the square will be selected
   - **Legal moves are highlighted in GREEN**

2. **Type the destination square** (e.g., `e4`)
   - The piece will move if it's a legal move
   - The move will be highlighted in BLUE

### Example Move Sequence

```
Type: e2
Result: Pawn on e2 is selected, legal moves (e3, e4) shown in green

Type: e4
Result: Pawn moves from e2 to e4, move highlighted in blue
```

## Visual Feedback

### Square Colors
- **Light Tan** - Light squares
- **Dark Brown** - Dark squares
- **Yellow** - Currently selected square
- **Green** - Legal move destinations for selected piece
- **Blue** - Last move made (both from and to squares)

### Status Messages
The controls panel shows:
- What square you selected
- If a move was made
- Error messages (e.g., "That's not your piece!")

## Keyboard Controls

### During Game
- **Type square** (e.g., `e2`, `d5`) - Select piece or move
- **Esc** - Clear selection and input buffer
- **Backspace** - Delete last character typed
- **u** - Undo last move
- **n** - New game (return to menu)
- **q** - Quit

### In Menu
- **↑/↓** - Navigate options
- **←/→** - Change selected option
- **Enter** - Confirm/start game
- **q** - Quit

## Game Rules

### Piece Movement
The game enforces all standard chess rules:
- Legal moves are automatically calculated by cozy-chess
- You can only select pieces of your color
- Only highlighted (green) squares are valid destinations
- Castling, en passant, and pawn promotion are supported

### Pawn Promotion
When a pawn reaches the last rank:
- It automatically promotes to a **Queen**
- (UI for choosing promotion piece coming soon)

### Game End
The game detects:
- **Checkmate** - Game over, winner declared
- **Stalemate** - Draw
- **Insufficient material** - Draw
- **50-move rule** - Draw (if applicable)

## Tips

1. **Take your time typing** - The game waits for 2 characters before parsing
2. **Use Esc liberally** - Clear selection if you change your mind
3. **Watch the status messages** - They tell you what's happening
4. **Green squares are your friends** - Only legal moves are shown
5. **Undo is your friend** - Made a mistake? Press `u`

## Common Issues

### "That's not your piece!"
- You selected an opponent's piece
- Make sure you're selecting your color

### "No piece on that square"
- The square you typed is empty
- Check your notation (file then rank: `a1` not `1a`)

### "Illegal move"
- The destination square isn't a legal move
- Only green-highlighted squares are valid

### Input not working
- Make sure you're typing lowercase letters
- Valid format: `[a-h][1-8]` (e.g., `e2`, `d7`)
- Press **Backspace** or **Esc** to clear and retry

## Advanced Features

### Difficulty Levels (vs Engine)
- **Beginner** - Stockfish Skill 2 (makes mistakes)
- **Intermediate** - Stockfish Skill 10 (club player)
- **Advanced** - Stockfish Skill 17 (expert)
- **Master** - Stockfish Skill 20 (near perfect play)

### Time Controls
- **None** - Unlimited time
- **Blitz** - 3 minutes per side
- **Rapid** - 10 minutes per side
- **Classical** - 30 minutes per side

(Time controls and engine play coming soon!)

## Enjoy!

ChessTTY is designed to be a fast, keyboard-driven chess experience.
No mouse needed - just type and play!

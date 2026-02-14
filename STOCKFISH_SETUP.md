# Stockfish Setup Guide

ChessTTY uses Stockfish as its chess engine. You need to install Stockfish to play against the computer.

## Quick Install

### macOS (Homebrew)
```bash
brew install stockfish
```

### Ubuntu/Debian
```bash
sudo apt-get install stockfish
```

### Fedora
```bash
sudo dnf install stockfish
```

### Arch Linux
```bash
sudo pacman -S stockfish
```

### Manual Installation

1. Download Stockfish from: https://stockfishchess.org/download/
2. Extract the archive
3. Copy the `stockfish` executable to one of these locations:
   - `/usr/local/bin/stockfish`
   - `/usr/bin/stockfish`
   - `/opt/homebrew/bin/stockfish` (macOS M1/M2)
   - Or ensure it's in your `PATH`

## Verify Installation

```bash
stockfish --help
```

If this shows help text, Stockfish is installed correctly!

## Troubleshooting

### "Stockfish not found" error

ChessTTY checks these locations automatically:
1. `/usr/local/bin/stockfish`
2. `/usr/bin/stockfish`
3. `/opt/homebrew/bin/stockfish`
4. `/usr/games/stockfish`
5. `stockfish` (in PATH)

If Stockfish is installed elsewhere, either:
- Create a symlink to one of the above locations
- Add Stockfish's directory to your PATH

### Permission denied

Make sure the Stockfish binary is executable:
```bash
chmod +x /path/to/stockfish
```

## Skill Levels

ChessTTY supports 4 difficulty levels that map to Stockfish settings:

| Level        | Skill | Think Time | Description |
|--------------|-------|------------|-------------|
| Beginner     | 2     | 200ms      | Makes mistakes, good for learning |
| Intermediate | 10    | 500ms      | Club player strength |
| Advanced     | 17    | 1s         | Expert level |
| Master       | 20    | 2s         | Near-perfect play |

## Testing

Once installed, select "Human vs Engine" from the menu and the engine should:
1. Show "Engine ready!" when initialized
2. Make moves automatically when it's its turn
3. Show "Engine thinking..." while calculating

Enjoy playing chess! ♟️

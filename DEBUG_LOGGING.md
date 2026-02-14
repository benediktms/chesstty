# Debug Logging

## Overview

ChessTTY now includes comprehensive debug logging using the `tracing` crate to help diagnose engine issues, particularly when the engine appears to hang or not make moves.

## Log File Location

All debug logs are written to: `chesstty.log`

The log file uses daily rotation, so you'll see files like:
- `chesstty.log` (today's log)
- `chesstty.log.2024-01-15` (previous days)

## Log Levels

The logging system supports different verbosity levels:

- **ERROR**: Critical errors that prevent functionality
- **WARN**: Warnings about potential issues
- **INFO**: Important state changes and events (default)
- **DEBUG**: Detailed execution flow
- **TRACE**: Very verbose, including all UCI communication

## Changing Log Level

Set the `RUST_LOG` environment variable before running:

```bash
# Default (info level)
cargo run

# Debug level (more detail)
RUST_LOG=debug cargo run

# Trace level (maximum verbosity, includes all UCI messages)
RUST_LOG=trace cargo run

# Only show errors
RUST_LOG=error cargo run

# Target specific modules
RUST_LOG=chesstty::engine=trace,chesstty::app=debug cargo run
```

## What's Logged

### Engine Communication
- **Engine spawn**: Process startup, UCI initialization
- **UCI messages**: All commands sent to and received from Stockfish (at trace level)
- **Engine commands**: SetPosition, Go, Stop, Quit with parameters
- **Engine responses**: BestMove, Info, Ready events
- **Timeouts**: If engine doesn't respond within expected time

### Game Flow
- **Engine turn detection**: When it's determined to be the engine's turn
- **Move triggering**: When `make_engine_move()` is called
- **Move processing**: When engine moves are received and applied
- **Game state**: FEN positions, skill level, move times

### Error Conditions
- Engine spawn failures
- UCI communication errors
- Channel send/receive failures
- Invalid moves
- Timeouts

## Typical Engine Hang Scenarios

When investigating a hang, look for these patterns in the logs:

### 1. Engine Not Responding to UCI
```
INFO  Starting engine calculation with movetime=500ms
DEBUG UCI >> go movetime 500
```
If you don't see a corresponding `Received bestmove` within the specified time, the engine may have crashed or hung.

### 2. Commands Not Being Sent
```
INFO  Triggering engine move after human move
DEBUG Queueing command: Go(...)
```
Check if you see the corresponding `UCI >>` line showing the command was actually sent to the engine.

### 3. Events Not Being Processed
```
INFO  Received bestmove from engine: ...
```
If the bestmove is received but the move isn't applied, there may be an issue in the game loop.

### 4. Engine Process Issues
```
ERROR Stockfish stdout EOF - engine closed
WARN  Output reader task exiting
```
This indicates the engine process terminated unexpectedly.

## Example Debug Session

1. Run with trace logging:
   ```bash
   RUST_LOG=trace cargo run
   ```

2. Play a few moves until the engine hangs

3. Exit the game (Ctrl+C or 'q')

4. Examine the log:
   ```bash
   tail -100 chesstty.log
   ```

5. Look for:
   - The last move that was made successfully
   - Whether `make_engine_move` was called
   - Whether commands were sent to the engine
   - Whether the engine responded
   - Any error messages

## Common Issues and Log Patterns

### Engine Hangs After Specific Move
Look for the pattern of moves leading up to the hang. The FEN string will be logged:
```
DEBUG Current FEN: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
```

### Commands Not Reaching Engine
If you see "Queueing command" but no "UCI >>" line, the stdin writer task may have failed.

### No Response from Engine
If you see "UCI >> go movetime X" but no "Received bestmove" within X milliseconds, the engine is stuck calculating or has crashed.

### Channel Errors
```
ERROR Failed to send command to queue: channel closed
```
This indicates the engine task has terminated.

## Performance Impact

- **INFO level** (default): Minimal impact, suitable for production use
- **DEBUG level**: Slight performance impact, good for troubleshooting
- **TRACE level**: Noticeable performance impact due to logging every UCI message, use only when debugging

## Cleaning Up Logs

Old log files are kept automatically. To clean up:

```bash
rm chesstty.log.*
```

## Getting Help

When reporting engine hang issues, please include:
1. The last 100-200 lines of the log file around when the hang occurred
2. The sequence of moves that led to the hang
3. Your Stockfish version: `stockfish --version`
4. Your system info (OS, terminal)

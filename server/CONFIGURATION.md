# ChessTTY Server Configuration

## Data Directory Configuration

The ChessTTY server stores runtime data (suspended sessions and user-created positions) in a configurable data directory.

### Directory Priority

The server determines the data directory using the following priority:

1. **`CHESSTTY_DATA_DIR` environment variable** (if set)
2. **`~/.config/chesstty/data`** (production default)
3. **`./data`** (development fallback)

### Environment Variable

Set the `CHESSTTY_DATA_DIR` environment variable to use a custom data directory:

```bash
# Development with custom location
export CHESSTTY_DATA_DIR=/tmp/chesstty_dev
cargo run --package chesstty-server

# Production deployment
export CHESSTTY_DATA_DIR=/var/lib/chesstty/data
./chesstty-server
```

### Data Structure

The data directory contains:

```
$CHESSTTY_DATA_DIR/
├── sessions/          # Suspended game sessions (JSON files)
│   ├── session_1234567890.json
│   └── session_9876543210.json
└── positions/         # User-created and default positions (JSON files)
    ├── default_standard_starting_position.json
    ├── default_sicilian_defense.json
    └── pos_1234567890.json
```

### Default Positions

Default chess positions (openings, endgames, puzzles) are stored in version control at:

```
server/defaults/positions/
```

On first run, the server copies these default positions into the runtime data directory. This allows:

- **Version control** of curated default positions
- **User customization** without modifying version-controlled files
- **Production deployments** to have consistent defaults

### Production Deployment

For production environments:

1. **Set environment variable**:
   ```bash
   export CHESSTTY_DATA_DIR=/var/lib/chesstty/data
   ```

2. **Create data directory**:
   ```bash
   mkdir -p /var/lib/chesstty/data
   ```

3. **Set permissions** (if running as non-root):
   ```bash
   chown -R chesstty:chesstty /var/lib/chesstty/data
   ```

4. **Start server**:
   ```bash
   ./chesstty-server
   ```

### Development Setup

For local development, no configuration is needed. The server will use `./data` by default.

To use a custom location for testing:

```bash
CHESSTTY_DATA_DIR=/tmp/chesstty_test cargo run --package chesstty-server
```

### Notes

- The data directory is **not** version controlled (ignored in `.gitignore`)
- Default positions in `server/defaults/` **are** version controlled
- Each server instance can have its own isolated data directory
- The server creates necessary subdirectories automatically on startup

# ChessTTY Server Configuration

## Persistence Configuration

The server stores runtime data in a SQLite database and supports one-time migration from legacy JSON files.

### Database Path Priority

`chesstty-server` resolves the SQLite database path in this order:

1. `CHESSTTY_DB_PATH` (if set)
2. Platform application data directory via `directories::ProjectDirs`:
   - macOS: `~/Library/Application Support/chesstty/chesstty.db`
   - Linux: `~/.local/share/chesstty/chesstty.db`
3. `./data/chesstty.db` (fallback)

Example:

```bash
export CHESSTTY_DB_PATH=/tmp/chesstty-dev.db
cargo run -p chesstty-server
```

## Legacy JSON Migration

On startup, the server checks for legacy JSON data and imports it into SQLite.

### Legacy Data Directory Priority

The migration source directory is resolved in this order:

1. `CHESSTTY_DATA_DIR` (if set)
2. `~/.config/chesstty/data`
3. `./data`

Example:

```bash
export CHESSTTY_DATA_DIR=/var/lib/chesstty/legacy-data
cargo run -p chesstty-server
```

Migration is idempotent: running the server repeatedly does not duplicate migrated records. Legacy JSON files are left in place as backup.

## Defaults Directory

Default positions are version-controlled in:

```text
server/defaults/
```

The defaults directory is resolved relative to the server crate and is not environment-configurable.

## Runtime Logging

```bash
RUST_LOG=debug cargo run -p chesstty-server
```

## Notes

- `CHESSTTY_DB_PATH` controls where live data is persisted.
- `CHESSTTY_DATA_DIR` is used only as a migration source for legacy JSON files.
- The server listens on `[::1]:50051` by default.

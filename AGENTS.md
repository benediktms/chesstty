# ChessTTY Agent Guidelines

Terminal-based chess app in Rust. Server-authoritative client-server architecture. Cargo workspace with 8 crates.

## Structure
```
chesstty/
├── proto/          # gRPC protocol (proto/proto/*.proto)
├── server/         # Game server (actor model)
├── chess-client/   # gRPC client library
├── client-tui/     # Ratatui TUI
├── chess/          # Core logic (cozy-chess)
├── engine/         # Stockfish UCI wrapper
├── analysis/       # Post-game analysis
└── chesstty/      # Main binary
```

## Commands
```bash
just build     # Build all
just test     # Run all tests
just lint     # Clippy
just server   # Run server
just tui      # Run TUI
```

Single test: `cargo test -p <crate> <test_name>`

## Lints (enforced)
```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
enum_glob_use = "deny"
```
**Never use**: unsafe blocks, enum glob imports, type suppression.

## Code Style
- Standard rustfmt (4-space indent)
- Imports: external crates first (alphabetical), then internal
- Types: `PascalCase`, functions/vars: `snake_case`, constants: `SCREAMING_SNAKE_CASE`

## Error Handling
- `thiserror` for domain errors with `#[from]` for conversion
- `anyhow` for application-level
- Result types: `pub type XxxResult<T> = Result<T, XxxError>;`

## Async
- tokio runtime with `#[tokio::main]`
- Channels: `mpsc`, `broadcast`, `oneshot`

## Testing
- Inline: `#[cfg(test)] mod tests { #[test] fn ... }`
- Integration: `client-tui/tests/` (only crate with tests/ dir)
- Async: `#[tokio::test]` in server/client-tui
- proptest in chess crate for property-based tests

## Key Patterns
- Domain/Proto separation (conversion at service boundaries)
- Actor model: `server/src/session/actor.rs`
- Snapshot-based state: `SessionSnapshot` on every change

## Deviation
`chesstty/Cargo.toml` hardcodes deps instead of using `workspace = true`.

## Adding Features
1. Domain types in appropriate crate
2. gRPC endpoint in proto + server
3. Client method in chess-client
4. UI in client-tui if needed

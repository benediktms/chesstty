# ChessTTY Agent Guidelines

Terminal-based chess app in Rust. Server-authoritative client-server architecture. Cargo workspace with 7 crates.

## Structure
```
chesstty/
├── proto/          # gRPC protocol (proto/proto/*.proto)
├── server/         # Game server (actor model)
├── chess-client/   # gRPC client library
├── client-tui/     # Ratatui TUI
├── chess/          # Core logic (cozy-chess)
├── engine/         # Stockfish UCI wrapper
└── analysis/       # Post-game analysis
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

## Git Conventions
- **All commits MUST use [Conventional Commits](https://www.conventionalcommits.org/)** format: `type(scope): description`
- Types: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `ci`, `build`
- Scope is optional but encouraged for crate-specific changes: `feat(server): add health endpoint`
- Breaking changes: add `!` after type/scope: `feat!: remove legacy API`
- Changelog is auto-generated from these commits via git-cliff (`cliff.toml`)
- `chore`, `ci`, `build` commits are excluded from the changelog

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

## Adding Features
1. Domain types in appropriate crate
2. gRPC endpoint in proto + server
3. Client method in chess-client
4. UI in client-tui if needed

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- bv-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
bd ready              # Show issues ready to work (no blockers)
bd list --status=open # All open issues
bd show <id>          # Full issue details with dependencies
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id> --reason="Completed"
bd close <id1> <id2>  # Close multiple issues at once
bd sync               # Commit and push changes
```

### Workflow Pattern

1. **Start**: Run `bd ready` to find actionable work
2. **Claim**: Use `bd update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `bd close <id>`
5. **Sync**: Always run `bd sync` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `bd ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers, not words)
- **Types**: task, bug, feature, epic, question, docs
- **Blocking**: `bd dep add <issue> <depends-on>` to add dependencies

### Session Protocol

**Before ending any session, run this checklist:**

```bash
git status              # Check what changed
git add <files>         # Stage code changes
bd sync                 # Commit beads changes
git commit -m "..."     # Commit code
bd sync                 # Commit any new beads changes
git push                # Push to remote
```

### Best Practices

- Check `bd ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `bd create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always `bd sync` before ending session

<!-- end-bv-agent-instructions -->

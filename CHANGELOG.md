# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Features

- Add automatic changelog generation

## [0.1.0] - 2026-02-24

### Bug Fixes

- Flakey test
- Include lock file
- Correctly use fallback socket and pid paths for tests
- Test ci pipeline env setting
- Mermaid syntax issue
- Tui crate name
- Un-expanding pane
- Session suspension and persitance and FEN setup
- Stockfish/cozy-chess casteling notation bridge
- Piece graphics

### Documentation

- Update daemon and shim entrypoint info
- Update readmes
- Fix stale references and add missing documentation
- Update READMEs

### Features

- Redirect shim server logs away from TUI
- Fix remaining TCP references to use UDS
- Use UDS instead of TCP for server connection
- Wire up CLI shim to spawn server and TUI
- Add start command to run shim CLI
- Migrate from TCP to Unix Domain Socket
- Add connect_uds for Unix Domain Socket connections
- Add wait module with socket polling and defaults
- Add daemon module with PID ops and double-fork daemonization
- Add dev socket/PID paths to justfile
- Add config module and wire CLI to shim crate
- Add config module and wire CLI to shim crate
- Scaffold chesstty CLI crate and add to workspace
- Wire typeahead squares into board overlay as outlines
- Wire outline rendering into BoardWidget render loop
- Fix panel scrolling in ComponentSelected context
- Drop tab selection in favour of numeric panel selection
- Adopt state-view-update approach
- Reword tui arcitecture
- Init beads
- Migrate to section layout
- Dynamic column replacement in layout during pane expansion
- Tab selection testing
- Consolidate pane managment and tab traversal
- Determine persistance directory based on config
- Implement pausing for EvE
- Clean up client logging file
- Rearchitect widget, add timer, session suspension
- Update UI positioning and fix UCI debug panel
- Move to client/server architecture
- Add FEN game setup
- Add promotion
- Add ascii piece prefix to move history
- Update pieces graphics
- Support vim key navigation in menu
- Add move history and board resizing
- Stockfish integration extension
- Bootstrap project

### Performance

- Multithread stockfish engine and use async client events

### Refactoring

- Address type complexity lints
- Modularize code
- Clean up dead code
- Remove polling implementation


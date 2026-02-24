# Changelog

All notable changes to this project will be documented in this file.

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

- Drop tab selection in favour of numeric panel selection
- Adopt state-view-update approach
- Reword tui arcitecture
- Init beads
- Migrate to section layout
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


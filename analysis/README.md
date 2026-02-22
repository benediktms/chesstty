# analysis - Post-Game Analysis & Board Evaluation

Post-game analysis library for ChessTTY. Classifies moves by quality, computes per-position tactical and structural features, detects critical moments, and builds psychological profiles of player behaviour across a game.

## Module Structure

```
analysis/src/
├── lib.rs                      # Public re-exports
├── review_types.rs             # MoveClassification, PositionReview, GameReview, compute_accuracy
└── board_analysis/
│   ├── mod.rs                  # Re-exports and detect_tactics() entry point
│   ├── tactical_types.rs       # TacticalTag, TacticalTagKind, TacticalEvidence, TacticalLine
│   ├── detector.rs             # TacticalDetector trait, TacticalContext
│   ├── attack_map.rs           # AttackMap, Attacker, PinInfo
│   ├── fork_detector.rs        # ForkDetector, DoubleAttackDetector
│   ├── pin_detector.rs         # PinDetector
│   ├── skewer_detector.rs      # SkewerDetector
│   ├── hanging_detector.rs     # HangingPieceDetector
│   ├── back_rank_detector.rs   # BackRankDetector
│   ├── discovered_attack_detector.rs  # DiscoveredAttackDetector
│   ├── mate_threat_detector.rs # MateThreatDetector
│   ├── sacrifice_detector.rs   # SacrificeDetector
│   ├── zwischenzug_detector.rs # ZwischenzugDetector
│   ├── reducer.rs              # reduce_tags (deduplication and ranking)
│   ├── king_safety.rs          # KingSafetyMetrics, PositionKingSafety, compute_king_safety
│   ├── tension.rs              # PositionTensionMetrics, compute_tension
│   └── helpers.rs              # attacked_squares, attackers_of, piece_attacks, piece_value
└── advanced/
    ├── mod.rs                  # Re-exports for advanced submodules
    ├── types.rs                # AdvancedPositionAnalysis, AdvancedGameAnalysis, PsychologicalProfile, AnalysisConfig
    ├── critical.rs             # is_critical_position (multi-signal criticality detection)
    └── psychological.rs        # compute_psychological_profile
```

## Key Types

### MoveClassification

Move quality relative to the engine's best move, derived from centipawn loss:

```rust
pub enum MoveClassification {
    Brilliant,    // Better than engine expected
    Best,         // 0 cp loss
    Excellent,    // 1-10 cp loss
    Good,         // 11-30 cp loss
    Inaccuracy,   // 31-100 cp loss
    Mistake,      // 101-300 cp loss
    Blunder,      // 300+ cp loss
    Forced,       // Only one legal move available
    Book,         // Opening book move
}
```

Maps to PGN NAG glyphs via `to_nag()`: `!!` (3), `!` (1), `?!` (6), `?` (2), `??` (4).

### PositionReview

Analysis result for a single ply:

```rust
pub struct PositionReview {
    pub ply: u32,
    pub fen: String,
    pub played_san: String,
    pub best_move_san: String,
    pub best_move_uci: String,
    pub eval_before: AnalysisScore,
    pub eval_after: AnalysisScore,
    pub eval_best: AnalysisScore,
    pub classification: MoveClassification,
    pub cp_loss: i32,
    pub pv: Vec<String>,
    pub depth: u32,
    pub clock_ms: Option<u64>,
}
```

### GameReview

Full review result for a complete game:

```rust
pub struct GameReview {
    pub game_id: String,
    pub status: ReviewStatus,           // Queued | Analyzing { current_ply, total_plies } | Complete | Failed
    pub positions: Vec<PositionReview>,
    pub white_accuracy: Option<f64>,
    pub black_accuracy: Option<f64>,
    pub total_plies: u32,
    pub analyzed_plies: u32,
    pub analysis_depth: u32,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub winner: Option<String>,
}
```

### compute_accuracy

Computes an accuracy percentage for one side using the formula:

```
accuracy = 103.1668 * exp(-0.006 * avg_cp_loss) - 3.1668
```

Clamped to `[0, 100]`. Individual `cp_loss` values are capped at 1000 to prevent mate-related outliers (where `to_cp()` returns 20000+) from skewing the average. Calibrated so ACPL=10 -> ~94%, ACPL=35 -> ~80%, ACPL=100 -> ~54%.

```rust
pub fn compute_accuracy(positions: &[PositionReview], is_white: bool) -> f64
```

## Board Analysis

### Tactical detection pipeline

Entry point:

```rust
pub fn detect_tactics(ctx: &TacticalContext, max_results: Option<usize>) -> Vec<TacticalTag>
```

`TacticalContext` holds `before`/`after` board positions, the move played, side to move, precomputed `AttackMap`s for both positions, optional eval scores, and an optional engine best-line.

Each detector implements the `TacticalDetector` trait and returns zero or more `TacticalTag` values. Tags are collected from all detectors, deduplicated, and ranked by `reduce_tags`.

| Detector | `TacticalTagKind` produced |
|----------|---------------------------|
| `MateThreatDetector` | `MateThreat` |
| `ForkDetector` | `Fork` |
| `DoubleAttackDetector` | `DoubleAttack` |
| `PinDetector` | `Pin` |
| `SkewerDetector` | `Skewer` |
| `DiscoveredAttackDetector` | `DiscoveredAttack` |
| `SacrificeDetector` | `Sacrifice` |
| `HangingPieceDetector` | `HangingPiece` |
| `BackRankDetector` | `BackRankWeakness` |
| `ZwischenzugDetector` | `Zwischenzug` |

`TacticalTag` fields:

```rust
pub struct TacticalTag {
    pub kind: TacticalTagKind,
    pub confidence: f32,          // 0.0 – 1.0
    pub attacker: Option<String>, // square name of the attacking piece, if applicable
    pub evidence: Vec<TacticalEvidence>,
    pub lines: Vec<TacticalLine>,
}
```

### king_safety.rs

Evaluates king safety for both sides simultaneously:

```rust
pub fn compute_king_safety(board: &Board) -> PositionKingSafety
```

`KingSafetyMetrics` fields per side:

```rust
pub struct KingSafetyMetrics {
    pub color: char,                      // 'w' or 'b'
    pub pawn_shield_count: u8,            // Shield pawns present (0-3)
    pub pawn_shield_max: u8,              // Always 3
    pub open_files_near_king: u8,         // Files adjacent to king without own pawns
    pub attacker_count: u8,               // Unique enemy pieces attacking king zone
    pub attack_weight: u16,               // Q=4, R=3, B=2, N=2, P=1
    pub attacked_king_zone_squares: u8,   // King zone squares under enemy attack
    pub king_zone_size: u8,               // King square + king move squares
    pub exposure_score: f32,              // 0.0 (safe) to 1.0 (exposed)
}
```

`exposure_score` is a weighted composite: 25% shield deficit + 20% open files + 30% attack weight + 25% zone control.

### tension.rs

Measures position volatility and forcing potential:

```rust
pub fn compute_tension(board: &Board) -> PositionTensionMetrics
```

```rust
pub struct PositionTensionMetrics {
    pub mutually_attacked_pairs: u8,  // Squares where both sides have attacked pieces
    pub contested_squares: u8,        // Squares attacked by both sides
    pub attacked_but_defended: u8,    // Pieces attacked by the opponent but defended
    pub forcing_moves: u8,            // Checks + captures available for side to move
    pub checks_available: u8,
    pub captures_available: u8,
    pub volatility_score: f32,        // 0.0 (quiet) to 1.0 (volatile)
}
```

`volatility_score` composite: 30% mutual attacks + 25% forcing moves + 25% contested squares + 20% defended pieces.

### helpers.rs

Low-level bitboard utilities used by the other board analysis modules:

| Function | Description |
|----------|-------------|
| `attacked_squares(board, color) -> BitBoard` | Union of all attack squares for a color |
| `attackers_of(board, sq, color) -> BitBoard` | All pieces of `color` that attack `sq` |
| `piece_attacks(board, sq, piece, color) -> BitBoard` | Attack bitboard for a specific piece on a square |
| `piece_value(piece) -> u16` | Standard centipawn values: P=100, N=320, B=330, R=500, Q=900, K=20000 |
| `king_zone_files(king_sq) -> impl Iterator<Item = File>` | The 1-3 files adjacent to and including the king's file |

## Advanced Analysis

### types.rs

Types for the multi-pass analysis pipeline:

```rust
pub struct AdvancedPositionAnalysis {
    pub ply: u32,
    pub tactical_tags_before: Vec<TacticalTag>,
    pub tactical_tags_after: Vec<TacticalTag>,
    pub king_safety: PositionKingSafety,
    pub tension: PositionTensionMetrics,
    pub is_critical: bool,
    pub deep_depth: Option<u32>,
}

pub struct AdvancedGameAnalysis {
    pub game_id: String,
    pub positions: Vec<AdvancedPositionAnalysis>,
    pub white_psychology: PsychologicalProfile,
    pub black_psychology: PsychologicalProfile,
    pub pipeline_version: u32,
    pub shallow_depth: u32,
    pub deep_depth: u32,
    pub critical_positions_count: u32,
    pub computed_at: u64,
}
```

`AnalysisConfig` controls the multi-pass pipeline (defaults: shallow=10, deep=22, max_critical=20).

### critical.rs

Flags a position as critical when at least 2 of 5 signals fire:

```rust
pub fn is_critical_position(
    position: &PositionReview,
    prev_position: Option<&PositionReview>,
    tactics: &[TacticalTag],
    king_safety: &PositionKingSafety,
    tension: &PositionTensionMetrics,
) -> bool
```

| Signal | Threshold |
|--------|-----------|
| High cp_loss | `cp_loss > 50` |
| Eval swing | `\|eval_after - prev_eval_after\| > 150 cp` |
| Tactical motif | Any `TacticalTag` present in `tactics` |
| High volatility | `volatility_score > 0.6` |
| King exposure | Either side `exposure_score > 0.7` |

### psychological.rs

Builds a per-player `PsychologicalProfile` from the full game's `PositionReview` slice:

```rust
pub fn compute_psychological_profile(
    positions: &[PositionReview],
    is_white: bool,
) -> PsychologicalProfile
```

```rust
pub struct PsychologicalProfile {
    pub color: char,
    pub max_consecutive_errors: u8,          // Longest streak of Inaccuracy/Mistake/Blunder
    pub error_streak_start_ply: Option<u32>,
    pub favorable_swings: u8,                // Eval swings >100cp in player's favour
    pub unfavorable_swings: u8,
    pub max_momentum_streak: u8,             // Longest run of consecutive favorable swings
    pub blunder_cluster_density: u8,         // Max blunders in any sliding window of 5 moves
    pub blunder_cluster_range: Option<(u32, u32)>,
    pub time_quality_correlation: Option<f32>, // Pearson r of time-per-move vs cp_loss
    pub avg_blunder_time_ms: Option<u64>,
    pub avg_good_move_time_ms: Option<u64>,
    pub opening_avg_cp_loss: f64,            // Plies 1-30
    pub middlegame_avg_cp_loss: f64,         // Plies 31-70
    pub endgame_avg_cp_loss: f64,            // Plies 71+
}
```

Time metrics (`time_quality_correlation`, `avg_blunder_time_ms`, `avg_good_move_time_ms`) are `None` when no clock data is present in `PositionReview::clock_ms`.

## Dependencies

- **`chess`** (workspace) - `AnalysisScore`, `is_white_ply`, and related domain types
- **`cozy-chess`** (workspace) - `Board`, `BitBoard`, `Color`, `Piece`, `Square`, and move generation primitives
- **`serde`** (workspace) - `Serialize`/`Deserialize` on all public types

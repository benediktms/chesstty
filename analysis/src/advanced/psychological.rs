use chess::is_white_ply;

use crate::review_types::{MoveClassification, PositionReview};

use super::types::PsychologicalProfile;

/// Compute psychological profile for one player from position reviews.
pub fn compute_psychological_profile(
    positions: &[PositionReview],
    is_white: bool,
) -> PsychologicalProfile {
    let color = if is_white { 'w' } else { 'b' };

    let side_positions: Vec<&PositionReview> = positions
        .iter()
        .filter(|p| is_white_ply(p.ply) == is_white)
        .collect();

    if side_positions.is_empty() {
        return empty_profile(color);
    }

    // Error streaks
    let (max_consecutive_errors, error_streak_start_ply) = compute_error_streaks(&side_positions);

    // Eval swings
    let (favorable_swings, unfavorable_swings, max_momentum_streak) =
        compute_eval_swings(positions, is_white);

    // Blunder clustering
    let (blunder_cluster_density, blunder_cluster_range) =
        compute_blunder_clustering(&side_positions);

    // Time-quality correlation
    let (time_quality_correlation, avg_blunder_time_ms, avg_good_move_time_ms) =
        compute_time_metrics(&side_positions);

    // Phase breakdown
    let (opening_avg_cp_loss, middlegame_avg_cp_loss, endgame_avg_cp_loss) =
        compute_phase_breakdown(&side_positions);

    PsychologicalProfile {
        color,
        max_consecutive_errors,
        error_streak_start_ply,
        favorable_swings,
        unfavorable_swings,
        max_momentum_streak,
        blunder_cluster_density,
        blunder_cluster_range,
        time_quality_correlation,
        avg_blunder_time_ms,
        avg_good_move_time_ms,
        opening_avg_cp_loss,
        middlegame_avg_cp_loss,
        endgame_avg_cp_loss,
    }
}

fn empty_profile(color: char) -> PsychologicalProfile {
    PsychologicalProfile {
        color,
        max_consecutive_errors: 0,
        error_streak_start_ply: None,
        favorable_swings: 0,
        unfavorable_swings: 0,
        max_momentum_streak: 0,
        blunder_cluster_density: 0,
        blunder_cluster_range: None,
        time_quality_correlation: None,
        avg_blunder_time_ms: None,
        avg_good_move_time_ms: None,
        opening_avg_cp_loss: 0.0,
        middlegame_avg_cp_loss: 0.0,
        endgame_avg_cp_loss: 0.0,
    }
}

fn is_error(classification: &MoveClassification) -> bool {
    matches!(
        classification,
        MoveClassification::Inaccuracy | MoveClassification::Mistake | MoveClassification::Blunder
    )
}

fn is_good_move(classification: &MoveClassification) -> bool {
    matches!(
        classification,
        MoveClassification::Best
            | MoveClassification::Excellent
            | MoveClassification::Good
            | MoveClassification::Brilliant
    )
}

/// Compute max consecutive error streak and its start ply.
fn compute_error_streaks(side_positions: &[&PositionReview]) -> (u8, Option<u32>) {
    let mut max_streak: u8 = 0;
    let mut max_streak_start: Option<u32> = None;
    let mut current_streak: u8 = 0;
    let mut current_streak_start: Option<u32> = None;

    for pos in side_positions {
        if is_error(&pos.classification) {
            if current_streak == 0 {
                current_streak_start = Some(pos.ply);
            }
            current_streak += 1;
            if current_streak > max_streak {
                max_streak = current_streak;
                max_streak_start = current_streak_start;
            }
        } else {
            current_streak = 0;
            current_streak_start = None;
        }
    }

    (max_streak, max_streak_start)
}

/// Compute eval swings and momentum streaks.
/// A swing > 100cp in our favor is favorable; against us is unfavorable.
fn compute_eval_swings(all_positions: &[PositionReview], is_white: bool) -> (u8, u8, u8) {
    let mut favorable: u8 = 0;
    let mut unfavorable: u8 = 0;
    let mut max_momentum: u8 = 0;
    let mut current_momentum: u8 = 0;

    // Compare successive eval_after values (from white's perspective since they're stored that way)
    for window in all_positions.windows(2) {
        let prev_eval = window[0].eval_after.to_cp();
        let curr_eval = window[1].eval_after.to_cp();

        // Only count swings on our moves
        if is_white_ply(window[1].ply) != is_white {
            continue;
        }

        let delta = curr_eval - prev_eval;
        let swing_threshold = 100;

        // For white, positive delta is favorable. For black, negative delta is favorable.
        let favorable_delta = if is_white { delta } else { -delta };

        if favorable_delta > swing_threshold {
            favorable = favorable.saturating_add(1);
            current_momentum += 1;
            if current_momentum > max_momentum {
                max_momentum = current_momentum;
            }
        } else if favorable_delta < -swing_threshold {
            unfavorable = unfavorable.saturating_add(1);
            current_momentum = 0;
        } else {
            current_momentum = 0;
        }
    }

    (favorable, unfavorable, max_momentum)
}

/// Compute blunder clustering: sliding window of 5 same-side moves.
fn compute_blunder_clustering(side_positions: &[&PositionReview]) -> (u8, Option<(u32, u32)>) {
    if side_positions.len() < 5 {
        let count = side_positions
            .iter()
            .filter(|p| matches!(p.classification, MoveClassification::Blunder))
            .count() as u8;
        if count > 0 {
            let first_ply = side_positions.first().map(|p| p.ply).unwrap_or(0);
            let last_ply = side_positions.last().map(|p| p.ply).unwrap_or(0);
            return (count, Some((first_ply, last_ply)));
        }
        return (0, None);
    }

    let mut max_density: u8 = 0;
    let mut max_range: Option<(u32, u32)> = None;

    for window in side_positions.windows(5) {
        let blunders = window
            .iter()
            .filter(|p| matches!(p.classification, MoveClassification::Blunder))
            .count() as u8;

        if blunders > max_density {
            max_density = blunders;
            max_range = Some((window[0].ply, window[4].ply));
        }
    }

    (max_density, if max_density > 0 { max_range } else { None })
}

/// Compute time-related metrics if clock data is available.
fn compute_time_metrics(
    side_positions: &[&PositionReview],
) -> (Option<f32>, Option<u64>, Option<u64>) {
    let has_clock = side_positions.iter().any(|p| p.clock_ms.is_some());
    if !has_clock {
        return (None, None, None);
    }

    // Compute time per move from clock differences
    let mut blunder_times: Vec<u64> = Vec::new();
    let mut good_times: Vec<u64> = Vec::new();
    let mut time_cp_pairs: Vec<(f64, f64)> = Vec::new();

    for window in side_positions.windows(2) {
        let prev_clock = match window[0].clock_ms {
            Some(c) => c,
            None => continue,
        };
        let curr_clock = match window[1].clock_ms {
            Some(c) => c,
            None => continue,
        };

        // Time spent = previous clock - current clock (clock counts down)
        let time_spent = prev_clock.saturating_sub(curr_clock);
        let cp_loss = window[1].cp_loss as f64;

        time_cp_pairs.push((time_spent as f64, cp_loss));

        if matches!(window[1].classification, MoveClassification::Blunder) {
            blunder_times.push(time_spent);
        }
        if is_good_move(&window[1].classification) {
            good_times.push(time_spent);
        }
    }

    let correlation = if time_cp_pairs.len() >= 3 {
        Some(pearson_correlation(&time_cp_pairs))
    } else {
        None
    };

    let avg_blunder_time = if blunder_times.is_empty() {
        None
    } else {
        Some(blunder_times.iter().sum::<u64>() / blunder_times.len() as u64)
    };

    let avg_good_time = if good_times.is_empty() {
        None
    } else {
        Some(good_times.iter().sum::<u64>() / good_times.len() as u64)
    };

    (correlation, avg_blunder_time, avg_good_time)
}

/// Compute Pearson correlation coefficient.
fn pearson_correlation(pairs: &[(f64, f64)]) -> f32 {
    let n = pairs.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let sum_x: f64 = pairs.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = pairs.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = pairs.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = pairs.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = pairs.iter().map(|(_, y)| y * y).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        (numerator / denominator) as f32
    }
}

/// Compute average cp_loss bucketed by game phase.
/// Opening: plies 1-30, Middlegame: plies 31-70, Endgame: plies 71+.
fn compute_phase_breakdown(side_positions: &[&PositionReview]) -> (f64, f64, f64) {
    let mut opening_losses: Vec<f64> = Vec::new();
    let mut middlegame_losses: Vec<f64> = Vec::new();
    let mut endgame_losses: Vec<f64> = Vec::new();

    for pos in side_positions {
        let loss = pos.cp_loss as f64;
        match pos.ply {
            1..=30 => opening_losses.push(loss),
            31..=70 => middlegame_losses.push(loss),
            _ => endgame_losses.push(loss),
        }
    }

    let avg = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };

    (
        avg(&opening_losses),
        avg(&middlegame_losses),
        avg(&endgame_losses),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_types::PositionReview;
    use chess::AnalysisScore;

    fn make_position(ply: u32, cp_loss: i32, classification: MoveClassification) -> PositionReview {
        PositionReview {
            ply,
            fen: String::new(),
            played_san: String::new(),
            best_move_san: String::new(),
            best_move_uci: String::new(),
            eval_before: AnalysisScore::Centipawns(0),
            eval_after: AnalysisScore::Centipawns(0),
            eval_best: AnalysisScore::Centipawns(0),
            classification,
            cp_loss,
            pv: vec![],
            depth: 18,
            clock_ms: None,
        }
    }

    #[test]
    fn test_empty_positions() {
        let profile = compute_psychological_profile(&[], true);
        assert_eq!(profile.color, 'w');
        assert_eq!(profile.max_consecutive_errors, 0);
    }

    #[test]
    fn test_error_streaks() {
        let positions = vec![
            make_position(1, 0, MoveClassification::Best),
            make_position(2, 50, MoveClassification::Inaccuracy),
            make_position(3, 150, MoveClassification::Mistake),
            make_position(4, 350, MoveClassification::Blunder),
            make_position(5, 200, MoveClassification::Mistake),
            make_position(6, 0, MoveClassification::Best),
            make_position(7, 0, MoveClassification::Best),
        ];
        // White moves are plies 1,3,5,7 — errors at 3,5
        let profile = compute_psychological_profile(&positions, true);
        assert_eq!(profile.max_consecutive_errors, 2);
        assert_eq!(profile.error_streak_start_ply, Some(3));
    }

    #[test]
    fn test_blunder_clustering() {
        let positions: Vec<PositionReview> = (0..12)
            .map(|i| {
                let ply = (i as u32) * 2 + 1; // odd plies for white
                let classification = if (4..=8).contains(&i) {
                    MoveClassification::Blunder
                } else {
                    MoveClassification::Best
                };
                make_position(
                    ply,
                    if classification == MoveClassification::Blunder {
                        400
                    } else {
                        0
                    },
                    classification,
                )
            })
            .collect();

        let profile = compute_psychological_profile(&positions, true);
        assert!(
            profile.blunder_cluster_density >= 4,
            "Should detect cluster of blunders"
        );
    }

    #[test]
    fn test_phase_breakdown() {
        let mut positions = Vec::new();
        // Opening moves (plies 1-30, white=odd)
        for i in 0..15 {
            positions.push(make_position(i * 2 + 1, 10, MoveClassification::Excellent));
        }
        // Middlegame moves
        for i in 15..35 {
            positions.push(make_position(i * 2 + 1, 50, MoveClassification::Inaccuracy));
        }
        // Endgame moves
        for i in 35..45 {
            positions.push(make_position(i * 2 + 1, 5, MoveClassification::Excellent));
        }

        let profile = compute_psychological_profile(&positions, true);
        assert!(
            profile.opening_avg_cp_loss < profile.middlegame_avg_cp_loss,
            "Opening should have lower cp_loss than middlegame"
        );
        assert!(
            profile.endgame_avg_cp_loss < profile.middlegame_avg_cp_loss,
            "Endgame should have lower cp_loss than middlegame"
        );
    }
}

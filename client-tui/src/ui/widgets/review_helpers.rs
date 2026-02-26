use crate::ui::theme::Theme;
use chess::is_white_ply;
use chess_client::{
    review_score, KingSafetyMetricsProto, MoveClassification, PositionKingSafetyProto,
    PositionReview, PositionTensionMetricsProto, TacticalTagKindProto, TacticalTagProto,
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::collections::HashMap;

// --- Classification helpers ---

pub fn count_classifications(
    positions: &[PositionReview],
) -> (HashMap<i32, u32>, HashMap<i32, u32>) {
    let mut white = HashMap::new();
    let mut black = HashMap::new();
    for p in positions {
        let map = if is_white_ply(p.ply) {
            &mut white
        } else {
            &mut black
        };
        *map.entry(p.classification).or_insert(0) += 1;
    }
    (white, black)
}

// --- Color helpers ---

pub fn accuracy_color(accuracy: f64, theme: &Theme) -> Color {
    if accuracy >= 90.0 {
        theme.accuracy_high
    } else if accuracy >= 70.0 {
        theme.accuracy_mid
    } else {
        theme.accuracy_low
    }
}

pub fn exposure_color(score: f32, theme: &Theme) -> Color {
    if score < 0.3 {
        theme.positive
    } else if score < 0.6 {
        theme.warning
    } else {
        theme.negative
    }
}

// --- Bar/graph helpers ---

pub fn accuracy_bar(accuracy: f64, width: usize) -> String {
    let filled = ((accuracy / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

pub fn volatility_bar(score: f32, width: usize) -> String {
    let filled = ((score.clamp(0.0, 1.0)) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

// --- Score formatting ---

pub fn score_to_cp_clamped(pos: &PositionReview) -> i32 {
    let cp = match pos.eval_before.as_ref().and_then(|s| s.score.as_ref()) {
        Some(review_score::Score::Centipawns(cp)) => *cp,
        Some(review_score::Score::Mate(m)) => {
            if *m > 0 {
                500
            } else {
                -500
            }
        }
        None => 0,
    };
    cp.clamp(-500, 500)
}

// --- Eval graph ---

pub fn build_eval_graph(
    positions: &[PositionReview],
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    if positions.is_empty() || width == 0 {
        return vec![];
    }

    let height = 5usize;
    let mid = height / 2; // row 2 = midline (0.0 eval)

    // Sample positions to fit the available width
    let total = positions.len();
    let cols: Vec<(i32, Option<Color>)> = (0..width)
        .map(|col| {
            let idx = col * total / width;
            let idx = idx.min(total - 1);
            let pos = &positions[idx];
            let cp = score_to_cp_clamped(pos);

            // Color critical moments
            let col_color = match MoveClassification::try_from(pos.classification) {
                Ok(MoveClassification::ClassificationBlunder) => Some(theme.move_blunder),
                Ok(MoveClassification::ClassificationMistake) => Some(theme.move_inaccuracy),
                _ => None,
            };
            (cp, col_color)
        })
        .collect();

    // Block characters for bar rendering (bottom to top within a cell)
    let blocks = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    // Build rows top to bottom
    let mut rows: Vec<Line<'static>> = Vec::with_capacity(height);
    for row in 0..height {
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for &(cp, critical_color) in &cols {
            // Map cp [-500, 500] to a fill level [0.0, height*8] sub-cells
            // midline at mid*8 sub-cells from bottom
            let max_sub = (height * 8) as f64;
            let mid_sub = (mid * 8) as f64;
            // cp=500 -> full top, cp=-500 -> full bottom, cp=0 -> midline
            let fill_sub = mid_sub + (cp as f64 / 500.0) * mid_sub;
            let fill_sub = fill_sub.clamp(0.0, max_sub) as usize;

            // This row spans sub-cells from (height-1-row)*8 to (height-row)*8
            let row_bottom = (height - 1 - row) * 8;
            let row_top = row_bottom + 8;

            let block_char = if fill_sub >= row_top {
                '\u{2588}' // fully filled
            } else if fill_sub <= row_bottom {
                ' ' // empty
            } else {
                blocks[fill_sub - row_bottom]
            };

            let fg = if row <= mid {
                // Above or at midline: white's territory
                critical_color.unwrap_or(theme.text_primary)
            } else {
                critical_color.unwrap_or(theme.text_secondary)
            };

            spans.push(Span::styled(
                block_char.to_string(),
                Style::default().fg(fg),
            ));
        }
        rows.push(Line::from(spans));
    }

    rows
}

// --- Advanced analysis rendering ---

pub fn tactical_tag_kind_name(kind: i32) -> &'static str {
    match TacticalTagKindProto::try_from(kind) {
        Ok(TacticalTagKindProto::TacticalTagKindFork) => "Fork",
        Ok(TacticalTagKindProto::TacticalTagKindPin) => "Pin",
        Ok(TacticalTagKindProto::TacticalTagKindSkewer) => "Skewer",
        Ok(TacticalTagKindProto::TacticalTagKindDiscoveredAttack) => "Discovered",
        Ok(TacticalTagKindProto::TacticalTagKindDoubleAttack) => "Double Attack",
        Ok(TacticalTagKindProto::TacticalTagKindHangingPiece) => "Hanging",
        Ok(TacticalTagKindProto::TacticalTagKindSacrifice) => "Sacrifice",
        Ok(TacticalTagKindProto::TacticalTagKindZwischenzug) => "Zwischenzug",
        Ok(TacticalTagKindProto::TacticalTagKindBackRankWeakness) => "Back Rank",
        Ok(TacticalTagKindProto::TacticalTagKindMateThreat) => "Mate Threat",
        _ => "Unknown",
    }
}

pub fn render_tactical_tags(lines: &mut Vec<Line<'_>>, tags: &[TacticalTagProto], theme: &Theme) {
    lines.push(Line::from(Span::styled(
        "  Tactical Tags",
        Style::default().fg(theme.warning),
    )));

    if tags.is_empty() {
        lines.push(Line::from(Span::styled(
            "    None detected",
            Style::default().fg(theme.muted),
        )));
        return;
    }

    for tag in tags {
        let kind_name = tactical_tag_kind_name(tag.kind);
        let conf = tag.confidence;
        let conf_color = if conf >= 0.8 {
            theme.positive_light
        } else if conf >= 0.5 {
            theme.warning
        } else {
            theme.muted
        };

        let mut spans = vec![
            Span::raw("    "),
            Span::styled(
                kind_name,
                Style::default().fg(conf_color).add_modifier(Modifier::BOLD),
            ),
        ];

        // Attacker info
        if let Some(ref attacker) = tag.attacker {
            spans.push(Span::raw(": "));
            spans.push(Span::styled(
                attacker.clone(),
                Style::default().fg(theme.text_primary),
            ));
        }

        // Victims
        if !tag.victims.is_empty() {
            spans.push(Span::raw(" \u{2192} "));
            spans.push(Span::styled(
                tag.victims.join(", "),
                Style::default().fg(theme.info_light),
            ));
        }

        // Confidence
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("{:.0}%", conf * 100.0),
            Style::default().fg(conf_color),
        ));

        lines.push(Line::from(spans));

        // Note (if present)
        if let Some(ref note) = tag.note {
            lines.push(Line::from(Span::styled(
                format!("      {}", note),
                Style::default().fg(theme.muted),
            )));
        }
    }
}

pub fn render_king_safety(lines: &mut Vec<Line<'_>>, ks: &PositionKingSafetyProto, theme: &Theme) {
    lines.push(Line::from(Span::styled(
        "  King Safety",
        Style::default().fg(theme.warning),
    )));

    let white = ks.white.as_ref();
    let black = ks.black.as_ref();

    let get_u32 = |opt: Option<&KingSafetyMetricsProto>, f: &str| -> u32 {
        opt.and_then(|m| match f {
            "pawn_shield_count" => Some(m.pawn_shield_count),
            "pawn_shield_max" => Some(m.pawn_shield_max),
            "open_files_near_king" => Some(m.open_files_near_king),
            _ => None,
        })
        .unwrap_or(0)
    };

    let get_f32 = |opt: Option<&KingSafetyMetricsProto>, f: &str| -> f32 {
        opt.and_then(|m| match f {
            "exposure_score" => Some(m.exposure_score),
            _ => None,
        })
        .unwrap_or(0.0)
    };

    // White king safety
    lines.push(Line::from(vec![
        Span::raw("  White: "),
        Span::raw("shield "),
        Span::styled(
            format!(
                "{}/{}",
                get_u32(white, "pawn_shield_count"),
                get_u32(white, "pawn_shield_max")
            ),
            Style::default().fg(if get_u32(white, "pawn_shield_count") >= 3 {
                theme.positive
            } else {
                theme.negative
            }),
        ),
        Span::raw("  open files "),
        Span::styled(
            format!("{}", get_u32(white, "open_files_near_king")),
            Style::default().fg(if get_u32(white, "open_files_near_king") == 0 {
                theme.positive
            } else {
                theme.negative
            }),
        ),
        Span::raw("  exposure "),
        Span::styled(
            format!("{:.1}", get_f32(white, "exposure_score")),
            Style::default().fg(exposure_color(get_f32(white, "exposure_score"), theme)),
        ),
    ]));

    // Black king safety
    lines.push(Line::from(vec![
        Span::raw("  Black: "),
        Span::raw("shield "),
        Span::styled(
            format!(
                "{}/{}",
                get_u32(black, "pawn_shield_count"),
                get_u32(black, "pawn_shield_max")
            ),
            Style::default().fg(if get_u32(black, "pawn_shield_count") >= 3 {
                theme.positive
            } else {
                theme.negative
            }),
        ),
        Span::raw("  open files "),
        Span::styled(
            format!("{}", get_u32(black, "open_files_near_king")),
            Style::default().fg(if get_u32(black, "open_files_near_king") == 0 {
                theme.positive
            } else {
                theme.negative
            }),
        ),
        Span::raw("  exposure "),
        Span::styled(
            format!("{:.1}", get_f32(black, "exposure_score")),
            Style::default().fg(exposure_color(get_f32(black, "exposure_score"), theme)),
        ),
    ]));
}

pub fn render_tension(
    lines: &mut Vec<Line<'_>>,
    tension: &PositionTensionMetricsProto,
    theme: &Theme,
) {
    lines.push(Line::from(Span::styled(
        "  Tension",
        Style::default().fg(theme.warning),
    )));

    // Volatility bar
    let vol_bar = volatility_bar(tension.volatility_score, 15);
    lines.push(Line::from(vec![
        Span::raw("  Volatility: "),
        Span::styled(vol_bar, Style::default().fg(theme.info_light)),
    ]));

    // Forcing moves
    lines.push(Line::from(vec![
        Span::raw("  Forcing moves: "),
        Span::styled(
            format!("{}", tension.forcing_moves),
            Style::default().fg(theme.info),
        ),
        Span::raw("  Checks: "),
        Span::styled(
            format!("{}", tension.checks_available),
            Style::default().fg(if tension.checks_available > 0 {
                theme.warning
            } else {
                theme.muted
            }),
        ),
        Span::raw("  Captures: "),
        Span::styled(
            format!("{}", tension.captures_available),
            Style::default().fg(if tension.captures_available > 0 {
                theme.warning
            } else {
                theme.muted
            }),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Mutually attacked: "),
        Span::raw(format!("{}", tension.mutually_attacked_pairs)),
        Span::raw("  Contested: "),
        Span::raw(format!("{}", tension.contested_squares)),
    ]));
}

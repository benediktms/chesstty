use crate::review_state::ReviewState;
use chess_client::{
    PositionKingSafetyProto, PositionTensionMetricsProto, TacticalTagKindProto, TacticalTagProto,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct AdvancedAnalysisPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
}

impl Widget for AdvancedAnalysisPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.expanded {
            "Advanced Analysis (Expanded)"
        } else if self.is_selected {
            "Advanced Analysis [SELECTED]"
        } else {
            "[3] Advanced Analysis"
        };

        let border_style = if self.is_selected || self.expanded {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Magenta)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line<'static>> = vec![];

        let advanced = match &self.review_state.advanced {
            Some(a) => a,
            None => {
                lines.push(Line::from(Span::styled(
                    "No advanced analysis available",
                    Style::default().fg(Color::DarkGray),
                )));
                let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
                paragraph.render(inner, buf);
                return;
            }
        };

        // Per-position analysis (changes with ply navigation)
        if let Some(adv_pos) = self.review_state.advanced_position() {
            lines.push(Line::from(Span::styled(
                "Position Analysis",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            // Critical position badge
            if adv_pos.is_critical {
                lines.push(Line::from(Span::styled(
                    "  \u{26A0} CRITICAL POSITION \u{26A0}",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }

            // Tactical tags
            if !adv_pos.tactical_tags_after.is_empty() {
                render_tactical_tags(&mut lines, &adv_pos.tactical_tags_after);
            }

            // King safety
            if let Some(ref ks) = adv_pos.king_safety {
                render_king_safety(&mut lines, ks);
            }

            // Tension metrics
            if let Some(ref tension) = adv_pos.tension {
                render_tension(&mut lines, tension);
            }

            lines.push(Line::raw(""));
        }

        // Game-wide analysis (static)
        lines.push(Line::from(Span::styled(
            "Game-wide Analysis",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        // Phase performance
        lines.push(Line::from(Span::styled(
            "Phase Performance (avg cp_loss)",
            Style::default().fg(Color::Cyan),
        )));

        let white_psy = advanced.white_psychology.as_ref();
        let black_psy = advanced.black_psychology.as_ref();

        let w_opening = white_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
        let b_opening = black_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
        let w_mid = white_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
        let b_mid = black_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
        let w_end = white_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);
        let b_end = black_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);

        let w_max_err = white_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
        let b_max_err = black_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
        let w_blunder = white_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);
        let b_blunder = black_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);

        let w_fav = white_psy.map(|p| p.favorable_swings).unwrap_or(0);
        let b_fav = black_psy.map(|p| p.favorable_swings).unwrap_or(0);
        let w_unfav = white_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
        let b_unfav = black_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
        let w_streak = white_psy.map(|p| p.max_momentum_streak).unwrap_or(0);
        let b_streak = black_psy.map(|p| p.max_momentum_streak).unwrap_or(0);

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Opening", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_opening),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_opening),
                Style::default().fg(Color::LightCyan),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Middlegame", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_mid),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_mid),
                Style::default().fg(Color::LightCyan),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Endgame", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_end),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_end),
                Style::default().fg(Color::LightCyan),
            ),
        ]));

        lines.push(Line::raw(""));

        // Error patterns
        lines.push(Line::from(Span::styled(
            "Error Patterns",
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(vec![
            Span::raw("  Max consecutive: "),
            Span::styled(
                format!("W:{}  B:{}", w_max_err, b_max_err),
                Style::default().fg(Color::LightRed),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Blunder cluster: "),
            Span::styled(
                format!("W:{}  B:{}", w_blunder, b_blunder),
                Style::default().fg(Color::LightMagenta),
            ),
        ]));

        lines.push(Line::raw(""));

        // Momentum
        lines.push(Line::from(Span::styled(
            "Momentum",
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(vec![
            Span::raw("  Favorable swings: "),
            Span::styled(
                format!("W:{}  B:{}", w_fav, b_fav),
                Style::default().fg(Color::Green),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Unfavorable swings: "),
            Span::styled(
                format!("W:{}  B:{}", w_unfav, b_unfav),
                Style::default().fg(Color::Red),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Max streak: "),
            Span::styled(
                format!("W:{}  B:{}", w_streak, b_streak),
                Style::default().fg(Color::Yellow),
            ),
        ]));

        lines.push(Line::raw(""));

        // Info
        lines.push(Line::from(vec![
            Span::styled("Critical positions: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", advanced.critical_positions_count)),
        ]));

        let content_height = lines.len() as u16;
        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);

        if content_height > inner.height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
            let mut scrollbar_state =
                ScrollbarState::new(content_height as usize).position(self.scroll as usize);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

fn render_tactical_tags(lines: &mut Vec<Line<'_>>, tags: &[TacticalTagProto]) {
    lines.push(Line::from(Span::styled(
        "  Tactical Tags",
        Style::default().fg(Color::Yellow),
    )));

    if tags.is_empty() {
        lines.push(Line::from(Span::styled(
            "    None detected",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }

    for tag in tags {
        let kind_name = tactical_tag_kind_name(tag.kind);
        let conf = tag.confidence;
        let conf_color = if conf >= 0.8 {
            Color::LightGreen
        } else if conf >= 0.5 {
            Color::Yellow
        } else {
            Color::DarkGray
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
                Style::default().fg(Color::White),
            ));
        }

        // Victims
        if !tag.victims.is_empty() {
            spans.push(Span::raw(" \u{2192} "));
            spans.push(Span::styled(
                tag.victims.join(", "),
                Style::default().fg(Color::LightCyan),
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
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
}

fn tactical_tag_kind_name(kind: i32) -> &'static str {
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

fn render_king_safety(lines: &mut Vec<Line<'_>>, ks: &PositionKingSafetyProto) {
    lines.push(Line::from(Span::styled(
        "  King Safety",
        Style::default().fg(Color::Yellow),
    )));

    let white = ks.white.as_ref();
    let black = ks.black.as_ref();

    let get_u32 = |opt: Option<&chess_client::KingSafetyMetricsProto>, f: &str| -> u32 {
        opt.and_then(|m| match f {
            "pawn_shield_count" => Some(m.pawn_shield_count),
            "pawn_shield_max" => Some(m.pawn_shield_max),
            "open_files_near_king" => Some(m.open_files_near_king),
            _ => None,
        })
        .unwrap_or(0)
    };

    let get_f32 = |opt: Option<&chess_client::KingSafetyMetricsProto>, f: &str| -> f32 {
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
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw("  open files "),
        Span::styled(
            format!("{}", get_u32(white, "open_files_near_king")),
            Style::default().fg(if get_u32(white, "open_files_near_king") == 0 {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw("  exposure "),
        Span::styled(
            format!("{:.1}", get_f32(white, "exposure_score")),
            Style::default().fg(exposure_color(get_f32(white, "exposure_score"))),
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
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw("  open files "),
        Span::styled(
            format!("{}", get_u32(black, "open_files_near_king")),
            Style::default().fg(if get_u32(black, "open_files_near_king") == 0 {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw("  exposure "),
        Span::styled(
            format!("{:.1}", get_f32(black, "exposure_score")),
            Style::default().fg(exposure_color(get_f32(black, "exposure_score"))),
        ),
    ]));
}

fn exposure_color(score: f32) -> Color {
    if score < 0.3 {
        Color::Green
    } else if score < 0.6 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn render_tension(lines: &mut Vec<Line<'_>>, tension: &PositionTensionMetricsProto) {
    lines.push(Line::from(Span::styled(
        "  Tension",
        Style::default().fg(Color::Yellow),
    )));

    // Volatility bar
    let vol_bar = volatility_bar(tension.volatility_score, 15);
    lines.push(Line::from(vec![
        Span::raw("  Volatility: "),
        Span::styled(vol_bar, Style::default().fg(Color::LightCyan)),
    ]));

    // Forcing moves
    lines.push(Line::from(vec![
        Span::raw("  Forcing moves: "),
        Span::styled(
            format!("{}", tension.forcing_moves),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  Checks: "),
        Span::styled(
            format!("{}", tension.checks_available),
            Style::default().fg(if tension.checks_available > 0 {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw("  Captures: "),
        Span::styled(
            format!("{}", tension.captures_available),
            Style::default().fg(if tension.captures_available > 0 {
                Color::Yellow
            } else {
                Color::DarkGray
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

fn volatility_bar(score: f32, width: usize) -> String {
    let filled = ((score.clamp(0.0, 1.0)) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

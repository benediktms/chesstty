use crate::review_state::ReviewState;
use crate::ui::widgets::game_info_panel::{classification_color, format_review_score};
use chess_client::{
    review_score, MoveClassification, PositionKingSafetyProto, PositionTensionMetricsProto,
    TacticalTagKindProto, TacticalTagProto,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct MoveAnalysisPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
}

impl<'a> MoveAnalysisPanel<'a> {
    pub fn new(review_state: &'a ReviewState, scroll: u16, is_selected: bool) -> Self {
        Self {
            review_state,
            scroll,
            is_selected,
            expanded: false,
        }
    }
}

impl Widget for MoveAnalysisPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.expanded {
            "Move Analysis (Expanded)"
        } else if self.is_selected {
            "Move Analysis [SELECTED]"
        } else {
            "Move Analysis"
        };

        let border_style = if self.is_selected || self.expanded {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        let Some(pos) = self.review_state.current_position() else {
            let text = "Navigate to a move to see analysis";
            let paragraph = Paragraph::new(Line::from(Span::styled(
                text,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
            paragraph.render(inner, buf);
            return;
        };

        let mut lines: Vec<Line<'static>> = vec![];

        // Classification badge (colored header)
        let class_name = classification_display_name(pos.classification);
        let class_color = classification_color(pos.classification);
        let badge_text = format!(" {} ", class_name);
        lines.push(Line::from(Span::styled(
            badge_text,
            Style::default()
                .fg(Color::Black)
                .bg(class_color)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));

        // Played move with marker
        let marker = classification_marker_str(pos.classification);
        lines.push(Line::from(vec![
            Span::styled("Played: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{}", pos.played_san, marker),
                Style::default()
                    .fg(class_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Best move
        if !pos.best_move_san.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Best:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    pos.best_move_san.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines.push(Line::raw(""));

        // Eval before
        if let Some(ref score) = pos.eval_before {
            let (text, color) = format_review_score(score);
            let mate_info = format_mate_info(score);
            lines.push(Line::from(vec![
                Span::styled("Before: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(mate_info),
            ]));
        }

        // Eval after
        if let Some(ref score) = pos.eval_after {
            let (text, color) = format_review_score(score);
            let mate_info = format_mate_info(score);
            lines.push(Line::from(vec![
                Span::styled("After:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(mate_info),
            ]));
        }

        // Eval best
        if let Some(ref score) = pos.eval_best {
            let (text, color) = format_review_score(score);
            lines.push(Line::from(vec![
                Span::styled("Best:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // CP Loss
        if pos.cp_loss > 0 {
            lines.push(Line::from(vec![
                Span::styled("Loss:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}cp", pos.cp_loss),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        // Depth
        if pos.depth > 0 {
            lines.push(Line::from(vec![
                Span::styled("Depth:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", pos.depth), Style::default().fg(Color::White)),
            ]));
        }

        // PV line
        if !pos.pv.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Principal Variation:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));

            let pv_text = pos.pv.join(" ");
            let max_width = (inner.width as usize).saturating_sub(2);
            for chunk in wrap_text(&pv_text, max_width) {
                lines.push(Line::from(Span::styled(
                    chunk,
                    Style::default().fg(Color::Cyan),
                )));
            }
        }

        // Advanced analysis (tactics, king safety, tension) - only if we have a position
        if let Some(adv_pos) = self.review_state.advanced_position() {
            lines.push(Line::raw(""));

            // Tactical tags (new pipeline)
            if !adv_pos.tactical_tags_after.is_empty() {
                render_tactical_tags_inline(&mut lines, &adv_pos.tactical_tags_after);
            }

            // King safety
            if let Some(ref ks) = adv_pos.king_safety {
                render_king_safety_inline(&mut lines, ks);
            }

            // Tension metrics
            if let Some(ref tension) = adv_pos.tension {
                render_tension_inline(&mut lines, tension);
            }
        }

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);
    }
}

fn classification_display_name(classification: i32) -> &'static str {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => "BRILLIANT",
        Ok(MoveClassification::ClassificationBest) => "BEST",
        Ok(MoveClassification::ClassificationExcellent) => "EXCELLENT",
        Ok(MoveClassification::ClassificationGood) => "GOOD",
        Ok(MoveClassification::ClassificationInaccuracy) => "INACCURACY",
        Ok(MoveClassification::ClassificationMistake) => "MISTAKE",
        Ok(MoveClassification::ClassificationBlunder) => "BLUNDER",
        Ok(MoveClassification::ClassificationForced) => "FORCED",
        Ok(MoveClassification::ClassificationBook) => "BOOK",
        _ => "UNKNOWN",
    }
}

fn classification_marker_str(classification: i32) -> &'static str {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => "!!",
        Ok(MoveClassification::ClassificationExcellent) => "!",
        Ok(MoveClassification::ClassificationInaccuracy) => "?!",
        Ok(MoveClassification::ClassificationMistake) => "?",
        Ok(MoveClassification::ClassificationBlunder) => "??",
        Ok(MoveClassification::ClassificationForced) => "[]",
        _ => "",
    }
}

fn format_mate_info(score: &chess_client::ReviewScore) -> String {
    match score.score.as_ref() {
        Some(review_score::Score::Mate(m)) => {
            format!("  (Mate in {})", m.abs())
        }
        _ => String::new(),
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn render_tactical_tags_inline(lines: &mut Vec<Line<'_>>, tags: &[TacticalTagProto]) {
    lines.push(Line::from(Span::styled(
        "Tactics",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));

    for tag in tags.iter().take(3) {
        let kind_name = tactical_tag_kind_name_inline(tag.kind);
        let conf = tag.confidence;
        let conf_color = if conf >= 0.8 {
            Color::LightGreen
        } else if conf >= 0.5 {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let mut spans = vec![
            Span::raw("  "),
            Span::styled(
                kind_name,
                Style::default().fg(conf_color).add_modifier(Modifier::BOLD),
            ),
        ];

        // Attacker info
        if let Some(ref attacker) = tag.attacker {
            spans.push(Span::raw(": "));
            spans.push(Span::styled(attacker.clone(), Style::default().fg(Color::White)));
        }

        // Victims
        if !tag.victims.is_empty() {
            spans.push(Span::raw(" -> "));
            spans.push(Span::styled(
                tag.victims.join(", "),
                Style::default().fg(Color::LightCyan),
            ));
        }

        // Confidence percentage
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("{:.0}%", conf * 100.0),
            Style::default().fg(conf_color),
        ));

        lines.push(Line::from(spans));
    }
}

fn tactical_tag_kind_name_inline(kind: i32) -> &'static str {
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

fn render_king_safety_inline(lines: &mut Vec<Line<'_>>, ks: &PositionKingSafetyProto) {
    lines.push(Line::from(Span::styled(
        "King Safety",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));

    // White king safety
    if let Some(ks_white) = &ks.white {
        let shield = format!(
            "{}/{}",
            ks_white.pawn_shield_count, ks_white.pawn_shield_max
        );
        let shield_color = if ks_white.pawn_shield_count >= 3 {
            Color::Green
        } else {
            Color::Red
        };
        let open_files = ks_white.open_files_near_king;
        let open_color = if open_files == 0 {
            Color::Green
        } else {
            Color::Red
        };
        let exp_color = exposure_color(ks_white.exposure_score);

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("White", Style::default().fg(Color::White)),
            Span::raw(" shield "),
            Span::styled(shield, Style::default().fg(shield_color)),
            Span::raw(" open "),
            Span::styled(format!("{}", open_files), Style::default().fg(open_color)),
            Span::raw(" exp "),
            Span::styled(
                format!("{:.1}", ks_white.exposure_score),
                Style::default().fg(exp_color),
            ),
        ]));
    }

    // Black king safety
    if let Some(ks_black) = &ks.black {
        let shield = format!(
            "{}/{}",
            ks_black.pawn_shield_count, ks_black.pawn_shield_max
        );
        let shield_color = if ks_black.pawn_shield_count >= 3 {
            Color::Green
        } else {
            Color::Red
        };
        let open_files = ks_black.open_files_near_king;
        let open_color = if open_files == 0 {
            Color::Green
        } else {
            Color::Red
        };
        let exp_color = exposure_color(ks_black.exposure_score);

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Black", Style::default().fg(Color::Gray)),
            Span::raw(" shield "),
            Span::styled(shield, Style::default().fg(shield_color)),
            Span::raw(" open "),
            Span::styled(format!("{}", open_files), Style::default().fg(open_color)),
            Span::raw(" exp "),
            Span::styled(
                format!("{:.1}", ks_black.exposure_score),
                Style::default().fg(exp_color),
            ),
        ]));
    }
}

fn render_tension_inline(lines: &mut Vec<Line<'_>>, tension: &PositionTensionMetricsProto) {
    lines.push(Line::from(Span::styled(
        "Tension",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));

    let vol_bar = volatility_bar_inline(tension.volatility_score, 10);
    lines.push(Line::from(vec![
        Span::raw("  Vol: "),
        Span::styled(vol_bar, Style::default().fg(Color::LightCyan)),
        Span::raw(" Forcing: "),
        Span::styled(
            format!("{}", tension.forcing_moves),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Checks: "),
        Span::styled(
            format!("{}", tension.checks_available),
            Style::default().fg(if tension.checks_available > 0 {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw(" Captures: "),
        Span::styled(
            format!("{}", tension.captures_available),
            Style::default().fg(if tension.captures_available > 0 {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw(" Attacked: "),
        Span::raw(format!("{}", tension.mutually_attacked_pairs)),
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

fn volatility_bar_inline(score: f32, width: usize) -> String {
    let filled = ((score.clamp(0.0, 1.0)) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

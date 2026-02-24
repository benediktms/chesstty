#![allow(dead_code)]

use crate::review_state::ReviewState;
use chess::is_white_ply;
use chess_client::{
    review_score, MoveClassification, PositionReview, TacticalTagKindProto, TacticalTagProto,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

pub const REVIEW_TAB_OVERVIEW: u8 = 0;
pub const REVIEW_TAB_POSITION: u8 = 1;

pub struct ReviewTabsPanel<'a> {
    pub review_state: &'a ReviewState,
    pub current_tab: u8,
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
    pub moves_selection: Option<u32>,
}

impl Widget for ReviewTabsPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.expanded {
            "\u{2606} Review (Expanded) \u{2606}"
        } else if self.is_selected {
            "\u{2606} Review \u{2606} [SELECTED]"
        } else {
            "\u{2606} Review \u{2606}"
        };

        let border_style = if self.is_selected || self.expanded {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        // Build horizontal tab line
        let tab_names = ["[1]Overview", "[2]Position"];
        let mut tab_spans: Vec<Span<'static>> = vec![];
        for (i, name) in tab_names.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::raw(" "));
            }
            if i == self.current_tab as usize {
                tab_spans.push(Span::styled(
                    format!("[{}]", name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                tab_spans.push(Span::styled(
                    format!(" {} ", name),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        tab_spans.push(Span::raw(""));

        let mut lines: Vec<Line<'static>> = vec![Line::from(tab_spans)];

        // Calculate auto-scroll for Moves tab
        let effective_scroll = self.scroll;

        let tab_content = match self.current_tab {
            REVIEW_TAB_OVERVIEW => self.render_overview(inner.width),
            REVIEW_TAB_POSITION => self.render_position(),
            _ => vec![],
        };
        lines.extend(tab_content);

        let content_height = lines.len() as u16;
        let paragraph = Paragraph::new(lines).scroll((effective_scroll, 0));
        paragraph.render(inner, buf);

        // Add scrollbar for all tabs
        if content_height > inner.height.saturating_sub(1) {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
            let mut scrollbar_state =
                ScrollbarState::new(content_height as usize).position(effective_scroll as usize);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

impl ReviewTabsPanel<'_> {
    fn render_overview(&self, inner_width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![];
        let review = &self.review_state.review;

        lines.push(Line::from(Span::styled(
            "Accuracy",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(white_acc) = review.white_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  W: ").style(Style::default()).fg(Color::White),
                Span::styled(
                    format!("{:.1}%", white_acc),
                    Style::default().fg(accuracy_color(white_acc)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(white_acc, 15)),
            ]));
        }
        if let Some(black_acc) = review.black_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  B: ")
                    .style(Style::default())
                    .fg(Color::DarkGray),
                Span::styled(
                    format!("{:.1}%", black_acc),
                    Style::default().fg(accuracy_color(black_acc)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(black_acc, 15)),
            ]));
        }

        // Eval graph
        if !review.positions.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Evaluation",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            let graph_width = (inner_width as usize).saturating_sub(4).min(60);
            let graph_lines = build_eval_graph(&review.positions, graph_width);
            lines.extend(graph_lines);
        }

        lines.push(Line::raw(""));

        lines.push(Line::from(Span::styled(
            "Move Quality",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        let (white_counts, black_counts) = count_classifications(&review.positions);
        // Categories with symbols - show all even if count is 0 (legend purposes)
        let categories = [
            (
                "!! Brilliant",
                MoveClassification::ClassificationBrilliant as i32,
                Color::Cyan,
            ),
            (
                "!  Excellent",
                MoveClassification::ClassificationExcellent as i32,
                Color::Cyan,
            ),
            (
                "   Good/Best",
                MoveClassification::ClassificationGood as i32,
                Color::White,
            ),
            (
                "?! Inaccuracy",
                MoveClassification::ClassificationInaccuracy as i32,
                Color::Yellow,
            ),
            (
                "?  Mistake",
                MoveClassification::ClassificationMistake as i32,
                Color::Magenta,
            ),
            (
                "?? Blunder",
                MoveClassification::ClassificationBlunder as i32,
                Color::Red,
            ),
            (
                "[] Forced",
                MoveClassification::ClassificationForced as i32,
                Color::DarkGray,
            ),
        ];

        // Show all categories (combined legend + counts)
        for (label, class_val, color) in &categories {
            let w = white_counts.get(class_val).copied().unwrap_or(0);
            let b = black_counts.get(class_val).copied().unwrap_or(0);
            lines.push(Line::from(vec![
                Span::styled(format!("  {:14}", label), Style::default().fg(*color)),
                Span::raw(format!("W:{:<3} B:{}", w, b)),
            ]));
        }

        lines.push(Line::raw(""));

        let critical: Vec<_> = review
            .positions
            .iter()
            .filter(|p| {
                matches!(
                    MoveClassification::try_from(p.classification),
                    Ok(MoveClassification::ClassificationBlunder)
                        | Ok(MoveClassification::ClassificationMistake)
                )
            })
            .collect();

        if !critical.is_empty() {
            lines.push(Line::from(Span::styled(
                "Critical Moments",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            for pos in critical.iter().take(8) {
                let is_white = is_white_ply(pos.ply);
                let move_num = pos.ply.div_ceil(2);
                let side = if is_white { "W" } else { "B" };
                let class_str = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => "??",
                    Ok(MoveClassification::ClassificationMistake) => "?",
                    _ => "",
                };
                let color = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => Color::Red,
                    _ => Color::Magenta,
                };

                lines.push(Line::from(vec![
                    Span::raw(format!("  {}. ", move_num)),
                    Span::raw(format!("[{}] ", side)),
                    Span::styled(
                        format!("{}{}", pos.played_san, class_str),
                        Style::default().fg(color),
                    ),
                ]));
            }
            lines.push(Line::raw(""));
        }

        if let Some(advanced) = &self.review_state.advanced {
            let white_psy = advanced.white_psychology.as_ref();
            let black_psy = advanced.black_psychology.as_ref();

            let w_opening = white_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
            let b_opening = black_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
            let w_mid = white_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
            let b_mid = black_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
            let w_end = white_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);
            let b_end = black_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);

            lines.push(Line::from(Span::styled(
                "Phase Performance (avg cp_loss)",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(vec![
                Span::raw("  Opening: "),
                Span::styled(
                    format!("W:{:.1} B:{:.1}", w_opening, b_opening),
                    Style::default().fg(Color::LightCyan),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Middlegame: "),
                Span::styled(
                    format!("W:{:.1} B:{:.1}", w_mid, b_mid),
                    Style::default().fg(Color::LightCyan),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Endgame: "),
                Span::styled(
                    format!("W:{:.1} B:{:.1}", w_end, b_end),
                    Style::default().fg(Color::LightCyan),
                ),
            ]));

            lines.push(Line::raw(""));

            let w_max_err = white_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
            let b_max_err = black_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
            let w_blunder = white_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);
            let b_blunder = black_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);

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

            let w_fav = white_psy.map(|p| p.favorable_swings).unwrap_or(0);
            let b_fav = black_psy.map(|p| p.favorable_swings).unwrap_or(0);
            let w_unfav = white_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
            let b_unfav = black_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
            let w_streak = white_psy.map(|p| p.max_momentum_streak).unwrap_or(0);
            let b_streak = black_psy.map(|p| p.max_momentum_streak).unwrap_or(0);

            lines.push(Line::from(Span::styled(
                "Momentum",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(vec![
                Span::raw("  Favorable: "),
                Span::styled(
                    format!("W:{}  B:{}", w_fav, b_fav),
                    Style::default().fg(Color::Green),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Unfavorable: "),
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
            lines.push(Line::from(vec![
                Span::styled("Critical positions: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}", advanced.critical_positions_count)),
            ]));
        }

        lines
    }

    fn render_position(&self) -> Vec<Line<'static>> {
        let mut lines = vec![];

        if self.review_state.current_ply == 0 {
            lines.push(Line::from(Span::styled(
                "No position selected",
                Style::default().fg(Color::DarkGray),
            )));
            return lines;
        }

        if let Some(pos) = self.review_state.current_position() {
            // Classification badge (colored header)
            let class_name = classification_display_name(pos.classification);
            let class_color = classification_color(pos.classification);
            lines.push(Line::from(Span::styled(
                format!(" {} ", class_name),
                Style::default()
                    .fg(Color::Black)
                    .bg(class_color)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::raw(""));

            // Move info
            let is_white = is_white_ply(pos.ply);
            let move_num = pos.ply.div_ceil(2);
            let side = if is_white { "White" } else { "Black" };
            let marker = classification_marker_str(pos.classification);

            lines.push(Line::from(vec![
                Span::styled("Move ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}. {}", move_num, side),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::raw(""));

            // Played move with marker
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

            // Evaluation before
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

            // Evaluation after
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

            // Evaluation best
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
            lines.push(Line::raw(""));

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
            lines.push(Line::raw(""));

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
                lines.push(Line::from(Span::styled(
                    format!(" {}", pv_text),
                    Style::default().fg(Color::Cyan),
                )));
            }
        }

        // Advanced position analysis
        if let Some(adv_pos) = self.review_state.advanced_position() {
            lines.push(Line::raw(""));

            if adv_pos.is_critical {
                lines.push(Line::from(Span::styled(
                    " \u{26A0} CRITICAL POSITION \u{26A0}",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }

            if !adv_pos.tactical_tags_after.is_empty() {
                render_tactical_tags(&mut lines, &adv_pos.tactical_tags_after);
            }

            if let Some(ref ks) = adv_pos.king_safety {
                render_king_safety(&mut lines, ks);
            }

            if let Some(ref tension) = adv_pos.tension {
                render_tension(&mut lines, tension);
            }
        }

        lines
    }
}

fn accuracy_color(accuracy: f64) -> Color {
    if accuracy >= 90.0 {
        Color::Green
    } else if accuracy >= 70.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn cp_loss_color(cp_loss: i32) -> Color {
    if cp_loss >= 100 {
        Color::Red
    } else if cp_loss >= 50 {
        Color::Magenta
    } else if cp_loss >= 25 {
        Color::Yellow
    } else {
        Color::White
    }
}

fn classification_marker(classification: &i32) -> Option<(&'static str, Color)> {
    match MoveClassification::try_from(*classification) {
        Ok(MoveClassification::ClassificationBrilliant) => Some(("!!", Color::Cyan)),
        Ok(MoveClassification::ClassificationExcellent) => Some(("!", Color::Cyan)),
        Ok(MoveClassification::ClassificationInaccuracy) => Some(("?!", Color::Yellow)),
        Ok(MoveClassification::ClassificationMistake) => Some(("?", Color::Magenta)),
        Ok(MoveClassification::ClassificationBlunder) => Some(("??", Color::Red)),
        Ok(MoveClassification::ClassificationForced) => Some(("[]", Color::DarkGray)),
        _ => None,
    }
}

fn count_classifications(
    positions: &[PositionReview],
) -> (
    std::collections::HashMap<i32, u32>,
    std::collections::HashMap<i32, u32>,
) {
    let mut white = std::collections::HashMap::new();
    let mut black = std::collections::HashMap::new();
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

fn render_tactical_tags(lines: &mut Vec<Line<'_>>, tags: &[TacticalTagProto]) {
    lines.push(Line::from(Span::styled(
        "  Tactics",
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

        let mut detail = kind_name.to_string();
        if let Some(ref attacker) = tag.attacker {
            detail.push_str(&format!(": {}", attacker));
        }
        if !tag.victims.is_empty() {
            detail.push_str(&format!(" -> {}", tag.victims.join(", ")));
        }
        detail.push_str(&format!("  {:.0}%", conf * 100.0));

        lines.push(Line::from(Span::styled(
            format!("    {}", detail),
            Style::default().fg(conf_color),
        )));
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

fn render_king_safety(lines: &mut Vec<Line<'_>>, ks: &chess_client::PositionKingSafetyProto) {
    lines.push(Line::from(Span::styled(
        "  King Safety",
        Style::default().fg(Color::Yellow),
    )));

    let white = ks.white.as_ref();
    let black = ks.black.as_ref();

    let get_f32 = |opt: Option<&chess_client::KingSafetyMetricsProto>| -> f32 {
        opt.map(|m| m.exposure_score).unwrap_or(0.0)
    };

    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!("White: {:.1}", get_f32(white)),
            Style::default().fg(Color::LightCyan),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Black: {:.1}", get_f32(black)),
            Style::default().fg(Color::LightCyan),
        ),
    ]));
}

fn render_tension(lines: &mut Vec<Line<'_>>, tension: &chess_client::PositionTensionMetricsProto) {
    lines.push(Line::from(Span::styled(
        "  Tension",
        Style::default().fg(Color::Yellow),
    )));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!("Attacked pairs: {}", tension.mutually_attacked_pairs),
            Style::default().fg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Contested: {}", tension.contested_squares),
            Style::default().fg(Color::White),
        ),
    ]));
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

fn classification_color(classification: i32) -> Color {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => Color::Cyan,
        Ok(MoveClassification::ClassificationBest) => Color::LightGreen,
        Ok(MoveClassification::ClassificationExcellent) => Color::Cyan,
        Ok(MoveClassification::ClassificationGood) => Color::White,
        Ok(MoveClassification::ClassificationInaccuracy) => Color::Yellow,
        Ok(MoveClassification::ClassificationMistake) => Color::Magenta,
        Ok(MoveClassification::ClassificationBlunder) => Color::Red,
        Ok(MoveClassification::ClassificationForced) => Color::DarkGray,
        _ => Color::White,
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

fn format_review_score(score: &chess_client::ReviewScore) -> (String, Color) {
    match score.score.as_ref() {
        Some(review_score::Score::Centipawns(cp)) => {
            let text = if *cp >= 0 {
                format!("+{}.{:02}", *cp / 100, (*cp % 100) / 10)
            } else {
                format!("{}.{:02}", *cp / 100, (-*cp % 100) / 10)
            };
            let color = if *cp > 0 {
                Color::LightCyan
            } else if *cp < 0 {
                Color::LightRed
            } else {
                Color::White
            };
            (text, color)
        }
        Some(review_score::Score::Mate(m)) => {
            let text = if *m > 0 {
                format!("M{}", m)
            } else {
                format!("-M{}", m.abs())
            };
            (
                text,
                if *m > 0 {
                    Color::LightCyan
                } else {
                    Color::LightRed
                },
            )
        }
        _ => ("0.00".to_string(), Color::White),
    }
}

fn format_mate_info(score: &chess_client::ReviewScore) -> String {
    match score.score.as_ref() {
        Some(review_score::Score::Mate(m)) => format!("  (Mate in {})", m.abs()),
        _ => String::new(),
    }
}

fn accuracy_bar(accuracy: f64, width: usize) -> String {
    let filled = ((accuracy / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

fn score_to_cp_clamped(pos: &PositionReview) -> i32 {
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

fn build_eval_graph(positions: &[PositionReview], width: usize) -> Vec<Line<'static>> {
    if positions.is_empty() || width == 0 {
        return vec![];
    }

    let height = 5usize;
    let mid = height / 2;

    let total = positions.len();
    let cols: Vec<(i32, Option<Color>)> = (0..width)
        .map(|col| {
            let idx = col * total / width;
            let idx = idx.min(total - 1);
            let pos = &positions[idx];
            let cp = score_to_cp_clamped(pos);

            let col_color = match MoveClassification::try_from(pos.classification) {
                Ok(MoveClassification::ClassificationBlunder) => Some(Color::Red),
                Ok(MoveClassification::ClassificationMistake) => Some(Color::Yellow),
                _ => None,
            };
            (cp, col_color)
        })
        .collect();

    let blocks = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(height);
    for row in 0..height {
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for &(cp, critical_color) in &cols {
            let max_sub = (height * 8) as f64;
            let mid_sub = (mid * 8) as f64;
            let fill_sub = mid_sub + (cp as f64 / 500.0) * mid_sub;
            let fill_sub = fill_sub.clamp(0.0, max_sub) as usize;

            let row_bottom = (height - 1 - row) * 8;
            let row_top = row_bottom + 8;

            let block_char = if fill_sub >= row_top {
                '\u{2588}'
            } else if fill_sub <= row_bottom {
                ' '
            } else {
                blocks[fill_sub - row_bottom]
            };

            let fg = if row <= mid {
                critical_color.unwrap_or(Color::White)
            } else {
                critical_color.unwrap_or(Color::Gray)
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

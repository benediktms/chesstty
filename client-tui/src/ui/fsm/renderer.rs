use crate::state::{GameMode, GameSession, PlayerColor};
use crate::ui::fsm::render_spec::{Component, Constraint, Layout, Overlay, Row};
use crate::ui::fsm::UiStateMachine;
use crate::ui::widgets::{
    advanced_analysis_panel::AdvancedAnalysisPanel, board_overlay::build_review_overlay,
    review_summary_panel::ReviewSummaryPanel, review_tabs_panel::ReviewTabsPanel, BoardWidget,
    MiniBoardWidget,
};
use ratatui::{layout::Rect, Frame};

pub struct Renderer;

impl Renderer {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        layout: &Layout,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        let row_areas = Self::split_vertical(area, &layout.rows);

        for (row, row_area) in layout.rows.iter().zip(row_areas.iter()) {
            let col_areas = Self::split_horizontal(*row_area, &row.columns);

            for (col, col_area) in row.columns.iter().zip(col_areas.iter()) {
                Self::render_column_content(frame, *col_area, &col.content, game_session, fsm);
            }
        }

        let overlay = fsm.overlay();
        if !matches!(overlay, Overlay::None) {
            Self::render_overlay(frame, area, overlay, game_session, fsm);
        }
    }

    fn split_vertical(area: Rect, rows: &[Row]) -> Vec<Rect> {
        if rows.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = rows
            .iter()
            .map(|r| Self::to_constraint(&r.height))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn split_horizontal(area: Rect, columns: &[crate::ui::fsm::render_spec::Column]) -> Vec<Rect> {
        if columns.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = columns
            .iter()
            .map(|c| Self::to_constraint(&c.constraint))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn split_vertical_nested(
        area: Rect,
        columns: &[crate::ui::fsm::render_spec::Column],
    ) -> Vec<Rect> {
        if columns.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = columns
            .iter()
            .map(|c| Self::to_constraint(&c.constraint))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn to_constraint(c: &Constraint) -> ratatui::layout::Constraint {
        match c {
            Constraint::Length(n) => ratatui::layout::Constraint::Length(*n),
            Constraint::Min(n) => ratatui::layout::Constraint::Min(*n),
            Constraint::Percentage(p) => ratatui::layout::Constraint::Percentage(*p),
            Constraint::Ratio(n, d) => ratatui::layout::Constraint::Ratio((*n).into(), (*d).into()),
        }
    }

    fn render_column_content(
        frame: &mut Frame,
        area: Rect,
        content: &crate::ui::fsm::render_spec::ColumnContent,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        match content {
            crate::ui::fsm::render_spec::ColumnContent::Component(component) => {
                Self::render_component(frame, area, component, game_session, fsm);
            }
            crate::ui::fsm::render_spec::ColumnContent::Nested(columns) => {
                let col_areas = Self::split_vertical_nested(area, columns);
                for (col, col_area) in columns.iter().zip(col_areas.iter()) {
                    Self::render_column_content(frame, *col_area, &col.content, game_session, fsm);
                }
            }
        }
    }

    fn render_component(
        frame: &mut Frame,
        area: Rect,
        component: &Component,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        use crate::ui::widgets::{
            EngineAnalysisPanel, GameInfoPanel, MoveHistoryPanel, TabInputWidget, UciDebugPanel,
        };

        match component {
            Component::Board => {
                let is_flipped = matches!(
                    game_session.mode,
                    GameMode::HumanVsEngine {
                        human_side: PlayerColor::Black
                    }
                );
                let board_overlay = if let Some(ref review) = game_session.review_state {
                    build_review_overlay(review)
                } else {
                    fsm.board_overlay(game_session)
                };
                let board_widget = BoardWidget {
                    board: game_session.board(),
                    overlay: &board_overlay,
                    flipped: is_flipped,
                };
                frame.render_widget(board_widget, area);
            }
            Component::TabInput => {
                let widget = TabInputWidget::new(game_session, fsm);
                frame.render_widget(widget, area);
            }
            Component::Controls => {
                use ratatui::style::{Color, Modifier, Style};
                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let mut controls_spans = Vec::new();
                let key_style = Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD);

                let is_review_mode = matches!(game_session.mode, GameMode::ReviewMode);

                if is_review_mode {
                    // Review mode controls
                    controls_spans.push(Span::styled("Tab", key_style));
                    controls_spans.push(Span::raw(" Tabs | "));
                    controls_spans.push(Span::styled("j/k", key_style));
                    controls_spans.push(Span::raw(" Moves | "));
                    controls_spans.push(Span::styled("Space", key_style));
                    controls_spans.push(Span::raw(" Auto | "));
                    controls_spans.push(Span::styled("Home/End", key_style));
                    controls_spans.push(Span::raw(" Jump | "));
                    controls_spans.push(Span::styled("Esc", key_style));
                    controls_spans.push(Span::raw(" Menu"));
                } else {
                    // Standard game controls
                    controls_spans.push(Span::styled("i", key_style));
                    controls_spans.push(Span::raw(" Input | "));

                    // Pause (HumanVsEngine or EngineVsEngine)
                    if matches!(
                        game_session.mode,
                        GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
                    ) {
                        if game_session.paused {
                            controls_spans.push(Span::styled(
                                "PAUSED",
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ));
                            controls_spans.push(Span::raw(" | "));
                        }
                        controls_spans.push(Span::styled("p", key_style));
                        controls_spans.push(Span::raw(" Pause | "));
                    }

                    // Undo
                    if game_session.is_undo_allowed() {
                        controls_spans.push(Span::styled("u", key_style));
                        controls_spans.push(Span::raw(" Undo | "));
                    }

                    controls_spans.push(Span::styled("Esc", key_style));
                    controls_spans.push(Span::raw(" Menu | "));
                    controls_spans.push(Span::styled("Tab", key_style));
                    controls_spans.push(Span::raw(" Panels | "));
                    controls_spans.push(Span::styled("@", key_style));
                    controls_spans.push(Span::raw(" UCI | "));
                    controls_spans.push(Span::styled(
                        "Ctrl+C",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ));
                    controls_spans.push(Span::raw(" Quit"));
                }

                let controls_line = Paragraph::new(Line::from(controls_spans))
                    .style(Style::default().bg(Color::Black));
                frame.render_widget(controls_line, area);
            }
            Component::InfoPanel => {
                let widget = GameInfoPanel::new(game_session, fsm);
                frame.render_widget(widget, area);
            }
            Component::HistoryPanel => {
                let scroll = fsm.component_manager.scroll(&Component::HistoryPanel);
                let is_selected =
                    fsm.component_manager.selected_component() == Some(Component::HistoryPanel);
                let review_positions = game_session
                    .review_state
                    .as_ref()
                    .map(|rs| rs.review.positions.as_slice());
                let current_ply = game_session.review_state.as_ref().map(|rs| rs.current_ply);
                let widget = MoveHistoryPanel::new(game_session.history(), scroll, is_selected)
                    .with_review_positions(review_positions)
                    .with_current_ply(current_ply);
                frame.render_widget(widget, area);
            }
            Component::EnginePanel => {
                let scroll = fsm.component_manager.scroll(&Component::EnginePanel);
                let is_selected =
                    fsm.component_manager.selected_component() == Some(Component::EnginePanel);
                let widget = EngineAnalysisPanel::new(
                    game_session.engine_info.as_ref(),
                    game_session.is_engine_thinking,
                    scroll,
                    is_selected,
                );
                frame.render_widget(widget, area);
            }
            Component::DebugPanel => {
                let scroll = fsm.component_manager.scroll(&Component::DebugPanel);
                let is_selected =
                    fsm.component_manager.selected_component() == Some(Component::DebugPanel);
                let widget = UciDebugPanel::new(&game_session.uci_log, scroll, is_selected);
                frame.render_widget(widget, area);
            }
            Component::ReviewTabs => {
                if let Some(ref review_state) = game_session.review_state {
                    let is_selected = fsm.component_manager.selected_component()
                        == Some(Component::ReviewSummary);
                    let scroll = fsm.component_manager.scroll(&Component::ReviewSummary);
                    let widget = ReviewTabsPanel {
                        review_state,
                        current_tab: fsm.review_tab,
                        scroll,
                        expanded: false,
                        is_selected,
                        moves_selection: None,
                    };
                    frame.render_widget(widget, area);
                }
            }
            Component::ReviewSummary => {
                if let Some(ref review_state) = game_session.review_state {
                    let is_selected = fsm.component_manager.selected_component()
                        == Some(Component::ReviewSummary);
                    let scroll = fsm.component_manager.scroll(&Component::ReviewSummary);
                    let widget = ReviewSummaryPanel {
                        review_state,
                        scroll,
                        is_selected,
                        expanded: false,
                    };
                    frame.render_widget(widget, area);
                }
            }
            Component::AdvancedAnalysis => {
                if let Some(ref review_state) = game_session.review_state {
                    let is_selected = fsm.component_manager.selected_component()
                        == Some(Component::AdvancedAnalysis);
                    let scroll = fsm.component_manager.scroll(&Component::AdvancedAnalysis);
                    let widget = AdvancedAnalysisPanel {
                        review_state,
                        scroll,
                        is_selected,
                        expanded: false,
                    };
                    frame.render_widget(widget, area);
                }
            }
        }
    }

    fn render_overlay(
        frame: &mut Frame,
        area: Rect,
        overlay: Overlay,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        use crate::ui::widgets::{
            EngineAnalysisPanel, GameInfoPanel, MoveAnalysisPanel, MoveHistoryPanel,
            PopupMenuWidget, PromotionWidget, ReviewSummaryPanel, SnapshotDialogWidget,
            UciDebugPanel,
        };

        match overlay {
            Overlay::None => {}
            Overlay::PopupMenu => {
                if let Some(ref state) = fsm.popup_menu {
                    let widget = PopupMenuWidget { state };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::SnapshotDialog => {
                if let Some(ref state) = fsm.snapshot_dialog {
                    let widget = SnapshotDialogWidget { state };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::PromotionDialog { .. } => {
                let widget = PromotionWidget {
                    selected_piece: fsm.selected_promotion_piece,
                };
                frame.render_widget(widget, area);
            }
        }
    }
}

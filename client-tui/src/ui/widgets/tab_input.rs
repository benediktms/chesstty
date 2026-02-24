use crate::state::GameSession;
use crate::ui::fsm::UiStateMachine;
use crate::ui::widgets::mini_board::piece_to_unicode;
use chess::{format_square, parse_square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Widget},
};

pub struct TabInputWidget<'a> {
    pub client_state: &'a GameSession,
    pub fsm: &'a UiStateMachine,
}

impl<'a> TabInputWidget<'a> {
    pub fn new(client_state: &'a GameSession, fsm: &'a UiStateMachine) -> Self {
        Self { client_state, fsm }
    }
}

impl<'a> Widget for TabInputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let active = self.fsm.tab_input.active;
        let current_tab = self.fsm.tab_input.current_tab;

        let border_style = if active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("Make Move");

        let inner = block.inner(area);
        block.render(area, buf);

        if !active {
            // Inactive: show greyed-out tab labels only
            let titles = vec!["Select Piece", "Select Destination"];
            let tabs = Tabs::new(titles)
                .style(Style::default().fg(Color::DarkGray))
                .select(0)
                .divider(" ");
            tabs.render(inner, buf);
            return;
        }

        // Active: split inner into tabs header + content
        if inner.height < 2 {
            return;
        }
        let tab_area = Rect { height: 1, ..inner };
        let content_area = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };

        // Render tab headers
        let titles = vec!["Select Piece", "Select Destination"];
        let tabs = Tabs::new(titles)
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(current_tab)
            .divider(" ");
        tabs.render(tab_area, buf);

        // Render content
        if current_tab == 0 {
            render_piece_content(self.client_state, self.fsm, buf, content_area);
        } else {
            render_destination_content(self.client_state, self.fsm, buf, content_area);
        }
    }
}

fn render_piece_content(state: &GameSession, fsm: &UiStateMachine, buf: &mut Buffer, area: Rect) {
    if area.height == 0 {
        return;
    }

    let typeahead = &fsm.tab_input.typeahead_buffer;
    let selectable_squares = &state.selectable_squares;

    let mut spans: Vec<Span> = vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(typeahead.as_str(), Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
    ];

    for square in selectable_squares {
        let square_str = format_square(*square);
        if !typeahead.is_empty() && !square_str.starts_with(typeahead) {
            continue;
        }
        if let (Some(piece), Some(color)) = (
            state.board().piece_on(*square),
            state.board().color_on(*square),
        ) {
            let symbol = piece_to_unicode(piece, color);
            let label = format!("{}{}", symbol, square_str);
            spans.push(Span::styled(label, Style::default().fg(Color::White)));
            spans.push(Span::raw("  "));
        }
    }

    Paragraph::new(Line::from(spans)).render(area, buf);
}

fn render_destination_content(
    state: &GameSession,
    fsm: &UiStateMachine,
    buf: &mut Buffer,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }

    let typeahead = &fsm.tab_input.typeahead_buffer;

    let moves = fsm
        .tab_input
        .from_square
        .and_then(|sq| state.legal_moves_from(sq))
        .map(|moves| {
            moves
                .iter()
                .filter_map(|m| parse_square(&m.to).map(format_square))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let mut spans: Vec<Span> = vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(typeahead.as_str(), Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
    ];

    for dest in &moves {
        if !typeahead.is_empty() && !dest.starts_with(typeahead) {
            continue;
        }
        spans.push(Span::styled(
            dest.as_str(),
            Style::default().fg(Color::White),
        ));
        spans.push(Span::raw("  "));
    }

    Paragraph::new(Line::from(spans)).render(area, buf);
}

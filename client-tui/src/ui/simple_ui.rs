use crate::state::ClientState;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

pub async fn run_simple_app() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = match ClientState::new("http://[::1]:50051").await {
        Ok(state) => state,
        Err(e) => {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
            return Err(anyhow::anyhow!("Failed to connect to server: {}", e));
        }
    };

    let result = run_ui_loop(&mut terminal, &mut state).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_ui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut ClientState,
) -> anyhow::Result<()> {
    let mut input_buffer = String::new();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                    Constraint::Length(5),
                ])
                .split(f.area());

            let title = Paragraph::new("ChessTTY - Connected to Server")
                .style(Style::default().fg(Color::Cyan))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            let info_text = vec![
                Line::from(vec![
                    Span::styled("FEN: ", Style::default().fg(Color::Yellow)),
                    Span::raw(state.fen()),
                ]),
                Line::from(vec![
                    Span::styled("To Move: ", Style::default().fg(Color::Yellow)),
                    Span::raw(state.side_to_move()),
                ]),
                Line::from(vec![
                    Span::styled("Moves: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", state.history().len())),
                ]),
            ];

            let info = Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title("Game Info"));
            f.render_widget(info, chunks[1]);

            let status_text = state
                .ui_state
                .status_message
                .clone()
                .unwrap_or_else(|| "Ready".to_string());
            let status = Paragraph::new(status_text)
                .style(Style::default().fg(Color::Green))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(status, chunks[2]);

            let help_text = vec![
                Line::from("Commands: m <from> <to> - Make move | u - Undo | r - Reset | q - Quit"),
                Line::from(format!("> {}", input_buffer)),
            ];

            let help = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(help, chunks[3]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') if input_buffer.is_empty() => break,
                    KeyCode::Char(c) => input_buffer.push(c),
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            handle_command(state, &input_buffer).await;
                            input_buffer.clear();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

async fn handle_command(state: &mut ClientState, command: &str) {
    use chess_common::parse_square;

    let parts: Vec<&str> = command.trim().split_whitespace().collect();

    match parts.as_slice() {
        ["m", from, to] => {
            if let (Some(from_sq), Some(to_sq)) = (parse_square(from), parse_square(to)) {
                state.select_square(from_sq);
                if let Err(e) = state.try_move_to(to_sq).await {
                    state.ui_state.status_message = Some(format!("Move error: {}", e));
                }
            } else {
                state.ui_state.status_message = Some("Invalid square(s)".to_string());
            }
        }
        ["u"] => {
            if let Err(e) = state.undo().await {
                state.ui_state.status_message = Some(format!("Undo error: {}", e));
            }
        }
        ["r"] => {
            if let Err(e) = state.reset(None).await {
                state.ui_state.status_message = Some(format!("Reset error: {}", e));
            }
        }
        _ => {
            state.ui_state.status_message = Some("Unknown command".to_string());
        }
    }
}

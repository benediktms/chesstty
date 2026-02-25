use crate::ui::theme::Theme;
use crate::ui::widgets::fen_dialog::FenDialogState;
use crate::ui::widgets::selectable_table::SelectableTableState;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    GameMode(GameModeOption),
    PlayAs(PlayAsOption),
    Difficulty(DifficultyOption),
    EngineThreads(ThreadsOption),
    EngineHash(HashOption),
    TimeControl(TimeControlOption),
    StartPosition(StartPositionOption),
    ResumeSession,
    ReviewGame,
    StartGame,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreadsOption {
    Auto, // Auto-detect, capped at 4
    One,
    Two,
    Four,
}

impl ThreadsOption {
    /// Resolve Auto to an actual thread count (capped at 4).
    pub fn resolve(&self) -> u32 {
        match self {
            ThreadsOption::Auto => std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(1)
                .min(4),
            ThreadsOption::One => 1,
            ThreadsOption::Two => 2,
            ThreadsOption::Four => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HashOption {
    Small,  // 32 MB
    Medium, // 128 MB
    Large,  // 256 MB
}

impl HashOption {
    pub fn megabytes(&self) -> u32 {
        match self {
            HashOption::Small => 32,
            HashOption::Medium => 128,
            HashOption::Large => 256,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameModeOption {
    HumanVsHuman,
    HumanVsEngine,
    EngineVsEngine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DifficultyOption {
    Beginner,     // Skill 1-3
    Intermediate, // Skill 8-12
    Advanced,     // Skill 15-18
    Master,       // Skill 20
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeControlOption {
    None,
    Blitz,     // 3 minutes
    Rapid,     // 10 minutes
    Classical, // 30 minutes
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartPositionOption {
    Standard,
    CustomFen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayAsOption {
    White,
    Black,
}

pub struct MenuState {
    pub selected_index: usize,
    pub game_mode: GameModeOption,
    pub play_as: PlayAsOption,
    pub difficulty: DifficultyOption,
    pub engine_threads: ThreadsOption,
    pub engine_hash: HashOption,
    pub time_control: TimeControlOption,
    pub start_position: StartPositionOption,
    pub fen_dialog_state: Option<FenDialogState>,
    pub saved_positions: Vec<chess_client::SavedPosition>,
    pub selected_fen: Option<String>,
    pub has_saved_session: bool,
    pub suspended_sessions: Vec<chess_client::SuspendedSessionInfo>,
    pub session_table: Option<SessionTableContext>,
    pub has_finished_games: bool,
    pub finished_games: Vec<chess_client::FinishedGameInfo>,
    pub review_table: Option<ReviewTableContext>,
}

/// Context for the review game selection table dialog.
pub struct ReviewTableContext {
    pub table_state: SelectableTableState,
    pub games: Vec<chess_client::FinishedGameInfo>,
}

/// Context for the session selection table dialog.
pub struct SessionTableContext {
    pub table_state: SelectableTableState,
    pub sessions: Vec<chess_client::SuspendedSessionInfo>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            selected_index: 0,
            game_mode: GameModeOption::HumanVsEngine,
            play_as: PlayAsOption::White,
            difficulty: DifficultyOption::Intermediate,
            engine_threads: ThreadsOption::Auto,
            engine_hash: HashOption::Medium,
            time_control: TimeControlOption::None,
            start_position: StartPositionOption::Standard,
            fen_dialog_state: None,
            saved_positions: vec![],
            selected_fen: None,
            has_saved_session: false,
            suspended_sessions: vec![],
            session_table: None,
            has_finished_games: false,
            finished_games: vec![],
            review_table: None,
        }
    }
}

impl MenuState {
    pub fn items(&self) -> Vec<MenuItem> {
        let has_engine = matches!(
            self.game_mode,
            GameModeOption::HumanVsEngine | GameModeOption::EngineVsEngine
        );

        let mut items = vec![MenuItem::GameMode(self.game_mode.clone())];

        // Show Play As only for Human vs Engine
        if self.game_mode == GameModeOption::HumanVsEngine {
            items.push(MenuItem::PlayAs(self.play_as));
        }

        items.push(MenuItem::Difficulty(self.difficulty));

        // Show engine tuning options when an engine is involved
        if has_engine {
            items.push(MenuItem::EngineThreads(self.engine_threads));
            items.push(MenuItem::EngineHash(self.engine_hash));
        }

        items.push(MenuItem::TimeControl(self.time_control));
        items.push(MenuItem::StartPosition(self.start_position));

        // Show Resume Session if a saved session exists
        if self.has_saved_session {
            items.push(MenuItem::ResumeSession);
        }

        // Show Review Game if finished games exist
        if self.has_finished_games {
            items.push(MenuItem::ReviewGame);
        }

        items.push(MenuItem::StartGame);
        items.push(MenuItem::Quit);
        items
    }

    pub fn cycle_play_as(&mut self) {
        self.play_as = match self.play_as {
            PlayAsOption::White => PlayAsOption::Black,
            PlayAsOption::Black => PlayAsOption::White,
        };
    }
}

pub struct MenuWidget<'a> {
    pub menu_state: &'a MenuState,
    pub theme: &'a Theme,
}

impl Widget for MenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the background
        Clear.render(area, buf);

        // Calculate centered menu area
        let menu_width = 60;
        let menu_height = 18;
        let x = (area.width.saturating_sub(menu_width)) / 2;
        let y = (area.height.saturating_sub(menu_height)) / 2;

        let menu_area = Rect {
            x: area.x + x,
            y: area.y + y,
            width: menu_width.min(area.width),
            height: menu_height.min(area.height),
        };

        let block = Block::default()
            .title("♔ ChessTTY - New Game ♔")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.info))
            .style(Style::default().bg(self.theme.dialog_bg));

        let inner = block.inner(menu_area);
        block.render(menu_area, buf);

        let items = self.menu_state.items();
        let mut lines = vec![
            Line::raw(""),
            Line::from(vec![Span::styled(
                "Welcome to ChessTTY!",
                Style::default()
                    .fg(self.theme.warning)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::raw(""),
        ];

        for (idx, item) in items.iter().enumerate() {
            let is_selected = idx == self.menu_state.selected_index;
            let prefix = if is_selected { "► " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(self.theme.dialog_highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_primary)
            };

            let line = match item {
                MenuItem::GameMode(mode) => {
                    let mode_str = match mode {
                        GameModeOption::HumanVsHuman => "Human vs Human",
                        GameModeOption::HumanVsEngine => "Human vs Engine",
                        GameModeOption::EngineVsEngine => "Engine vs Engine",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Game Mode: ", style),
                        Span::styled(mode_str, style.fg(self.theme.info)),
                        Span::styled(" [←/→]", Style::default().fg(self.theme.muted)),
                    ])
                }
                MenuItem::Difficulty(diff) => {
                    let diff_str = match diff {
                        DifficultyOption::Beginner => "Beginner",
                        DifficultyOption::Intermediate => "Intermediate",
                        DifficultyOption::Advanced => "Advanced",
                        DifficultyOption::Master => "Master",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Difficulty: ", style),
                        Span::styled(diff_str, style.fg(self.theme.positive)),
                        Span::styled(" [←/→]", Style::default().fg(self.theme.muted)),
                    ])
                }
                MenuItem::TimeControl(time) => {
                    let time_str = match time {
                        TimeControlOption::None => "None",
                        TimeControlOption::Blitz => "Blitz (3 min)",
                        TimeControlOption::Rapid => "Rapid (10 min)",
                        TimeControlOption::Classical => "Classical (30 min)",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Time Control: ", style),
                        Span::styled(time_str, style.fg(self.theme.secondary)),
                        Span::styled(" [←/→]", Style::default().fg(self.theme.muted)),
                    ])
                }
                MenuItem::EngineThreads(threads) => {
                    let threads_str = match threads {
                        ThreadsOption::Auto => {
                            let resolved = threads.resolve();
                            format!("Auto ({})", resolved)
                        }
                        ThreadsOption::One => "1".to_string(),
                        ThreadsOption::Two => "2".to_string(),
                        ThreadsOption::Four => "4".to_string(),
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Engine Threads: ", style),
                        Span::styled(threads_str, style.fg(self.theme.info)),
                        Span::styled(
                            " [\u{2190}/\u{2192}]",
                            Style::default().fg(self.theme.muted),
                        ),
                    ])
                }
                MenuItem::EngineHash(hash) => {
                    let hash_str = match hash {
                        HashOption::Small => "32 MB",
                        HashOption::Medium => "128 MB",
                        HashOption::Large => "256 MB",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Engine Hash: ", style),
                        Span::styled(hash_str, style.fg(self.theme.info)),
                        Span::styled(
                            " [\u{2190}/\u{2192}]",
                            Style::default().fg(self.theme.muted),
                        ),
                    ])
                }
                MenuItem::StartPosition(pos) => {
                    let pos_str = match pos {
                        StartPositionOption::Standard => "Standard",
                        StartPositionOption::CustomFen => "Custom FEN",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Start Position: ", style),
                        Span::styled(pos_str, style.fg(self.theme.warning)),
                        Span::styled(" [←/→]", Style::default().fg(self.theme.muted)),
                    ])
                }
                MenuItem::PlayAs(play_as) => {
                    let play_as_str = match play_as {
                        PlayAsOption::White => "White",
                        PlayAsOption::Black => "Black",
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled("Play As: ", style),
                        Span::styled(play_as_str, style.fg(self.theme.warning)),
                        Span::styled(
                            " [\u{2190}/\u{2192}]",
                            Style::default().fg(self.theme.muted),
                        ),
                    ])
                }
                MenuItem::ResumeSession => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(
                        "\u{25b6} Resume Session",
                        style.fg(self.theme.info),
                    ),
                ]),
                MenuItem::ReviewGame => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(
                        "\u{25b6} Review Game",
                        style.fg(self.theme.positive),
                    ),
                ]),
                MenuItem::StartGame => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(
                        "\u{25b6} Start Game",
                        style.fg(self.theme.positive),
                    ),
                ]),
                MenuItem::Quit => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled("\u{2715} Quit", style.fg(self.theme.negative)),
                ]),
            };

            lines.push(line);
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled(
            "↑/↓: Navigate  ←/→: Change  Enter: Select",
            Style::default().fg(self.theme.muted),
        )]));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
        paragraph.render(inner, buf);
    }
}

impl TimeControlOption {
    pub fn seconds(&self) -> Option<u64> {
        match self {
            TimeControlOption::None => None,
            TimeControlOption::Blitz => Some(3 * 60),
            TimeControlOption::Rapid => Some(10 * 60),
            TimeControlOption::Classical => Some(30 * 60),
        }
    }
}

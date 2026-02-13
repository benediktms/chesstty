use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    GameMode(GameModeOption),
    Difficulty(DifficultyOption),
    TimeControl(TimeControlOption),
    StartGame,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameModeOption {
    HumanVsHuman,
    HumanVsEngine,
    EngineVsEngine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DifficultyOption {
    Beginner,      // Skill 1-3
    Intermediate,  // Skill 8-12
    Advanced,      // Skill 15-18
    Master,        // Skill 20
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeControlOption {
    None,
    Blitz,      // 3 minutes
    Rapid,      // 10 minutes
    Classical,  // 30 minutes
}

pub struct MenuState {
    pub selected_index: usize,
    pub game_mode: GameModeOption,
    pub difficulty: DifficultyOption,
    pub time_control: TimeControlOption,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            selected_index: 0,
            game_mode: GameModeOption::HumanVsEngine,
            difficulty: DifficultyOption::Intermediate,
            time_control: TimeControlOption::None,
        }
    }
}

impl MenuState {
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self, max: usize) {
        if self.selected_index < max - 1 {
            self.selected_index += 1;
        }
    }

    pub fn items(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::GameMode(self.game_mode.clone()),
            MenuItem::Difficulty(self.difficulty),
            MenuItem::TimeControl(self.time_control),
            MenuItem::StartGame,
            MenuItem::Quit,
        ]
    }

    pub fn cycle_game_mode(&mut self) {
        self.game_mode = match self.game_mode {
            GameModeOption::HumanVsHuman => GameModeOption::HumanVsEngine,
            GameModeOption::HumanVsEngine => GameModeOption::EngineVsEngine,
            GameModeOption::EngineVsEngine => GameModeOption::HumanVsHuman,
        };
    }

    pub fn cycle_difficulty(&mut self) {
        self.difficulty = match self.difficulty {
            DifficultyOption::Beginner => DifficultyOption::Intermediate,
            DifficultyOption::Intermediate => DifficultyOption::Advanced,
            DifficultyOption::Advanced => DifficultyOption::Master,
            DifficultyOption::Master => DifficultyOption::Beginner,
        };
    }

    pub fn cycle_time_control(&mut self) {
        self.time_control = match self.time_control {
            TimeControlOption::None => TimeControlOption::Blitz,
            TimeControlOption::Blitz => TimeControlOption::Rapid,
            TimeControlOption::Rapid => TimeControlOption::Classical,
            TimeControlOption::Classical => TimeControlOption::None,
        };
    }
}

pub struct MenuWidget<'a> {
    pub menu_state: &'a MenuState,
}

impl<'a> MenuWidget<'a> {
    pub fn new(menu_state: &'a MenuState) -> Self {
        Self { menu_state }
    }
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
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(menu_area);
        block.render(menu_area, buf);

        let items = self.menu_state.items();
        let mut lines = vec![
            Line::raw(""),
            Line::from(vec![
                Span::styled(
                    "Welcome to ChessTTY!",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::raw(""),
        ];

        for (idx, item) in items.iter().enumerate() {
            let is_selected = idx == self.menu_state.selected_index;
            let prefix = if is_selected { "► " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
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
                        Span::styled(mode_str, style.fg(Color::Cyan)),
                        Span::styled(" [←/→]", Style::default().fg(Color::DarkGray)),
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
                        Span::styled(diff_str, style.fg(Color::Green)),
                        Span::styled(" [←/→]", Style::default().fg(Color::DarkGray)),
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
                        Span::styled(time_str, style.fg(Color::Magenta)),
                        Span::styled(" [←/→]", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MenuItem::StartGame => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled("▶ Start Game", style.fg(Color::Green)),
                ]),
                MenuItem::Quit => Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled("✕ Quit", style.fg(Color::Red)),
                ]),
            };

            lines.push(line);
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(
                "↑/↓: Navigate  ←/→: Change  Enter: Select",
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
        paragraph.render(inner, buf);
    }
}

impl DifficultyOption {
    pub fn skill_level(&self) -> u8 {
        match self {
            DifficultyOption::Beginner => 2,
            DifficultyOption::Intermediate => 10,
            DifficultyOption::Advanced => 17,
            DifficultyOption::Master => 20,
        }
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

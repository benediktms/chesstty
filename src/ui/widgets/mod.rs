pub mod board;
pub mod controls_panel;
pub mod game_info_panel;
pub mod menu;
pub mod move_history_panel;

pub use board::BoardWidget;
pub use controls_panel::ControlsPanel;
pub use game_info_panel::GameInfoPanel;
pub use menu::{MenuWidget, MenuItem, MenuState};
pub use move_history_panel::MoveHistoryPanel;

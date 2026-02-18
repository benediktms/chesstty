// UI modules
pub mod context;
pub mod fsm;
pub mod menu_app;
pub mod pane;
pub mod widgets;

// Main entry points
pub mod input;
pub mod render_loop;

pub use render_loop::run_app;

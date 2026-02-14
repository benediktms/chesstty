// UI modules
pub mod context;
mod full_ui;
mod input;
mod menu_app;
pub mod pane;
mod simple_ui;
pub mod widgets;

// Main entry point - use full UI by default
pub use full_ui::run_app;

// Also export simple UI for those who prefer it
pub use simple_ui::run_simple_app;

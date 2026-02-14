// UI modules
mod full_ui;
mod simple_ui;
mod menu_app;
pub mod widgets;

// Main entry point - use full UI by default
pub use full_ui::run_app;

// Also export simple UI for those who prefer it
pub use simple_ui::run_simple_app;

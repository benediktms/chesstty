//! Common utilities for ChessTTY
//!
//! This crate provides shared conversion utilities and UCI helpers
//! used across the ChessTTY client, server, and engine components.

pub mod converters;
pub mod uci;

// Re-export commonly used items
pub use converters::*;
pub use uci::*;

//! Endpoint handlers organized by domain

pub mod session;
pub mod game;
pub mod engine;
pub mod events;
pub mod persistence;
pub mod positions;

pub use session::SessionEndpoints;
pub use game::GameEndpoints;
pub use engine::EngineEndpoints;
pub use events::EventsEndpoints;
pub use persistence::PersistenceEndpoints;
pub use positions::PositionsEndpoints;

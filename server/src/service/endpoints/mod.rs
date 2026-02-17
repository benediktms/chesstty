//! Endpoint handlers organized by domain

pub mod engine;
pub mod events;
pub mod game;
pub mod persistence;
pub mod positions;
pub mod review;
pub mod session;

pub use engine::EngineEndpoints;
pub use events::EventsEndpoints;
pub use game::GameEndpoints;
pub use persistence::PersistenceEndpoints;
pub use positions::PositionsEndpoints;
pub use review::ReviewEndpoints;
pub use session::SessionEndpoints;

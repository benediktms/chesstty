use crate::state::PlayerColor;
use std::time::{Duration, Instant};

/// Manages two chess clocks (one per player).
pub struct ChessTimer {
    white_remaining: Duration,
    black_remaining: Duration,
    active_side: Option<PlayerColor>,
    last_tick: Instant,
}

impl ChessTimer {
    /// Create a new timer with equal initial time for both sides.
    pub fn new(initial_time: Duration) -> Self {
        Self {
            white_remaining: initial_time,
            black_remaining: initial_time,
            active_side: None,
            last_tick: Instant::now(),
        }
    }

    /// Tick the timer — deducts elapsed time from the active side's clock.
    /// Call this once per frame in the game loop.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let elapsed = now - self.last_tick;
        self.last_tick = now;
        self.tick_with_elapsed(elapsed);
    }

    /// Tick with a specific elapsed duration (useful for testing).
    pub fn tick_with_elapsed(&mut self, elapsed: Duration) {
        if let Some(side) = self.active_side {
            match side {
                PlayerColor::White => {
                    self.white_remaining = self.white_remaining.saturating_sub(elapsed);
                }
                PlayerColor::Black => {
                    self.black_remaining = self.black_remaining.saturating_sub(elapsed);
                }
            }
        }
    }

    /// Start or switch the clock to the given side.
    pub fn switch_to(&mut self, side: PlayerColor) {
        self.last_tick = Instant::now();
        self.active_side = Some(side);
    }

    /// Pause the timer (no side is active).
    pub fn pause(&mut self) {
        // Deduct any remaining elapsed time before pausing
        self.tick();
        self.active_side = None;
    }

    /// Get remaining time for a side.
    pub fn remaining(&self, side: PlayerColor) -> Duration {
        match side {
            PlayerColor::White => self.white_remaining,
            PlayerColor::Black => self.black_remaining,
        }
    }

    /// Check if a side's time has run out.
    pub fn is_flag_fallen(&self, side: PlayerColor) -> bool {
        self.remaining(side) == Duration::ZERO
    }

    /// Get which side's clock is currently running, if any.
    pub fn active_side(&self) -> Option<PlayerColor> {
        self.active_side
    }

    /// Format a duration for display. MM:SS or M:SS.s when under 10 seconds.
    pub fn format_time(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;

        if total_secs < 10 {
            let tenths = duration.subsec_millis() / 100;
            format!("{}:{:02}.{}", minutes, seconds, tenths)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_timer() {
        let timer = ChessTimer::new(Duration::from_secs(180));
        assert_eq!(timer.remaining(PlayerColor::White), Duration::from_secs(180));
        assert_eq!(timer.remaining(PlayerColor::Black), Duration::from_secs(180));
    }

    #[test]
    fn test_tick_reduces_active_side() {
        let mut timer = ChessTimer::new(Duration::from_secs(180));
        timer.switch_to(PlayerColor::White);
        timer.tick_with_elapsed(Duration::from_secs(1));
        assert_eq!(timer.remaining(PlayerColor::White), Duration::from_secs(179));
    }

    #[test]
    fn test_tick_does_not_reduce_inactive() {
        let mut timer = ChessTimer::new(Duration::from_secs(180));
        timer.switch_to(PlayerColor::White);
        timer.tick_with_elapsed(Duration::from_secs(5));
        assert_eq!(timer.remaining(PlayerColor::Black), Duration::from_secs(180));
    }

    #[test]
    fn test_paused_timer_does_not_tick() {
        let mut timer = ChessTimer::new(Duration::from_secs(180));
        // No active side by default
        timer.tick_with_elapsed(Duration::from_secs(10));
        assert_eq!(timer.remaining(PlayerColor::White), Duration::from_secs(180));
        assert_eq!(timer.remaining(PlayerColor::Black), Duration::from_secs(180));
    }

    #[test]
    fn test_flag_fallen() {
        let mut timer = ChessTimer::new(Duration::from_secs(5));
        timer.switch_to(PlayerColor::White);
        timer.tick_with_elapsed(Duration::from_secs(5));
        assert!(timer.is_flag_fallen(PlayerColor::White));
        assert!(!timer.is_flag_fallen(PlayerColor::Black));
    }

    #[test]
    fn test_saturating_subtraction() {
        let mut timer = ChessTimer::new(Duration::from_secs(3));
        timer.switch_to(PlayerColor::Black);
        timer.tick_with_elapsed(Duration::from_secs(10)); // More than available
        assert_eq!(timer.remaining(PlayerColor::Black), Duration::ZERO);
    }

    #[test]
    fn test_switch_sides() {
        let mut timer = ChessTimer::new(Duration::from_secs(60));
        timer.switch_to(PlayerColor::White);
        assert_eq!(timer.active_side(), Some(PlayerColor::White));
        timer.switch_to(PlayerColor::Black);
        assert_eq!(timer.active_side(), Some(PlayerColor::Black));
    }

    #[test]
    fn test_remaining() {
        let mut timer = ChessTimer::new(Duration::from_secs(300));
        timer.switch_to(PlayerColor::White);
        timer.tick_with_elapsed(Duration::from_secs(30));
        assert_eq!(timer.remaining(PlayerColor::White), Duration::from_secs(270));
        assert_eq!(timer.remaining(PlayerColor::Black), Duration::from_secs(300));
    }

    #[test]
    fn test_format_time_minutes() {
        assert_eq!(ChessTimer::format_time(Duration::from_secs(180)), "3:00");
        assert_eq!(ChessTimer::format_time(Duration::from_secs(65)), "1:05");
    }

    #[test]
    fn test_format_time_under_10_seconds() {
        assert_eq!(
            ChessTimer::format_time(Duration::from_millis(5300)),
            "0:05.3"
        );
    }
}

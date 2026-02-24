use crate::state::{GameMode, PlayerColor};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MatchSummaryState {
    pub game_result: Option<(i32, String)>,
    pub move_count: u32,
    pub game_mode: GameMode,
    pub winner: Option<PlayerColor>,
}

impl Default for MatchSummaryState {
    fn default() -> Self {
        Self {
            game_result: None,
            move_count: 0,
            game_mode: GameMode::HumanVsHuman,
            winner: None,
        }
    }
}

#[allow(dead_code)]
impl MatchSummaryState {
    pub fn new(result: Option<(i32, String)>, move_count: u32, game_mode: GameMode) -> Self {
        let winner = result.as_ref().and_then(|(status, _)| {
            if *status == 1 {
                Some(PlayerColor::Black)
            } else {
                None
            }
        });

        Self {
            game_result: result,
            move_count,
            game_mode,
            winner,
        }
    }
}

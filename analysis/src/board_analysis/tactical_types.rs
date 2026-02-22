use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TacticalTagKind {
    Fork,
    Pin,
    Skewer,
    DiscoveredAttack,
    DoubleAttack,
    HangingPiece,
    Sacrifice,
    Zwischenzug,
    BackRankWeakness,
    MateThreat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TacticalLine {
    pub from: String,
    pub through: Vec<String>,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TacticalEvidence {
    pub lines: Vec<TacticalLine>,
    pub threatened_pieces: Vec<String>,
    pub defended_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TacticalTag {
    pub kind: TacticalTagKind,
    pub attacker: Option<String>,
    pub victims: Vec<String>,
    pub target_square: Option<String>,
    pub confidence: f32,
    pub note: Option<String>,
    pub evidence: TacticalEvidence,
}

#[cfg(test)]
mod tests {
    use super::{TacticalEvidence, TacticalLine, TacticalTag, TacticalTagKind};

    #[test]
    fn tactical_tag_round_trip_serialization() {
        let tag = TacticalTag {
            kind: TacticalTagKind::Skewer,
            attacker: Some(String::from("c4")),
            victims: vec![String::from("f7"), String::from("g8")],
            target_square: Some(String::from("f7")),
            confidence: 0.92,
            note: Some(String::from("bishop pressure on diagonal")),
            evidence: TacticalEvidence {
                lines: vec![TacticalLine {
                    from: String::from("c4"),
                    through: vec![String::from("d5"), String::from("e6")],
                    to: String::from("f7"),
                }],
                threatened_pieces: vec![String::from("f7"), String::from("g8")],
                defended_by: vec![String::from("e8")],
            },
        };

        let serialized = serde_json::to_string(&tag).expect("serialize tactical tag");
        let restored: TacticalTag =
            serde_json::from_str(&serialized).expect("deserialize tactical tag");

        assert_eq!(tag, restored);
    }
}

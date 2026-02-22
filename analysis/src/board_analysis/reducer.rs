use super::tactical_types::{TacticalTag, TacticalTagKind};

/// Returns a priority value for a TacticalTagKind (lower = higher priority).
fn kind_priority(kind: &TacticalTagKind) -> u8 {
    match kind {
        TacticalTagKind::MateThreat => 0,
        TacticalTagKind::Fork => 1,
        TacticalTagKind::Pin => 2,
        TacticalTagKind::Skewer => 3,
        TacticalTagKind::DiscoveredAttack => 4,
        TacticalTagKind::DoubleAttack => 5,
        TacticalTagKind::Sacrifice => 6,
        TacticalTagKind::BackRankWeakness => 7,
        TacticalTagKind::HangingPiece => 8,
        TacticalTagKind::Zwischenzug => 9,
    }
}

/// Deduplication key for a tactical tag.
///
/// Two tags are duplicates if they have the same kind, same attacker, and
/// the same victims (as a sorted set).
#[derive(PartialEq, Eq, Hash)]
struct DedupeKey {
    kind: String,
    attacker: Option<String>,
    victims: Vec<String>, // sorted
}

impl DedupeKey {
    fn from_tag(tag: &TacticalTag) -> Self {
        let mut victims = tag.victims.clone();
        victims.sort();
        Self {
            kind: format!("{:?}", tag.kind),
            attacker: tag.attacker.clone(),
            victims,
        }
    }
}

/// Reduces a collection of tactical tags by deduplicating and ranking.
///
/// 1. Deduplicates tags that share the same kind, attacker, and victims
///    (as a sorted set), keeping the one with the highest confidence.
/// 2. Sorts by confidence descending, then by kind priority ascending.
/// 3. Truncates to `max_results` if provided.
pub fn reduce_tags(tags: Vec<TacticalTag>, max_results: Option<usize>) -> Vec<TacticalTag> {
    // Deduplicate: use an index map to track the best tag per key.
    let mut seen: std::collections::HashMap<DedupeKey, TacticalTag> =
        std::collections::HashMap::new();

    for tag in tags {
        let key = DedupeKey::from_tag(&tag);
        seen.entry(key)
            .and_modify(|existing| {
                if tag.confidence > existing.confidence {
                    *existing = tag.clone();
                }
            })
            .or_insert(tag);
    }

    let mut result: Vec<TacticalTag> = seen.into_values().collect();

    // Sort: primary = confidence descending, secondary = kind priority ascending.
    result.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| kind_priority(&a.kind).cmp(&kind_priority(&b.kind)))
    });

    if let Some(n) = max_results {
        result.truncate(n);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_analysis::tactical_types::{TacticalEvidence, TacticalTagKind};

    fn make_tag(kind: TacticalTagKind, attacker: Option<&str>, victims: Vec<&str>, confidence: f32) -> TacticalTag {
        TacticalTag {
            kind,
            attacker: attacker.map(String::from),
            victims: victims.into_iter().map(String::from).collect(),
            target_square: None,
            confidence,
            note: None,
            evidence: TacticalEvidence::default(),
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = reduce_tags(vec![], None);
        assert!(result.is_empty());
    }

    #[test]
    fn deduplicates_same_tag() {
        // Two fork tags with same attacker and victims — keep the higher confidence one.
        let tag_low = make_tag(TacticalTagKind::Fork, Some("d5"), vec!["c3", "f6"], 0.7);
        let tag_high = make_tag(TacticalTagKind::Fork, Some("d5"), vec!["c3", "f6"], 0.95);

        let result = reduce_tags(vec![tag_low, tag_high], None);

        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.95).abs() < 1e-6);
    }

    #[test]
    fn deduplicates_same_tag_victims_order_independent() {
        // Victims in different order should still be considered duplicates.
        let tag_a = make_tag(TacticalTagKind::Fork, Some("d5"), vec!["f6", "c3"], 0.8);
        let tag_b = make_tag(TacticalTagKind::Fork, Some("d5"), vec!["c3", "f6"], 0.6);

        let result = reduce_tags(vec![tag_a, tag_b], None);

        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn sorts_by_confidence_descending() {
        let tag_low = make_tag(TacticalTagKind::Pin, Some("a1"), vec!["c3"], 0.5);
        let tag_mid = make_tag(TacticalTagKind::Pin, Some("b2"), vec!["d4"], 0.75);
        let tag_high = make_tag(TacticalTagKind::Pin, Some("c3"), vec!["e5"], 0.9);

        let result = reduce_tags(vec![tag_low, tag_mid, tag_high], None);

        assert_eq!(result.len(), 3);
        assert!((result[0].confidence - 0.9).abs() < 1e-6);
        assert!((result[1].confidence - 0.75).abs() < 1e-6);
        assert!((result[2].confidence - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sorts_by_kind_priority() {
        // Same confidence — kind priority should break the tie.
        let pin_tag = make_tag(TacticalTagKind::Pin, Some("a1"), vec!["b2"], 0.8);
        let fork_tag = make_tag(TacticalTagKind::Fork, Some("c3"), vec!["d4"], 0.8);
        let mate_tag = make_tag(TacticalTagKind::MateThreat, None, vec!["e8"], 0.8);

        let result = reduce_tags(vec![pin_tag, fork_tag, mate_tag], None);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].kind, TacticalTagKind::MateThreat);
        assert_eq!(result[1].kind, TacticalTagKind::Fork);
        assert_eq!(result[2].kind, TacticalTagKind::Pin);
    }

    #[test]
    fn limits_max_results() {
        let tags = vec![
            make_tag(TacticalTagKind::MateThreat, None, vec!["e8"], 0.95),
            make_tag(TacticalTagKind::Fork, Some("d5"), vec!["c3", "f6"], 0.85),
            make_tag(TacticalTagKind::Pin, Some("a4"), vec!["c6"], 0.75),
        ];

        let result = reduce_tags(tags, Some(2));

        assert_eq!(result.len(), 2);
        // Should have the top-2 by confidence.
        assert!((result[0].confidence - 0.95).abs() < 1e-6);
        assert!((result[1].confidence - 0.85).abs() < 1e-6);
    }
}

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Smoothing factor for running-average relationship scores.
/// Lower values mean slower change (more memory of past interactions).
const SMOOTHING_ALPHA: f64 = 0.2;

/// Social component tracking relationships with other entities.
///
/// Each entity maintains a map of entity IDs to relationship scores
/// (range -1.0 to 1.0). Scores are updated as a running average
/// whenever an interaction occurs. The `last_positive_contact_tick`
/// field tracks recency of positive social contact for computing
/// the social need drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Social {
    /// Entity ID (as u64 bits) -> relationship score in [-1.0, 1.0].
    pub relationships: HashMap<u64, f64>,
    /// Tick of the most recent positive social interaction.
    pub last_positive_contact_tick: u64,
}

impl Default for Social {
    fn default() -> Self {
        Self {
            relationships: HashMap::new(),
            last_positive_contact_tick: 0,
        }
    }
}

impl Social {
    /// Record an interaction with another entity.
    ///
    /// `valence` should be in [-1.0, 1.0] where positive means friendly
    /// and negative means hostile. The relationship score is updated as
    /// an exponentially-weighted running average:
    ///   score = (1 - alpha) * old_score + alpha * valence
    ///
    /// If `current_tick` is provided and the valence is positive, updates
    /// `last_positive_contact_tick`.
    pub fn record_interaction(&mut self, entity_id: u64, valence: f64, current_tick: Option<u64>) {
        let valence = valence.clamp(-1.0, 1.0);
        let old = self.relationships.get(&entity_id).copied().unwrap_or(0.0);
        let new_score = ((1.0 - SMOOTHING_ALPHA) * old + SMOOTHING_ALPHA * valence).clamp(-1.0, 1.0);
        self.relationships.insert(entity_id, new_score);

        if valence > 0.0 {
            if let Some(tick) = current_tick {
                self.last_positive_contact_tick = self.last_positive_contact_tick.max(tick);
            }
        }
    }

    /// Get the relationship score with another entity (0.0 if unknown).
    pub fn get_relationship(&self, entity_id: u64) -> f64 {
        self.relationships.get(&entity_id).copied().unwrap_or(0.0)
    }

    /// Return the entity with the highest (most positive) relationship score.
    pub fn best_relationship(&self) -> Option<(u64, f64)> {
        self.relationships
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, &score)| (id, score))
    }

    /// Return the entity with the lowest (most negative) relationship score.
    pub fn worst_relationship(&self) -> Option<(u64, f64)> {
        self.relationships
            .iter()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, &score)| (id, score))
    }

    /// Remove entries for entities that are no longer alive.
    pub fn cleanup_dead(&mut self, alive_ids: &HashSet<u64>) {
        self.relationships.retain(|id, _| alive_ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_relationships() {
        let s = Social::default();
        assert!(s.relationships.is_empty());
        assert_eq!(s.last_positive_contact_tick, 0);
    }

    #[test]
    fn record_positive_interaction_creates_positive_score() {
        let mut s = Social::default();
        s.record_interaction(100, 1.0, Some(10));

        let score = s.get_relationship(100);
        assert!(score > 0.0, "positive interaction should produce positive score, got {}", score);
        // First interaction: (1 - 0.2) * 0.0 + 0.2 * 1.0 = 0.2
        assert!((score - 0.2).abs() < 1e-10);
        assert_eq!(s.last_positive_contact_tick, 10);
    }

    #[test]
    fn record_negative_interaction_creates_negative_score() {
        let mut s = Social::default();
        s.record_interaction(200, -1.0, Some(5));

        let score = s.get_relationship(200);
        assert!(score < 0.0, "negative interaction should produce negative score, got {}", score);
        assert!((score - -0.2).abs() < 1e-10);
        // Negative valence should not update last_positive_contact_tick
        assert_eq!(s.last_positive_contact_tick, 0);
    }

    #[test]
    fn running_average_converges() {
        let mut s = Social::default();
        // Many positive interactions should push score toward 1.0
        for tick in 0..50 {
            s.record_interaction(42, 1.0, Some(tick));
        }
        let score = s.get_relationship(42);
        assert!(score > 0.95, "50 positive interactions should push score near 1.0, got {}", score);
    }

    #[test]
    fn mixed_interactions_produce_moderate_score() {
        let mut s = Social::default();
        // Alternate positive and negative
        for i in 0..20 {
            let valence = if i % 2 == 0 { 1.0 } else { -1.0 };
            s.record_interaction(42, valence, Some(i));
        }
        let score = s.get_relationship(42);
        // Score should be near zero with alternating +1/-1
        assert!(score.abs() < 0.3, "alternating interactions should keep score moderate, got {}", score);
    }

    #[test]
    fn best_and_worst_relationship() {
        let mut s = Social::default();
        s.record_interaction(1, 1.0, None);
        s.record_interaction(2, -1.0, None);
        s.record_interaction(3, 0.5, None);

        let (best_id, best_score) = s.best_relationship().unwrap();
        assert_eq!(best_id, 1);
        assert!(best_score > 0.0);

        let (worst_id, worst_score) = s.worst_relationship().unwrap();
        assert_eq!(worst_id, 2);
        assert!(worst_score < 0.0);
    }

    #[test]
    fn best_worst_on_empty_returns_none() {
        let s = Social::default();
        assert!(s.best_relationship().is_none());
        assert!(s.worst_relationship().is_none());
    }

    #[test]
    fn get_unknown_entity_returns_zero() {
        let s = Social::default();
        assert_eq!(s.get_relationship(999), 0.0);
    }

    #[test]
    fn cleanup_dead_removes_stale_entries() {
        let mut s = Social::default();
        s.record_interaction(1, 0.5, None);
        s.record_interaction(2, -0.3, None);
        s.record_interaction(3, 0.8, None);

        let alive: HashSet<u64> = [1, 3].into_iter().collect();
        s.cleanup_dead(&alive);

        assert!(s.relationships.contains_key(&1));
        assert!(!s.relationships.contains_key(&2));
        assert!(s.relationships.contains_key(&3));
    }

    #[test]
    fn valence_is_clamped() {
        let mut s = Social::default();
        s.record_interaction(1, 5.0, None);
        assert!(s.get_relationship(1) <= 1.0);

        s.record_interaction(2, -5.0, None);
        assert!(s.get_relationship(2) >= -1.0);
    }

    #[test]
    fn score_stays_in_bounds_after_many_interactions() {
        let mut s = Social::default();
        for tick in 0..100 {
            s.record_interaction(1, 1.0, Some(tick));
        }
        let score = s.get_relationship(1);
        assert!(score >= -1.0 && score <= 1.0, "score should stay in [-1, 1], got {}", score);
    }

    #[test]
    fn last_positive_contact_tracks_max_tick() {
        let mut s = Social::default();
        s.record_interaction(1, 1.0, Some(100));
        s.record_interaction(2, 1.0, Some(50));
        // Should keep the max (100), not overwrite with 50
        assert_eq!(s.last_positive_contact_tick, 100);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut s = Social::default();
        s.record_interaction(1, 0.7, Some(42));
        s.record_interaction(2, -0.3, None);

        let json = serde_json::to_string(&s).unwrap();
        let d: Social = serde_json::from_str(&json).unwrap();

        assert_eq!(d.last_positive_contact_tick, s.last_positive_contact_tick);
        assert_eq!(d.relationships.len(), s.relationships.len());
        assert!((d.get_relationship(1) - s.get_relationship(1)).abs() < 1e-10);
        assert!((d.get_relationship(2) - s.get_relationship(2)).abs() < 1e-10);
    }
}

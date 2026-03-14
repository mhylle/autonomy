use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Component: tribe membership for an entity.
///
/// `None` means the entity belongs to no tribe.
/// A `Some(id)` links the entity to a `Tribe` stored in `SimulationWorld.tribes`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TribeId(pub Option<u64>);

/// A tribe is a persistent social group detected from mutual positive relationships.
///
/// Tribes are stored in `SimulationWorld.tribes` and referenced by `TribeId` components
/// on member entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tribe {
    /// Unique identifier for this tribe.
    pub id: u64,
    /// Entity IDs (from `Entity::to_bits().get()`) of current members.
    pub member_ids: HashSet<u64>,
    /// X coordinate of the territory centroid (average of member positions).
    pub territory_centroid_x: f64,
    /// Y coordinate of the territory centroid (average of member positions).
    pub territory_centroid_y: f64,
    /// Tick when this tribe was first formed.
    pub founding_tick: u64,
}

impl Tribe {
    /// Create a new tribe with the given founding members.
    pub fn new(id: u64, member_ids: HashSet<u64>, centroid_x: f64, centroid_y: f64, founding_tick: u64) -> Self {
        Self {
            id,
            member_ids,
            territory_centroid_x: centroid_x,
            territory_centroid_y: centroid_y,
            founding_tick,
        }
    }

    /// Whether an entity is a member of this tribe.
    pub fn contains(&self, entity_id: u64) -> bool {
        self.member_ids.contains(&entity_id)
    }

    /// Number of members in this tribe.
    pub fn size(&self) -> usize {
        self.member_ids.len()
    }
}

/// Minimum number of mutually positive entities required to form a tribe.
pub const MIN_TRIBE_SIZE: usize = 3;

/// Minimum relationship score between all pairs for tribe formation.
pub const TRIBE_RELATIONSHIP_THRESHOLD: f64 = 0.3;

/// Maximum distance between entities for them to form a tribe together.
pub const TRIBE_FORMATION_RANGE: f64 = 80.0;

/// Below this member count, a tribe dissolves.
pub const MIN_TRIBE_SURVIVAL_SIZE: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tribe_id_default_is_none() {
        let tid = TribeId::default();
        assert!(tid.0.is_none());
    }

    #[test]
    fn tribe_id_can_be_set() {
        let tid = TribeId(Some(42));
        assert_eq!(tid.0, Some(42));
    }

    #[test]
    fn tribe_new_sets_fields() {
        let members: HashSet<u64> = [1, 2, 3].into_iter().collect();
        let tribe = Tribe::new(100, members.clone(), 50.0, 60.0, 10);
        assert_eq!(tribe.id, 100);
        assert_eq!(tribe.member_ids, members);
        assert_eq!(tribe.territory_centroid_x, 50.0);
        assert_eq!(tribe.territory_centroid_y, 60.0);
        assert_eq!(tribe.founding_tick, 10);
    }

    #[test]
    fn tribe_contains_member() {
        let members: HashSet<u64> = [1, 2, 3].into_iter().collect();
        let tribe = Tribe::new(100, members, 0.0, 0.0, 0);
        assert!(tribe.contains(1));
        assert!(tribe.contains(2));
        assert!(tribe.contains(3));
        assert!(!tribe.contains(4));
    }

    #[test]
    fn tribe_size() {
        let members: HashSet<u64> = [10, 20, 30, 40].into_iter().collect();
        let tribe = Tribe::new(1, members, 0.0, 0.0, 0);
        assert_eq!(tribe.size(), 4);
    }

    #[test]
    fn tribe_serialization_roundtrip() {
        let members: HashSet<u64> = [1, 2, 3].into_iter().collect();
        let tribe = Tribe::new(42, members, 10.0, 20.0, 5);
        let json = serde_json::to_string(&tribe).unwrap();
        let restored: Tribe = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, 42);
        assert_eq!(restored.size(), 3);
        assert!(restored.contains(1));
        assert!(restored.contains(2));
        assert!(restored.contains(3));
        assert_eq!(restored.territory_centroid_x, 10.0);
        assert_eq!(restored.territory_centroid_y, 20.0);
        assert_eq!(restored.founding_tick, 5);
    }

    #[test]
    fn tribe_id_serialization_roundtrip() {
        let tid = TribeId(Some(99));
        let json = serde_json::to_string(&tid).unwrap();
        let restored: TribeId = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.0, Some(99));

        let tid_none = TribeId(None);
        let json2 = serde_json::to_string(&tid_none).unwrap();
        let restored2: TribeId = serde_json::from_str(&json2).unwrap();
        assert!(restored2.0.is_none());
    }

    #[test]
    fn tribe_empty_membership() {
        let tribe = Tribe::new(1, HashSet::new(), 0.0, 0.0, 0);
        assert_eq!(tribe.size(), 0);
        assert!(!tribe.contains(1));
    }
}

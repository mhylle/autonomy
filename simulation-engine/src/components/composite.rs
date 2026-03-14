use serde::{Deserialize, Serialize};

/// Role a member cell plays within a composite organism.
///
/// Assigned based on the member's genome traits when it joins the composite.
/// Determines which aggregate stats the member contributes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellRole {
    /// Contributes to composite movement speed.
    Locomotion,
    /// Extends composite sensor range.
    Sensing,
    /// Increases composite attack force.
    Attack,
    /// Increases composite health/damage resistance.
    Defense,
    /// Improves composite feeding efficiency (energy gain multiplier).
    Digestion,
    /// Improves composite reproduction capability.
    Reproduction,
    /// No specialization; contributes a small amount to everything.
    Undifferentiated,
}

/// Information about a single member within a composite organism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeMember {
    /// The entity ID bits of this member.
    pub entity_id: u64,
    /// The role this member plays.
    pub role: CellRole,
}

/// Component attached to the **composite leader** entity.
///
/// Tracks all member entities by ID. Members retain their own
/// Genome, Memory, and Identity but their movement/decision systems
/// are suppressed while attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeBody {
    /// Member entities and their assigned roles.
    pub members: Vec<CompositeMember>,
    /// Entity ID bits of the leader (the entity holding this component).
    pub leader_id: u64,
    /// Tick when the composite was formed.
    pub formed_tick: u64,
}

impl CompositeBody {
    /// Create a new composite body.
    pub fn new(leader_id: u64, formed_tick: u64) -> Self {
        Self {
            members: Vec::new(),
            leader_id,
            formed_tick,
        }
    }

    /// Number of members (excluding the leader).
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Total size of the composite (leader + members).
    pub fn total_size(&self) -> usize {
        1 + self.members.len()
    }

    /// Add a member with the given role.
    pub fn add_member(&mut self, entity_id: u64, role: CellRole) {
        self.members.push(CompositeMember { entity_id, role });
    }

    /// Remove a member by entity ID, returning the removed member if found.
    pub fn remove_member(&mut self, entity_id: u64) -> Option<CompositeMember> {
        if let Some(idx) = self.members.iter().position(|m| m.entity_id == entity_id) {
            Some(self.members.remove(idx))
        } else {
            None
        }
    }

    /// Check if a specific entity is a member.
    pub fn has_member(&self, entity_id: u64) -> bool {
        self.members.iter().any(|m| m.entity_id == entity_id)
    }
}

/// Marker component attached to entities that are members of a composite.
///
/// While this component is present, the entity's movement and decision
/// systems are suppressed -- the composite leader acts on their behalf.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeMemberMarker {
    /// Entity ID bits of the composite leader this member belongs to.
    pub leader_id: u64,
}

/// Aggregate capabilities computed from composite members.
///
/// Attached to composite leader entities and recalculated each tick.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateStats {
    /// Total speed contribution from Locomotion members.
    pub speed: f64,
    /// Maximum sensor range from Sensing members.
    pub sensor_range: f64,
    /// Total attack force from Attack members.
    pub attack_force: f64,
    /// Total defense bonus from Defense members.
    pub defense_bonus: f64,
    /// Feeding efficiency multiplier from Digestion members.
    pub feeding_efficiency: f64,
    /// Reproduction bonus from Reproduction members.
    pub reproduction_bonus: f64,
    /// Total number of members (excluding leader).
    pub member_count: usize,
}

/// Assign a CellRole based on genome traits.
///
/// The trait with the highest normalized score determines the role:
/// - max_speed -> Locomotion
/// - sensor_range -> Sensing
/// - size (large) -> Defense
/// - aggression drive weight -> Attack
/// - metabolism_rate (low = efficient) -> Digestion
/// - reproductive drive weight -> Reproduction
pub fn assign_role(
    max_speed: f64,
    sensor_range: f64,
    size: f64,
    aggression: f64,
    metabolism_rate: f64,
    reproductive: f64,
) -> CellRole {
    let scores = [
        (CellRole::Locomotion, max_speed / 4.0),
        (CellRole::Sensing, sensor_range / 100.0),
        (CellRole::Defense, size / 10.0),
        (CellRole::Attack, aggression),
        (CellRole::Digestion, 1.0 - metabolism_rate.min(1.0)),
        (CellRole::Reproduction, reproductive),
    ];

    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(role, _)| *role)
        .unwrap_or(CellRole::Undifferentiated)
}

/// Assign a role from a genome's traits (convenience wrapper).
pub fn assign_role_from_genome(genome: &super::genome::Genome) -> CellRole {
    assign_role(
        genome.max_speed,
        genome.sensor_range,
        genome.size,
        genome.drive_weights.base_aggression,
        genome.metabolism_rate,
        genome.drive_weights.base_reproductive,
    )
}

/// Compute aggregate stats from member roles and their trait values.
///
/// `role_stats` contains `(entity_id, role, trait_value)` where `trait_value`
/// is the relevant trait for that role (speed for Locomotion, range for Sensing, etc.).
pub fn compute_aggregate_stats(
    members: &[CompositeMember],
    role_stats: &[(u64, CellRole, f64)],
) -> AggregateStats {
    let mut stats = AggregateStats {
        member_count: members.len(),
        feeding_efficiency: 1.0,
        ..Default::default()
    };

    for (_entity_id, role, value) in role_stats {
        match role {
            CellRole::Locomotion => stats.speed += value,
            CellRole::Sensing => {
                if *value > stats.sensor_range {
                    stats.sensor_range = *value;
                }
            }
            CellRole::Attack => stats.attack_force += value,
            CellRole::Defense => stats.defense_bonus += value,
            CellRole::Digestion => stats.feeding_efficiency += value * 0.5,
            CellRole::Reproduction => stats.reproduction_bonus += value,
            CellRole::Undifferentiated => {
                stats.speed += value * 0.2;
                stats.defense_bonus += value * 0.2;
            }
        }
    }

    stats
}

/// Describes the composition pattern of a composite organism.
///
/// This is the "genome" of the composite's structure: it records
/// how many members of each role the composite should ideally have.
/// Offspring composites inherit (with mutation) this pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionPattern {
    /// Target count for Sensing role members.
    pub sensing_count: u8,
    /// Target count for Locomotion role members.
    pub locomotion_count: u8,
    /// Target count for Attack role members.
    pub attack_count: u8,
    /// Target count for Defense role members.
    pub defense_count: u8,
    /// Target count for Digestion role members.
    pub digestion_count: u8,
    /// Target count for Reproduction role members.
    pub reproduction_count: u8,
}

impl Default for CompositionPattern {
    fn default() -> Self {
        Self {
            sensing_count: 1,
            locomotion_count: 1,
            attack_count: 1,
            defense_count: 1,
            digestion_count: 0,
            reproduction_count: 0,
        }
    }
}

impl CompositionPattern {
    /// Total number of member slots (excluding the leader).
    pub fn total_member_slots(&self) -> u8 {
        self.sensing_count
            + self.locomotion_count
            + self.attack_count
            + self.defense_count
            + self.digestion_count
            + self.reproduction_count
    }

    /// Returns the roles as a flat list.
    pub fn member_roles(&self) -> Vec<CellRole> {
        let mut roles = Vec::new();
        for _ in 0..self.sensing_count {
            roles.push(CellRole::Sensing);
        }
        for _ in 0..self.locomotion_count {
            roles.push(CellRole::Locomotion);
        }
        for _ in 0..self.attack_count {
            roles.push(CellRole::Attack);
        }
        for _ in 0..self.defense_count {
            roles.push(CellRole::Defense);
        }
        for _ in 0..self.digestion_count {
            roles.push(CellRole::Digestion);
        }
        for _ in 0..self.reproduction_count {
            roles.push(CellRole::Reproduction);
        }
        roles
    }

    /// Build a pattern from a list of members by counting roles.
    pub fn from_members(members: &[CompositeMember]) -> Self {
        let mut pattern = CompositionPattern {
            sensing_count: 0,
            locomotion_count: 0,
            attack_count: 0,
            defense_count: 0,
            digestion_count: 0,
            reproduction_count: 0,
        };
        for member in members {
            match member.role {
                CellRole::Sensing => pattern.sensing_count += 1,
                CellRole::Locomotion => pattern.locomotion_count += 1,
                CellRole::Attack => pattern.attack_count += 1,
                CellRole::Defense => pattern.defense_count += 1,
                CellRole::Digestion => pattern.digestion_count += 1,
                CellRole::Reproduction => pattern.reproduction_count += 1,
                CellRole::Undifferentiated => {} // not counted in pattern
            }
        }
        pattern
    }
}

/// Energy fraction below which a composite fully decomposes.
pub const DECOMPOSITION_ENERGY_THRESHOLD: f64 = 0.15;

/// Energy fraction below which the weakest member is shed (partial decomposition).
pub const PARTIAL_DECOMPOSITION_THRESHOLD: f64 = 0.3;

/// Minimum composition_affinity for two entities to merge.
pub const MIN_COMPOSITION_AFFINITY: f64 = 0.3;

/// Maximum distance for composition attempts.
pub const COMPOSITION_RANGE: f64 = 15.0;

/// Maximum number of members in a composite (excluding leader).
pub const MAX_COMPOSITE_SIZE: usize = 7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::genome::Genome;

    #[test]
    fn composite_body_new() {
        let body = CompositeBody::new(42, 100);
        assert_eq!(body.leader_id, 42);
        assert_eq!(body.formed_tick, 100);
        assert_eq!(body.member_count(), 0);
        assert_eq!(body.total_size(), 1);
    }

    #[test]
    fn add_and_remove_members() {
        let mut body = CompositeBody::new(1, 0);
        body.add_member(10, CellRole::Locomotion);
        body.add_member(20, CellRole::Sensing);
        assert_eq!(body.member_count(), 2);
        assert_eq!(body.total_size(), 3);
        assert!(body.has_member(10));
        assert!(body.has_member(20));
        assert!(!body.has_member(30));

        let removed = body.remove_member(10);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().entity_id, 10);
        assert_eq!(body.member_count(), 1);
        assert!(!body.has_member(10));
    }

    #[test]
    fn remove_nonexistent_member() {
        let mut body = CompositeBody::new(1, 0);
        body.add_member(10, CellRole::Locomotion);
        let removed = body.remove_member(99);
        assert!(removed.is_none());
        assert_eq!(body.member_count(), 1);
    }

    #[test]
    fn assign_role_locomotion() {
        let role = assign_role(4.0, 10.0, 2.0, 0.1, 0.5, 0.1);
        assert_eq!(role, CellRole::Locomotion);
    }

    #[test]
    fn assign_role_sensing() {
        let role = assign_role(1.0, 100.0, 2.0, 0.1, 0.5, 0.1);
        assert_eq!(role, CellRole::Sensing);
    }

    #[test]
    fn assign_role_defense() {
        let role = assign_role(1.0, 10.0, 10.0, 0.1, 0.5, 0.1);
        assert_eq!(role, CellRole::Defense);
    }

    #[test]
    fn assign_role_attack() {
        let role = assign_role(1.0, 10.0, 2.0, 0.95, 0.5, 0.1);
        assert_eq!(role, CellRole::Attack);
    }

    #[test]
    fn assign_role_digestion() {
        // Low metabolism -> high digestion score (1.0 - 0.01 = 0.99)
        let role = assign_role(1.0, 10.0, 2.0, 0.1, 0.01, 0.1);
        assert_eq!(role, CellRole::Digestion);
    }

    #[test]
    fn assign_role_reproduction() {
        let role = assign_role(1.0, 10.0, 2.0, 0.1, 0.5, 0.95);
        assert_eq!(role, CellRole::Reproduction);
    }

    #[test]
    fn assign_role_from_genome_works() {
        let genome = Genome {
            max_speed: 20.0, // very fast
            sensor_range: 10.0,
            size: 1.0,
            ..Genome::default()
        };
        assert_eq!(assign_role_from_genome(&genome), CellRole::Locomotion);
    }

    #[test]
    fn compute_aggregate_stats_empty() {
        let stats = compute_aggregate_stats(&[], &[]);
        assert_eq!(stats.member_count, 0);
        assert_eq!(stats.speed, 0.0);
        assert_eq!(stats.sensor_range, 0.0);
    }

    #[test]
    fn compute_aggregate_stats_speed_sums() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Locomotion },
            CompositeMember { entity_id: 2, role: CellRole::Locomotion },
        ];
        let role_stats = vec![
            (1, CellRole::Locomotion, 2.0),
            (2, CellRole::Locomotion, 3.0),
        ];
        let stats = compute_aggregate_stats(&members, &role_stats);
        assert_eq!(stats.speed, 5.0);
        assert_eq!(stats.member_count, 2);
    }

    #[test]
    fn compute_aggregate_stats_sensor_takes_max() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Sensing },
            CompositeMember { entity_id: 2, role: CellRole::Sensing },
        ];
        let role_stats = vec![
            (1, CellRole::Sensing, 50.0),
            (2, CellRole::Sensing, 80.0),
        ];
        let stats = compute_aggregate_stats(&members, &role_stats);
        assert_eq!(stats.sensor_range, 80.0);
    }

    #[test]
    fn compute_aggregate_stats_mixed_roles() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Locomotion },
            CompositeMember { entity_id: 2, role: CellRole::Sensing },
            CompositeMember { entity_id: 3, role: CellRole::Attack },
            CompositeMember { entity_id: 4, role: CellRole::Defense },
        ];
        let role_stats = vec![
            (1, CellRole::Locomotion, 3.0),
            (2, CellRole::Sensing, 60.0),
            (3, CellRole::Attack, 0.8),
            (4, CellRole::Defense, 7.0),
        ];
        let stats = compute_aggregate_stats(&members, &role_stats);
        assert_eq!(stats.speed, 3.0);
        assert_eq!(stats.sensor_range, 60.0);
        assert!((stats.attack_force - 0.8).abs() < f64::EPSILON);
        assert_eq!(stats.defense_bonus, 7.0);
        assert_eq!(stats.member_count, 4);
    }

    #[test]
    fn composite_member_marker_stores_leader() {
        let marker = CompositeMemberMarker { leader_id: 99 };
        assert_eq!(marker.leader_id, 99);
    }

    #[test]
    fn cell_role_serialization_roundtrip() {
        let roles = vec![
            CellRole::Locomotion,
            CellRole::Sensing,
            CellRole::Attack,
            CellRole::Defense,
            CellRole::Digestion,
            CellRole::Reproduction,
            CellRole::Undifferentiated,
        ];
        for role in &roles {
            let json = serde_json::to_string(role).unwrap();
            let deserialized: CellRole = serde_json::from_str(&json).unwrap();
            assert_eq!(*role, deserialized);
        }
    }

    #[test]
    fn composite_body_serialization_roundtrip() {
        let mut body = CompositeBody::new(1, 42);
        body.add_member(10, CellRole::Locomotion);
        body.add_member(20, CellRole::Sensing);

        let json = serde_json::to_string(&body).unwrap();
        let restored: CompositeBody = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.leader_id, 1);
        assert_eq!(restored.formed_tick, 42);
        assert_eq!(restored.member_count(), 2);
        assert_eq!(restored.members[0].role, CellRole::Locomotion);
        assert_eq!(restored.members[1].role, CellRole::Sensing);
    }

    #[test]
    fn undifferentiated_role_contributes_partially() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Undifferentiated },
        ];
        let role_stats = vec![
            (1, CellRole::Undifferentiated, 5.0),
        ];
        let stats = compute_aggregate_stats(&members, &role_stats);
        assert!((stats.speed - 1.0).abs() < f64::EPSILON); // 5.0 * 0.2
        assert!((stats.defense_bonus - 1.0).abs() < f64::EPSILON); // 5.0 * 0.2
    }

    #[test]
    fn digestion_role_boosts_feeding_efficiency() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Digestion },
        ];
        let role_stats = vec![
            (1, CellRole::Digestion, 0.8),
        ];
        let stats = compute_aggregate_stats(&members, &role_stats);
        // base 1.0 + 0.8 * 0.5 = 1.4
        assert!((stats.feeding_efficiency - 1.4).abs() < f64::EPSILON);
    }

    #[test]
    fn composition_pattern_default() {
        let p = CompositionPattern::default();
        assert_eq!(p.sensing_count, 1);
        assert_eq!(p.locomotion_count, 1);
        assert_eq!(p.attack_count, 1);
        assert_eq!(p.defense_count, 1);
        assert_eq!(p.digestion_count, 0);
        assert_eq!(p.reproduction_count, 0);
        assert_eq!(p.total_member_slots(), 4);
    }

    #[test]
    fn composition_pattern_member_roles() {
        let p = CompositionPattern {
            sensing_count: 2,
            locomotion_count: 1,
            attack_count: 0,
            defense_count: 1,
            digestion_count: 0,
            reproduction_count: 1,
        };
        let roles = p.member_roles();
        assert_eq!(roles.len(), 5);
        assert_eq!(roles.iter().filter(|r| **r == CellRole::Sensing).count(), 2);
        assert_eq!(roles.iter().filter(|r| **r == CellRole::Locomotion).count(), 1);
        assert_eq!(roles.iter().filter(|r| **r == CellRole::Defense).count(), 1);
        assert_eq!(roles.iter().filter(|r| **r == CellRole::Reproduction).count(), 1);
    }

    #[test]
    fn composition_pattern_from_members() {
        let members = vec![
            CompositeMember { entity_id: 1, role: CellRole::Sensing },
            CompositeMember { entity_id: 2, role: CellRole::Sensing },
            CompositeMember { entity_id: 3, role: CellRole::Attack },
            CompositeMember { entity_id: 4, role: CellRole::Locomotion },
            CompositeMember { entity_id: 5, role: CellRole::Undifferentiated },
        ];
        let pattern = CompositionPattern::from_members(&members);
        assert_eq!(pattern.sensing_count, 2);
        assert_eq!(pattern.attack_count, 1);
        assert_eq!(pattern.locomotion_count, 1);
        assert_eq!(pattern.defense_count, 0);
        // Undifferentiated is not counted in pattern
        assert_eq!(pattern.total_member_slots(), 4);
    }

    #[test]
    fn composition_pattern_serialization_roundtrip() {
        let p = CompositionPattern {
            sensing_count: 3,
            locomotion_count: 2,
            attack_count: 1,
            defense_count: 4,
            digestion_count: 0,
            reproduction_count: 1,
        };
        let json = serde_json::to_string(&p).unwrap();
        let restored: CompositionPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sensing_count, 3);
        assert_eq!(restored.locomotion_count, 2);
        assert_eq!(restored.attack_count, 1);
        assert_eq!(restored.defense_count, 4);
        assert_eq!(restored.digestion_count, 0);
        assert_eq!(restored.reproduction_count, 1);
    }
}

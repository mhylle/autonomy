//! Emergent hierarchy detection.
//!
//! Detects leadership and role specialization within tribes by analyzing
//! social connection patterns and entity behavior.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Detected leader of a tribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderInfo {
    /// Entity ID of the leader.
    pub entity_id: u64,
    /// Tribe ID the leader belongs to.
    pub tribe_id: u64,
    /// Number of positive social connections.
    pub connection_count: usize,
    /// Average relationship score with tribe members.
    pub avg_relationship: f64,
}

/// Role specialization detected from entity behavior patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DetectedRole {
    /// Entity frequently gathers food.
    Gatherer,
    /// Entity frequently fights or defends.
    Warrior,
    /// Entity frequently builds structures.
    Builder,
    /// Entity moves between locations (potential trader).
    Explorer,
    /// No clear specialization detected.
    Generalist,
}

/// Role assignment for a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub entity_id: u64,
    pub role: DetectedRole,
    /// Confidence score for this role assignment (0.0 to 1.0).
    pub confidence: f64,
}

/// Detect the leader of a tribe based on social connections.
///
/// The leader is the entity with the most positive relationships
/// (score > 0.0) with other tribe members.
pub fn detect_leader(
    tribe_id: u64,
    member_relationships: &[(u64, HashMap<u64, f64>)], // (entity_id, relationships)
) -> Option<LeaderInfo> {
    if member_relationships.is_empty() {
        return None;
    }

    let member_ids: std::collections::HashSet<u64> =
        member_relationships.iter().map(|(id, _)| *id).collect();

    let mut best_entity: Option<u64> = Some(member_relationships[0].0);
    let mut best_count = 0usize;
    let mut best_avg = 0.0f64;

    for (entity_id, relationships) in member_relationships {
        // Count positive connections with tribe members.
        let positive_connections: Vec<f64> = relationships
            .iter()
            .filter(|(target_id, score)| member_ids.contains(target_id) && **score > 0.0)
            .map(|(_, score)| *score)
            .collect();

        let count = positive_connections.len();
        let avg = if count > 0 {
            positive_connections.iter().sum::<f64>() / count as f64
        } else {
            0.0
        };

        if count > best_count || (count == best_count && avg > best_avg) {
            best_entity = Some(*entity_id);
            best_count = count;
            best_avg = avg;
        }
    }

    best_entity.map(|entity_id| LeaderInfo {
        entity_id,
        tribe_id,
        connection_count: best_count,
        avg_relationship: best_avg,
    })
}

/// Detect role specialization from behavior counters.
///
/// Each entity has counters for different activities. The dominant activity
/// determines the detected role.
pub fn detect_role(
    entity_id: u64,
    feed_count: u64,
    attack_count: u64,
    build_count: u64,
    move_distance: f64,
) -> RoleAssignment {
    let total = feed_count + attack_count + build_count;
    if total == 0 {
        return RoleAssignment {
            entity_id,
            role: DetectedRole::Generalist,
            confidence: 0.0,
        };
    }

    // Check if movement-heavy (explorer).
    let movement_threshold = 500.0;
    if move_distance > movement_threshold && total < 10 {
        return RoleAssignment {
            entity_id,
            role: DetectedRole::Explorer,
            confidence: (move_distance / (movement_threshold * 2.0)).min(1.0),
        };
    }

    let max_count = feed_count.max(attack_count).max(build_count);
    let confidence = max_count as f64 / total as f64;

    let role = if max_count == feed_count {
        DetectedRole::Gatherer
    } else if max_count == attack_count {
        DetectedRole::Warrior
    } else {
        DetectedRole::Builder
    };

    RoleAssignment {
        entity_id,
        role,
        confidence,
    }
}

/// Compute role distribution for a tribe.
///
/// Returns the count of entities in each detected role.
pub fn role_distribution(assignments: &[RoleAssignment]) -> HashMap<DetectedRole, usize> {
    let mut dist = HashMap::new();
    for assignment in assignments {
        *dist.entry(assignment.role).or_insert(0) += 1;
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_leader_most_connections() {
        let relationships = vec![
            (1u64, {
                let mut m = HashMap::new();
                m.insert(2, 0.5);
                m.insert(3, 0.3);
                m.insert(4, 0.6);
                m
            }),
            (2, {
                let mut m = HashMap::new();
                m.insert(1, 0.4);
                m
            }),
            (3, {
                let mut m = HashMap::new();
                m.insert(1, 0.3);
                m.insert(2, 0.2);
                m
            }),
            (4, {
                let mut m = HashMap::new();
                m.insert(1, 0.5);
                m
            }),
        ];

        let leader = detect_leader(1, &relationships).unwrap();
        assert_eq!(leader.entity_id, 1); // Entity 1 has 3 positive connections
        assert_eq!(leader.connection_count, 3);
        assert!(leader.avg_relationship > 0.0);
    }

    #[test]
    fn detect_leader_empty_tribe() {
        let leader = detect_leader(1, &[]);
        assert!(leader.is_none());
    }

    #[test]
    fn detect_leader_no_positive_connections() {
        let relationships = vec![
            (1u64, {
                let mut m = HashMap::new();
                m.insert(2u64, -0.5);
                m
            }),
            (2, {
                let mut m = HashMap::new();
                m.insert(1u64, -0.3);
                m
            }),
        ];

        let leader = detect_leader(1, &relationships).unwrap();
        assert_eq!(leader.connection_count, 0);
    }

    #[test]
    fn detect_role_gatherer() {
        let role = detect_role(1, 20, 2, 1, 100.0);
        assert_eq!(role.role, DetectedRole::Gatherer);
        assert!(role.confidence > 0.8);
    }

    #[test]
    fn detect_role_warrior() {
        let role = detect_role(1, 3, 25, 1, 50.0);
        assert_eq!(role.role, DetectedRole::Warrior);
    }

    #[test]
    fn detect_role_builder() {
        let role = detect_role(1, 1, 2, 30, 50.0);
        assert_eq!(role.role, DetectedRole::Builder);
    }

    #[test]
    fn detect_role_explorer() {
        let role = detect_role(1, 2, 1, 0, 800.0);
        assert_eq!(role.role, DetectedRole::Explorer);
    }

    #[test]
    fn detect_role_generalist_no_activity() {
        let role = detect_role(1, 0, 0, 0, 0.0);
        assert_eq!(role.role, DetectedRole::Generalist);
        assert_eq!(role.confidence, 0.0);
    }

    #[test]
    fn role_distribution_counts() {
        let assignments = vec![
            RoleAssignment { entity_id: 1, role: DetectedRole::Gatherer, confidence: 0.9 },
            RoleAssignment { entity_id: 2, role: DetectedRole::Gatherer, confidence: 0.7 },
            RoleAssignment { entity_id: 3, role: DetectedRole::Warrior, confidence: 0.8 },
            RoleAssignment { entity_id: 4, role: DetectedRole::Builder, confidence: 0.6 },
        ];
        let dist = role_distribution(&assignments);
        assert_eq!(dist[&DetectedRole::Gatherer], 2);
        assert_eq!(dist[&DetectedRole::Warrior], 1);
        assert_eq!(dist[&DetectedRole::Builder], 1);
    }
}

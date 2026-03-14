//! Settlement detection and tracking.
//!
//! Settlements are detected when a cluster of entities sharing the same
//! TribeId persistently occupy a region. This is an observer system that
//! reads positions, tribe memberships, and structures to identify and
//! update settlements.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Minimum number of tribe members in range to form a settlement.
pub const MIN_SETTLEMENT_POPULATION: usize = 3;

/// Radius around centroid within which entities count toward a settlement.
pub const SETTLEMENT_RADIUS: f64 = 60.0;

/// A detected settlement: a persistent cluster of same-tribe entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    /// Unique settlement identifier.
    pub id: u64,
    /// Display name (auto-generated from tribe + id).
    pub name: String,
    /// Centroid x coordinate.
    pub x: f64,
    /// Centroid y coordinate.
    pub y: f64,
    /// Current population count.
    pub population: usize,
    /// Tribe that owns this settlement.
    pub tribe_id: u64,
    /// IDs of structures within the settlement radius.
    pub structure_ids: Vec<u64>,
    /// Tick when this settlement was first detected.
    pub founding_tick: u64,
    /// Resource production statistics: resource_type -> cumulative count.
    pub production_stats: HashMap<String, u64>,
    /// Defense score computed from nearby structures.
    pub defense_score: f64,
}

impl Settlement {
    /// Create a new settlement.
    pub fn new(
        id: u64,
        tribe_id: u64,
        x: f64,
        y: f64,
        population: usize,
        founding_tick: u64,
    ) -> Self {
        Self {
            id,
            name: format!("Settlement-{}-T{}", id, tribe_id),
            x,
            y,
            population,
            tribe_id,
            structure_ids: Vec::new(),
            founding_tick,
            production_stats: HashMap::new(),
            defense_score: 0.0,
        }
    }

    /// Update the centroid position.
    pub fn update_centroid(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    /// Record production of a resource type.
    pub fn record_production(&mut self, resource_type: &str, count: u64) {
        *self
            .production_stats
            .entry(resource_type.to_string())
            .or_insert(0) += count;
    }

    /// Get the dominant resource type (highest production count).
    pub fn dominant_resource(&self) -> Option<(&str, u64)> {
        self.production_stats
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(name, &count)| (name.as_str(), count))
    }
}

/// Input data for settlement detection: entity positions and tribe membership.
#[derive(Debug, Clone)]
pub struct EntityClusterData {
    pub entity_id: u64,
    pub x: f64,
    pub y: f64,
    pub tribe_id: u64,
}

/// Detect settlements from entity positions grouped by tribe.
///
/// Returns a map of (tribe_id -> Vec<(centroid_x, centroid_y, member_ids)>).
/// Each entry represents a detected cluster that qualifies as a settlement.
pub fn detect_clusters(
    entities: &[EntityClusterData],
) -> HashMap<u64, Vec<(f64, f64, Vec<u64>)>> {
    let mut result: HashMap<u64, Vec<(f64, f64, Vec<u64>)>> = HashMap::new();

    // Group entities by tribe.
    let mut by_tribe: HashMap<u64, Vec<&EntityClusterData>> = HashMap::new();
    for e in entities {
        by_tribe.entry(e.tribe_id).or_default().push(e);
    }

    for (tribe_id, members) in &by_tribe {
        let clusters = cluster_entities(members);
        for cluster in clusters {
            if cluster.len() >= MIN_SETTLEMENT_POPULATION {
                let (cx, cy) = compute_centroid(&cluster);
                let ids: Vec<u64> = cluster.iter().map(|e| e.entity_id).collect();
                result.entry(*tribe_id).or_default().push((cx, cy, ids));
            }
        }
    }

    result
}

/// Simple single-linkage clustering within SETTLEMENT_RADIUS.
fn cluster_entities<'a>(
    entities: &[&'a EntityClusterData],
) -> Vec<Vec<&'a EntityClusterData>> {
    if entities.is_empty() {
        return Vec::new();
    }

    let mut visited = vec![false; entities.len()];
    let mut clusters = Vec::new();

    for i in 0..entities.len() {
        if visited[i] {
            continue;
        }
        let mut cluster = Vec::new();
        let mut stack = vec![i];

        while let Some(idx) = stack.pop() {
            if visited[idx] {
                continue;
            }
            visited[idx] = true;
            cluster.push(entities[idx]);

            for j in 0..entities.len() {
                if !visited[j] {
                    let dx = entities[idx].x - entities[j].x;
                    let dy = entities[idx].y - entities[j].y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq <= SETTLEMENT_RADIUS * SETTLEMENT_RADIUS {
                        stack.push(j);
                    }
                }
            }
        }

        clusters.push(cluster);
    }

    clusters
}

/// Compute the centroid of a cluster.
fn compute_centroid(entities: &[&EntityClusterData]) -> (f64, f64) {
    if entities.is_empty() {
        return (0.0, 0.0);
    }
    let n = entities.len() as f64;
    let sum_x: f64 = entities.iter().map(|e| e.x).sum();
    let sum_y: f64 = entities.iter().map(|e| e.y).sum();
    (sum_x / n, sum_y / n)
}

/// Update existing settlements map with newly detected clusters.
///
/// Matches new clusters to existing settlements by proximity and tribe.
/// Creates new settlements for unmatched clusters and removes settlements
/// that are no longer detected.
pub fn update_settlements(
    settlements: &mut HashMap<u64, Settlement>,
    next_id: &mut u64,
    clusters: &HashMap<u64, Vec<(f64, f64, Vec<u64>)>>,
    current_tick: u64,
) {
    let mut matched_ids: HashSet<u64> = HashSet::new();

    for (tribe_id, tribe_clusters) in clusters {
        for (cx, cy, member_ids) in tribe_clusters {
            // Try to match to an existing settlement (same tribe, within radius).
            let matched = settlements
                .values()
                .find(|s| {
                    s.tribe_id == *tribe_id && {
                        let dx = s.x - cx;
                        let dy = s.y - cy;
                        (dx * dx + dy * dy).sqrt() < SETTLEMENT_RADIUS
                    }
                })
                .map(|s| s.id);

            if let Some(sid) = matched {
                // Update existing settlement.
                if let Some(settlement) = settlements.get_mut(&sid) {
                    settlement.update_centroid(*cx, *cy);
                    settlement.population = member_ids.len();
                }
                matched_ids.insert(sid);
            } else {
                // Create new settlement.
                let id = *next_id;
                *next_id += 1;
                let settlement =
                    Settlement::new(id, *tribe_id, *cx, *cy, member_ids.len(), current_tick);
                settlements.insert(id, settlement);
                matched_ids.insert(id);
            }
        }
    }

    // Remove settlements that were not matched (no longer have enough population).
    settlements.retain(|id, _| matched_ids.contains(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: u64, x: f64, y: f64, tribe_id: u64) -> EntityClusterData {
        EntityClusterData {
            entity_id: id,
            x,
            y,
            tribe_id,
        }
    }

    #[test]
    fn settlement_new_creates_with_defaults() {
        let s = Settlement::new(1, 42, 10.0, 20.0, 5, 100);
        assert_eq!(s.id, 1);
        assert_eq!(s.tribe_id, 42);
        assert_eq!(s.x, 10.0);
        assert_eq!(s.y, 20.0);
        assert_eq!(s.population, 5);
        assert_eq!(s.founding_tick, 100);
        assert!(s.structure_ids.is_empty());
        assert!(s.production_stats.is_empty());
        assert_eq!(s.defense_score, 0.0);
        assert!(s.name.contains("Settlement"));
    }

    #[test]
    fn settlement_record_production() {
        let mut s = Settlement::new(1, 1, 0.0, 0.0, 3, 0);
        s.record_production("food", 10);
        s.record_production("food", 5);
        s.record_production("stone", 3);
        assert_eq!(s.production_stats["food"], 15);
        assert_eq!(s.production_stats["stone"], 3);
    }

    #[test]
    fn settlement_dominant_resource() {
        let mut s = Settlement::new(1, 1, 0.0, 0.0, 3, 0);
        s.record_production("food", 10);
        s.record_production("stone", 20);
        let (name, count) = s.dominant_resource().unwrap();
        assert_eq!(name, "stone");
        assert_eq!(count, 20);
    }

    #[test]
    fn settlement_dominant_resource_empty() {
        let s = Settlement::new(1, 1, 0.0, 0.0, 3, 0);
        assert!(s.dominant_resource().is_none());
    }

    #[test]
    fn detect_clusters_finds_nearby_same_tribe() {
        let entities = vec![
            make_entity(1, 10.0, 10.0, 1),
            make_entity(2, 15.0, 10.0, 1),
            make_entity(3, 12.0, 12.0, 1),
        ];
        let clusters = detect_clusters(&entities);
        assert_eq!(clusters.len(), 1); // one tribe
        let tribe_clusters = &clusters[&1];
        assert_eq!(tribe_clusters.len(), 1); // one cluster
        assert_eq!(tribe_clusters[0].2.len(), 3); // all 3 entities
    }

    #[test]
    fn detect_clusters_separates_tribes() {
        let entities = vec![
            make_entity(1, 10.0, 10.0, 1),
            make_entity(2, 15.0, 10.0, 1),
            make_entity(3, 12.0, 12.0, 1),
            make_entity(4, 10.0, 10.0, 2),
            make_entity(5, 15.0, 10.0, 2),
            make_entity(6, 12.0, 12.0, 2),
        ];
        let clusters = detect_clusters(&entities);
        assert_eq!(clusters.len(), 2); // two tribes
        assert_eq!(clusters[&1][0].2.len(), 3);
        assert_eq!(clusters[&2][0].2.len(), 3);
    }

    #[test]
    fn detect_clusters_rejects_small_groups() {
        let entities = vec![
            make_entity(1, 10.0, 10.0, 1),
            make_entity(2, 15.0, 10.0, 1),
        ];
        let clusters = detect_clusters(&entities);
        // 2 entities < MIN_SETTLEMENT_POPULATION (3), so no clusters
        assert!(clusters.is_empty() || clusters.values().all(|v| v.is_empty()));
    }

    #[test]
    fn detect_clusters_splits_distant_groups() {
        let entities = vec![
            make_entity(1, 10.0, 10.0, 1),
            make_entity(2, 15.0, 10.0, 1),
            make_entity(3, 12.0, 12.0, 1),
            // Far away cluster
            make_entity(4, 500.0, 500.0, 1),
            make_entity(5, 505.0, 500.0, 1),
            make_entity(6, 502.0, 502.0, 1),
        ];
        let clusters = detect_clusters(&entities);
        assert_eq!(clusters[&1].len(), 2); // two separate clusters
    }

    #[test]
    fn update_settlements_creates_new() {
        let mut settlements = HashMap::new();
        let mut next_id = 1u64;
        let mut clusters = HashMap::new();
        clusters.insert(1u64, vec![(10.0, 10.0, vec![1, 2, 3])]);

        update_settlements(&mut settlements, &mut next_id, &clusters, 100);

        assert_eq!(settlements.len(), 1);
        let s = settlements.values().next().unwrap();
        assert_eq!(s.tribe_id, 1);
        assert_eq!(s.population, 3);
        assert_eq!(s.founding_tick, 100);
    }

    #[test]
    fn update_settlements_updates_existing() {
        let mut settlements = HashMap::new();
        let mut next_id = 1u64;

        // Create initial settlement.
        let mut clusters = HashMap::new();
        clusters.insert(1u64, vec![(10.0, 10.0, vec![1, 2, 3])]);
        update_settlements(&mut settlements, &mut next_id, &clusters, 100);

        // Update with moved centroid and new population.
        let mut clusters2 = HashMap::new();
        clusters2.insert(1u64, vec![(15.0, 15.0, vec![1, 2, 3, 4])]);
        update_settlements(&mut settlements, &mut next_id, &clusters2, 200);

        assert_eq!(settlements.len(), 1);
        let s = settlements.values().next().unwrap();
        assert_eq!(s.population, 4);
        assert!((s.x - 15.0).abs() < 0.01);
        // Founding tick should not change.
        assert_eq!(s.founding_tick, 100);
    }

    #[test]
    fn update_settlements_removes_disbanded() {
        let mut settlements = HashMap::new();
        let mut next_id = 1u64;

        // Create settlement.
        let mut clusters = HashMap::new();
        clusters.insert(1u64, vec![(10.0, 10.0, vec![1, 2, 3])]);
        update_settlements(&mut settlements, &mut next_id, &clusters, 100);
        assert_eq!(settlements.len(), 1);

        // No clusters detected -> settlement removed.
        let empty_clusters = HashMap::new();
        update_settlements(&mut settlements, &mut next_id, &empty_clusters, 200);
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn compute_centroid_basic() {
        let entities = vec![
            EntityClusterData { entity_id: 1, x: 0.0, y: 0.0, tribe_id: 1 },
            EntityClusterData { entity_id: 2, x: 10.0, y: 10.0, tribe_id: 1 },
        ];
        let refs: Vec<&EntityClusterData> = entities.iter().collect();
        let (cx, cy) = compute_centroid(&refs);
        assert!((cx - 5.0).abs() < 0.001);
        assert!((cy - 5.0).abs() < 0.001);
    }

    #[test]
    fn settlement_update_centroid() {
        let mut s = Settlement::new(1, 1, 10.0, 20.0, 3, 0);
        s.update_centroid(30.0, 40.0);
        assert_eq!(s.x, 30.0);
        assert_eq!(s.y, 40.0);
    }
}

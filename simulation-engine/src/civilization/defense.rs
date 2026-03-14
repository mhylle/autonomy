//! Defense and warfare metrics.
//!
//! Computes defense scores for settlements based on nearby structures
//! and garrison strength (entities that prefer to stay near the settlement).

use serde::{Deserialize, Serialize};

/// Defense bonus per Wall structure within settlement radius.
pub const WALL_DEFENSE_BONUS: f64 = 10.0;

/// Defense bonus per Shelter structure within settlement radius.
pub const SHELTER_DEFENSE_BONUS: f64 = 5.0;

/// Defense bonus per garrison entity (tribe member near settlement).
pub const GARRISON_DEFENSE_PER_ENTITY: f64 = 2.0;

/// Data about a structure for defense computation.
#[derive(Debug, Clone)]
pub struct StructureDefenseData {
    pub x: f64,
    pub y: f64,
    pub structure_type: StructureDefenseType,
    pub durability_ratio: f64,
}

/// Simplified structure type for defense calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureDefenseType {
    Wall,
    Shelter,
    Other,
}

/// Compute the defense score for a settlement.
///
/// Defense is based on:
/// - Number and type of nearby structures (walls > shelters > other)
/// - Durability of those structures
/// - Number of garrison entities (tribe members near the settlement)
pub fn compute_defense_score(
    settlement_x: f64,
    settlement_y: f64,
    settlement_radius: f64,
    structures: &[StructureDefenseData],
    garrison_count: usize,
) -> f64 {
    let mut score = 0.0;

    for structure in structures {
        let dx = structure.x - settlement_x;
        let dy = structure.y - settlement_y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist <= settlement_radius {
            let type_bonus = match structure.structure_type {
                StructureDefenseType::Wall => WALL_DEFENSE_BONUS,
                StructureDefenseType::Shelter => SHELTER_DEFENSE_BONUS,
                StructureDefenseType::Other => 1.0,
            };
            score += type_bonus * structure.durability_ratio;
        }
    }

    score += garrison_count as f64 * GARRISON_DEFENSE_PER_ENTITY;

    score
}

/// Determine how many entities are "garrisoned" -- staying near the settlement.
///
/// An entity counts as garrisoned if it is within the settlement radius
/// and belongs to the same tribe.
pub fn count_garrison(
    settlement_x: f64,
    settlement_y: f64,
    settlement_radius: f64,
    entity_positions: &[(u64, f64, f64)], // (entity_id, x, y) -- already filtered to same tribe
) -> usize {
    entity_positions
        .iter()
        .filter(|(_, ex, ey)| {
            let dx = ex - settlement_x;
            let dy = ey - settlement_y;
            (dx * dx + dy * dy).sqrt() <= settlement_radius
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defense_score_empty() {
        let score = compute_defense_score(0.0, 0.0, 100.0, &[], 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn defense_score_with_wall() {
        let structures = vec![StructureDefenseData {
            x: 10.0,
            y: 10.0,
            structure_type: StructureDefenseType::Wall,
            durability_ratio: 1.0,
        }];
        let score = compute_defense_score(0.0, 0.0, 100.0, &structures, 0);
        assert!((score - WALL_DEFENSE_BONUS).abs() < 0.001);
    }

    #[test]
    fn defense_score_with_damaged_wall() {
        let structures = vec![StructureDefenseData {
            x: 10.0,
            y: 10.0,
            structure_type: StructureDefenseType::Wall,
            durability_ratio: 0.5,
        }];
        let score = compute_defense_score(0.0, 0.0, 100.0, &structures, 0);
        assert!((score - WALL_DEFENSE_BONUS * 0.5).abs() < 0.001);
    }

    #[test]
    fn defense_score_ignores_distant_structures() {
        let structures = vec![StructureDefenseData {
            x: 500.0,
            y: 500.0,
            structure_type: StructureDefenseType::Wall,
            durability_ratio: 1.0,
        }];
        let score = compute_defense_score(0.0, 0.0, 100.0, &structures, 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn defense_score_includes_garrison() {
        let score = compute_defense_score(0.0, 0.0, 100.0, &[], 5);
        assert!((score - 5.0 * GARRISON_DEFENSE_PER_ENTITY).abs() < 0.001);
    }

    #[test]
    fn count_garrison_filters_by_radius() {
        let entities = vec![
            (1, 10.0, 10.0),
            (2, 20.0, 20.0),
            (3, 500.0, 500.0),
        ];
        let count = count_garrison(0.0, 0.0, 50.0, &entities);
        assert_eq!(count, 2);
    }
}

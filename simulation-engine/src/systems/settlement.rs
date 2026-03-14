//! Settlement detection and civilization analysis system.
//!
//! Runs every ANALYSIS_INTERVAL ticks to detect settlements, compute
//! defense scores, detect trade routes, identify leaders, and build
//! cultural identity profiles. All operations are read-only observers
//! that compute metrics from existing simulation state.

use std::collections::HashMap;

use crate::civilization::culture::{
    build_signal_profile, compute_bt_pattern_hash, cultural_distance, CulturalIdentity,
};
use crate::civilization::defense::{
    compute_defense_score, count_garrison, StructureDefenseData, StructureDefenseType,
};
use crate::civilization::hierarchy::{detect_leader, LeaderInfo};
use crate::civilization::settlement::{
    detect_clusters, update_settlements, EntityClusterData, Settlement, SETTLEMENT_RADIUS,
};
use crate::civilization::trade::{
    detect_trade_trips, prune_inactive_routes, EntityMovementRecord, TradeRoute,
};
use crate::components::spatial::Position;
use crate::components::tribe::TribeId;
use crate::core::world::SimulationWorld;

/// How often (in ticks) the civilization analysis runs.
/// Running every tick is expensive; every 100 ticks is sufficient for these
/// slow-changing metrics.
pub const ANALYSIS_INTERVAL: u64 = 100;

/// Maximum signal types for cultural profiling.
const MAX_SIGNAL_TYPES: usize = 16;

/// Max age for trade route pruning (ticks).
const TRADE_ROUTE_MAX_AGE: u64 = 500;

/// Civilization analysis state stored on the world.
///
/// Keeps settlement data, trade routes, cultural identities, and
/// hierarchy information computed by the settlement system.
pub struct CivilizationState {
    /// Detected settlements, keyed by settlement ID.
    pub settlements: HashMap<u64, Settlement>,
    /// Next settlement ID to assign.
    pub next_settlement_id: u64,
    /// Detected trade routes, keyed by sorted settlement ID pair.
    pub trade_routes: HashMap<(u64, u64), TradeRoute>,
    /// Entity movement records for trade detection.
    pub movement_records: HashMap<u64, EntityMovementRecord>,
    /// Cultural identities per tribe.
    pub cultural_identities: HashMap<u64, CulturalIdentity>,
    /// Detected tribe leaders.
    pub leaders: HashMap<u64, LeaderInfo>,
}

impl CivilizationState {
    /// Create a new empty civilization state.
    pub fn new() -> Self {
        Self {
            settlements: HashMap::new(),
            next_settlement_id: 1,
            trade_routes: HashMap::new(),
            movement_records: HashMap::new(),
            cultural_identities: HashMap::new(),
            leaders: HashMap::new(),
        }
    }
}

impl Default for CivilizationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the civilization analysis system.
///
/// This is designed to be called from the tick pipeline. It only runs
/// every ANALYSIS_INTERVAL ticks to keep overhead low.
pub fn run(world: &mut SimulationWorld) {
    if world.tick % ANALYSIS_INTERVAL != 0 {
        return;
    }

    let current_tick = world.tick;

    // 1. Gather entity data for settlement detection.
    let entity_data: Vec<(u64, f64, f64, Option<u64>)> = world
        .ecs
        .query::<(&Position, &TribeId)>()
        .iter()
        .map(|(entity, (pos, tid))| (entity.to_bits().get(), pos.x, pos.y, tid.0))
        .collect();

    // Filter to entities with tribes.
    let cluster_data: Vec<EntityClusterData> = entity_data
        .iter()
        .filter_map(|(id, x, y, tid)| {
            tid.map(|t| EntityClusterData {
                entity_id: *id,
                x: *x,
                y: *y,
                tribe_id: t,
            })
        })
        .collect();

    // 2. Detect settlement clusters.
    let clusters = detect_clusters(&cluster_data);

    // 3. Update settlements.
    let civ = &mut world.civilization;
    update_settlements(
        &mut civ.settlements,
        &mut civ.next_settlement_id,
        &clusters,
        current_tick,
    );

    // 4. Compute defense scores for each settlement.
    let structure_data: Vec<StructureDefenseData> = world
        .structures
        .iter()
        .map(|s| {
            let defense_type = match s.structure_type {
                crate::environment::structures::StructureType::Wall => StructureDefenseType::Wall,
                crate::environment::structures::StructureType::Shelter => {
                    StructureDefenseType::Shelter
                }
                _ => StructureDefenseType::Other,
            };
            StructureDefenseData {
                x: s.x,
                y: s.y,
                structure_type: defense_type,
                durability_ratio: if s.max_durability > 0.0 {
                    s.durability / s.max_durability
                } else {
                    0.0
                },
            }
        })
        .collect();

    for settlement in civ.settlements.values_mut() {
        // Entities of the same tribe.
        let tribe_entities: Vec<(u64, f64, f64)> = entity_data
            .iter()
            .filter(|(_, _, _, tid)| *tid == Some(settlement.tribe_id))
            .map(|(id, x, y, _)| (*id, *x, *y))
            .collect();

        let garrison = count_garrison(
            settlement.x,
            settlement.y,
            SETTLEMENT_RADIUS,
            &tribe_entities,
        );

        settlement.defense_score = compute_defense_score(
            settlement.x,
            settlement.y,
            SETTLEMENT_RADIUS,
            &structure_data,
            garrison,
        );

        // Update structure IDs within settlement radius.
        settlement.structure_ids = world
            .structures
            .iter()
            .filter(|s| {
                let dx = s.x - settlement.x;
                let dy = s.y - settlement.y;
                (dx * dx + dy * dy).sqrt() <= SETTLEMENT_RADIUS
            })
            .map(|s| s.id)
            .collect();
    }

    // 5. Trade route detection.
    // Build settlement position data for trade detection.
    let settlement_positions: Vec<(u64, f64, f64, f64)> = civ
        .settlements
        .values()
        .map(|s| (s.id, s.x, s.y, SETTLEMENT_RADIUS))
        .collect();

    // Check if entities are carrying items.
    let entity_positions: Vec<(u64, f64, f64, bool)> = entity_data
        .iter()
        .map(|(id, x, y, _)| {
            let carrying = world.objects.iter().any(|o| o.held_by == Some(*id));
            (*id, *x, *y, carrying)
        })
        .collect();

    detect_trade_trips(
        &mut civ.movement_records,
        &entity_positions,
        &settlement_positions,
        &mut civ.trade_routes,
        current_tick,
    );

    prune_inactive_routes(&mut civ.trade_routes, current_tick, TRADE_ROUTE_MAX_AGE);

    // 6. Hierarchy detection.
    civ.leaders.clear();
    for (tribe_id, tribe) in &world.tribes {
        let member_relationships: Vec<(u64, HashMap<u64, f64>)> = tribe
            .member_ids
            .iter()
            .filter_map(|&member_id| {
                let bits = std::num::NonZeroU64::new(member_id)?;
                let entity = hecs::Entity::from_bits(bits.get())?;
                world
                    .ecs
                    .get::<&crate::components::social::Social>(entity)
                    .ok()
                    .map(|social| (member_id, social.relationships.clone()))
            })
            .collect();

        if let Some(leader) = detect_leader(*tribe_id, &member_relationships) {
            civ.leaders.insert(*tribe_id, leader);
        }
    }

    // 7. Cultural identity computation.
    civ.cultural_identities.clear();
    for (tribe_id, tribe) in &world.tribes {
        let mut identity = CulturalIdentity::new(*tribe_id);

        // Aggregate signal usage from the world's signals emitted by tribe members.
        for signal in &world.signals {
            if tribe.member_ids.contains(&signal.emitter_id) {
                *identity
                    .signal_usage
                    .entry(signal.signal_type)
                    .or_insert(0) += 1;
            }
        }

        // Build signal profile.
        identity.signal_profile =
            build_signal_profile(&identity.signal_usage, MAX_SIGNAL_TYPES);

        // Compute BT pattern hash from tribe members.
        // For efficiency, we just hash the tribe ID + member count as a proxy.
        // A full implementation would inspect actual BT node types.
        let bt_counts: Vec<HashMap<String, u64>> = tribe
            .member_ids
            .iter()
            .map(|_| {
                // Placeholder: in a full implementation, we'd count BT node types
                let mut m = HashMap::new();
                m.insert(format!("tribe_{}", tribe_id), 1);
                m
            })
            .collect();
        identity.bt_pattern_hash = compute_bt_pattern_hash(&bt_counts);

        identity.compute_complexity();
        civ.cultural_identities.insert(*tribe_id, identity);
    }
}

/// Get the cultural distance between two tribes.
///
/// Returns None if either tribe has no cultural identity computed.
pub fn get_cultural_distance(
    civ: &CivilizationState,
    tribe_a: u64,
    tribe_b: u64,
) -> Option<f64> {
    let a = civ.cultural_identities.get(&tribe_a)?;
    let b = civ.cultural_identities.get(&tribe_b)?;
    Some(cultural_distance(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    #[test]
    fn run_does_nothing_outside_interval() {
        let mut world = test_world();
        world.tick = 1; // Not a multiple of ANALYSIS_INTERVAL
        run(&mut world);
        assert!(world.civilization.settlements.is_empty());
    }

    #[test]
    fn run_on_empty_world_at_interval() {
        let mut world = test_world();
        world.tick = ANALYSIS_INTERVAL;
        run(&mut world);
        // No entities -> no settlements
        assert!(world.civilization.settlements.is_empty());
    }

    #[test]
    fn civilization_state_default() {
        let state = CivilizationState::default();
        assert!(state.settlements.is_empty());
        assert!(state.trade_routes.is_empty());
        assert!(state.cultural_identities.is_empty());
        assert!(state.leaders.is_empty());
    }

    #[test]
    fn get_cultural_distance_missing_tribe() {
        let state = CivilizationState::new();
        assert!(get_cultural_distance(&state, 1, 2).is_none());
    }

    #[test]
    fn get_cultural_distance_with_identities() {
        let mut state = CivilizationState::new();
        let mut a = CulturalIdentity::new(1);
        a.signal_profile = vec![0.5, 0.3, 0.2];
        let mut b = CulturalIdentity::new(2);
        b.signal_profile = vec![0.5, 0.3, 0.2];
        state.cultural_identities.insert(1, a);
        state.cultural_identities.insert(2, b);

        let dist = get_cultural_distance(&state, 1, 2).unwrap();
        assert!(dist < 0.01); // Same profiles
    }
}

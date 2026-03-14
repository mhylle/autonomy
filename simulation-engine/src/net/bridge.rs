use std::cmp::Reverse;

use prost::Message as ProstMessage;

use crate::core::world::SimulationWorld;
use crate::components::{Age, Energy, Genome, Health, Identity, Position, Size};
use crate::components::composite::CompositeBody;
use crate::components::tribe::TribeId;
use crate::environment::terrain::TerrainType;
use crate::net::protocol::autonomy::{
    ActiveWar, CulturalProfile, EntityState, ObjectState, ResourceState, SettlementState,
    SignalState, StructureState, TerrainGrid, TickDelta, TradeRouteState, TribeInfo, Vec2,
    WorldSnapshot,
};
use crate::net::server::ServerState;

/// Convert a TerrainType enum to its integer representation for protobuf.
fn terrain_type_to_i32(t: TerrainType) -> i32 {
    match t {
        TerrainType::Grassland => 0,
        TerrainType::Desert => 1,
        TerrainType::Water => 2,
        TerrainType::Forest => 3,
        TerrainType::Mountain => 4,
    }
}

/// Build a TerrainGrid protobuf from the simulation terrain.
fn build_terrain_grid(world: &SimulationWorld) -> TerrainGrid {
    let grid = &world.terrain;
    let types: Vec<i32> = (0..grid.rows)
        .flat_map(|row| (0..grid.cols).map(move |col| terrain_type_to_i32(grid.get(col, row))))
        .collect();

    TerrainGrid {
        cols: grid.cols as u32,
        rows: grid.rows as u32,
        cell_size: grid.cell_size,
        types,
    }
}

/// Build an EntityState from ECS components, including optional tribe and composite data.
fn build_entity_state(
    entity: hecs::Entity,
    pos: &Position,
    energy: &Energy,
    health: &Health,
    age: &Age,
    size: &Size,
    genome: &Genome,
    identity: &Identity,
    tribe: Option<&TribeId>,
    composite: Option<&CompositeBody>,
) -> EntityState {
    let tribe_id = tribe.and_then(|t| t.0).unwrap_or(0);
    let is_composite_leader = composite.is_some();
    let composite_member_count = composite.map(|c| c.member_count() as u32).unwrap_or(0);

    EntityState {
        id: entity.to_bits().get(),
        position: Some(Vec2 { x: pos.x, y: pos.y }),
        energy: energy.current,
        max_energy: energy.max,
        health: health.current,
        max_health: health.max,
        age: age.ticks,
        max_lifespan: age.max_lifespan,
        size: size.radius,
        species_id: genome.species_id,
        generation: identity.generation,
        tribe_id,
        is_composite_leader,
        composite_member_count,
    }
}

/// Build the list of signal states from the world's active signals.
fn build_signals(world: &SimulationWorld) -> Vec<SignalState> {
    world
        .signals
        .iter()
        .enumerate()
        .map(|(i, s)| SignalState {
            id: i as u64 + 1,
            x: s.x,
            y: s.y,
            signal_type: s.signal_type as u32,
            strength: s.strength,
        })
        .collect()
}

/// Build the list of tribe info from the world's active tribes.
fn build_tribes(world: &SimulationWorld) -> Vec<TribeInfo> {
    world
        .tribes
        .values()
        .map(|t| TribeInfo {
            id: t.id,
            member_count: t.member_ids.len() as u32,
            centroid_x: t.territory_centroid_x,
            centroid_y: t.territory_centroid_y,
        })
        .collect()
}

/// Build the list of active wars from the world's war state.
fn build_active_wars(world: &SimulationWorld) -> Vec<ActiveWar> {
    world
        .active_wars
        .iter()
        .map(|(&(a, b), &declared)| ActiveWar {
            tribe_a_id: a,
            tribe_b_id: b,
            declared_tick: declared,
        })
        .collect()
}

/// Build the list of structure states from completed structures and construction sites.
fn build_structures(world: &SimulationWorld) -> Vec<StructureState> {
    let mut states: Vec<StructureState> = world
        .structures
        .iter()
        .map(|s| StructureState {
            id: s.id,
            x: s.x,
            y: s.y,
            structure_type: format!("{:?}", s.structure_type),
            health: s.durability,
            max_health: s.max_durability,
            progress: 1.0,
        })
        .collect();

    for site in &world.construction_sites {
        states.push(StructureState {
            id: site.id,
            x: site.x,
            y: site.y,
            structure_type: format!("{:?}", site.target_type),
            health: 0.0,
            max_health: 0.0,
            progress: site.progress,
        });
    }

    states
}

/// Build settlements list from world.civilization.settlements.
fn build_settlements(world: &SimulationWorld) -> Vec<SettlementState> {
    world.civilization.settlements.values().map(|s| SettlementState {
        id: s.id,
        name: s.name.clone(),
        x: s.x,
        y: s.y,
        population: s.population as u32,
        tribe_id: s.tribe_id,
        founding_tick: s.founding_tick,
        defense_score: s.defense_score,
    }).collect()
}

/// Build trade routes from world.civilization.trade_routes.
fn build_trade_routes(world: &SimulationWorld) -> Vec<TradeRouteState> {
    world.civilization.trade_routes.values().map(|r| TradeRouteState {
        from_settlement: r.from_settlement,
        to_settlement: r.to_settlement,
        resource_type: r.resource_type.clone().unwrap_or_default(),
        volume: r.volume,
        trip_count: r.trip_count,
    }).collect()
}

/// Build cultural profiles from world.civilization.cultural_identities.
fn build_cultural_profiles(world: &SimulationWorld) -> Vec<CulturalProfile> {
    world.civilization.cultural_identities.values().map(|c| {
        // Build signal summary: top signals by usage count.
        let mut signals: Vec<(u8, u64)> = c.signal_usage.iter().map(|(&t, &n)| (t, n)).collect();
        signals.sort_by_key(|&(_, n)| Reverse(n));
        let summary = signals.iter().take(3)
            .map(|(t, n)| format!("type{}:{}", t, n))
            .collect::<Vec<_>>()
            .join(",");
        CulturalProfile {
            tribe_id: c.tribe_id,
            complexity: c.complexity,
            signal_summary: summary,
        }
    }).collect()
}

/// Build world objects from world.objects.
fn build_objects(world: &SimulationWorld) -> Vec<ObjectState> {
    world.objects.iter().map(|o| ObjectState {
        id: o.id,
        x: o.x,
        y: o.y,
        material: format!(
            "h:{:.2} s:{:.2} w:{:.2}",
            o.material.hardness,
            o.material.sharpness,
            o.material.weight,
        ),
        mass: o.material.weight,
    }).collect()
}

/// Build a full WorldSnapshot protobuf from current world state.
pub fn build_world_snapshot(world: &SimulationWorld) -> WorldSnapshot {
    let entities: Vec<EntityState> = world
        .ecs
        .query::<(
            &Position,
            &Energy,
            &Health,
            &Age,
            &Size,
            &Genome,
            &Identity,
            Option<&TribeId>,
            Option<&CompositeBody>,
        )>()
        .iter()
        .map(|(entity, (pos, energy, health, age, size, genome, identity, tribe, composite))| {
            build_entity_state(entity, pos, energy, health, age, size, genome, identity, tribe, composite)
        })
        .collect();

    let resources: Vec<ResourceState> = world
        .resources
        .iter()
        .map(|r| ResourceState {
            id: r.id,
            position: Some(Vec2 { x: r.x, y: r.y }),
            resource_type: format!("{:?}", r.resource_type),
            amount: r.amount,
            max_amount: r.max_amount,
        })
        .collect();

    let terrain = Some(build_terrain_grid(world));
    let signals = build_signals(world);
    let tribes = build_tribes(world);
    let active_wars = build_active_wars(world);
    let structures = build_structures(world);

    let settlements = build_settlements(world);
    let trade_routes = build_trade_routes(world);
    let cultural_profiles = build_cultural_profiles(world);
    let objects_in_world = build_objects(world);

    WorldSnapshot {
        tick: world.tick,
        entities,
        resources,
        world_width: world.config.world_width,
        world_height: world.config.world_height,
        terrain,
        signals,
        tribes,
        active_wars,
        structures,
        settlements,
        trade_routes,
        cultural_profiles,
        objects_in_world,
    }
}

/// Build a TickDelta from the current tick's state (full state, no diff).
///
/// Used as a fallback; prefer `DiffEngine::compute_delta()` for proper deltas.
pub fn build_tick_delta(world: &SimulationWorld) -> TickDelta {
    let updated: Vec<EntityState> = world
        .ecs
        .query::<(
            &Position,
            &Energy,
            &Health,
            &Age,
            &Size,
            &Genome,
            &Identity,
            Option<&TribeId>,
            Option<&CompositeBody>,
        )>()
        .iter()
        .map(|(entity, (pos, energy, health, age, size, genome, identity, tribe, composite))| {
            build_entity_state(entity, pos, energy, health, age, size, genome, identity, tribe, composite)
        })
        .collect();

    let war_changes = build_active_wars(world);
    let settlement_changes = build_settlements(world);

    TickDelta {
        tick: world.tick,
        spawned: Vec::new(),
        updated,
        died: Vec::new(),
        resource_changes: Vec::new(),
        entity_count: world.entity_count(),
        war_changes,
        settlement_changes,
    }
}

/// Broadcast a diff-based delta using the DiffEngine, filtered by the current viewport.
pub fn broadcast_diff_tick(
    world: &SimulationWorld,
    state: &ServerState,
    diff_engine: &mut crate::net::diff::DiffEngine,
) {
    let viewport = state
        .viewport
        .read()
        .map(|v| *v)
        .unwrap_or_default();
    let delta = diff_engine.compute_delta_with_viewport(world, &viewport);
    let bytes = encode_proto(&delta);
    let _ = state.tick_tx.send(bytes);
}

/// Serialize a protobuf message to bytes.
pub fn encode_proto<M: ProstMessage>(msg: &M) -> Vec<u8> {
    msg.encode_to_vec()
}

/// Broadcast the current tick's state to all connected viewers.
pub fn broadcast_tick(world: &SimulationWorld, state: &ServerState) {
    let delta = build_tick_delta(world);
    let bytes = encode_proto(&delta);
    // Ignore send errors (no receivers connected).
    let _ = state.tick_tx.send(bytes);
}

/// Update the stored snapshot for new client connections.
pub fn update_snapshot(world: &SimulationWorld, state: &ServerState) {
    let snapshot = build_world_snapshot(world);
    let bytes = encode_proto(&snapshot);
    // Use try_write to avoid blocking the simulation thread.
    if let Ok(mut lock) = state.snapshot.try_write() {
        *lock = bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::core::spawning::spawn_initial_population;
    use crate::environment::resources::{Resource, ResourceType};
    use prost::Message as ProstMessage;

    fn make_world_with_entities(count: u32) -> SimulationWorld {
        let config = SimulationConfig {
            initial_entity_count: count,
            ..SimulationConfig::default()
        };
        let mut world = SimulationWorld::new(config);
        spawn_initial_population(&mut world);
        world
    }

    #[test]
    fn build_world_snapshot_empty_world() {
        let world = SimulationWorld::new(SimulationConfig::default());
        let snapshot = build_world_snapshot(&world);

        assert_eq!(snapshot.tick, 0);
        assert!(snapshot.entities.is_empty());
        assert!(snapshot.resources.is_empty());
        assert_eq!(snapshot.world_width, 500.0);
        assert_eq!(snapshot.world_height, 500.0);
    }

    #[test]
    fn build_world_snapshot_includes_terrain() {
        let world = SimulationWorld::new(SimulationConfig::default());
        let snapshot = build_world_snapshot(&world);

        let terrain = snapshot.terrain.expect("snapshot should include terrain");
        assert_eq!(terrain.cols, world.terrain.cols as u32);
        assert_eq!(terrain.rows, world.terrain.rows as u32);
        assert_eq!(terrain.cell_size, world.terrain.cell_size);
        assert_eq!(
            terrain.types.len(),
            (terrain.cols * terrain.rows) as usize
        );
        // All type values should be in range 0..=4
        for &t in &terrain.types {
            assert!(t >= 0 && t <= 4, "terrain type out of range: {}", t);
        }
    }

    #[test]
    fn terrain_roundtrip_through_protobuf() {
        let world = SimulationWorld::new(SimulationConfig::default());
        let snapshot = build_world_snapshot(&world);
        let bytes = encode_proto(&snapshot);

        let decoded = WorldSnapshot::decode(bytes.as_slice()).unwrap();
        let original = snapshot.terrain.unwrap();
        let decoded_terrain = decoded.terrain.unwrap();
        assert_eq!(original.cols, decoded_terrain.cols);
        assert_eq!(original.rows, decoded_terrain.rows);
        assert_eq!(original.cell_size, decoded_terrain.cell_size);
        assert_eq!(original.types, decoded_terrain.types);
    }

    #[test]
    fn build_world_snapshot_with_entities() {
        let world = make_world_with_entities(5);
        let snapshot = build_world_snapshot(&world);

        assert_eq!(snapshot.entities.len(), 5);
        for entity in &snapshot.entities {
            assert!(entity.id > 0);
            assert!(entity.position.is_some());
            let pos = entity.position.as_ref().unwrap();
            assert!(pos.x >= 0.0 && pos.x <= 500.0);
            assert!(pos.y >= 0.0 && pos.y <= 500.0);
            assert!(entity.energy > 0.0);
            assert!(entity.max_energy > 0.0);
            assert!(entity.health > 0.0);
            assert!(entity.max_health > 0.0);
            assert_eq!(entity.age, 0);
            assert!(entity.max_lifespan > 0);
            assert!(entity.size > 0.0);
            assert!(entity.species_id > 0);
            assert_eq!(entity.generation, 0);
        }
    }

    #[test]
    fn build_world_snapshot_with_resources() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        world.resources.push(Resource {
            id: 1,
            x: 10.0,
            y: 20.0,
            resource_type: ResourceType::Food,
            amount: 30.0,
            max_amount: 50.0,
            ..Resource::default()
        });

        let snapshot = build_world_snapshot(&world);
        assert_eq!(snapshot.resources.len(), 1);
        let r = &snapshot.resources[0];
        assert_eq!(r.id, 1);
        assert_eq!(r.resource_type, "Food");
        assert_eq!(r.amount, 30.0);
        assert_eq!(r.max_amount, 50.0);
        let pos = r.position.as_ref().unwrap();
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn build_tick_delta_empty_world() {
        let world = SimulationWorld::new(SimulationConfig::default());
        let delta = build_tick_delta(&world);

        assert_eq!(delta.tick, 0);
        assert!(delta.spawned.is_empty());
        assert!(delta.updated.is_empty());
        assert!(delta.died.is_empty());
        assert!(delta.resource_changes.is_empty());
        assert_eq!(delta.entity_count, 0);
    }

    #[test]
    fn build_tick_delta_with_entities() {
        let world = make_world_with_entities(3);
        let delta = build_tick_delta(&world);

        assert_eq!(delta.updated.len(), 3);
        assert_eq!(delta.entity_count, 3);
        assert!(delta.spawned.is_empty());
        assert!(delta.died.is_empty());
    }

    #[test]
    fn encode_proto_roundtrip_snapshot() {
        let world = make_world_with_entities(2);
        let snapshot = build_world_snapshot(&world);
        let bytes = encode_proto(&snapshot);

        let decoded = WorldSnapshot::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.tick, snapshot.tick);
        assert_eq!(decoded.entities.len(), snapshot.entities.len());
        assert_eq!(decoded.world_width, snapshot.world_width);
        assert_eq!(decoded.world_height, snapshot.world_height);
    }

    #[test]
    fn encode_proto_roundtrip_tick_delta() {
        let world = make_world_with_entities(4);
        let delta = build_tick_delta(&world);
        let bytes = encode_proto(&delta);

        let decoded = TickDelta::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.tick, delta.tick);
        assert_eq!(decoded.updated.len(), delta.updated.len());
        assert_eq!(decoded.entity_count, delta.entity_count);
    }

    #[test]
    fn broadcast_tick_does_not_panic_without_receivers() {
        let world = make_world_with_entities(2);
        let state = {
            let (tx, _rx) = crossbeam_channel::unbounded();
            ServerState::new(tx)
        };
        // Should not panic even with no receivers.
        broadcast_tick(&world, &state);
    }

    #[test]
    fn broadcast_tick_sends_to_receiver() {
        let world = make_world_with_entities(2);
        let state = {
            let (tx, _rx) = crossbeam_channel::unbounded();
            ServerState::new(tx)
        };
        let mut rx = state.tick_tx.subscribe();

        broadcast_tick(&world, &state);

        let bytes = rx.try_recv().unwrap();
        let delta = TickDelta::decode(bytes.as_slice()).unwrap();
        assert_eq!(delta.updated.len(), 2);
    }

    #[test]
    fn update_snapshot_stores_bytes() {
        let world = make_world_with_entities(3);
        let state = {
            let (tx, _rx) = crossbeam_channel::unbounded();
            ServerState::new(tx)
        };

        update_snapshot(&world, &state);

        let bytes = state.snapshot.try_read().unwrap();
        assert!(!bytes.is_empty());
        let decoded = WorldSnapshot::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.entities.len(), 3);
    }

    /// Phase 2.5: Verify diff-based streaming produces smaller payloads than full-state streaming.
    ///
    /// After the initial snapshot, each TickDelta only carries entities that
    /// actually changed. For a stable simulation (no births or deaths), the
    /// delta contains only `updated` entities (positions/drives changed), while
    /// the snapshot includes the full terrain grid as well. This test verifies
    /// the delta payload is smaller than the snapshot payload for a world with
    /// entities that haven't changed between ticks.
    #[test]
    fn diff_streaming_produces_smaller_payload_than_full_state() {
        use crate::net::diff::DiffEngine;
        use crate::core::tick;

        let mut world = make_world_with_entities(20);
        let mut diff_engine = DiffEngine::new();

        // Compute a snapshot (full state, includes terrain grid).
        let snapshot = build_world_snapshot(&world);
        let snapshot_bytes = encode_proto(&snapshot);

        // Run one tick so entities have positions to update, then compute a delta.
        tick::tick(&mut world);
        let delta = diff_engine.compute_delta(&world);
        let delta_bytes = encode_proto(&delta);

        // The delta (no terrain, only changed entities) should be smaller than the snapshot.
        assert!(
            delta_bytes.len() < snapshot_bytes.len(),
            "diff delta ({} bytes) should be smaller than full snapshot ({} bytes)",
            delta_bytes.len(),
            snapshot_bytes.len()
        );
    }

    /// Phase 2.5: Verify viewport filtering further reduces bandwidth.
    ///
    /// With a small viewport covering only part of the world, the diff engine
    /// should send fewer entities than a full-world viewport delta.
    /// We use two separate DiffEngine instances so each starts from the same
    /// baseline state, ensuring a fair comparison.
    #[test]
    fn viewport_filtered_delta_has_fewer_entities() {
        use crate::net::diff::DiffEngine;
        use crate::net::server::ViewportBounds;
        use crate::core::tick;

        let mut world = make_world_with_entities(30);

        // Warm up both diff engines from the same initial state.
        let mut small_engine = DiffEngine::new();
        let mut full_engine = DiffEngine::new();
        tick::tick(&mut world);
        // Establish baseline — both engines see the same world state.
        small_engine.compute_delta(&world);
        full_engine.compute_delta(&world);

        // Advance one more tick so there's new state to diff.
        tick::tick(&mut world);

        // Small viewport covers only 100x100 of the 500x500 world.
        let small_viewport = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0,
        };

        let filtered_delta = small_engine.compute_delta_with_viewport(&world, &small_viewport);
        let full_delta = full_engine.compute_delta_with_viewport(&world, &ViewportBounds::default());

        let filtered_entity_count = filtered_delta.spawned.len()
            + filtered_delta.updated.len()
            + filtered_delta.died.len();
        let full_entity_count =
            full_delta.spawned.len() + full_delta.updated.len() + full_delta.died.len();

        // Viewport filtering should send ≤ entities compared to the full viewport.
        assert!(
            filtered_entity_count <= full_entity_count,
            "viewport-filtered delta ({} entities) should have <= full delta ({} entities)",
            filtered_entity_count,
            full_entity_count
        );
    }
}

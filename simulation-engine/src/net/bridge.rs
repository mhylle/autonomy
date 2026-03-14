use prost::Message as ProstMessage;

use crate::core::world::SimulationWorld;
use crate::components::{Age, Energy, Genome, Health, Identity, Position, Size};
use crate::environment::terrain::TerrainType;
use crate::net::protocol::autonomy::{
    EntityState, ResourceState, TerrainGrid, TickDelta, Vec2, WorldSnapshot,
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

/// Build a full WorldSnapshot protobuf from current world state.
pub fn build_world_snapshot(world: &SimulationWorld) -> WorldSnapshot {
    let entities: Vec<EntityState> = world
        .ecs
        .query::<(&Position, &Energy, &Health, &Age, &Size, &Genome, &Identity)>()
        .iter()
        .map(
            |(entity, (pos, energy, health, age, size, genome, identity))| EntityState {
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
            },
        )
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

    WorldSnapshot {
        tick: world.tick,
        entities,
        resources,
        world_width: world.config.world_width,
        world_height: world.config.world_height,
        terrain,
    }
}

/// Build a TickDelta from the current tick's state (full state, no diff).
///
/// Used as a fallback; prefer `DiffEngine::compute_delta()` for proper deltas.
pub fn build_tick_delta(world: &SimulationWorld) -> TickDelta {
    let updated: Vec<EntityState> = world
        .ecs
        .query::<(&Position, &Energy, &Health, &Age, &Size, &Genome, &Identity)>()
        .iter()
        .map(
            |(entity, (pos, energy, health, age, size, genome, identity))| EntityState {
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
            },
        )
        .collect();

    TickDelta {
        tick: world.tick,
        spawned: Vec::new(),
        updated,
        died: Vec::new(),
        resource_changes: Vec::new(),
        entity_count: world.entity_count(),
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
}

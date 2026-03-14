//! Snapshot serialization and replay.
//!
//! Captures the full simulation state (ECS entities with all components,
//! resources, terrain, climate, config, tick, species history) into a
//! compact binary format (bincode + zstd compression) and writes it to
//! disk. Snapshots can be loaded to reconstruct a `SimulationWorld` and
//! replay the simulation forward to any target tick.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::components::action::Action;
use crate::components::behavior_tree::BtNode;
use crate::components::drives::Drives;
use crate::components::genome::Genome;
use crate::components::identity::Identity;
use crate::components::memory::Memory;
use crate::components::perception::Perception;
use crate::components::social::Social;
use crate::components::{Age, Energy, Health, Position, Size, Velocity};
use crate::environment::climate::Climate;
use crate::environment::resources::Resource;
use crate::environment::spatial_index::SpatialIndex;
use crate::environment::terrain::TerrainGrid;
use crate::events::EventLog;

use super::config::SimulationConfig;
use super::rng::SimulationRng;
use super::world::SimulationWorld;

/// All components for a single entity, packed for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntity {
    pub position: Position,
    pub velocity: Velocity,
    pub energy: Energy,
    pub health: Health,
    pub age: Age,
    pub size: Size,
    pub genome: Genome,
    pub identity: Identity,
    pub perception: Perception,
    pub drives: Drives,
    pub social: Social,
    pub memory: Memory,
    pub bt: BtNode,
    pub action: Action,
}

/// Complete serializable world state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableWorld {
    pub config: SimulationConfig,
    pub tick: u64,
    pub entities: Vec<SerializableEntity>,
    pub resources: Vec<Resource>,
    pub terrain: TerrainGrid,
    pub climate: Climate,
    pub species_history: Vec<(u64, HashMap<u64, u32>)>,
    pub paused: bool,
    pub speed_multiplier: f64,
}

/// Extract all entities from the hecs world into serializable form.
fn extract_entities(world: &SimulationWorld) -> Vec<SerializableEntity> {
    let mut entities = Vec::new();
    for (_entity, (pos, vel, energy, health, age, size, genome, identity, perception, drives, social, memory, bt, action)) in
        world.ecs.query::<(
            &Position,
            &Velocity,
            &Energy,
            &Health,
            &Age,
            &Size,
            &Genome,
            &Identity,
            &Perception,
            &Drives,
            &Social,
            &Memory,
            &BtNode,
            &Action,
        )>().iter()
    {
        entities.push(SerializableEntity {
            position: pos.clone(),
            velocity: vel.clone(),
            energy: energy.clone(),
            health: health.clone(),
            age: age.clone(),
            size: size.clone(),
            genome: genome.clone(),
            identity: identity.clone(),
            perception: perception.clone(),
            drives: drives.clone(),
            social: social.clone(),
            memory: memory.clone(),
            bt: bt.clone(),
            action: action.clone(),
        });
    }
    entities
}

/// Capture the full world state into a `SerializableWorld`.
pub fn capture(world: &SimulationWorld) -> SerializableWorld {
    SerializableWorld {
        config: world.config.clone(),
        tick: world.tick,
        entities: extract_entities(world),
        resources: world.resources.clone(),
        terrain: world.terrain.clone(),
        climate: world.climate.clone(),
        species_history: world.species_history.clone(),
        paused: world.paused,
        speed_multiplier: world.speed_multiplier,
    }
}

/// Reconstruct a `SimulationWorld` from a `SerializableWorld`.
pub fn restore(snap: SerializableWorld) -> SimulationWorld {
    let rng = SimulationRng::new(snap.config.seed);
    let spatial_index = SpatialIndex::new(
        snap.config.world_width,
        snap.config.world_height,
        50.0,
    );

    let mut world = SimulationWorld {
        ecs: hecs::World::new(),
        config: snap.config,
        rng,
        tick: snap.tick,
        resources: snap.resources,
        spatial_index,
        terrain: snap.terrain,
        climate: snap.climate,
        event_log: EventLog::new(),
        paused: snap.paused,
        speed_multiplier: snap.speed_multiplier,
        species_history: snap.species_history,
        kill_matrix: std::collections::HashMap::new(),
    };

    for ent in snap.entities {
        world.ecs.spawn((
            ent.position,
            ent.velocity,
            ent.energy,
            ent.health,
            ent.age,
            ent.size,
            ent.genome,
            ent.identity,
            ent.perception,
            ent.drives,
            ent.social,
            ent.memory,
            ent.bt,
            ent.action,
        ));
    }

    world
}

/// Serialize a snapshot to bytes (bincode + zstd compression).
pub fn serialize_snapshot(snap: &SerializableWorld) -> Result<Vec<u8>, SnapshotError> {
    let raw = bincode::serialize(snap)
        .map_err(|e| SnapshotError::Serialize(e.to_string()))?;

    let compressed = zstd::encode_all(raw.as_slice(), 3)
        .map_err(|e| SnapshotError::Compress(e.to_string()))?;

    Ok(compressed)
}

/// Deserialize a snapshot from bytes (zstd decompress + bincode).
pub fn deserialize_snapshot(data: &[u8]) -> Result<SerializableWorld, SnapshotError> {
    let decompressed = zstd::decode_all(data)
        .map_err(|e| SnapshotError::Decompress(e.to_string()))?;

    let snap: SerializableWorld = bincode::deserialize(&decompressed)
        .map_err(|e| SnapshotError::Deserialize(e.to_string()))?;

    Ok(snap)
}

/// Save a snapshot of the world to a file at the given path.
pub fn save_snapshot(world: &SimulationWorld, path: &Path) -> Result<(), SnapshotError> {
    let snap = capture(world);
    let bytes = serialize_snapshot(&snap)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| SnapshotError::Io(e.to_string()))?;
    }

    let mut file = std::fs::File::create(path)
        .map_err(|e| SnapshotError::Io(e.to_string()))?;
    file.write_all(&bytes)
        .map_err(|e| SnapshotError::Io(e.to_string()))?;

    info!(
        path = %path.display(),
        tick = world.tick,
        entities = world.entity_count(),
        bytes = bytes.len(),
        "snapshot saved"
    );

    Ok(())
}

/// Save a snapshot into a directory, using a tick-based filename.
pub fn save_snapshot_to_dir(world: &SimulationWorld, dir: &str) -> Result<(), SnapshotError> {
    let filename = format!("snapshot_tick_{:08}.bin", world.tick);
    let path = Path::new(dir).join(filename);
    save_snapshot(world, &path)
}

/// Load a snapshot from a file and reconstruct the world.
pub fn load_snapshot(path: &Path) -> Result<SimulationWorld, SnapshotError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| SnapshotError::Io(e.to_string()))?;

    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| SnapshotError::Io(e.to_string()))?;

    let snap = deserialize_snapshot(&data)?;
    let tick = snap.tick;
    let entity_count = snap.entities.len();

    let world = restore(snap);

    info!(
        path = %path.display(),
        tick = tick,
        entities = entity_count,
        "snapshot loaded"
    );

    Ok(world)
}

/// Errors that can occur during snapshot operations.
#[derive(Debug, Clone)]
pub enum SnapshotError {
    Serialize(String),
    Deserialize(String),
    Compress(String),
    Decompress(String),
    Io(String),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::Serialize(e) => write!(f, "serialize error: {}", e),
            SnapshotError::Deserialize(e) => write!(f, "deserialize error: {}", e),
            SnapshotError::Compress(e) => write!(f, "compression error: {}", e),
            SnapshotError::Decompress(e) => write!(f, "decompression error: {}", e),
            SnapshotError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::core::tick;

    /// Create a test world with a small population.
    fn test_world_with_entities(count: u32) -> SimulationWorld {
        let config = SimulationConfig {
            initial_entity_count: count,
            world_width: 200.0,
            world_height: 200.0,
            snapshot_interval: 100,
            ..SimulationConfig::default()
        };
        let mut world = SimulationWorld::new(config);
        crate::environment::spawning::scatter_resources(&mut world);
        crate::core::spawning::spawn_initial_population(&mut world);
        world
    }

    #[test]
    fn capture_and_restore_preserves_entity_count() {
        let world = test_world_with_entities(20);
        let original_count = world.entity_count();
        assert!(original_count > 0);

        let snap = capture(&world);
        assert_eq!(snap.entities.len() as u32, original_count);

        let restored = restore(snap);
        assert_eq!(restored.entity_count(), original_count);
    }

    #[test]
    fn capture_and_restore_preserves_tick() {
        let mut world = test_world_with_entities(10);
        for _ in 0..50 {
            tick::tick(&mut world);
        }
        assert_eq!(world.tick, 50);

        let snap = capture(&world);
        let restored = restore(snap);
        assert_eq!(restored.tick, 50);
    }

    #[test]
    fn capture_and_restore_preserves_config() {
        let world = test_world_with_entities(10);
        let snap = capture(&world);
        let restored = restore(snap);
        assert_eq!(restored.config.seed, world.config.seed);
        assert_eq!(restored.config.world_width, world.config.world_width);
        assert_eq!(restored.config.world_height, world.config.world_height);
        assert_eq!(restored.config.initial_entity_count, world.config.initial_entity_count);
    }

    #[test]
    fn capture_and_restore_preserves_resources() {
        let world = test_world_with_entities(5);
        let resource_count = world.resources.len();
        assert!(resource_count > 0);

        let snap = capture(&world);
        let restored = restore(snap);
        assert_eq!(restored.resources.len(), resource_count);

        // Check first resource matches
        if !world.resources.is_empty() {
            assert_eq!(restored.resources[0].x, world.resources[0].x);
            assert_eq!(restored.resources[0].y, world.resources[0].y);
            assert_eq!(restored.resources[0].amount, world.resources[0].amount);
        }
    }

    #[test]
    fn capture_and_restore_preserves_climate() {
        let mut world = test_world_with_entities(5);
        // Run enough ticks to change climate
        for _ in 0..100 {
            tick::tick(&mut world);
        }
        let snap = capture(&world);
        let restored = restore(snap);

        assert_eq!(restored.climate.temperature, world.climate.temperature);
        assert_eq!(restored.climate.season, world.climate.season);
        assert_eq!(restored.climate.drought_active, world.climate.drought_active);
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let world = test_world_with_entities(15);
        let snap = capture(&world);

        let bytes = serialize_snapshot(&snap).expect("serialize should succeed");
        assert!(!bytes.is_empty());

        let restored_snap = deserialize_snapshot(&bytes).expect("deserialize should succeed");
        assert_eq!(restored_snap.tick, snap.tick);
        assert_eq!(restored_snap.entities.len(), snap.entities.len());
        assert_eq!(restored_snap.resources.len(), snap.resources.len());
    }

    #[test]
    fn compression_reduces_size() {
        let world = test_world_with_entities(50);
        let snap = capture(&world);

        let raw = bincode::serialize(&snap).expect("raw serialize");
        let compressed = serialize_snapshot(&snap).expect("compressed serialize");

        // Compressed should be smaller than raw bincode
        assert!(
            compressed.len() < raw.len(),
            "compressed ({}) should be smaller than raw ({})",
            compressed.len(),
            raw.len()
        );
    }

    #[test]
    fn save_and_load_snapshot_file() {
        let world = test_world_with_entities(10);
        let original_count = world.entity_count();
        let original_tick = world.tick;

        let dir = std::env::temp_dir().join("autonomy_test_snapshots");
        let path = dir.join("test_snapshot.bin");

        save_snapshot(&world, &path).expect("save should succeed");
        assert!(path.exists(), "snapshot file should exist");

        let restored = load_snapshot(&path).expect("load should succeed");
        assert_eq!(restored.entity_count(), original_count);
        assert_eq!(restored.tick, original_tick);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_snapshot_to_dir_creates_file() {
        let mut world = test_world_with_entities(5);
        for _ in 0..10 {
            tick::tick(&mut world);
        }
        assert_eq!(world.tick, 10);

        let dir = std::env::temp_dir().join("autonomy_test_snapshots_dir");
        let dir_str = dir.to_string_lossy().to_string();

        save_snapshot_to_dir(&world, &dir_str).expect("save_to_dir should succeed");

        let expected = dir.join("snapshot_tick_00000010.bin");
        assert!(expected.exists(), "expected snapshot file at {:?}", expected);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replay_forward_matches_fresh_run() {
        // Run a simulation for 50 ticks, snapshot at tick 20, then:
        // 1. Continue to tick 50 (reference)
        // 2. Load snapshot at tick 20, replay to tick 50
        // Both should produce the same entity count and tick.
        let mut world = test_world_with_entities(10);
        for _ in 0..20 {
            tick::tick(&mut world);
        }
        assert_eq!(world.tick, 20);

        // Save snapshot at tick 20
        let snap = capture(&world);

        // Continue reference to tick 50
        for _ in 0..30 {
            tick::tick(&mut world);
        }
        assert_eq!(world.tick, 50);
        let reference_count = world.entity_count();

        // Replay from snapshot at tick 20 to tick 50
        let mut replayed = restore(snap);
        assert_eq!(replayed.tick, 20);
        for _ in 0..30 {
            tick::tick(&mut replayed);
        }
        assert_eq!(replayed.tick, 50);

        // Since the simulation is deterministic, entity counts should match
        assert_eq!(
            replayed.entity_count(),
            reference_count,
            "replay should produce same entity count as reference"
        );
    }

    #[test]
    fn entity_component_values_preserved_after_roundtrip() {
        let mut world = test_world_with_entities(5);
        // Run a few ticks so entities have non-default state
        for _ in 0..10 {
            tick::tick(&mut world);
        }

        // Collect positions before snapshot
        let mut original_positions: Vec<(f64, f64)> = Vec::new();
        for (_e, pos) in world.ecs.query::<&Position>().iter() {
            original_positions.push((pos.x, pos.y));
        }
        original_positions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));

        let snap = capture(&world);
        let restored = restore(snap);

        // Collect positions after restore
        let mut restored_positions: Vec<(f64, f64)> = Vec::new();
        for (_e, pos) in restored.ecs.query::<&Position>().iter() {
            restored_positions.push((pos.x, pos.y));
        }
        restored_positions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));

        assert_eq!(
            original_positions.len(),
            restored_positions.len(),
            "position count should match"
        );
        for (orig, rest) in original_positions.iter().zip(restored_positions.iter()) {
            assert!(
                (orig.0 - rest.0).abs() < f64::EPSILON && (orig.1 - rest.1).abs() < f64::EPSILON,
                "positions should match: {:?} vs {:?}",
                orig,
                rest
            );
        }
    }

    #[test]
    fn deserialize_invalid_data_returns_error() {
        let bad_data = vec![0u8, 1, 2, 3, 4, 5];
        let result = deserialize_snapshot(&bad_data);
        assert!(result.is_err(), "invalid data should produce an error");
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = load_snapshot(Path::new("/tmp/nonexistent_snapshot_xyz_42.bin"));
        assert!(result.is_err(), "missing file should produce an error");
    }

    #[test]
    fn snapshot_error_display() {
        let err = SnapshotError::Io("file not found".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("IO error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn empty_world_snapshot_roundtrip() {
        let world = SimulationWorld::new(SimulationConfig::default());
        assert_eq!(world.entity_count(), 0);

        let snap = capture(&world);
        assert!(snap.entities.is_empty());

        let bytes = serialize_snapshot(&snap).expect("serialize empty");
        let restored_snap = deserialize_snapshot(&bytes).expect("deserialize empty");
        assert!(restored_snap.entities.is_empty());

        let restored = restore(restored_snap);
        assert_eq!(restored.entity_count(), 0);
        assert_eq!(restored.tick, 0);
    }
}

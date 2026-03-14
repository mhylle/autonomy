use rayon::prelude::*;

use crate::components::genome::Genome;
use crate::components::perception::{PerceivedEntity, PerceivedResource, PerceivedSignal, Perception};
use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::components::world_object::PerceivedObject;
use crate::core::lod::LodLevel;
use crate::core::world::SimulationWorld;
use crate::environment::signals::SignalManager;

/// Maximum noise factor applied to energy estimates at maximum sensor range.
/// At distance == sensor_range, the estimate can be off by up to 30%.
const MAX_ENERGY_NOISE: f64 = 0.3;

/// Minimum number of perceivers to trigger parallel computation.
/// Below this threshold, serial execution has less overhead.
const PARALLEL_THRESHOLD: usize = 64;

/// Populates each entity's `Perception` component by querying the spatial
/// index for entities and resources within the entity's `sensor_range`.
///
/// Energy estimates for perceived entities have noise proportional to
/// distance: the farther away, the less accurate the estimate.
///
/// Entities with LOD level `Reduced` or `Minimal` skip perception entirely.
/// Uses rayon for parallel perception computation when entity count exceeds
/// the threshold.
pub fn run(world: &mut SimulationWorld) {
    // 1. Collect perceiver data: (entity, x, y, sensor_range, species_id, entity_id_bits).
    let perceivers: Vec<_> = world
        .ecs
        .query::<(&Position, &Genome, &Perception)>()
        .iter()
        .map(|(entity, (pos, genome, _))| {
            let id_bits = entity.to_bits().get();
            (
                entity,
                pos.x,
                pos.y,
                genome.sensor_range,
                genome.species_id,
                id_bits,
            )
        })
        .collect();

    // 2. Build a lookup table for entity energies (id_bits -> (current, max, species_id)).
    let energy_lookup: Vec<(u64, f64, f64, u64)> = world
        .ecs
        .query::<(&Energy, &Genome)>()
        .iter()
        .map(|(e, (energy, genome))| {
            (e.to_bits().get(), energy.current, energy.max, genome.species_id)
        })
        .collect();

    let energy_map: std::collections::HashMap<u64, (f64, f64, u64)> = energy_lookup
        .into_iter()
        .map(|(id, cur, max, sp)| (id, (cur, max, sp)))
        .collect();

    // 3. Compute perception for each perceiver.
    //    - Entities with Reduced or Minimal LOD skip perception.
    //    - Uses parallel computation via rayon when above threshold.
    //
    // The spatial index and resource list are immutable references during
    // perception computation, making this safe to parallelize.

    let spatial_index = &world.spatial_index;
    let resources = &world.resources;
    let signals = &world.signals;
    let objects = &world.objects;
    let lod_assignments = &world.lod_assignments;
    let master_seed = world.rng.master_seed();
    let tick = world.tick;

    // Build the input data for parallel processing.
    let perceiver_inputs: Vec<_> = perceivers
        .iter()
        .map(|(entity, x, y, sensor_range, species_id, id_bits)| {
            (*entity, *x, *y, *sensor_range, *species_id, *id_bits)
        })
        .collect();

    let perception_updates: Vec<_> = if perceiver_inputs.len() >= PARALLEL_THRESHOLD {
        // Parallel perception computation using rayon.
        perceiver_inputs
            .par_iter()
            .map(|&(entity, x, y, sensor_range, species_id, id_bits)| {
                // Check LOD: skip perception for Reduced and Minimal entities.
                let lod = lod_assignments
                    .get(&id_bits)
                    .copied()
                    .unwrap_or(LodLevel::Full);

                if lod == LodLevel::Reduced || lod == LodLevel::Minimal {
                    return (entity, Vec::new(), Vec::new(), Vec::new(), Vec::new());
                }

                compute_perception_for_entity(
                    entity, x, y, sensor_range, species_id, id_bits,
                    spatial_index, resources, signals, objects, &energy_map,
                    master_seed, tick,
                )
            })
            .collect()
    } else {
        // Serial perception computation.
        perceiver_inputs
            .iter()
            .map(|&(entity, x, y, sensor_range, species_id, id_bits)| {
                let lod = lod_assignments
                    .get(&id_bits)
                    .copied()
                    .unwrap_or(LodLevel::Full);

                if lod == LodLevel::Reduced || lod == LodLevel::Minimal {
                    return (entity, Vec::new(), Vec::new(), Vec::new(), Vec::new());
                }

                compute_perception_for_entity(
                    entity, x, y, sensor_range, species_id, id_bits,
                    spatial_index, resources, signals, objects, &energy_map,
                    master_seed, tick,
                )
            })
            .collect()
    };

    // 4. Apply perception updates.
    for (entity, entities, resources, signals, perceived_objs) in perception_updates {
        if let Ok(mut perception) = world.ecs.get::<&mut Perception>(entity) {
            perception.perceived_entities = entities;
            perception.perceived_resources = resources;
            perception.perceived_signals = signals;
            perception.perceived_objects = perceived_objs;
        }
    }
}

/// Compute perception for a single entity. This function is pure (no mutation
/// of shared state) and can be called from parallel contexts.
///
/// Uses per-entity deterministic RNG seeded from (master_seed, tick, entity_id)
/// to ensure determinism regardless of execution order.
fn compute_perception_for_entity(
    entity: hecs::Entity,
    x: f64,
    y: f64,
    sensor_range: f64,
    species_id: u64,
    id_bits: u64,
    spatial_index: &crate::environment::spatial_index::SpatialIndex,
    resources: &[crate::environment::resources::Resource],
    signals: &[crate::environment::signals::Signal],
    objects: &[crate::components::world_object::WorldObject],
    energy_map: &std::collections::HashMap<u64, (f64, f64, u64)>,
    master_seed: u64,
    tick: u64,
) -> (
    hecs::Entity,
    Vec<PerceivedEntity>,
    Vec<PerceivedResource>,
    Vec<PerceivedSignal>,
    Vec<PerceivedObject>,
) {
    // Per-entity deterministic RNG: seed from master_seed + tick + entity_id.
    // This ensures deterministic output regardless of iteration order.
    let entity_seed = master_seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(tick.wrapping_mul(1_442_695_040_888_963_407))
        .wrapping_add(id_bits);
    let mut rng: rand_chacha::ChaCha8Rng = rand::SeedableRng::seed_from_u64(entity_seed);

    // Query nearby entities.
    let nearby_entities = spatial_index.query_entities_in_radius(x, y, sensor_range);

    let mut perceived_entities = Vec::new();
    for (other_id_bits, ex, ey) in &nearby_entities {
        if *other_id_bits == id_bits {
            continue; // Don't perceive self.
        }

        let dx = ex - x;
        let dy = ey - y;
        let distance = (dx * dx + dy * dy).sqrt();

        // Look up energy and add noise proportional to distance.
        let (energy_estimate, is_kin) = if let Some(&(current, _max, other_species)) =
            energy_map.get(other_id_bits)
        {
            let noise_factor = (distance / sensor_range) * MAX_ENERGY_NOISE;
            let noise =
                (rand::Rng::gen::<f64>(&mut rng) * 2.0 - 1.0) * noise_factor * current;
            let estimate = (current + noise).max(0.0);
            let kin = other_species == species_id;
            (estimate, kin)
        } else {
            (0.0, false)
        };

        perceived_entities.push(PerceivedEntity {
            entity_id: *other_id_bits,
            x: *ex,
            y: *ey,
            distance,
            energy_estimate,
            is_kin,
        });
    }

    // Query nearby resources (only available ones).
    let nearby_resources = spatial_index.query_resources_in_radius(x, y, sensor_range);

    let mut perceived_resources = Vec::new();
    for (res_idx, rx, ry) in &nearby_resources {
        if !resources[*res_idx].is_available() {
            continue;
        }
        let dx = rx - x;
        let dy = ry - y;
        let distance = (dx * dx + dy * dy).sqrt();

        perceived_resources.push(PerceivedResource {
            resource_index: *res_idx,
            x: *rx,
            y: *ry,
            distance,
        });
    }

    // Query nearby signals within sensor range.
    let perceived_signals = perceive_signals(signals, x, y, sensor_range, id_bits);

    // Query nearby world objects on the ground within sensor range.
    let perceived_objects = perceive_objects(objects, x, y, sensor_range);

    (entity, perceived_entities, perceived_resources, perceived_signals, perceived_objects)
}

/// Build the list of perceived objects on the ground within sensor range.
fn perceive_objects(
    objects: &[crate::components::world_object::WorldObject],
    px: f64,
    py: f64,
    sensor_range: f64,
) -> Vec<PerceivedObject> {
    objects
        .iter()
        .filter(|obj| obj.is_on_ground() && obj.is_intact())
        .filter_map(|obj| {
            let dx = obj.x - px;
            let dy = obj.y - py;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance <= sensor_range {
                Some(PerceivedObject {
                    object_id: obj.id,
                    x: obj.x,
                    y: obj.y,
                    distance,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Build the list of perceived signals for a perceiver at (px, py) with given sensor_range.
///
/// Excludes signals emitted by the perceiver itself.
fn perceive_signals(
    signals: &[crate::environment::signals::Signal],
    px: f64,
    py: f64,
    sensor_range: f64,
    self_id: u64,
) -> Vec<PerceivedSignal> {
    let nearby = SignalManager::query_at(signals, px, py, sensor_range);
    nearby
        .into_iter()
        .filter(|s| s.emitter_id != self_id)
        .map(|s| {
            let dx = s.x - px;
            let dy = s.y - py;
            let distance = (dx * dx + dy * dy).sqrt();
            let (dir_x, dir_y) = if distance > 0.001 {
                (dx / distance, dy / distance)
            } else {
                (0.0, 0.0)
            };
            let strength = s.strength_at_distance(distance);

            PerceivedSignal {
                signal_type: s.signal_type,
                distance,
                direction_x: dir_x,
                direction_y: dir_y,
                strength,
                source_x: s.x,
                source_y: s.y,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::perception::Perception;
    use crate::core::config::SimulationConfig;
    use crate::environment::resources::Resource;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn spawn_perceiver(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        sensor_range: f64,
    ) -> hecs::Entity {
        let genome = Genome {
            sensor_range,
            ..Genome::default()
        };
        world.ecs.spawn((
            Position { x, y },
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            genome,
            Perception::default(),
        ))
    }

    fn add_entity_to_spatial(world: &mut SimulationWorld, entity: hecs::Entity) {
        let pos = world.ecs.get::<&Position>(entity).unwrap();
        let x = pos.x;
        let y = pos.y;
        world
            .spatial_index
            .insert_entity(entity.to_bits().get(), x, y);
    }

    fn add_resource(world: &mut SimulationWorld, x: f64, y: f64, amount: f64) {
        let idx = world.resources.len();
        world.resources.push(Resource {
            id: idx as u64,
            x,
            y,
            amount,
            max_amount: amount,
            ..Default::default()
        });
        world.spatial_index.insert_resource(idx, x, y);
    }

    #[test]
    fn perceives_nearby_entity() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let _target = spawn_perceiver(&mut world, 70.0, 50.0, 50.0);

        // Add both to spatial index.
        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(
            perception.perceived_entities.len(),
            1,
            "should perceive exactly one other entity"
        );
        assert!(
            (perception.perceived_entities[0].distance - 20.0).abs() < 0.01,
            "distance should be 20.0"
        );
    }

    #[test]
    fn does_not_perceive_self() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        add_entity_to_spatial(&mut world, perceiver);

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert!(
            perception.perceived_entities.is_empty(),
            "should not perceive self"
        );
    }

    #[test]
    fn does_not_perceive_entity_outside_range() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 30.0);
        let _far = spawn_perceiver(&mut world, 150.0, 150.0, 50.0);

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert!(
            perception.perceived_entities.is_empty(),
            "should not perceive entity at distance ~141 with sensor_range=30"
        );
    }

    #[test]
    fn sensor_range_50_only_perceives_within_50() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 50.0);
        let _near = spawn_perceiver(&mut world, 80.0, 50.0, 50.0); // distance 30
        let _far = spawn_perceiver(&mut world, 120.0, 50.0, 50.0); // distance 70

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(
            perception.perceived_entities.len(),
            1,
            "should perceive only the entity within 50 units"
        );
        assert!(
            perception.perceived_entities[0].distance < 50.0,
            "perceived entity should be within range"
        );
    }

    #[test]
    fn perceives_nearby_resources() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 40.0);
        add_entity_to_spatial(&mut world, perceiver);

        add_resource(&mut world, 60.0, 50.0, 30.0); // distance 10
        add_resource(&mut world, 200.0, 200.0, 30.0); // distance ~212

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(
            perception.perceived_resources.len(),
            1,
            "should perceive only the nearby resource"
        );
        assert_eq!(perception.perceived_resources[0].resource_index, 0);
    }

    #[test]
    fn depleted_resources_not_perceived() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        add_entity_to_spatial(&mut world, perceiver);

        // Add a depleted resource.
        let idx = world.resources.len();
        world.resources.push(Resource {
            id: idx as u64,
            x: 55.0,
            y: 50.0,
            amount: 0.0,
            max_amount: 30.0,
            depleted: true,
            ..Default::default()
        });
        world.spatial_index.insert_resource(idx, 55.0, 50.0);

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert!(
            perception.perceived_resources.is_empty(),
            "should not perceive depleted resources"
        );
    }

    #[test]
    fn energy_estimate_has_noise() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let target = spawn_perceiver(&mut world, 90.0, 50.0, 50.0); // distance 40

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        let actual_energy = world.ecs.get::<&Energy>(target).unwrap().current;

        // Run perception many times with different ticks to check noise.
        let mut estimates = Vec::new();
        for tick in 1..=20 {
            world.tick = tick;
            run(&mut world);
            let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
            if let Some(pe) = perception.perceived_entities.first() {
                estimates.push(pe.energy_estimate);
            }
        }

        assert!(
            !estimates.is_empty(),
            "should have collected energy estimates"
        );

        // At least some estimates should differ from actual (noise).
        let exact_matches = estimates
            .iter()
            .filter(|e| (**e - actual_energy).abs() < 0.01)
            .count();
        assert!(
            exact_matches < estimates.len(),
            "some estimates should have noise, but all {} were exact",
            estimates.len()
        );
    }

    #[test]
    fn kin_detection_same_species() {
        let mut world = test_world();
        // Both have default genome -> same species_id.
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let _kin = spawn_perceiver(&mut world, 60.0, 50.0, 50.0);

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(perception.perceived_entities.len(), 1);
        assert!(
            perception.perceived_entities[0].is_kin,
            "entities with same species_id should be kin"
        );
    }

    #[test]
    fn kin_detection_different_species() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);

        // Spawn an entity with a different species_id.
        // Use values that differ enough to escape quantization buckets,
        // and recompute species_id from the new values.
        let mut different_genome = Genome {
            max_energy: 500.0,
            metabolism_rate: 0.5,
            max_speed: 8.0,
            size: 20.0,
            max_lifespan: 10000,
            sensor_range: 50.0,
            ..Genome::default()
        };
        different_genome.species_id = crate::components::genome::compute_species_id(&different_genome);
        let stranger = world.ecs.spawn((
            Position { x: 60.0, y: 50.0 },
            Energy {
                current: 80.0,
                max: 200.0,
                metabolism_rate: 0.1,
            },
            different_genome,
            Perception::default(),
        ));

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        // Verify they actually have different species_ids.
        let sp1 = world.ecs.get::<&Genome>(perceiver).unwrap().species_id;
        let sp2 = world.ecs.get::<&Genome>(stranger).unwrap().species_id;
        assert_ne!(sp1, sp2, "test setup: species should differ");

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(perception.perceived_entities.len(), 1);
        assert!(
            !perception.perceived_entities[0].is_kin,
            "entities with different species_id should not be kin"
        );
    }

    #[test]
    fn perception_is_deterministic() {
        let mut world1 = test_world();
        let p1 = spawn_perceiver(&mut world1, 50.0, 50.0, 100.0);
        let _t1 = spawn_perceiver(&mut world1, 70.0, 50.0, 50.0);
        let entities: Vec<_> = world1.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world1, e);
        }

        let mut world2 = test_world();
        let p2 = spawn_perceiver(&mut world2, 50.0, 50.0, 100.0);
        let _t2 = spawn_perceiver(&mut world2, 70.0, 50.0, 50.0);
        let entities: Vec<_> = world2.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world2, e);
        }

        world1.tick = 5;
        world2.tick = 5;
        run(&mut world1);
        run(&mut world2);

        let perc1 = world1.ecs.get::<&Perception>(p1).unwrap();
        let perc2 = world2.ecs.get::<&Perception>(p2).unwrap();

        assert_eq!(perc1.perceived_entities.len(), perc2.perceived_entities.len());
        for (a, b) in perc1
            .perceived_entities
            .iter()
            .zip(perc2.perceived_entities.iter())
        {
            assert_eq!(a.energy_estimate, b.energy_estimate);
        }
    }

    #[test]
    fn lod_reduced_skips_perception() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let _target = spawn_perceiver(&mut world, 70.0, 50.0, 50.0);

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        // Mark perceiver as Reduced LOD.
        let perceiver_bits = perceiver.to_bits().get();
        world.lod_assignments.insert(perceiver_bits, LodLevel::Reduced);

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert!(
            perception.perceived_entities.is_empty(),
            "Reduced LOD entities should skip perception"
        );
    }

    #[test]
    fn lod_minimal_skips_perception() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let _target = spawn_perceiver(&mut world, 70.0, 50.0, 50.0);

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        // Mark perceiver as Minimal LOD.
        let perceiver_bits = perceiver.to_bits().get();
        world.lod_assignments.insert(perceiver_bits, LodLevel::Minimal);

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert!(
            perception.perceived_entities.is_empty(),
            "Minimal LOD entities should skip perception"
        );
    }

    #[test]
    fn lod_full_runs_perception_normally() {
        let mut world = test_world();
        let perceiver = spawn_perceiver(&mut world, 50.0, 50.0, 100.0);
        let _target = spawn_perceiver(&mut world, 70.0, 50.0, 50.0);

        let entities: Vec<_> = world.ecs.query::<&Position>().iter().map(|(e, _)| e).collect();
        for e in entities {
            add_entity_to_spatial(&mut world, e);
        }

        // Mark perceiver as Full LOD.
        let perceiver_bits = perceiver.to_bits().get();
        world.lod_assignments.insert(perceiver_bits, LodLevel::Full);

        run(&mut world);

        let perception = world.ecs.get::<&Perception>(perceiver).unwrap();
        assert_eq!(
            perception.perceived_entities.len(),
            1,
            "Full LOD entities should perceive normally"
        );
    }
}

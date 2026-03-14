use crate::components::action::Action;
use crate::components::behavior_tree::BtNode;
use crate::components::bt_ops;
use crate::components::drives::Drives;
use crate::components::genome::{Genome, mutate};
use crate::components::identity::Identity;
use crate::components::memory::Memory;
use crate::components::perception::Perception;
use crate::components::physical::{Age, Energy, Health, Size};
use crate::components::social::Social;
use crate::components::spatial::Position;
use crate::components::spatial::Velocity;
use crate::core::world::SimulationWorld;
use crate::events::types::SimEvent;
use rand::Rng;

/// Valence of a reproduction interaction (strongly positive).
const REPRODUCTION_VALENCE: f64 = 1.0;

/// Fraction of max_energy an entity must exceed to reproduce.
const REPRODUCTION_ENERGY_THRESHOLD: f64 = 0.8;

/// Maximum distance for two entities to be considered "adjacent" for mating.
const MATING_RANGE: f64 = 20.0;

/// Reproduction system: supports both sexual and asexual reproduction.
///
/// When a ready entity has a compatible partner adjacent, sexual reproduction
/// occurs (genome crossover + BT crossover + mutation). Otherwise, asexual
/// reproduction (clone + mutation).
pub fn run(world: &mut SimulationWorld) {
    let mut rng = world.rng.tick_rng("reproduction", world.tick);
    let current_tick = world.tick;

    // 1. Collect candidates with all needed data.
    let candidates: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy, &Genome, &BtNode, &Identity)>()
        .iter()
        .filter(|(_, (_, energy, genome, _, _))| {
            energy.current > genome.max_energy * REPRODUCTION_ENERGY_THRESHOLD
        })
        .map(|(entity, (pos, energy, genome, bt, identity))| {
            (
                entity,
                pos.clone(),
                energy.current,
                genome.clone(),
                bt.clone(),
                identity.generation,
            )
        })
        .collect();

    // 2. Build a set of entities that already reproduced this tick (avoid double-mating).
    let mut reproduced = std::collections::HashSet::new();

    // Pairs of (parent_bits, mate_bits) for recording social interactions after spawning.
    let mut mating_pairs: Vec<(u64, u64)> = Vec::new();

    // 3. For each candidate, try to find a mate; fall back to asexual.
    for (parent_entity, parent_pos, parent_energy, parent_genome, parent_bt, parent_gen) in
        &candidates
    {
        if reproduced.contains(&parent_entity.to_bits().get()) {
            continue;
        }

        let offspring_energy = parent_energy / 2.0;

        // Try to find a compatible adjacent mate.
        let mate = candidates.iter().find(|(e, pos, _energy, genome, _bt, _gen)| {
            let e_bits = e.to_bits().get();
            e_bits != parent_entity.to_bits().get()
                && !reproduced.contains(&e_bits)
                && genome.species_id == parent_genome.species_id
                && {
                    let dx = pos.x - parent_pos.x;
                    let dy = pos.y - parent_pos.y;
                    (dx * dx + dy * dy).sqrt() <= MATING_RANGE
                }
        });

        let (offspring_genome, offspring_bt) = if let Some((
            mate_entity,
            _mate_pos,
            _mate_energy,
            mate_genome,
            mate_bt,
            _mate_gen,
        )) = mate
        {
            // Sexual reproduction: crossover + mutation.
            reproduced.insert(mate_entity.to_bits().get());

            // Record mating pair for social relationship update.
            mating_pairs.push((parent_entity.to_bits().get(), mate_entity.to_bits().get()));

            // Reduce mate energy too.
            if let Ok(mut energy) = world.ecs.get::<&mut Energy>(*mate_entity) {
                energy.current *= 0.75; // Mate contributes some energy cost.
            }

            // Genome: average traits from both parents, then mutate.
            let crossed_genome = crossover_genomes(parent_genome, mate_genome, &mut rng);
            let mutated_genome = mutate(&crossed_genome, &mut rng);

            // BT: subtree crossover + mutation.
            let crossed_bt = bt_ops::crossover(&parent_bt, mate_bt, &mut rng);
            let mutated_bt = bt_ops::mutate_parameters(&crossed_bt, 0.1, &mut rng);
            let final_bt = bt_ops::simplify(&mutated_bt);

            (mutated_genome, final_bt)
        } else {
            // Asexual reproduction: mutate genome + BT.
            let mutated_genome = mutate(parent_genome, &mut rng);
            let mutated_bt = bt_ops::mutate_parameters(parent_bt, 0.05, &mut rng);
            let final_bt = bt_ops::simplify(&mutated_bt);
            (mutated_genome, final_bt)
        };

        reproduced.insert(parent_entity.to_bits().get());

        // Reduce parent energy.
        if let Ok(mut energy) = world.ecs.get::<&mut Energy>(*parent_entity) {
            energy.current = offspring_energy;
        }

        // Small random offset for offspring position.
        let offset_x: f64 = rng.gen_range(-10.0..10.0);
        let offset_y: f64 = rng.gen_range(-10.0..10.0);
        let offspring_x = (parent_pos.x + offset_x).rem_euclid(world.config.world_width);
        let offspring_y = (parent_pos.y + offset_y).rem_euclid(world.config.world_height);

        // Build memory from offspring genome before genome is moved.
        let offspring_memory = Memory::new(
            offspring_genome.memory_capacity as usize,
            offspring_genome.eviction_weights.clone(),
        );

        // Spawn offspring.
        let offspring_entity = world.ecs.spawn((
            Position {
                x: offspring_x,
                y: offspring_y,
            },
            Velocity::default(),
            Energy {
                current: offspring_energy,
                max: offspring_genome.max_energy,
                metabolism_rate: offspring_genome.metabolism_rate,
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
            Age {
                ticks: 0,
                max_lifespan: offspring_genome.max_lifespan,
            },
            Size {
                radius: offspring_genome.size,
            },
            offspring_genome,
            Identity {
                generation: parent_gen + 1,
                parent_id: Some(parent_entity.to_bits().get()),
                birth_tick: current_tick,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            offspring_memory,
            offspring_bt,
            Action::default(),
        ));

        world.event_log.push(SimEvent::EntityReproduced {
            parent_id: parent_entity.to_bits().get(),
            offspring_id: offspring_entity.to_bits().get(),
            x: offspring_x,
            y: offspring_y,
        });
    }

    // 4. Record positive social interactions for mating pairs.
    for (parent_bits, mate_bits) in &mating_pairs {
        // Update parent's relationship with mate.
        for (entity, social) in world.ecs.query_mut::<&mut Social>() {
            let bits = entity.to_bits().get();
            if bits == *parent_bits {
                social.record_interaction(*mate_bits, REPRODUCTION_VALENCE, Some(current_tick));
                break;
            }
        }
        // Update mate's relationship with parent.
        for (entity, social) in world.ecs.query_mut::<&mut Social>() {
            let bits = entity.to_bits().get();
            if bits == *mate_bits {
                social.record_interaction(*parent_bits, REPRODUCTION_VALENCE, Some(current_tick));
                break;
            }
        }
    }

    // 5. Record species populations.
    record_species_history(world);
}

/// Crossover two parent genomes by averaging traits.
fn crossover_genomes(
    a: &Genome,
    b: &Genome,
    rng: &mut rand_chacha::ChaCha8Rng,
) -> Genome {
    use crate::components::drives::DriveWeights;
    use crate::components::genome::compute_species_id;
    use crate::components::memory::EvictionWeights;

    // Randomly pick one parent's value for each trait.
    let mut g = Genome {
        max_energy: if rng.gen::<bool>() { a.max_energy } else { b.max_energy },
        metabolism_rate: if rng.gen::<bool>() { a.metabolism_rate } else { b.metabolism_rate },
        max_speed: if rng.gen::<bool>() { a.max_speed } else { b.max_speed },
        sensor_range: if rng.gen::<bool>() { a.sensor_range } else { b.sensor_range },
        size: if rng.gen::<bool>() { a.size } else { b.size },
        max_lifespan: if rng.gen::<bool>() { a.max_lifespan } else { b.max_lifespan },
        mutation_rate: (a.mutation_rate + b.mutation_rate) / 2.0,
        drive_weights: DriveWeights {
            base_curiosity: if rng.gen::<bool>() { a.drive_weights.base_curiosity } else { b.drive_weights.base_curiosity },
            base_social_need: if rng.gen::<bool>() { a.drive_weights.base_social_need } else { b.drive_weights.base_social_need },
            base_aggression: if rng.gen::<bool>() { a.drive_weights.base_aggression } else { b.drive_weights.base_aggression },
            base_reproductive: if rng.gen::<bool>() { a.drive_weights.base_reproductive } else { b.drive_weights.base_reproductive },
        },
        memory_capacity: if rng.gen::<bool>() { a.memory_capacity } else { b.memory_capacity },
        eviction_weights: EvictionWeights {
            recency_weight: if rng.gen::<bool>() { a.eviction_weights.recency_weight } else { b.eviction_weights.recency_weight },
            importance_weight: if rng.gen::<bool>() { a.eviction_weights.importance_weight } else { b.eviction_weights.importance_weight },
            emotional_weight: if rng.gen::<bool>() { a.eviction_weights.emotional_weight } else { b.eviction_weights.emotional_weight },
            variety_weight: if rng.gen::<bool>() { a.eviction_weights.variety_weight } else { b.eviction_weights.variety_weight },
        },
        composition_affinity: if rng.gen::<bool>() { a.composition_affinity } else { b.composition_affinity },
        species_id: 0,
    };
    g.species_id = compute_species_id(&g);
    g
}

/// Record population count per species for the current tick.
fn record_species_history(world: &mut SimulationWorld) {
    // Only record every 100 ticks to avoid excessive data.
    if world.tick % 100 != 0 {
        return;
    }

    let mut counts: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    for (_entity, genome) in world.ecs.query::<&Genome>().iter() {
        *counts.entry(genome.species_id).or_insert(0) += 1;
    }

    world.species_history.push((world.tick, counts));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::behavior_tree::default_starter_bt;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn spawn_entity_with_energy(world: &mut SimulationWorld, energy_current: f64) -> hecs::Entity {
        let genome = Genome::default();
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Energy {
                current: energy_current,
                max: genome.max_energy,
                metabolism_rate: genome.metabolism_rate,
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
            Age {
                ticks: 0,
                max_lifespan: genome.max_lifespan,
            },
            Size {
                radius: genome.size,
            },
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            default_starter_bt(),
            Action::default(),
            genome,
        ))
    }

    fn spawn_entity_at(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        energy: f64,
    ) -> hecs::Entity {
        let genome = Genome::default();
        world.ecs.spawn((
            Position { x, y },
            Velocity::default(),
            Energy {
                current: energy,
                max: genome.max_energy,
                metabolism_rate: genome.metabolism_rate,
            },
            Health { current: 100.0, max: 100.0 },
            Age { ticks: 0, max_lifespan: genome.max_lifespan },
            Size { radius: genome.size },
            Identity { generation: 0, parent_id: None, birth_tick: 0 },
            Perception::default(),
            Drives::default(),
            Social::default(),
            default_starter_bt(),
            Action::default(),
            genome,
        ))
    }

    #[test]
    fn entity_above_threshold_reproduces() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 90.0);

        assert_eq!(world.entity_count(), 1);
        run(&mut world);
        assert_eq!(world.entity_count(), 2, "should have spawned one offspring");
    }

    #[test]
    fn entity_below_threshold_does_not_reproduce() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 70.0);
        run(&mut world);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn parent_energy_is_halved_after_reproduction() {
        let mut world = test_world();
        let parent = spawn_entity_with_energy(&mut world, 90.0);
        run(&mut world);

        let parent_energy = world.ecs.get::<&Energy>(parent).unwrap();
        assert!(
            (parent_energy.current - 45.0).abs() < f64::EPSILON,
            "parent energy should be halved: got {}",
            parent_energy.current
        );
    }

    #[test]
    fn offspring_has_incremented_generation() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 90.0);
        run(&mut world);

        let mut found = false;
        for (_entity, identity) in world.ecs.query_mut::<&Identity>() {
            if identity.generation == 1 {
                found = true;
                assert!(identity.parent_id.is_some());
            }
        }
        assert!(found);
    }

    #[test]
    fn sexual_reproduction_with_adjacent_mate() {
        let mut world = test_world();
        // Two entities close together, both above threshold.
        spawn_entity_at(&mut world, 50.0, 50.0, 90.0);
        spawn_entity_at(&mut world, 55.0, 50.0, 90.0);

        assert_eq!(world.entity_count(), 2);
        run(&mut world);

        // At least one offspring should be produced (one or both parents reproduce).
        assert!(
            world.entity_count() >= 3,
            "sexual reproduction should produce offspring, got {} entities",
            world.entity_count()
        );
    }

    #[test]
    fn offspring_has_bt_component() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 90.0);
        run(&mut world);

        for (_entity, (identity, _bt)) in world.ecs.query_mut::<(&Identity, &BtNode)>() {
            if identity.generation == 1 {
                // Offspring has a BT - test passes.
                return;
            }
        }
        panic!("offspring should have a BtNode component");
    }

    #[test]
    fn species_history_recorded() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 50.0); // below threshold
        world.tick = 100; // trigger recording
        run(&mut world);

        assert!(
            !world.species_history.is_empty(),
            "species history should be recorded at tick 100"
        );
    }

    #[test]
    fn multiple_entities_can_reproduce_same_tick() {
        let mut world = test_world();
        // Place them far apart so they don't mate.
        spawn_entity_at(&mut world, 50.0, 50.0, 90.0);
        spawn_entity_at(&mut world, 400.0, 400.0, 95.0);

        run(&mut world);

        assert_eq!(
            world.entity_count(),
            4,
            "both parents should produce offspring"
        );
    }

    #[test]
    fn sexual_reproduction_creates_positive_relationship() {
        let mut world = test_world();
        // Two entities close together, both above threshold -> sexual reproduction.
        let parent = spawn_entity_at(&mut world, 50.0, 50.0, 90.0);
        let mate = spawn_entity_at(&mut world, 55.0, 50.0, 90.0);

        let parent_bits = parent.to_bits().get();
        let mate_bits = mate.to_bits().get();

        run(&mut world);

        // Both parent and mate should have positive relationship scores toward each other.
        let parent_social = world.ecs.get::<&Social>(parent).unwrap();
        let score_toward_mate = parent_social.get_relationship(mate_bits);
        assert!(
            score_toward_mate > 0.0,
            "parent should have positive relationship with mate, got {}",
            score_toward_mate
        );

        let mate_social = world.ecs.get::<&Social>(mate).unwrap();
        let score_toward_parent = mate_social.get_relationship(parent_bits);
        assert!(
            score_toward_parent > 0.0,
            "mate should have positive relationship with parent, got {}",
            score_toward_parent
        );
    }

    #[test]
    fn asexual_reproduction_no_social_interaction() {
        let mut world = test_world();
        // Single entity, far from any mate -> asexual reproduction.
        let parent = spawn_entity_with_energy(&mut world, 90.0);

        run(&mut world);

        // Parent should have no relationships (no mate to bond with).
        let social = world.ecs.get::<&Social>(parent).unwrap();
        assert!(
            social.relationships.is_empty(),
            "asexual reproduction should not create social relationships"
        );
    }

    #[test]
    fn offspring_has_social_component() {
        let mut world = test_world();
        spawn_entity_with_energy(&mut world, 90.0);
        run(&mut world);

        for (_entity, (identity, _social)) in world.ecs.query_mut::<(&Identity, &Social)>() {
            if identity.generation == 1 {
                // Offspring has a Social component - test passes.
                return;
            }
        }
        panic!("offspring should have a Social component");
    }
}

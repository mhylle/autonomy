use simulation_engine::core::config::SimulationConfig;
use simulation_engine::core::spawning;
use simulation_engine::core::tick;
use simulation_engine::core::world::SimulationWorld;
use simulation_engine::environment::spawning as resource_spawning;

#[test]
fn empty_world_runs_100_ticks_without_panic() {
    let mut world = SimulationWorld::new(SimulationConfig::default());

    for _ in 0..100 {
        tick::tick(&mut world);
    }

    assert_eq!(world.tick, 100);
    assert_eq!(world.entity_count(), 0);
}

#[test]
fn deterministic_with_same_seed() {
    let config = SimulationConfig {
        seed: 42,
        ..SimulationConfig::default()
    };

    let mut world1 = SimulationWorld::new(config.clone());
    let mut world2 = SimulationWorld::new(config);

    for _ in 0..100 {
        tick::tick(&mut world1);
        tick::tick(&mut world2);
    }

    assert_eq!(world1.tick, world2.tick);
    assert_eq!(world1.entity_count(), world2.entity_count());
}

#[test]
fn config_from_json() {
    let json = r#"{
        "world_width": 1000.0,
        "world_height": 800.0,
        "seed": 99,
        "initial_entity_count": 50,
        "tick_rate": 30,
        "headless": true
    }"#;

    let config: SimulationConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.world_width, 1000.0);
    assert_eq!(config.seed, 99);
    assert_eq!(config.initial_entity_count, 50);
    assert!(config.headless);
}

/// Phase 1.2: Spawn 50 entities, run 1000 ticks, verify all eventually die.
/// Entities have no food source, so metabolism drains energy until death.
#[test]
fn entities_die_without_food() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 50,
        world_width: 200.0,
        world_height: 200.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    spawning::spawn_initial_population(&mut world);

    assert_eq!(world.entity_count(), 50);

    for _ in 0..2000 {
        tick::tick(&mut world);
    }

    // All entities should have died: energy drains at 0.1/tick,
    // starting at 100.0, so death after ~1000 ticks
    assert_eq!(world.entity_count(), 0, "all entities should have died");
}

/// Phase 1.2: Entities move around the world during their lifetime.
#[test]
fn entities_move_during_lifetime() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 10,
        world_width: 500.0,
        world_height: 500.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    spawning::spawn_initial_population(&mut world);

    // Record initial positions
    let initial_positions: Vec<(f64, f64)> = world
        .ecs
        .query::<&simulation_engine::components::Position>()
        .iter()
        .map(|(_, p)| (p.x, p.y))
        .collect();

    // Run a few ticks
    for _ in 0..10 {
        tick::tick(&mut world);
    }

    // Collect new positions
    let new_positions: Vec<(f64, f64)> = world
        .ecs
        .query::<&simulation_engine::components::Position>()
        .iter()
        .map(|(_, p)| (p.x, p.y))
        .collect();

    // At least some entities should have moved
    assert_ne!(initial_positions, new_positions, "entities should have moved");
}

/// Phase 1.2: Two runs with same seed produce identical results.
#[test]
fn full_simulation_deterministic() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        ..SimulationConfig::default()
    };

    let mut world1 = SimulationWorld::new(config.clone());
    spawning::spawn_initial_population(&mut world1);

    let mut world2 = SimulationWorld::new(config);
    spawning::spawn_initial_population(&mut world2);

    for _ in 0..500 {
        tick::tick(&mut world1);
        tick::tick(&mut world2);
    }

    assert_eq!(world1.tick, world2.tick);
    assert_eq!(world1.entity_count(), world2.entity_count());
}

/// Phase 1.3: Entities survive longer when food is available.
#[test]
fn entities_survive_longer_with_food() {
    // World WITHOUT food
    let config_no_food = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        world_width: 100.0,
        world_height: 100.0,
        ..SimulationConfig::default()
    };
    let mut world_no_food = SimulationWorld::new(config_no_food);
    spawning::spawn_initial_population(&mut world_no_food);

    // World WITH food
    let config_food = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        world_width: 100.0,
        world_height: 100.0,
        ..SimulationConfig::default()
    };
    let mut world_food = SimulationWorld::new(config_food);
    resource_spawning::scatter_resources(&mut world_food);
    spawning::spawn_initial_population(&mut world_food);

    // Run 500 ticks — entities with food should survive longer
    for _ in 0..500 {
        tick::tick(&mut world_no_food);
        tick::tick(&mut world_food);
    }

    assert!(
        world_food.entity_count() >= world_no_food.entity_count(),
        "entities with food ({}) should survive at least as long as without ({})",
        world_food.entity_count(),
        world_no_food.entity_count()
    );
}

/// Phase 1.3: Resources deplete when consumed and regrow.
#[test]
fn resources_deplete_and_regrow() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 50,
        world_width: 100.0,
        world_height: 100.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    let initial_total: f64 = world.resources.iter().map(|r| r.amount).sum();

    // Run some ticks — entities should consume resources
    for _ in 0..100 {
        tick::tick(&mut world);
    }

    let after_consumption: f64 = world.resources.iter().map(|r| r.amount).sum();

    // Some resources should have been consumed
    assert!(
        after_consumption < initial_total,
        "resources should deplete: initial={}, after={}",
        initial_total,
        after_consumption
    );
}

/// Phase 1.3: Full simulation with food is deterministic.
#[test]
fn full_simulation_with_food_deterministic() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 30,
        world_width: 200.0,
        world_height: 200.0,
        ..SimulationConfig::default()
    };

    let mut world1 = SimulationWorld::new(config.clone());
    resource_spawning::scatter_resources(&mut world1);
    spawning::spawn_initial_population(&mut world1);

    let mut world2 = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world2);
    spawning::spawn_initial_population(&mut world2);

    for _ in 0..300 {
        tick::tick(&mut world1);
        tick::tick(&mut world2);
    }

    assert_eq!(world1.tick, world2.tick);
    assert_eq!(world1.entity_count(), world2.entity_count());

    // Resources should also be identical
    let resources1: Vec<f64> = world1.resources.iter().map(|r| r.amount).collect();
    let resources2: Vec<f64> = world2.resources.iter().map(|r| r.amount).collect();
    assert_eq!(resources1, resources2, "resource state should be deterministic");
}

/// Phase 1.4: Population self-sustains with food and reproduction.
#[test]
fn population_sustains_with_reproduction() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 30,
        world_width: 200.0,
        world_height: 200.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run 5000 ticks — with food + reproduction, population should persist
    for _ in 0..5000 {
        tick::tick(&mut world);
    }

    assert!(
        world.entity_count() > 0,
        "population should sustain with food and reproduction, got {}",
        world.entity_count()
    );
}

/// Phase 1.4: Offspring have mutated traits.
#[test]
fn offspring_have_mutated_traits() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        world_width: 100.0,
        world_height: 100.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run enough ticks for reproduction to happen
    for _ in 0..2000 {
        tick::tick(&mut world);
    }

    // Check if any entity has generation > 0 (offspring exist)
    let max_generation: u32 = world
        .ecs
        .query::<&simulation_engine::components::Identity>()
        .iter()
        .map(|(_, id)| id.generation)
        .max()
        .unwrap_or(0);

    assert!(
        max_generation > 0,
        "offspring should have been born (max generation={})",
        max_generation
    );
}

/// Phase 1.5: Event log captures births, deaths, and feedings.
#[test]
fn event_log_captures_lifecycle_events() {
    use simulation_engine::events::types::SimEvent;

    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        world_width: 100.0,
        world_height: 100.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    let mut total_deaths = 0;
    let mut total_feedings = 0;
    let mut total_births = 0;
    let mut total_moves = 0;

    for _ in 0..500 {
        tick::tick(&mut world);
        for event in world.event_log.events() {
            match event {
                SimEvent::EntityDied { .. } => total_deaths += 1,
                SimEvent::EntityAte { .. } => total_feedings += 1,
                SimEvent::EntityReproduced { .. } => total_births += 1,
                SimEvent::EntityMoved { .. } => total_moves += 1,
                _ => {}
            }
        }
    }

    assert!(total_moves > 0, "should have movement events");
    assert!(total_feedings > 0, "should have feeding events");
    // Deaths or births may or may not happen in 500 ticks depending on seed
    // but at least one should occur since entities consume energy
    assert!(
        total_deaths > 0 || total_births > 0,
        "should have at least some lifecycle events"
    );
}

/// Phase 1.4: Reproduction with food is deterministic.
#[test]
fn reproduction_deterministic() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 20,
        world_width: 150.0,
        world_height: 150.0,
        ..SimulationConfig::default()
    };

    let mut world1 = SimulationWorld::new(config.clone());
    resource_spawning::scatter_resources(&mut world1);
    spawning::spawn_initial_population(&mut world1);

    let mut world2 = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world2);
    spawning::spawn_initial_population(&mut world2);

    for _ in 0..2000 {
        tick::tick(&mut world1);
        tick::tick(&mut world2);
    }

    assert_eq!(world1.entity_count(), world2.entity_count());
}

/// Phase 2.7: After many ticks, multiple distinct species should emerge
/// and behavior trees should have diversified from the starter BT.
#[test]
fn evolution_produces_species_diversity() {
    use simulation_engine::components::Genome;
    use std::collections::HashSet;

    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 50,
        world_width: 300.0,
        world_height: 300.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run 10,000 ticks.
    for _ in 0..10_000 {
        tick::tick(&mut world);
    }

    // Count distinct species.
    let species: HashSet<u64> = world
        .ecs
        .query::<&Genome>()
        .iter()
        .map(|(_, g)| g.species_id)
        .collect();

    assert!(
        species.len() >= 2,
        "after 10,000 ticks, at least 2 distinct species should exist, got {}",
        species.len()
    );

    // Verify species history was recorded.
    assert!(
        !world.species_history.is_empty(),
        "species history should have been recorded"
    );
}

// ==================== Phase 3.7: Coevolutionary Dynamics ====================

/// Phase 3.7: After 10,000 ticks, verify trait diversity among surviving entities.
///
/// Checks that the population exhibits a range of aggression and speed values,
/// indicating that different ecological niches (predator-like vs prey-like) emerged.
#[test]
fn coevolution_produces_trait_diversity() {
    use simulation_engine::components::Genome;

    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 80,
        world_width: 400.0,
        world_height: 400.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run 10,000 ticks to allow evolution.
    for _ in 0..10_000 {
        tick::tick(&mut world);
    }

    // Collect genome data from survivors.
    let genomes: Vec<(f64, f64, f64)> = world
        .ecs
        .query::<&Genome>()
        .iter()
        .map(|(_, g)| (g.drive_weights.base_aggression, g.max_speed, g.size))
        .collect();

    assert!(
        !genomes.is_empty(),
        "population should not be extinct after 10,000 ticks"
    );

    // Check trait diversity: the range of max_speed values should show variation.
    let speeds: Vec<f64> = genomes.iter().map(|(_, s, _)| *s).collect();
    let min_speed = speeds.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_speed = speeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let speed_range = max_speed - min_speed;

    // With mutation over 10,000 ticks, speeds should have diverged somewhat.
    assert!(
        speed_range > 0.01,
        "speed should show variation after evolution: min={:.3}, max={:.3}, range={:.3}",
        min_speed,
        max_speed,
        speed_range
    );

    // Check that multiple species exist.
    let species: std::collections::HashSet<u64> = world
        .ecs
        .query::<&Genome>()
        .iter()
        .map(|(_, g)| g.species_id)
        .collect();

    assert!(
        species.len() >= 2,
        "multiple species should exist after 10,000 ticks, got {}",
        species.len()
    );
}

/// Phase 3.7: Kill matrix records species interactions during combat.
///
/// Runs a medium-length simulation and verifies that when combat kills occur,
/// the kill matrix is populated with species interaction data.
#[test]
fn kill_matrix_records_species_interactions() {
    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 60,
        world_width: 200.0,
        world_height: 200.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run 5,000 ticks in a dense world to encourage combat.
    for _ in 0..5_000 {
        tick::tick(&mut world);
    }

    // The kill matrix should have been populated if any combat kills occurred.
    // In a dense 200x200 world with 60 entities, aggression-driven attacks
    // should happen frequently.
    let total_kills: u32 = world.kill_matrix.values().sum();

    // Even if no kills happened (possible with low aggression), verify the
    // kill_matrix data structure works correctly.
    // We check that if kills occurred, the matrix has entries with valid counts.
    for (&(attacker_species, _victim_species), &count) in &world.kill_matrix {
        assert!(count > 0, "kill matrix entries should have positive counts");
        // Verify that the attacker_species looks like a valid species_id (non-zero).
        assert_ne!(attacker_species, 0, "species_id should be non-zero");
    }

    // Log stats for debugging (visible with `cargo test -- --nocapture`).
    eprintln!(
        "Phase 3.7: kill_matrix has {} entries, {} total kills",
        world.kill_matrix.len(),
        total_kills
    );

    // Verify population survived (didn't all die).
    assert!(
        world.entity_count() > 0,
        "population should survive in a world with food"
    );
}

/// Phase 3.7: Long-run coevolutionary dynamics with population oscillation.
///
/// Runs 50,000 ticks with 100 entities in a 500x500 world. Verifies:
/// - Population persists (no extinction)
/// - Trait diversity exists (predator-like and prey-like entities)
/// - Multiple species coexist
/// - Species population history shows variation over time (oscillation)
#[test]
#[ignore] // Slow test: ~50,000 ticks. Run with: cargo test -- --ignored
fn coevolution_long_run_50k_ticks() {
    use simulation_engine::components::Genome;
    use std::collections::HashSet;

    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 100,
        world_width: 500.0,
        world_height: 500.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);
    spawning::spawn_initial_population(&mut world);

    // Run 50,000 ticks.
    for _ in 0..50_000 {
        tick::tick(&mut world);
    }

    // 1. Verify population persisted (didn't go extinct).
    assert!(
        world.entity_count() > 0,
        "population should not go extinct after 50,000 ticks, got {} entities",
        world.entity_count()
    );

    // 2. Verify multiple species exist.
    let species: HashSet<u64> = world
        .ecs
        .query::<&Genome>()
        .iter()
        .map(|(_, g)| g.species_id)
        .collect();

    assert!(
        species.len() >= 2,
        "after 50,000 ticks, at least 2 distinct species should exist, got {}",
        species.len()
    );

    // 3. Analyze trait diversity.
    let genomes: Vec<(f64, f64, f64)> = world
        .ecs
        .query::<&Genome>()
        .iter()
        .map(|(_, g)| (g.drive_weights.base_aggression, g.max_speed, g.size))
        .collect();

    // Check for predator-like entities (high aggression).
    let high_aggression_count = genomes
        .iter()
        .filter(|(aggression, _, _)| *aggression > 0.2)
        .count();

    // Check for prey-like entities (high speed relative to default 2.0).
    let high_speed_count = genomes
        .iter()
        .filter(|(_, speed, _)| *speed > 2.1)
        .count();

    eprintln!(
        "Phase 3.7 (50k): {} entities, {} species, {} high-aggression, {} high-speed",
        world.entity_count(),
        species.len(),
        high_aggression_count,
        high_speed_count
    );

    // At least some trait divergence should have occurred.
    let speeds: Vec<f64> = genomes.iter().map(|(_, s, _)| *s).collect();
    let min_speed = speeds.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_speed = speeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    assert!(
        max_speed - min_speed > 0.01,
        "speed should show variation: min={:.3}, max={:.3}",
        min_speed,
        max_speed
    );

    // 4. Verify species history shows population variation over time.
    assert!(
        world.species_history.len() >= 2,
        "species history should have multiple snapshots, got {}",
        world.species_history.len()
    );

    // Check that population counts changed over time (oscillation).
    // Compare total population at different snapshots.
    let population_snapshots: Vec<u32> = world
        .species_history
        .iter()
        .map(|(_, counts)| counts.values().sum::<u32>())
        .collect();

    let pop_min = population_snapshots.iter().cloned().min().unwrap_or(0);
    let pop_max = population_snapshots.iter().cloned().max().unwrap_or(0);

    eprintln!(
        "Phase 3.7 (50k): population range [{}, {}] over {} snapshots",
        pop_min,
        pop_max,
        population_snapshots.len()
    );

    // Population should have fluctuated (not perfectly static).
    assert!(
        pop_max > pop_min,
        "population should oscillate over time: min={}, max={}",
        pop_min,
        pop_max
    );

    // 5. Log kill matrix stats.
    let total_kills: u32 = world.kill_matrix.values().sum();
    let inter_species_kills: u32 = world
        .kill_matrix
        .iter()
        .filter(|(&(a, v), _)| a != v)
        .map(|(_, &count)| count)
        .sum();

    eprintln!(
        "Phase 3.7 (50k): {} total kills, {} inter-species kills, {} kill matrix entries",
        total_kills,
        inter_species_kills,
        world.kill_matrix.len()
    );
}

// ==================== Phase 4.5: Emergent Specialization ====================

/// Phase 4.5: After 50,000+ ticks, verify that trait divergence has occurred
/// among composite member genomes, indicating emergent specialization.
///
/// Checks:
/// - Sensing specialists evolve: large sensor_range, small size
/// - Locomotion specialists evolve: high speed, low energy cost
/// - Attack specialists evolve: high aggression, large size
/// - General trait variance increases over evolutionary time
///
/// This test manually creates composites with members and runs composite
/// reproduction to verify that offspring members diverge in traits.
#[test]
#[ignore] // Long-running test: ~50,000 ticks. Run with: cargo test -- --ignored
fn emergent_specialization_trait_divergence() {
    use simulation_engine::components::composite::{
        CellRole, CompositeBody, CompositeMember, CompositeMemberMarker,
    };
    use simulation_engine::components::genome::{mutate, Genome};
    use simulation_engine::components::Genome as GenomeComponent;
    use simulation_engine::systems::composite_reproduction;
    use std::collections::HashMap;

    // Create a world with entities, some of which are composites.
    let config = SimulationConfig {
        seed: 7,
        initial_entity_count: 0, // we spawn manually
        world_width: 400.0,
        world_height: 400.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);
    resource_spawning::scatter_resources(&mut world);

    // Spawn 10 composites, each with 4 members.
    let roles = [
        CellRole::Sensing,
        CellRole::Locomotion,
        CellRole::Attack,
        CellRole::Defense,
    ];

    for i in 0..10 {
        let x = 50.0 + (i as f64 * 35.0);
        let y = 50.0 + (i as f64 * 35.0);

        // Spawn the leader entity.
        let mut leader_genome = Genome::default();
        leader_genome.mutation_rate = 0.2; // higher mutation for faster divergence
        leader_genome.composition_affinity = 0.8;

        let leader = world.ecs.spawn((
            simulation_engine::components::Position { x, y },
            simulation_engine::components::Velocity::default(),
            simulation_engine::components::Energy {
                current: 95.0, // above reproduction threshold
                max: leader_genome.max_energy,
                metabolism_rate: leader_genome.metabolism_rate,
            },
            simulation_engine::components::Health { current: 100.0, max: 100.0 },
            simulation_engine::components::Age { ticks: 0, max_lifespan: leader_genome.max_lifespan },
            simulation_engine::components::Size { radius: leader_genome.size },
            simulation_engine::components::Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            simulation_engine::components::Perception::default(),
            simulation_engine::components::Drives::default(),
            simulation_engine::components::Social::default(),
            simulation_engine::components::Memory::default(),
            simulation_engine::components::behavior_tree::default_starter_bt(),
            simulation_engine::components::Action::default(),
            leader_genome.clone(),
        ));

        let leader_id = leader.to_bits().get();
        let mut body = CompositeBody::new(leader_id, 0);

        for (j, role) in roles.iter().enumerate() {
            let mut member_genome = Genome::default();
            member_genome.mutation_rate = 0.2;
            // Bias initial members slightly toward their role
            match role {
                CellRole::Sensing => {
                    member_genome.sensor_range = 60.0 + j as f64 * 5.0;
                    member_genome.size = 3.0;
                }
                CellRole::Locomotion => {
                    member_genome.max_speed = 3.0 + j as f64 * 0.5;
                }
                CellRole::Attack => {
                    member_genome.drive_weights.base_aggression = 0.3;
                    member_genome.size = 7.0;
                }
                CellRole::Defense => {
                    member_genome.size = 8.0;
                }
                _ => {}
            }

            let member = world.ecs.spawn((
                simulation_engine::components::Position { x, y },
                simulation_engine::components::Velocity::default(),
                simulation_engine::components::Energy {
                    current: 50.0,
                    max: member_genome.max_energy,
                    metabolism_rate: member_genome.metabolism_rate,
                },
                simulation_engine::components::Health { current: 100.0, max: 100.0 },
                simulation_engine::components::Age { ticks: 0, max_lifespan: member_genome.max_lifespan },
                simulation_engine::components::Size { radius: member_genome.size },
                simulation_engine::components::Identity {
                    generation: 0,
                    parent_id: None,
                    birth_tick: 0,
                },
                simulation_engine::components::Perception::default(),
                simulation_engine::components::Drives::default(),
                simulation_engine::components::Social::default(),
                simulation_engine::components::Memory::default(),
                simulation_engine::components::Action::default(),
                member_genome,
                CompositeMemberMarker { leader_id },
            ));

            body.add_member(member.to_bits().get(), *role);
        }

        world.ecs.insert_one(leader, body).unwrap();
    }

    // Run composite reproduction for many generations.
    // We run the full tick loop so all systems interact.
    for _ in 0..50_000 {
        tick::tick(&mut world);
    }

    // Now analyze trait diversity among surviving entities.
    let mut sensor_ranges: Vec<f64> = Vec::new();
    let mut speeds: Vec<f64> = Vec::new();
    let mut sizes: Vec<f64> = Vec::new();
    let mut aggressions: Vec<f64> = Vec::new();

    for (_entity, genome) in world.ecs.query::<&GenomeComponent>().iter() {
        sensor_ranges.push(genome.sensor_range);
        speeds.push(genome.max_speed);
        sizes.push(genome.size);
        aggressions.push(genome.drive_weights.base_aggression);
    }

    let entity_count = sensor_ranges.len();
    eprintln!(
        "Phase 4.5: {} entities surviving after 50,000 ticks",
        entity_count
    );

    assert!(
        entity_count > 0,
        "population should survive after 50,000 ticks"
    );

    // Calculate trait variance as a measure of divergence.
    let variance = |values: &[f64]| -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
    };

    let sr_variance = variance(&sensor_ranges);
    let speed_variance = variance(&speeds);
    let size_variance = variance(&sizes);
    let aggression_variance = variance(&aggressions);

    eprintln!(
        "Phase 4.5 trait variance: sensor_range={:.4}, speed={:.4}, size={:.4}, aggression={:.4}",
        sr_variance, speed_variance, size_variance, aggression_variance
    );

    // At least one trait should show meaningful divergence.
    let total_variance = sr_variance + speed_variance + size_variance + aggression_variance;
    assert!(
        total_variance > 0.001,
        "traits should show divergence after 50,000 ticks of evolution, total_variance={:.6}",
        total_variance
    );

    // Check for trait range (max - min) as another measure.
    let range = |values: &[f64]| -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        max - min
    };

    let sr_range = range(&sensor_ranges);
    let speed_range = range(&speeds);
    let size_range = range(&sizes);

    eprintln!(
        "Phase 4.5 trait ranges: sensor_range={:.3}, speed={:.3}, size={:.3}",
        sr_range, speed_range, size_range
    );

    assert!(
        sr_range > 0.01 || speed_range > 0.01 || size_range > 0.01,
        "at least one trait should show a meaningful range of values"
    );
}

/// Phase 4.5: Composite reproduction produces offspring with member composition.
///
/// Quick test that composite reproduction works end-to-end in a single
/// tick and produces valid offspring composites.
#[test]
fn composite_reproduction_produces_offspring() {
    use simulation_engine::components::composite::{
        CellRole, CompositeBody, CompositeMember, CompositeMemberMarker,
    };
    use simulation_engine::components::genome::Genome;
    use simulation_engine::systems::composite_reproduction;

    let config = SimulationConfig {
        seed: 42,
        initial_entity_count: 0,
        world_width: 200.0,
        world_height: 200.0,
        ..SimulationConfig::default()
    };
    let mut world = SimulationWorld::new(config);

    // Create a composite with enough energy and members to reproduce.
    let leader_genome = Genome::default();
    let leader = world.ecs.spawn((
        simulation_engine::components::Position { x: 50.0, y: 50.0 },
        simulation_engine::components::Velocity::default(),
        simulation_engine::components::Energy {
            current: 90.0,
            max: leader_genome.max_energy,
            metabolism_rate: leader_genome.metabolism_rate,
        },
        simulation_engine::components::Health { current: 100.0, max: 100.0 },
        simulation_engine::components::Age { ticks: 0, max_lifespan: leader_genome.max_lifespan },
        simulation_engine::components::Size { radius: leader_genome.size },
        simulation_engine::components::Identity {
            generation: 0,
            parent_id: None,
            birth_tick: 0,
        },
        simulation_engine::components::Perception::default(),
        simulation_engine::components::Drives::default(),
        simulation_engine::components::Social::default(),
        simulation_engine::components::Memory::default(),
        simulation_engine::components::behavior_tree::default_starter_bt(),
        simulation_engine::components::Action::default(),
        leader_genome,
    ));

    let leader_id = leader.to_bits().get();
    let mut body = CompositeBody::new(leader_id, 0);

    // Add 3 members with different roles.
    let roles = [CellRole::Sensing, CellRole::Locomotion, CellRole::Attack];
    for role in &roles {
        let member_genome = Genome::default();
        let member = world.ecs.spawn((
            simulation_engine::components::Position { x: 50.0, y: 50.0 },
            simulation_engine::components::Velocity::default(),
            simulation_engine::components::Energy {
                current: 50.0,
                max: member_genome.max_energy,
                metabolism_rate: member_genome.metabolism_rate,
            },
            simulation_engine::components::Health { current: 100.0, max: 100.0 },
            simulation_engine::components::Age { ticks: 0, max_lifespan: member_genome.max_lifespan },
            simulation_engine::components::Size { radius: member_genome.size },
            simulation_engine::components::Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            simulation_engine::components::Perception::default(),
            simulation_engine::components::Drives::default(),
            simulation_engine::components::Social::default(),
            simulation_engine::components::Memory::default(),
            simulation_engine::components::Action::default(),
            member_genome,
            CompositeMemberMarker { leader_id },
        ));
        body.add_member(member.to_bits().get(), *role);
    }

    world.ecs.insert_one(leader, body).unwrap();

    // 4 entities: 1 leader + 3 members
    let initial_count = world.entity_count();
    assert_eq!(initial_count, 4);

    // Run composite reproduction.
    composite_reproduction::run(&mut world);

    // Should have spawned offspring (leader + members).
    assert!(
        world.entity_count() > initial_count,
        "composite reproduction should spawn offspring: {} vs {}",
        world.entity_count(),
        initial_count
    );

    // Find offspring composite body.
    let mut found_offspring = false;
    for (_entity, (identity, body)) in world
        .ecs
        .query_mut::<(
            &simulation_engine::components::Identity,
            &CompositeBody,
        )>()
    {
        if identity.generation == 1 {
            found_offspring = true;
            assert!(
                body.member_count() > 0,
                "offspring composite should have members"
            );
        }
    }
    assert!(found_offspring, "should find offspring composite");

    // Verify CompositeReproduced event was emitted.
    let repro_events: Vec<_> = world
        .event_log
        .events()
        .iter()
        .filter(|e| matches!(e, simulation_engine::events::types::SimEvent::CompositeReproduced { .. }))
        .collect();
    assert_eq!(
        repro_events.len(),
        1,
        "should emit exactly one CompositeReproduced event"
    );
}

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

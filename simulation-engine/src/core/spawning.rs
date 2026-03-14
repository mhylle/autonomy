use hecs::Entity;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::world::SimulationWorld;
use crate::components::action::Action;
use crate::components::behavior_tree::default_starter_bt;
use crate::components::drives::Drives;
use crate::components::genome::Genome;
use crate::components::identity::Identity;
use crate::components::memory::Memory;
use crate::components::perception::Perception;
use crate::components::social::Social;
use crate::components::{Age, Energy, Health, Position, Size, Velocity};

/// Spawn a single entity with random position and genome-derived components.
///
/// Entities will not be placed on impassable terrain (e.g. Water).
pub fn spawn_entity(world: &mut SimulationWorld, rng: &mut ChaCha8Rng) -> Entity {
    let position = random_passable_position(rng, world);
    let velocity = Velocity::default();
    let genome = Genome::default();
    let energy = Energy {
        current: genome.max_energy,
        max: genome.max_energy,
        metabolism_rate: genome.metabolism_rate,
    };
    let health = Health::default();
    let age = Age {
        ticks: 0,
        max_lifespan: genome.max_lifespan,
    };
    let size = Size {
        radius: genome.size,
    };
    let identity = Identity {
        generation: 0,
        parent_id: None,
        birth_tick: world.tick,
    };

    let perception = Perception::default();
    let drives = Drives::default();
    let social = Social::default();
    let memory = Memory::new(
        genome.memory_capacity as usize,
        genome.eviction_weights.clone(),
    );
    let bt = default_starter_bt();
    let action = Action::default();

    world
        .ecs
        .spawn((position, velocity, energy, health, age, size, genome, identity, perception, drives, social, memory, bt, action))
}

/// Spawn the initial population of entities.
pub fn spawn_initial_population(world: &mut SimulationWorld) {
    let count = world.config.initial_entity_count;
    let mut rng = world.rng.system_rng("spawning");

    for _ in 0..count {
        spawn_entity(world, &mut rng);
    }

    tracing::info!(count = count, "spawned initial population");
}

/// Generate a random position on passable terrain.
///
/// Retries up to 1000 times to find a non-water cell. If all attempts
/// fail (extremely unlikely), falls back to position (0, 0).
fn random_passable_position(rng: &mut ChaCha8Rng, world: &SimulationWorld) -> Position {
    let width = world.config.world_width;
    let height = world.config.world_height;

    for _ in 0..1000 {
        let x = rng.gen_range(0.0..width);
        let y = rng.gen_range(0.0..height);
        if world.terrain.is_passable(x, y) {
            return Position { x, y };
        }
    }

    // Fallback: find the first passable cell.
    for row in 0..world.terrain.rows {
        for col in 0..world.terrain.cols {
            if world.terrain.get(col, row).is_passable() {
                return Position {
                    x: col as f64 * world.terrain.cell_size + 1.0,
                    y: row as f64 * world.terrain.cell_size + 1.0,
                };
            }
        }
    }

    Position { x: 0.0, y: 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    #[test]
    fn spawn_entity_adds_to_world() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut rng = world.rng.system_rng("test");
        assert_eq!(world.entity_count(), 0);

        spawn_entity(&mut world, &mut rng);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn spawn_entity_has_all_components() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut rng = world.rng.system_rng("test");
        let entity = spawn_entity(&mut world, &mut rng);

        assert!(world.ecs.get::<&Position>(entity).is_ok());
        assert!(world.ecs.get::<&Velocity>(entity).is_ok());
        assert!(world.ecs.get::<&Energy>(entity).is_ok());
        assert!(world.ecs.get::<&Health>(entity).is_ok());
        assert!(world.ecs.get::<&Age>(entity).is_ok());
        assert!(world.ecs.get::<&Size>(entity).is_ok());
        assert!(world.ecs.get::<&Genome>(entity).is_ok());
        assert!(world.ecs.get::<&Identity>(entity).is_ok());
        assert!(world.ecs.get::<&Social>(entity).is_ok());
        assert!(world.ecs.get::<&Memory>(entity).is_ok());
    }

    #[test]
    fn spawn_entity_uses_genome_values() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut rng = world.rng.system_rng("test");
        let entity = spawn_entity(&mut world, &mut rng);

        let genome = world.ecs.get::<&Genome>(entity).unwrap();
        let energy = world.ecs.get::<&Energy>(entity).unwrap();
        let age = world.ecs.get::<&Age>(entity).unwrap();
        let size = world.ecs.get::<&Size>(entity).unwrap();

        assert_eq!(energy.max, genome.max_energy);
        assert_eq!(energy.current, genome.max_energy);
        assert_eq!(energy.metabolism_rate, genome.metabolism_rate);
        assert_eq!(age.max_lifespan, genome.max_lifespan);
        assert_eq!(size.radius, genome.size);
    }

    #[test]
    fn spawn_entity_has_generation_zero() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut rng = world.rng.system_rng("test");
        let entity = spawn_entity(&mut world, &mut rng);

        let identity = world.ecs.get::<&Identity>(entity).unwrap();
        assert_eq!(identity.generation, 0);
        assert!(identity.parent_id.is_none());
    }

    #[test]
    fn spawn_entity_position_within_bounds() {
        let config = SimulationConfig {
            world_width: 100.0,
            world_height: 200.0,
            ..SimulationConfig::default()
        };
        let mut world = SimulationWorld::new(config);
        let mut rng = world.rng.system_rng("test");

        for _ in 0..100 {
            let entity = spawn_entity(&mut world, &mut rng);
            let pos = world.ecs.get::<&Position>(entity).unwrap();
            assert!(pos.x >= 0.0 && pos.x < 100.0);
            assert!(pos.y >= 0.0 && pos.y < 200.0);
        }
    }

    #[test]
    fn spawn_initial_population_correct_count() {
        let config = SimulationConfig {
            initial_entity_count: 50,
            ..SimulationConfig::default()
        };
        let mut world = SimulationWorld::new(config);
        spawn_initial_population(&mut world);
        assert_eq!(world.entity_count(), 50);
    }

    #[test]
    fn spawn_is_deterministic() {
        let config = SimulationConfig {
            seed: 42,
            initial_entity_count: 10,
            ..SimulationConfig::default()
        };

        let mut world1 = SimulationWorld::new(config.clone());
        spawn_initial_population(&mut world1);

        let mut world2 = SimulationWorld::new(config);
        spawn_initial_population(&mut world2);

        // Collect positions from both worlds
        let positions1: Vec<(f64, f64)> = world1
            .ecs
            .query::<&Position>()
            .iter()
            .map(|(_, p)| (p.x, p.y))
            .collect();
        let positions2: Vec<(f64, f64)> = world2
            .ecs
            .query::<&Position>()
            .iter()
            .map(|(_, p)| (p.x, p.y))
            .collect();

        assert_eq!(positions1, positions2);
    }
}

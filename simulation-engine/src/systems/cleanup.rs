use crate::components::physical::{is_dead, Age, Energy};
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::events::types::{DeathCause, SimEvent};

/// Despawns entities that are dead.
///
/// Queries all entities with `Energy` and `Age` (immutable), collects
/// those for which `is_dead` returns true, then removes them from the
/// ECS world. Emits `EntityDied` events and logs the count of removed
/// entities.
pub fn run(world: &mut SimulationWorld) {
    let dead_entities: Vec<_> = world
        .ecs
        .query::<(&Energy, &Age)>()
        .iter()
        .filter(|(_entity, (energy, age))| is_dead(energy, age))
        .map(|(entity, (energy, age))| {
            let cause = if energy.current <= 0.0 {
                DeathCause::Starvation
            } else {
                DeathCause::OldAge
            };
            (entity, age.ticks, cause)
        })
        .collect();

    let count = dead_entities.len();

    for (entity, age_ticks, cause) in dead_entities {
        // Read position before despawning (may not exist on all entities).
        let (x, y) = world
            .ecs
            .get::<&Position>(entity)
            .map(|pos| (pos.x, pos.y))
            .unwrap_or((0.0, 0.0));

        world.event_log.push(SimEvent::EntityDied {
            entity_id: entity.to_bits().get(),
            x,
            y,
            age: age_ticks,
            cause,
        });

        world.ecs.despawn(entity).ok();
    }

    if count > 0 {
        tracing::debug!(removed = count, "cleaned up dead entities");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    #[test]
    fn removes_entity_with_zero_energy() {
        let mut world = test_world();
        world.ecs.spawn((
            Energy {
                current: 0.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Age::default(),
        ));

        assert_eq!(world.entity_count(), 1);
        run(&mut world);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn removes_entity_with_negative_energy() {
        let mut world = test_world();
        world.ecs.spawn((
            Energy {
                current: -5.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Age::default(),
        ));

        run(&mut world);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn removes_entity_that_exceeded_lifespan() {
        let mut world = test_world();
        world.ecs.spawn((
            Energy::default(),
            Age {
                ticks: 5000,
                max_lifespan: 5000,
            },
        ));

        run(&mut world);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn keeps_alive_entities() {
        let mut world = test_world();
        world.ecs.spawn((Energy::default(), Age::default()));

        run(&mut world);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn removes_only_dead_entities() {
        let mut world = test_world();

        // Alive entity
        world.ecs.spawn((Energy::default(), Age::default()));

        // Dead entity (no energy)
        world.ecs.spawn((
            Energy {
                current: 0.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Age::default(),
        ));

        // Dead entity (exceeded lifespan)
        world.ecs.spawn((
            Energy::default(),
            Age {
                ticks: 6000,
                max_lifespan: 5000,
            },
        ));

        assert_eq!(world.entity_count(), 3);
        run(&mut world);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn no_crash_on_empty_world() {
        let mut world = test_world();
        run(&mut world);
        assert_eq!(world.entity_count(), 0);
    }
}

use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::events::types::SimEvent;

/// How close an entity must be to a resource to consume it.
const FEEDING_RANGE: f64 = 10.0;

/// How much energy an entity tries to consume per tick.
const FEED_AMOUNT: f64 = 20.0;

/// Feeds entities from nearby resources.
///
/// Each entity with `Position` and `Energy` checks for available resources
/// within `FEEDING_RANGE`. If a nearby resource is found, the entity
/// consumes up to `FEED_AMOUNT` energy from the closest one, capped at
/// its maximum energy.
pub fn run(world: &mut SimulationWorld) {
    // 1. Collect entity feeding data to avoid borrow conflicts.
    let feeders: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy)>()
        .iter()
        .map(|(entity, (pos, energy))| (entity, pos.x, pos.y, energy.current, energy.max))
        .collect();

    // 2. Process feeding: find nearby resources and consume from the closest.
    let mut energy_gains: Vec<(hecs::Entity, f64, usize)> = Vec::new();

    for (entity, x, y, current, max) in &feeders {
        if *current >= *max {
            continue;
        }

        let nearby = world
            .spatial_index
            .query_resources_in_radius(*x, *y, FEEDING_RANGE);

        // Find the closest available resource.
        let closest = nearby
            .iter()
            .filter(|(idx, _, _)| world.resources[*idx].is_available())
            .min_by(|(_, ax, ay), (_, bx, by)| {
                let dist_a = (ax - x).powi(2) + (ay - y).powi(2);
                let dist_b = (bx - x).powi(2) + (by - y).powi(2);
                dist_a.partial_cmp(&dist_b).unwrap()
            });

        if let Some(&(res_idx, _, _)) = closest {
            let room = max - current;
            let want = FEED_AMOUNT.min(room);
            let gained = world.resources[res_idx].consume(want);
            energy_gains.push((*entity, gained, res_idx));
        }
    }

    // 3. Apply energy gains back to entities and emit events.
    for (entity, gain, res_idx) in &energy_gains {
        if let Ok(mut energy) = world.ecs.get::<&mut Energy>(*entity) {
            energy.current = (energy.current + *gain).min(energy.max);
        }

        world.event_log.push(SimEvent::EntityAte {
            entity_id: entity.to_bits().get(),
            resource_id: world.resources[*res_idx].id,
            energy_gained: *gain,
        });

        // Emit ResourceDepleted if the resource was fully consumed.
        if world.resources[*res_idx].depleted {
            let res = &world.resources[*res_idx];
            world.event_log.push(SimEvent::ResourceDepleted {
                resource_id: res.id,
                x: res.x,
                y: res.y,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::environment::resources::Resource;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Helper: insert a resource into the world and spatial index.
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
    fn entity_near_resource_gains_energy() {
        let mut world = test_world();

        // Entity at (50, 50) with half energy.
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Energy {
                current: 50.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));

        // Resource very close to the entity.
        add_resource(&mut world, 52.0, 50.0, 50.0);

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert!(
                energy.current > 50.0,
                "entity should have gained energy, got {}",
                energy.current
            );
        }
    }

    #[test]
    fn entity_far_from_resource_does_not_gain_energy() {
        let mut world = test_world();

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Energy {
                current: 50.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));

        // Resource far away (distance > FEEDING_RANGE).
        add_resource(&mut world, 200.0, 200.0, 50.0);

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert_eq!(
                energy.current, 50.0,
                "entity should not have gained energy"
            );
        }
    }

    #[test]
    fn resource_amount_decreases_after_feeding() {
        let mut world = test_world();

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Energy {
                current: 50.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));

        add_resource(&mut world, 52.0, 50.0, 50.0);

        run(&mut world);

        assert!(
            world.resources[0].amount < 50.0,
            "resource amount should have decreased, got {}",
            world.resources[0].amount
        );
    }

    #[test]
    fn entity_energy_does_not_exceed_max() {
        let mut world = test_world();

        // Entity nearly full.
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Energy {
                current: 95.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));

        add_resource(&mut world, 52.0, 50.0, 50.0);

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert!(
                energy.current <= energy.max,
                "energy {} should not exceed max {}",
                energy.current,
                energy.max
            );
        }
    }

    #[test]
    fn depleted_resource_is_skipped() {
        let mut world = test_world();

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Energy {
                current: 50.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));

        // Depleted resource nearby.
        let idx = world.resources.len();
        world.resources.push(Resource {
            id: idx as u64,
            x: 52.0,
            y: 50.0,
            amount: 0.0,
            max_amount: 50.0,
            depleted: true,
            ..Default::default()
        });
        world.spatial_index.insert_resource(idx, 52.0, 50.0);

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert_eq!(
                energy.current, 50.0,
                "entity should not gain energy from depleted resource"
            );
        }
    }
}

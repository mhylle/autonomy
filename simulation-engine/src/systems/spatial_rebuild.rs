//! Rebuilds the spatial index each tick.
//!
//! This system must run **before** any system that needs proximity queries
//! (e.g. feeding, reproduction, flocking). It clears the spatial index and
//! re-inserts every positioned entity and every available resource.

use crate::components::Position;
use crate::core::world::SimulationWorld;

/// Clear and rebuild the spatial index from the current ECS state and
/// resource list.
pub fn run(world: &mut SimulationWorld) {
    world.spatial_index.clear();

    // Insert entities with a Position component.
    for (entity, pos) in world.ecs.query::<&Position>().iter() {
        world.spatial_index.insert_entity(entity.to_bits().get(), pos.x, pos.y);
    }

    // Insert available (non-depleted) resources.
    for (i, resource) in world.resources.iter().enumerate() {
        if resource.is_available() {
            world.spatial_index.insert_resource(i, resource.x, resource.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::environment::resources::{Resource, ResourceType};

    fn make_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    #[test]
    fn entity_found_after_rebuild() {
        let mut world = make_world();

        // Spawn an entity with a Position.
        let pos = Position { x: 100.0, y: 100.0, z: 0.0 };
        let entity = world.ecs.spawn((pos,));

        run(&mut world);

        let results = world
            .spatial_index
            .query_entities_in_radius(100.0, 100.0, 10.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, entity.to_bits().get());
    }

    #[test]
    fn resource_found_after_rebuild() {
        let mut world = make_world();

        world.resources.push(Resource {
            id: 1,
            x: 50.0,
            y: 50.0,
            resource_type: ResourceType::Food,
            amount: 30.0,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            depleted: false,
        });

        run(&mut world);

        let results = world
            .spatial_index
            .query_resources_in_radius(50.0, 50.0, 10.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0); // resource index 0
    }

    #[test]
    fn depleted_resources_are_not_inserted() {
        let mut world = make_world();

        world.resources.push(Resource {
            id: 1,
            x: 50.0,
            y: 50.0,
            resource_type: ResourceType::Food,
            amount: 0.0,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            depleted: true,
        });

        run(&mut world);

        let results = world
            .spatial_index
            .query_resources_in_radius(50.0, 50.0, 100.0);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_world_does_not_panic() {
        let mut world = make_world();
        run(&mut world);

        let entities = world
            .spatial_index
            .query_entities_in_radius(0.0, 0.0, 1000.0);
        let resources = world
            .spatial_index
            .query_resources_in_radius(0.0, 0.0, 1000.0);

        assert!(entities.is_empty());
        assert!(resources.is_empty());
    }
}

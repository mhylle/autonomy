use crate::components::Position;
use crate::core::world::SimulationWorld;

/// System that manages chunk activation and deactivation each tick.
///
/// When chunks are enabled (`config.enable_chunks`), this system:
/// 1. Rebuilds entity-to-chunk assignments based on current positions.
/// 2. Ticks the chunk manager to handle dormant/unload transitions.
///
/// When chunks are disabled, this system is a no-op.
pub fn tick_chunk_manager(world: &mut SimulationWorld) {
    let chunk_manager = match world.chunk_manager.as_mut() {
        Some(cm) => cm,
        None => return, // Chunks disabled
    };

    // Clear all entity assignments in chunks before rebuilding.
    for chunk in chunk_manager.loaded_coords() {
        if let Some(c) = chunk_manager.get_chunk_mut(chunk) {
            c.entity_ids.clear();
        }
    }

    // Rebuild entity-to-chunk assignments from current positions.
    let entity_positions: Vec<(u64, f64, f64)> = world
        .ecs
        .query::<&Position>()
        .iter()
        .map(|(entity, pos)| (entity.to_bits().get(), pos.x, pos.y))
        .collect();

    let chunk_manager = world.chunk_manager.as_mut().unwrap();
    for (entity_id, x, y) in entity_positions {
        chunk_manager.add_entity(entity_id, x, y);
    }

    // Run chunk lifecycle (deactivate empty, tick dormant, unload timed-out).
    chunk_manager.tick();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Position;
    use crate::core::config::SimulationConfig;

    fn make_world_with_chunks() -> SimulationWorld {
        let mut config = SimulationConfig::default();
        config.enable_chunks = true;
        config.chunk_size = 256.0;
        SimulationWorld::new(config)
    }

    fn make_world_without_chunks() -> SimulationWorld {
        let config = SimulationConfig::default();
        SimulationWorld::new(config)
    }

    #[test]
    fn tick_is_noop_when_chunks_disabled() {
        let mut world = make_world_without_chunks();
        assert!(world.chunk_manager.is_none());
        // Should not panic
        tick_chunk_manager(&mut world);
    }

    #[test]
    fn tick_assigns_entities_to_chunks() {
        let mut world = make_world_with_chunks();

        // Spawn an entity with a position
        world.ecs.spawn((Position { x: 100.0, y: 100.0, z: 0.0 },));

        tick_chunk_manager(&mut world);

        let cm = world.chunk_manager.as_ref().unwrap();
        assert!(cm.loaded_chunk_count() > 0);
        let chunk = cm.get_chunk((0, 0)).unwrap();
        assert_eq!(chunk.entity_count(), 1);
    }

    #[test]
    fn tick_reassigns_entities_after_movement() {
        let mut world = make_world_with_chunks();

        let entity = world.ecs.spawn((Position { x: 100.0, y: 100.0, z: 0.0 },));

        tick_chunk_manager(&mut world);

        // Move entity to a different chunk
        world
            .ecs
            .query_one_mut::<&mut Position>(entity)
            .unwrap()
            .x = 300.0;

        tick_chunk_manager(&mut world);

        let cm = world.chunk_manager.as_ref().unwrap();
        let chunk_1_0 = cm.get_chunk((1, 0)).unwrap();
        assert_eq!(chunk_1_0.entity_count(), 1);
    }

    #[test]
    fn empty_chunks_become_dormant_after_tick() {
        let mut world = make_world_with_chunks();

        // Ensure a chunk exists
        world.chunk_manager.as_mut().unwrap().ensure_chunk((5, 5));

        tick_chunk_manager(&mut world);

        let cm = world.chunk_manager.as_ref().unwrap();
        // Chunk (5,5) has no entities and no viewer, should be dormant
        let chunk = cm.get_chunk((5, 5)).unwrap();
        assert_eq!(
            chunk.state,
            crate::environment::chunks::ChunkState::Dormant
        );
    }
}

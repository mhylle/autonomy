use crate::components::spatial::{Position, Velocity};
use crate::core::world::SimulationWorld;
use crate::environment::terrain::TerrainGrid;
use crate::events::types::SimEvent;

/// Applies velocity to position, scaled by terrain, and wraps around world boundaries.
///
/// Every entity with both a `Position` and `Velocity` is moved by its
/// velocity vector each tick, multiplied by the terrain speed factor at
/// the entity's current position. If the destination cell is impassable
/// (e.g. Water), the move is rejected and the entity stays put.
/// Positions wrap toroidally so entities leaving one edge reappear on
/// the opposite side.
pub fn run(world: &mut SimulationWorld) {
    let width = world.config.world_width;
    let height = world.config.world_height;
    let terrain = &world.terrain;

    // Collect move events to avoid borrow conflict with event_log.
    let mut move_events: Vec<SimEvent> = Vec::new();

    for (entity, (pos, vel)) in world.ecs.query_mut::<(&mut Position, &Velocity)>() {
        let old_x = pos.x;
        let old_y = pos.y;

        let speed_mult = terrain.movement_multiplier_at(pos.x, pos.y);
        let new_x = wrap(pos.x + vel.dx * speed_mult, width);
        let new_y = wrap(pos.y + vel.dy * speed_mult, height);

        // Only move if the destination terrain is passable.
        if is_destination_passable(terrain, new_x, new_y) {
            pos.x = new_x;
            pos.y = new_y;
        }

        move_events.push(SimEvent::EntityMoved {
            entity_id: entity.to_bits().get(),
            from_x: old_x,
            from_y: old_y,
            to_x: pos.x,
            to_y: pos.y,
        });
    }

    for event in move_events {
        world.event_log.push(event);
    }
}

/// Check whether a destination position is on passable terrain.
fn is_destination_passable(terrain: &TerrainGrid, x: f64, y: f64) -> bool {
    terrain.is_passable(x, y)
}

/// Wraps a coordinate into [0, bound).
fn wrap(value: f64, bound: f64) -> f64 {
    ((value % bound) + bound) % bound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::environment::terrain::TerrainType;

    fn test_world() -> SimulationWorld {
        let mut config = SimulationConfig::default();
        config.world_width = 100.0;
        config.world_height = 100.0;
        SimulationWorld::new(config)
    }

    /// Find a position on the given terrain type within a 100x100 world.
    fn find_terrain_position(world: &SimulationWorld, target: TerrainType) -> (f64, f64) {
        for row in 0..world.terrain.rows {
            for col in 0..world.terrain.cols {
                if world.terrain.get(col, row) == target {
                    let x = col as f64 * world.terrain.cell_size + 5.0;
                    let y = row as f64 * world.terrain.cell_size + 5.0;
                    return (x, y);
                }
            }
        }
        panic!("could not find terrain type {:?} in test world", target);
    }

    #[test]
    fn applies_velocity_scaled_by_terrain() {
        let mut world = test_world();
        // Place entity and check terrain multiplier is applied.
        let (x, y) = find_terrain_position(&world, TerrainType::Grassland);
        let mult = world.terrain.movement_multiplier_at(x, y);
        assert_eq!(mult, 1.0, "grassland should have 1.0 multiplier");

        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Velocity { dx: 3.0, dy: 0.0, dz: 0.0 },
        ));

        run(&mut world);

        for (_id, pos) in world.ecs.query_mut::<&Position>() {
            let expected_x = wrap(x + 3.0 * mult, 100.0);
            assert!(
                (pos.x - expected_x).abs() < 0.01,
                "expected x~{}, got {}",
                expected_x,
                pos.x
            );
        }
    }

    #[test]
    fn terrain_slows_movement_on_mountain() {
        let mut world = test_world();
        // Find a mountain cell.
        if let Some((mx, my)) = try_find_terrain(&world, TerrainType::Mountain) {
            let mult = TerrainType::Mountain.movement_speed_multiplier();
            assert!(mult < 1.0, "mountain should slow movement");

            world.ecs.spawn((
                Position { x: mx, y: my, z: 0.0 },
                Velocity { dx: 10.0, dy: 0.0, dz: 0.0 },
            ));

            run(&mut world);

            for (_id, pos) in world.ecs.query_mut::<&Position>() {
                let expected_x = wrap(mx + 10.0 * mult, 100.0);
                assert!(
                    (pos.x - expected_x).abs() < 0.01,
                    "expected x~{}, got {}",
                    expected_x,
                    pos.x
                );
            }
        }
        // If no mountain exists in this seed, the test is vacuously true.
    }

    #[test]
    fn entity_blocked_by_water() {
        let mut world = test_world();
        // Find a passable cell adjacent to water.
        if let Some((px, py, wx, wy)) = find_passable_near_water(&world) {
            world.ecs.spawn((
                Position { x: px, y: py, z: 0.0 },
                Velocity {
                    dx: wx - px,
                    dy: wy - py,
                    dz: 0.0,
                },
            ));

            run(&mut world);

            for (_id, pos) in world.ecs.query_mut::<&Position>() {
                // Entity should stay at its original position.
                assert!(
                    (pos.x - px).abs() < 0.01 && (pos.y - py).abs() < 0.01,
                    "entity should not enter water: was at ({}, {}), now at ({}, {})",
                    px,
                    py,
                    pos.x,
                    pos.y
                );
            }
        }
    }

    #[test]
    fn entity_without_velocity_is_unaffected() {
        let mut world = test_world();
        let (x, y) = find_terrain_position(&world, TerrainType::Grassland);
        world.ecs.spawn((Position { x, y, z: 0.0 },));

        run(&mut world);

        for (_id, pos) in world.ecs.query_mut::<&Position>() {
            assert!((pos.x - x).abs() < f64::EPSILON);
            assert!((pos.y - y).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn wrap_helper_positive_overflow() {
        assert!((wrap(105.0, 100.0) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wrap_helper_negative_underflow() {
        assert!((wrap(-3.0, 100.0) - 97.0).abs() < f64::EPSILON);
    }

    #[test]
    fn water_has_zero_movement_multiplier() {
        assert_eq!(TerrainType::Water.movement_speed_multiplier(), 0.0);
    }

    #[test]
    fn movement_emits_events() {
        let mut world = test_world();
        let (x, y) = find_terrain_position(&world, TerrainType::Grassland);
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Velocity { dx: 1.0, dy: 0.0, dz: 0.0 },
        ));

        run(&mut world);

        assert!(
            !world.event_log.events().is_empty(),
            "movement should emit events"
        );
    }

    /// Try to find a position on the given terrain type.
    fn try_find_terrain(world: &SimulationWorld, target: TerrainType) -> Option<(f64, f64)> {
        for row in 0..world.terrain.rows {
            for col in 0..world.terrain.cols {
                if world.terrain.get(col, row) == target {
                    let x = col as f64 * world.terrain.cell_size + 5.0;
                    let y = row as f64 * world.terrain.cell_size + 5.0;
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Find a passable cell with a water cell to its right.
    fn find_passable_near_water(world: &SimulationWorld) -> Option<(f64, f64, f64, f64)> {
        for row in 0..world.terrain.rows {
            for col in 0..(world.terrain.cols - 1) {
                let here = world.terrain.get(col, row);
                let right = world.terrain.get(col + 1, row);
                if here.is_passable() && right == TerrainType::Water {
                    let px = col as f64 * world.terrain.cell_size + 5.0;
                    let py = row as f64 * world.terrain.cell_size + 5.0;
                    let wx = (col + 1) as f64 * world.terrain.cell_size + 5.0;
                    let wy = py;
                    return Some((px, py, wx, wy));
                }
            }
        }
        None
    }
}

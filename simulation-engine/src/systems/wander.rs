use rand::Rng;
use std::f64::consts::TAU;

use crate::components::action::Action;
use crate::components::spatial::{Position, Velocity};
use crate::core::world::SimulationWorld;

/// Converts `Action` components into `Velocity` for the movement system.
///
/// - Action::MoveTo → velocity toward the target
/// - Action::MoveDirection → velocity in the given direction
/// - Action::Wander → random velocity
/// - Action::Rest → zero velocity
/// - Action::Eat → zero velocity (feeding system handles the rest)
/// - Action::None → zero velocity
pub fn run(world: &mut SimulationWorld) {
    let mut rng = world.rng.tick_rng("wander", world.tick);

    // Entities with Action, Position, and Velocity.
    let updates: Vec<_> = world
        .ecs
        .query::<(&Position, &Velocity, &Action)>()
        .iter()
        .map(|(entity, (pos, _vel, action))| {
            let (dx, dy) = match action {
                Action::MoveTo { x, y, speed } => {
                    let dx = x - pos.x;
                    let dy = y - pos.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.01 {
                        (dx / dist * speed, dy / dist * speed)
                    } else {
                        (0.0, 0.0)
                    }
                }
                Action::MoveDirection { dx, dy, speed } => {
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.01 {
                        (dx / len * speed, dy / len * speed)
                    } else {
                        (0.0, 0.0)
                    }
                }
                Action::Wander { speed } => {
                    let angle: f64 = rng.gen_range(0.0..TAU);
                    (angle.cos() * speed, angle.sin() * speed)
                }
                Action::Rest | Action::Eat | Action::None => (0.0, 0.0),
            };
            (entity, dx, dy)
        })
        .collect();

    for (entity, dx, dy) in updates {
        if let Ok(mut vel) = world.ecs.get::<&mut Velocity>(entity) {
            vel.dx = dx;
            vel.dy = dy;
        }
    }

    // Entities with Velocity but no Action get random wander (backward compat).
    let no_action: Vec<_> = world
        .ecs
        .query::<&Velocity>()
        .without::<&Action>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in no_action {
        let angle: f64 = rng.gen_range(0.0..TAU);
        let speed: f64 = rng.gen_range(1.0..2.0);
        if let Ok(mut vel) = world.ecs.get::<&mut Velocity>(entity) {
            vel.dx = angle.cos() * speed;
            vel.dy = angle.sin() * speed;
        }
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
    fn move_to_sets_velocity_toward_target() {
        let mut world = test_world();
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Action::MoveTo {
                x: 80.0,
                y: 50.0,
                speed: 2.0,
            },
        ));

        run(&mut world);

        for (_id, vel) in world.ecs.query_mut::<&Velocity>() {
            assert!(vel.dx > 1.9, "should move right, got dx={}", vel.dx);
            assert!(vel.dy.abs() < 0.01, "dy should be ~0, got {}", vel.dy);
        }
    }

    #[test]
    fn wander_sets_random_velocity() {
        let mut world = test_world();
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Action::Wander { speed: 1.5 },
        ));

        run(&mut world);

        for (_id, vel) in world.ecs.query_mut::<&Velocity>() {
            let speed = (vel.dx * vel.dx + vel.dy * vel.dy).sqrt();
            assert!(
                (speed - 1.5).abs() < 0.01,
                "speed should be 1.5, got {}",
                speed
            );
        }
    }

    #[test]
    fn rest_sets_zero_velocity() {
        let mut world = test_world();
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity { dx: 5.0, dy: 5.0 },
            Action::Rest,
        ));

        run(&mut world);

        for (_id, vel) in world.ecs.query_mut::<&Velocity>() {
            assert_eq!(vel.dx, 0.0);
            assert_eq!(vel.dy, 0.0);
        }
    }

    #[test]
    fn eat_sets_zero_velocity() {
        let mut world = test_world();
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity { dx: 5.0, dy: 5.0 },
            Action::Eat,
        ));

        run(&mut world);

        for (_id, vel) in world.ecs.query_mut::<&Velocity>() {
            assert_eq!(vel.dx, 0.0);
            assert_eq!(vel.dy, 0.0);
        }
    }

    #[test]
    fn entity_without_action_wanders() {
        let mut world = test_world();
        world.ecs.spawn((Velocity::default(),));

        run(&mut world);

        for (_id, vel) in world.ecs.query_mut::<&Velocity>() {
            let speed = (vel.dx * vel.dx + vel.dy * vel.dy).sqrt();
            assert!(speed >= 1.0 && speed <= 2.0);
        }
    }

    #[test]
    fn deterministic_with_same_seed() {
        let mut world1 = test_world();
        let mut world2 = test_world();
        world1.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Action::Wander { speed: 1.5 },
        ));
        world2.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Action::Wander { speed: 1.5 },
        ));

        run(&mut world1);
        run(&mut world2);

        let vel1: Vec<_> = world1.ecs.query_mut::<&Velocity>().into_iter().map(|(_, v)| (v.dx, v.dy)).collect();
        let vel2: Vec<_> = world2.ecs.query_mut::<&Velocity>().into_iter().map(|(_, v)| (v.dx, v.dy)).collect();
        assert_eq!(vel1, vel2);
    }
}

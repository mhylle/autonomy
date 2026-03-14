//! Signal system: processes EmitSignal actions and manages signal decay.
//!
//! Each tick this system:
//! 1. Decays existing signals and removes expired ones.
//! 2. Processes EmitSignal actions from entities and creates new signals.

use crate::components::action::Action;
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::environment::signals::{
    Signal, SignalManager, DEFAULT_SIGNAL_DECAY_RATE, DEFAULT_SIGNAL_RADIUS,
    DEFAULT_SIGNAL_STRENGTH,
};

/// Run the signal system: decay existing signals and process new emissions.
pub fn run(world: &mut SimulationWorld) {
    // 1. Decay and clean up existing signals.
    SignalManager::tick(&mut world.signals);

    // 2. Collect emission requests from entities with EmitSignal actions.
    let emissions: Vec<(u64, u8, f64, f64)> = world
        .ecs
        .query::<(&Action, &Position)>()
        .iter()
        .filter_map(|(entity, (action, pos))| {
            if let Action::EmitSignal { signal_type } = action {
                Some((entity.to_bits().get(), *signal_type, pos.x, pos.y))
            } else {
                None
            }
        })
        .collect();

    // 3. Create new signals.
    for (emitter_id, signal_type, x, y) in emissions {
        let signal = Signal::new(
            emitter_id,
            signal_type,
            x,
            y,
            DEFAULT_SIGNAL_RADIUS,
            DEFAULT_SIGNAL_STRENGTH,
            DEFAULT_SIGNAL_DECAY_RATE,
        );
        SignalManager::emit(&mut world.signals, signal);
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
    fn emit_signal_creates_signal_in_world() {
        let mut world = test_world();

        // Spawn an entity with EmitSignal action.
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::EmitSignal { signal_type: 1 },
        ));

        run(&mut world);

        assert_eq!(world.signals.len(), 1);
        assert_eq!(world.signals[0].signal_type, 1);
        assert_eq!(world.signals[0].x, 50.0);
        assert_eq!(world.signals[0].y, 50.0);
        assert_eq!(world.signals[0].strength, DEFAULT_SIGNAL_STRENGTH);
    }

    #[test]
    fn signals_decay_each_tick() {
        let mut world = test_world();

        // Pre-place a signal.
        world.signals.push(Signal::new(
            1, 0, 50.0, 50.0, 80.0, 1.0, 0.1,
        ));

        run(&mut world);

        assert_eq!(world.signals.len(), 1);
        assert!((world.signals[0].strength - 0.9).abs() < 1e-9);
    }

    #[test]
    fn expired_signals_are_removed() {
        let mut world = test_world();

        // Signal that will expire on this tick.
        world.signals.push(Signal::new(
            1, 0, 50.0, 50.0, 80.0, 0.05, 0.1,
        ));

        run(&mut world);

        assert!(world.signals.is_empty());
    }

    #[test]
    fn non_emit_actions_do_not_create_signals() {
        let mut world = test_world();

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::Wander { speed: 1.0 },
        ));

        run(&mut world);

        assert!(world.signals.is_empty());
    }

    #[test]
    fn multiple_emitters_create_multiple_signals() {
        let mut world = test_world();

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::EmitSignal { signal_type: 1 },
        ));
        world.ecs.spawn((
            Position { x: 100.0, y: 100.0 },
            Action::EmitSignal { signal_type: 2 },
        ));

        run(&mut world);

        assert_eq!(world.signals.len(), 2);
        let types: Vec<u8> = world.signals.iter().map(|s| s.signal_type).collect();
        assert!(types.contains(&1));
        assert!(types.contains(&2));
    }
}

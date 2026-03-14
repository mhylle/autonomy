use tracing::trace;

use super::world::SimulationWorld;
use crate::systems;

/// Advance the simulation by one tick.
///
/// Runs all systems in deterministic order. Systems are added as the
/// project progresses through implementation phases.
pub fn tick(world: &mut SimulationWorld) {
    world.tick += 1;
    world.event_log.clear();
    trace!(tick = world.tick, "tick start");

    // Deterministic system execution order:
    crate::environment::climate::run(world);         //  0. climate update
    crate::environment::regrowth::run(world);        //  1. environment
    systems::spatial_rebuild::run(world);            //  (rebuild spatial index)
    systems::perception::run(world);                 //  2. perception
    systems::drives::run(world);                     //  3. drives
    systems::decision::run(world);                   //  4. decision (BT -> Action)
    systems::wander::run(world);                     //  4b. action -> velocity
    systems::movement::run(world);                   //  5. movement
    systems::feeding::run(world);                    //  6. feeding
    //  7. combat       (Phase 3.6+)
    systems::reproduction::run(world);               //  8. reproduction
    //  9. composition  (Phase 4.1+)
    // 10. memory       (Phase 3.2+)
    systems::aging::run(world);                      // 11. aging
    systems::cleanup::run(world);                    // 12. cleanup
    // 13. event_emit   (Phase 1.5+)
    // 14. snapshot     (Phase 3.8+)
    // 15. network_sync (Phase 1.7+)

    trace!(
        tick = world.tick,
        entities = world.entity_count(),
        "tick end"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    #[test]
    fn tick_increments_counter() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        assert_eq!(world.tick, 0);
        tick(&mut world);
        assert_eq!(world.tick, 1);
        tick(&mut world);
        assert_eq!(world.tick, 2);
    }

    #[test]
    fn multiple_ticks_run_without_panic() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        for _ in 0..100 {
            tick(&mut world);
        }
        assert_eq!(world.tick, 100);
    }
}

use crate::core::world::SimulationWorld;
use crate::events::types::{DeathCause, SimEvent};

/// Number of recent ticks to track inter-tribe kills.
const WAR_KILL_WINDOW: u64 = 100;

/// Minimum inter-tribe kills in the window before a war is declared.
const WAR_DECLARE_THRESHOLD: usize = 5;

/// Ticks of no inter-tribe kills before a war is considered over.
const PEACE_TICKS: u64 = 150;

/// Detects, declares, and ends wars between tribes.
///
/// Runs after combat each tick. Scans combat-death events to find
/// cross-tribe kills, records them in a rolling history, and emits
/// `WarDeclared` / `WarEnded` events at the appropriate thresholds.
pub fn run(world: &mut SimulationWorld) {
    let current_tick = world.tick;

    // 1. Collect inter-tribe kills from this tick's event log.
    let kills = collect_inter_tribe_kills(world);

    // 2. Record kills in rolling history.
    for pair in kills {
        world
            .war_kill_history
            .entry(pair)
            .or_default()
            .push_back(current_tick);
    }

    // 3. Prune history older than the kill window.
    let cutoff = current_tick.saturating_sub(WAR_KILL_WINDOW);
    for kills in world.war_kill_history.values_mut() {
        while kills.front().map_or(false, |&t| t < cutoff) {
            kills.pop_front();
        }
    }

    // 4. Declare new wars where kill count has hit the threshold.
    let pairs_over_threshold: Vec<(u64, u64)> = world
        .war_kill_history
        .iter()
        .filter(|(pair, kills)| {
            kills.len() >= WAR_DECLARE_THRESHOLD && !world.active_wars.contains_key(*pair)
        })
        .map(|(&pair, _)| pair)
        .collect();

    let mut war_events: Vec<SimEvent> = Vec::new();

    for pair in pairs_over_threshold {
        world.active_wars.insert(pair, current_tick);
        war_events.push(SimEvent::WarDeclared {
            tribe_a_id: pair.0,
            tribe_b_id: pair.1,
            tick: current_tick,
        });
    }

    // 5. End wars that have had no kills for PEACE_TICKS.
    let wars_to_end: Vec<((u64, u64), u64)> = world
        .active_wars
        .iter()
        .filter(|(pair, _)| {
            let last_kill = world
                .war_kill_history
                .get(pair)
                .and_then(|h| h.back().copied())
                .unwrap_or(0);
            current_tick.saturating_sub(last_kill) >= PEACE_TICKS
        })
        .map(|(&pair, &declared)| (pair, declared))
        .collect();

    for (pair, declared_tick) in wars_to_end {
        world.active_wars.remove(&pair);
        war_events.push(SimEvent::WarEnded {
            tribe_a_id: pair.0,
            tribe_b_id: pair.1,
            started_tick: declared_tick,
            ended_tick: current_tick,
            duration: current_tick.saturating_sub(declared_tick),
        });
    }

    for event in war_events {
        world.event_log.push(event);
    }
}

/// Scan this tick's events for combat kills between different tribes.
///
/// Returns normalized (tribe_a, tribe_b) pairs where tribe_a <= tribe_b.
fn collect_inter_tribe_kills(world: &SimulationWorld) -> Vec<(u64, u64)> {
    let mut result = Vec::new();

    for event in world.event_log.events() {
        if let SimEvent::EntityDied {
            entity_id: victim_id,
            cause: DeathCause::Combat { killer_id },
            ..
        } = event
        {
            let killer_tribe = entity_tribe(world, *killer_id);
            let victim_tribe = entity_tribe(world, *victim_id);

            if let (Some(ta), Some(tb)) = (killer_tribe, victim_tribe) {
                if ta != tb {
                    result.push(normalize(ta, tb));
                }
            }
        }
    }

    result
}

/// Look up the tribe ID of an entity by its id-bits value.
fn entity_tribe(world: &SimulationWorld, entity_id: u64) -> Option<u64> {
    let bits = std::num::NonZeroU64::new(entity_id)?;
    let entity = hecs::Entity::from_bits(bits.get())?;
    world
        .ecs
        .get::<&crate::components::tribe::TribeId>(entity)
        .ok()
        .and_then(|tid| tid.0)
}

/// Return a pair with the smaller id first so HashMap keys are canonical.
fn normalize(a: u64, b: u64) -> (u64, u64) {
    if a <= b { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Manually inject a WarDeclared-worthy kill history for a pair.
    fn inject_kills(world: &mut SimulationWorld, pair: (u64, u64), count: usize) {
        let tick = world.tick;
        let deque = world.war_kill_history.entry(pair).or_default();
        for i in 0..count as u64 {
            deque.push_back(tick.saturating_sub(i));
        }
    }

    #[test]
    fn war_declared_when_kill_threshold_met() {
        let mut world = test_world();
        world.tick = 200;

        let pair = (1u64, 2u64);
        inject_kills(&mut world, pair, WAR_DECLARE_THRESHOLD);

        run(&mut world);

        assert!(
            world.active_wars.contains_key(&pair),
            "war should be declared when threshold is met"
        );

        let war_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::WarDeclared { .. }))
            .collect();
        assert_eq!(war_events.len(), 1, "should emit one WarDeclared event");
    }

    #[test]
    fn war_not_declared_below_threshold() {
        let mut world = test_world();
        world.tick = 200;

        let pair = (1u64, 2u64);
        inject_kills(&mut world, pair, WAR_DECLARE_THRESHOLD - 1);

        run(&mut world);

        assert!(
            !world.active_wars.contains_key(&pair),
            "war should not be declared below threshold"
        );

        let war_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::WarDeclared { .. }))
            .collect();
        assert_eq!(war_events.len(), 0, "should not emit WarDeclared");
    }

    #[test]
    fn war_not_declared_twice() {
        let mut world = test_world();
        world.tick = 200;

        let pair = (1u64, 2u64);
        inject_kills(&mut world, pair, WAR_DECLARE_THRESHOLD);
        world.active_wars.insert(pair, 100); // already at war

        run(&mut world);

        // Should not emit another WarDeclared.
        let war_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::WarDeclared { .. }))
            .collect();
        assert_eq!(war_events.len(), 0, "should not re-declare an active war");
    }

    #[test]
    fn war_ends_after_peace_period() {
        let mut world = test_world();
        world.tick = 500;

        let pair = (1u64, 2u64);
        // War declared at tick 100, last kill at tick 300 (well beyond PEACE_TICKS before 500).
        world.active_wars.insert(pair, 100);
        let deque = world.war_kill_history.entry(pair).or_default();
        deque.push_back(300); // last kill was 200 ticks ago; PEACE_TICKS=150

        run(&mut world);

        assert!(
            !world.active_wars.contains_key(&pair),
            "war should end after peace period"
        );

        let end_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::WarEnded { .. }))
            .collect();
        assert_eq!(end_events.len(), 1, "should emit WarEnded event");

        if let SimEvent::WarEnded {
            tribe_a_id,
            tribe_b_id,
            started_tick,
            ended_tick,
            duration,
        } = &end_events[0]
        {
            assert_eq!(*tribe_a_id, 1);
            assert_eq!(*tribe_b_id, 2);
            assert_eq!(*started_tick, 100);
            assert_eq!(*ended_tick, 500);
            assert_eq!(*duration, 400);
        }
    }

    #[test]
    fn war_does_not_end_with_recent_kills() {
        let mut world = test_world();
        world.tick = 300;

        let pair = (1u64, 2u64);
        world.active_wars.insert(pair, 100);
        // Last kill at tick 290, only 10 ticks ago — well within PEACE_TICKS.
        let deque = world.war_kill_history.entry(pair).or_default();
        deque.push_back(290);

        run(&mut world);

        assert!(
            world.active_wars.contains_key(&pair),
            "war should not end while combat is recent"
        );
    }

    #[test]
    fn old_kills_pruned_from_history() {
        let mut world = test_world();
        world.tick = 500;

        let pair = (1u64, 2u64);
        let deque = world.war_kill_history.entry(pair).or_default();
        // Push kills from tick 1 through 4 (all before cutoff).
        for t in 1..5u64 {
            deque.push_back(t);
        }
        // Push one recent kill.
        deque.push_back(450);

        run(&mut world);

        let remaining = world.war_kill_history.get(&pair).unwrap().len();
        assert_eq!(remaining, 1, "old kills should be pruned, only recent one remains");
    }

    #[test]
    fn normalize_pair_is_canonical() {
        assert_eq!(normalize(3, 1), (1, 3));
        assert_eq!(normalize(1, 3), (1, 3));
        assert_eq!(normalize(5, 5), (5, 5));
    }

    #[test]
    fn no_war_events_when_no_tribes() {
        let mut world = test_world();
        // Inject a combat death event but no TribeId components.
        world.event_log.push(SimEvent::EntityDied {
            entity_id: 99,
            x: 0.0,
            y: 0.0,
            age: 10,
            cause: DeathCause::Combat { killer_id: 1 },
        });

        run(&mut world);

        let war_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::WarDeclared { .. }))
            .collect();
        assert_eq!(war_events.len(), 0, "no war events without tribe membership");
    }
}

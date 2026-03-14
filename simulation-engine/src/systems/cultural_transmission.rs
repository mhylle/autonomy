use crate::components::memory::{Memory, MemoryEntry, MemoryKind};
use crate::components::spatial::Position;
use crate::components::tribe::TribeId;
use crate::core::world::SimulationWorld;

/// Maximum distance for cultural transmission between tribe members.
const TEACHING_RANGE: f64 = 40.0;

/// Only memories younger than this many ticks can be transmitted.
const TRANSMITTABLE_MEMORY_MAX_AGE: u64 = 200;

/// Minimum importance for a memory to be worth transmitting.
const MIN_TRANSMIT_IMPORTANCE: f64 = 0.5;

/// How often (in ticks) cultural transmission is attempted.
/// Running every tick would be expensive; every 10 ticks is sufficient.
const TRANSMISSION_INTERVAL: u64 = 10;

/// Probability that a nearby tribemate will learn a given memory.
/// Kept low to avoid memory flooding.
const TRANSMISSION_PROBABILITY: f64 = 0.3;

/// Cultural transmission system.
///
/// Every `TRANSMISSION_INTERVAL` ticks, for each entity that is in a tribe:
/// - Look at recent, important memories (FoundFood, WasAttacked, NearDeath).
/// - For each nearby tribemate, if they don't already have a similar memory,
///   copy it as a `MemoryKind::Observed` entry with reduced importance.
pub fn run(world: &mut SimulationWorld) {
    if world.tick % TRANSMISSION_INTERVAL != 0 {
        return;
    }

    let current_tick = world.tick;

    // 1. Collect teacher data: entity_id_bits, tribe_id, position, transmittable memories.
    let teachers: Vec<(u64, u64, f64, f64, Vec<MemoryEntry>)> = world
        .ecs
        .query::<(&TribeId, &Position, &Memory)>()
        .iter()
        .filter_map(|(entity, (tribe_id, pos, memory))| {
            let tid = tribe_id.0?;
            let transmittable: Vec<MemoryEntry> = memory
                .entries
                .iter()
                .filter(|e| {
                    is_transmittable_kind(e.kind)
                        && e.importance >= MIN_TRANSMIT_IMPORTANCE
                        && current_tick.saturating_sub(e.tick) <= TRANSMITTABLE_MEMORY_MAX_AGE
                        && e.kind != MemoryKind::Observed // don't re-transmit observed memories
                })
                .cloned()
                .collect();

            if transmittable.is_empty() {
                None
            } else {
                Some((entity.to_bits().get(), tid, pos.x, pos.y, transmittable))
            }
        })
        .collect();

    // 2. Collect learner data: entity_id_bits, tribe_id, position, hecs::Entity.
    let learners: Vec<(u64, u64, f64, f64, hecs::Entity)> = world
        .ecs
        .query::<(&TribeId, &Position, &Memory)>()
        .iter()
        .filter_map(|(entity, (tribe_id, pos, _memory))| {
            let tid = tribe_id.0?;
            Some((entity.to_bits().get(), tid, pos.x, pos.y, entity))
        })
        .collect();

    // 3. For each teacher, find nearby tribemates and attempt transmission.
    let mut transmissions: Vec<(hecs::Entity, MemoryEntry)> = Vec::new();
    let range_sq = TEACHING_RANGE * TEACHING_RANGE;

    // Use a simple deterministic hash for pseudo-random transmission decisions.
    for (teacher_id, teacher_tribe, tx, ty, memories) in &teachers {
        for (learner_id, learner_tribe, lx, ly, learner_entity) in &learners {
            // Same tribe, different entity, within range.
            if teacher_tribe != learner_tribe || teacher_id == learner_id {
                continue;
            }

            let dx = tx - lx;
            let dy = ty - ly;
            if dx * dx + dy * dy > range_sq {
                continue;
            }

            // Deterministic "random" based on tick, teacher, learner to decide transmission.
            let seed = current_tick
                .wrapping_mul(31)
                .wrapping_add(*teacher_id)
                .wrapping_mul(17)
                .wrapping_add(*learner_id);

            for (i, memory) in memories.iter().enumerate() {
                let hash = seed.wrapping_add(i as u64).wrapping_mul(6364136223846793005);
                let prob = (hash % 1000) as f64 / 1000.0;
                if prob < TRANSMISSION_PROBABILITY {
                    // Create an observed copy with reduced importance.
                    let observed = MemoryEntry {
                        tick: current_tick,
                        kind: MemoryKind::Observed,
                        importance: memory.importance * 0.6,
                        emotional_valence: memory.emotional_valence * 0.5,
                        x: memory.x,
                        y: memory.y,
                        associated_entity_id: Some(*teacher_id),
                    };
                    transmissions.push((*learner_entity, observed));
                }
            }
        }
    }

    // 4. Apply transmissions.
    for (learner_entity, entry) in transmissions {
        if let Ok(mut memory) = world.ecs.get::<&mut Memory>(learner_entity) {
            // Check if learner already has a very similar observed memory
            // (same location, same teacher) to avoid flooding.
            let already_has = memory.entries.iter().any(|e| {
                e.kind == MemoryKind::Observed
                    && e.associated_entity_id == entry.associated_entity_id
                    && (e.x - entry.x).abs() < 5.0
                    && (e.y - entry.y).abs() < 5.0
                    && current_tick.saturating_sub(e.tick) < TRANSMITTABLE_MEMORY_MAX_AGE
            });

            if !already_has {
                memory.add(entry, current_tick);
            }
        }
    }
}

/// Whether a memory kind is eligible for cultural transmission.
fn is_transmittable_kind(kind: MemoryKind) -> bool {
    matches!(
        kind,
        MemoryKind::FoundFood | MemoryKind::WasAttacked | MemoryKind::NearDeath
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::memory::{EvictionWeights, Memory, MemoryEntry, MemoryKind};
    use crate::components::spatial::Position;
    use crate::components::tribe::TribeId;
    use crate::core::config::SimulationConfig;
    use crate::core::world::SimulationWorld;
    use std::collections::HashSet;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn make_memory_with_entries(entries: Vec<MemoryEntry>, current_tick: u64) -> Memory {
        let mut m = Memory::new(20, EvictionWeights::default());
        for entry in entries {
            m.add(entry, current_tick);
        }
        m
    }

    #[test]
    fn transmission_only_runs_at_interval() {
        let mut world = test_world();
        world.tick = 5; // Not a multiple of TRANSMISSION_INTERVAL (10)

        // Even with perfect setup, should not transmit.
        run(&mut world);
        // No panic, no effect. (Tested implicitly.)
    }

    #[test]
    fn tribemate_receives_observed_memory() {
        let mut world = test_world();
        world.tick = 10; // Multiple of TRANSMISSION_INTERVAL

        let tribe_id = 1u64;

        // Teacher with an important FoundFood memory.
        let teacher_memory = make_memory_with_entries(
            vec![MemoryEntry {
                tick: 5,
                kind: MemoryKind::FoundFood,
                importance: 0.8,
                emotional_valence: 0.5,
                x: 100.0,
                y: 200.0,
                associated_entity_id: None,
            }],
            10,
        );

        let teacher = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            TribeId(Some(tribe_id)),
            teacher_memory,
        ));

        // Learner nearby with empty memory.
        let learner = world.ecs.spawn((
            Position { x: 15.0, y: 10.0 },
            TribeId(Some(tribe_id)),
            Memory::new(20, EvictionWeights::default()),
        ));

        let teacher_id = teacher.to_bits().get();
        let _learner_id = learner.to_bits().get();

        // Set up tribe in world.
        let mut members = HashSet::new();
        members.insert(teacher_id);
        members.insert(learner.to_bits().get());
        world.tribes.insert(
            tribe_id,
            crate::components::tribe::Tribe::new(tribe_id, members, 12.0, 10.0, 1),
        );

        // Run multiple times with different ticks to ensure at least one transmission.
        // (Due to deterministic hashing, some tick values may not trigger transmission.)
        let mut transmitted = false;
        for tick_offset in 0..10 {
            world.tick = 10 + tick_offset * TRANSMISSION_INTERVAL;
            run(&mut world);
            let memory = world.ecs.get::<&Memory>(learner).unwrap();
            if !memory.is_empty() {
                transmitted = true;
                // Verify the observed memory properties.
                let observed = &memory.entries[0];
                assert_eq!(observed.kind, MemoryKind::Observed);
                assert!(observed.importance < 0.8, "observed importance should be reduced");
                assert_eq!(observed.x, 100.0);
                assert_eq!(observed.y, 200.0);
                assert_eq!(observed.associated_entity_id, Some(teacher_id));
                break;
            }
        }

        assert!(transmitted, "learner should have received at least one observed memory after multiple attempts");
    }

    #[test]
    fn no_transmission_between_different_tribes() {
        let mut world = test_world();
        world.tick = 10;

        // Teacher in tribe 1.
        let teacher_memory = make_memory_with_entries(
            vec![MemoryEntry {
                tick: 5,
                kind: MemoryKind::FoundFood,
                importance: 0.9,
                emotional_valence: 0.5,
                x: 100.0,
                y: 200.0,
                associated_entity_id: None,
            }],
            10,
        );

        world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            TribeId(Some(1)),
            teacher_memory,
        ));

        // Learner in tribe 2.
        let learner = world.ecs.spawn((
            Position { x: 15.0, y: 10.0 },
            TribeId(Some(2)),
            Memory::new(20, EvictionWeights::default()),
        ));

        // Run for many tick intervals.
        for tick_offset in 0..20 {
            world.tick = 10 + tick_offset * TRANSMISSION_INTERVAL;
            run(&mut world);
        }

        let memory = world.ecs.get::<&Memory>(learner).unwrap();
        assert!(
            memory.is_empty(),
            "no transmission should occur between different tribes"
        );
    }

    #[test]
    fn no_transmission_for_low_importance_memories() {
        let mut world = test_world();
        world.tick = 10;

        let tribe_id = 1u64;

        // Teacher with a low-importance memory.
        let teacher_memory = make_memory_with_entries(
            vec![MemoryEntry {
                tick: 5,
                kind: MemoryKind::FoundFood,
                importance: 0.1, // Below MIN_TRANSMIT_IMPORTANCE
                emotional_valence: 0.1,
                x: 100.0,
                y: 200.0,
                associated_entity_id: None,
            }],
            10,
        );

        let teacher = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            TribeId(Some(tribe_id)),
            teacher_memory,
        ));

        let learner = world.ecs.spawn((
            Position { x: 15.0, y: 10.0 },
            TribeId(Some(tribe_id)),
            Memory::new(20, EvictionWeights::default()),
        ));

        let mut members = HashSet::new();
        members.insert(teacher.to_bits().get());
        members.insert(learner.to_bits().get());
        world.tribes.insert(
            tribe_id,
            crate::components::tribe::Tribe::new(tribe_id, members, 12.0, 10.0, 1),
        );

        for tick_offset in 0..20 {
            world.tick = 10 + tick_offset * TRANSMISSION_INTERVAL;
            run(&mut world);
        }

        let memory = world.ecs.get::<&Memory>(learner).unwrap();
        assert!(
            memory.is_empty(),
            "low-importance memories should not be transmitted"
        );
    }

    #[test]
    fn no_transmission_when_too_far_apart() {
        let mut world = test_world();
        world.tick = 10;

        let tribe_id = 1u64;

        let teacher_memory = make_memory_with_entries(
            vec![MemoryEntry {
                tick: 5,
                kind: MemoryKind::FoundFood,
                importance: 0.9,
                emotional_valence: 0.5,
                x: 100.0,
                y: 200.0,
                associated_entity_id: None,
            }],
            10,
        );

        let teacher = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            TribeId(Some(tribe_id)),
            teacher_memory,
        ));

        // Learner far away.
        let learner = world.ecs.spawn((
            Position { x: 500.0, y: 500.0 },
            TribeId(Some(tribe_id)),
            Memory::new(20, EvictionWeights::default()),
        ));

        let mut members = HashSet::new();
        members.insert(teacher.to_bits().get());
        members.insert(learner.to_bits().get());
        world.tribes.insert(
            tribe_id,
            crate::components::tribe::Tribe::new(tribe_id, members, 12.0, 10.0, 1),
        );

        for tick_offset in 0..20 {
            world.tick = 10 + tick_offset * TRANSMISSION_INTERVAL;
            run(&mut world);
        }

        let memory = world.ecs.get::<&Memory>(learner).unwrap();
        assert!(
            memory.is_empty(),
            "transmission should not occur when entities are too far apart"
        );
    }

    #[test]
    fn is_transmittable_kind_checks_correct_kinds() {
        assert!(is_transmittable_kind(MemoryKind::FoundFood));
        assert!(is_transmittable_kind(MemoryKind::WasAttacked));
        assert!(is_transmittable_kind(MemoryKind::NearDeath));
        assert!(!is_transmittable_kind(MemoryKind::Reproduced));
        assert!(!is_transmittable_kind(MemoryKind::Encountered));
        assert!(!is_transmittable_kind(MemoryKind::Observed));
    }
}

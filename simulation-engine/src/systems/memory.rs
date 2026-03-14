use crate::components::memory::{Memory, MemoryEntry, MemoryKind};
use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::events::types::SimEvent;

/// Threshold: if energy falls below this fraction of max, create a NearDeath memory.
const NEAR_DEATH_THRESHOLD: f64 = 0.10;

/// Importance assigned to a NearDeath memory.
const NEAR_DEATH_IMPORTANCE: f64 = 1.0;

/// Emotional valence for NearDeath memories (strongly negative).
const NEAR_DEATH_VALENCE: f64 = -0.9;

/// Maximum energy gain used for normalizing FoundFood importance (0..1).
const MAX_ENERGY_GAIN_FOR_NORMALIZATION: f64 = 50.0;

/// Base emotional valence for eating (positive).
const FOUND_FOOD_VALENCE: f64 = 0.5;

/// Importance assigned to a WasAttacked memory.
const WAS_ATTACKED_IMPORTANCE: f64 = 0.9;

/// Emotional valence for being attacked (strongly negative).
const WAS_ATTACKED_VALENCE: f64 = -0.8;

/// Importance assigned to an AttackedOther memory.
const ATTACKED_OTHER_IMPORTANCE: f64 = 0.5;

/// Emotional valence for attacking another entity (mildly negative).
const ATTACKED_OTHER_VALENCE: f64 = -0.2;

/// Importance assigned to a Reproduced memory.
const REPRODUCED_IMPORTANCE: f64 = 0.8;

/// Emotional valence for reproducing (strongly positive).
const REPRODUCED_VALENCE: f64 = 0.9;

/// Memory formation system.
///
/// Runs after reproduction and before aging. Reads the event log for the
/// current tick and creates appropriate `MemoryEntry` values in each involved
/// entity's `Memory` component. Also checks each entity's energy level and
/// creates a `NearDeath` memory if below the threshold.
pub fn run(world: &mut SimulationWorld) {
    let current_tick = world.tick;

    // 1. Collect memory-forming data from events.
    let pending = collect_memories_from_events(world, current_tick);

    // 2. Apply collected memories to entity Memory components.
    apply_memories(world, &pending, current_tick);

    // 3. Check all entities for near-death condition.
    check_near_death(world, current_tick);
}

/// A pending memory to be applied to an entity.
struct PendingMemory {
    /// The hecs entity handle to receive this memory.
    entity: hecs::Entity,
    /// The memory entry to add.
    entry: MemoryEntry,
}

/// Scan the event log and build a list of memories to create.
fn collect_memories_from_events(
    world: &SimulationWorld,
    current_tick: u64,
) -> Vec<PendingMemory> {
    let mut pending = Vec::new();

    for event in world.event_log.events() {
        match event {
            SimEvent::EntityAte {
                entity_id,
                resource_id: _,
                energy_gained,
            } => {
                if let Some(entity) = find_entity(world, *entity_id) {
                    let (x, y) = entity_position(world, entity);
                    let importance =
                        (*energy_gained / MAX_ENERGY_GAIN_FOR_NORMALIZATION).clamp(0.0, 1.0);
                    pending.push(PendingMemory {
                        entity,
                        entry: MemoryEntry {
                            tick: current_tick,
                            kind: MemoryKind::FoundFood,
                            importance,
                            emotional_valence: FOUND_FOOD_VALENCE,
                            x,
                            y,
                            associated_entity_id: None,
                        },
                    });
                }
            }

            SimEvent::EntityAttacked {
                attacker_id,
                target_id,
                damage: _,
                target_health_remaining: _,
            } => {
                // Target gets a WasAttacked memory.
                if let Some(target_entity) = find_entity(world, *target_id) {
                    let (x, y) = entity_position(world, target_entity);
                    pending.push(PendingMemory {
                        entity: target_entity,
                        entry: MemoryEntry {
                            tick: current_tick,
                            kind: MemoryKind::WasAttacked,
                            importance: WAS_ATTACKED_IMPORTANCE,
                            emotional_valence: WAS_ATTACKED_VALENCE,
                            x,
                            y,
                            associated_entity_id: Some(*attacker_id),
                        },
                    });
                }

                // Attacker gets an AttackedOther memory.
                if let Some(attacker_entity) = find_entity(world, *attacker_id) {
                    let (x, y) = entity_position(world, attacker_entity);
                    pending.push(PendingMemory {
                        entity: attacker_entity,
                        entry: MemoryEntry {
                            tick: current_tick,
                            kind: MemoryKind::AttackedOther,
                            importance: ATTACKED_OTHER_IMPORTANCE,
                            emotional_valence: ATTACKED_OTHER_VALENCE,
                            x,
                            y,
                            associated_entity_id: Some(*target_id),
                        },
                    });
                }
            }

            SimEvent::EntityReproduced {
                parent_id,
                offspring_id,
                x,
                y,
            } => {
                if let Some(parent_entity) = find_entity(world, *parent_id) {
                    pending.push(PendingMemory {
                        entity: parent_entity,
                        entry: MemoryEntry {
                            tick: current_tick,
                            kind: MemoryKind::Reproduced,
                            importance: REPRODUCED_IMPORTANCE,
                            emotional_valence: REPRODUCED_VALENCE,
                            x: *x,
                            y: *y,
                            associated_entity_id: Some(*offspring_id),
                        },
                    });
                }
            }

            // Other event types do not form memories in this phase.
            _ => {}
        }
    }

    pending
}

/// Apply pending memories to entity Memory components.
fn apply_memories(
    world: &mut SimulationWorld,
    pending: &[PendingMemory],
    current_tick: u64,
) {
    for pm in pending {
        if let Ok(mut memory) = world.ecs.get::<&mut Memory>(pm.entity) {
            memory.add(pm.entry.clone(), current_tick);
        }
    }
}

/// Check all entities for near-death energy levels and create NearDeath memories.
fn check_near_death(world: &mut SimulationWorld, current_tick: u64) {
    // Collect entities that are near death.
    let near_death_entities: Vec<(hecs::Entity, f64, f64)> = world
        .ecs
        .query::<(&Energy, &Position)>()
        .iter()
        .filter(|(_, (energy, _))| {
            energy.max > 0.0 && energy.current > 0.0
                && (energy.current / energy.max) < NEAR_DEATH_THRESHOLD
        })
        .map(|(entity, (_, pos))| (entity, pos.x, pos.y))
        .collect();

    // Apply NearDeath memories.
    for (entity, x, y) in near_death_entities {
        if let Ok(mut memory) = world.ecs.get::<&mut Memory>(entity) {
            // Avoid duplicate NearDeath memories for the same tick.
            let already_has = memory
                .entries
                .iter()
                .any(|e| e.kind == MemoryKind::NearDeath && e.tick == current_tick);
            if !already_has {
                memory.add(
                    MemoryEntry {
                        tick: current_tick,
                        kind: MemoryKind::NearDeath,
                        importance: NEAR_DEATH_IMPORTANCE,
                        emotional_valence: NEAR_DEATH_VALENCE,
                        x,
                        y,
                        associated_entity_id: None,
                    },
                    current_tick,
                );
            }
        }
    }
}

/// Find an entity by its `to_bits().get()` ID value.
fn find_entity(world: &SimulationWorld, id: u64) -> Option<hecs::Entity> {
    let bits = std::num::NonZeroU64::new(id)?;
    let entity = hecs::Entity::from_bits(bits.get())?;
    if world.ecs.contains(entity) {
        Some(entity)
    } else {
        None
    }
}

/// Get the position of an entity, defaulting to (0, 0) if not found.
fn entity_position(world: &SimulationWorld, entity: hecs::Entity) -> (f64, f64) {
    world
        .ecs
        .get::<&Position>(entity)
        .map(|p| (p.x, p.y))
        .unwrap_or((0.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::memory::{EvictionWeights, Memory, MemoryKind};
    use crate::components::physical::Energy;
    use crate::components::spatial::Position;
    use crate::core::config::SimulationConfig;
    use crate::events::types::SimEvent;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Spawn a minimal entity with Position, Energy, and Memory components.
    fn spawn_entity(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        energy_current: f64,
        energy_max: f64,
        memory_capacity: usize,
    ) -> hecs::Entity {
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            Memory::new(memory_capacity, EvictionWeights::default()),
        ))
    }

    // -----------------------------------------------------------------------
    // Test 1: EntityAte event creates FoundFood memory
    // -----------------------------------------------------------------------
    #[test]
    fn entity_ate_creates_found_food_memory() {
        let mut world = test_world();
        world.tick = 10;

        let entity = spawn_entity(&mut world, 50.0, 50.0, 80.0, 100.0, 20);
        let entity_id = entity.to_bits().get();

        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 42,
            energy_gained: 25.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory.entries[0].kind, MemoryKind::FoundFood);
        assert_eq!(memory.entries[0].tick, 10);
        assert_eq!(memory.entries[0].x, 50.0);
        assert_eq!(memory.entries[0].y, 50.0);
    }

    // -----------------------------------------------------------------------
    // Test 2: FoundFood importance scales with energy gained
    // -----------------------------------------------------------------------
    #[test]
    fn found_food_importance_scales_with_energy_gained() {
        let mut world = test_world();
        world.tick = 5;

        let entity = spawn_entity(&mut world, 0.0, 0.0, 80.0, 100.0, 20);
        let entity_id = entity.to_bits().get();

        // Small meal.
        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 1,
            energy_gained: 5.0,
        });
        // Large meal.
        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 2,
            energy_gained: 50.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert_eq!(memory.len(), 2);

        let small = &memory.entries[0];
        let large = &memory.entries[1];
        assert!(
            large.importance > small.importance,
            "larger meal should have higher importance: small={}, large={}",
            small.importance,
            large.importance
        );
        // 5/50 = 0.1, 50/50 = 1.0
        assert!((small.importance - 0.1).abs() < f64::EPSILON);
        assert!((large.importance - 1.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Test 3: EntityAttacked creates WasAttacked for target and AttackedOther for attacker
    // -----------------------------------------------------------------------
    #[test]
    fn entity_attacked_creates_memories_for_both_parties() {
        let mut world = test_world();
        world.tick = 20;

        let target = spawn_entity(&mut world, 10.0, 20.0, 80.0, 100.0, 20);
        let attacker = spawn_entity(&mut world, 15.0, 20.0, 90.0, 100.0, 20);
        let target_id = target.to_bits().get();
        let attacker_id = attacker.to_bits().get();

        world.event_log.push(SimEvent::EntityAttacked {
            attacker_id,
            target_id,
            damage: 25.0,
            target_health_remaining: 75.0,
        });

        run(&mut world);

        // Target should have WasAttacked memory.
        let target_memory = world.ecs.get::<&Memory>(target).unwrap();
        assert_eq!(target_memory.len(), 1);
        assert_eq!(target_memory.entries[0].kind, MemoryKind::WasAttacked);
        assert_eq!(
            target_memory.entries[0].associated_entity_id,
            Some(attacker_id)
        );
        assert!(target_memory.entries[0].emotional_valence < 0.0);

        // Attacker should have AttackedOther memory.
        let attacker_memory = world.ecs.get::<&Memory>(attacker).unwrap();
        assert_eq!(attacker_memory.len(), 1);
        assert_eq!(attacker_memory.entries[0].kind, MemoryKind::AttackedOther);
        assert_eq!(
            attacker_memory.entries[0].associated_entity_id,
            Some(target_id)
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: EntityReproduced creates Reproduced memory for parent
    // -----------------------------------------------------------------------
    #[test]
    fn entity_reproduced_creates_reproduced_memory() {
        let mut world = test_world();
        world.tick = 30;

        let parent = spawn_entity(&mut world, 100.0, 200.0, 50.0, 100.0, 20);
        let parent_id = parent.to_bits().get();
        let offspring_id = 9999; // Doesn't need to exist for parent's memory.

        world.event_log.push(SimEvent::EntityReproduced {
            parent_id,
            offspring_id,
            x: 105.0,
            y: 205.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(parent).unwrap();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory.entries[0].kind, MemoryKind::Reproduced);
        assert!(memory.entries[0].emotional_valence > 0.0);
        assert_eq!(
            memory.entries[0].associated_entity_id,
            Some(offspring_id)
        );
        // Location should be the reproduction location from the event.
        assert_eq!(memory.entries[0].x, 105.0);
        assert_eq!(memory.entries[0].y, 205.0);
    }

    // -----------------------------------------------------------------------
    // Test 5: Near-death energy creates NearDeath memory
    // -----------------------------------------------------------------------
    #[test]
    fn near_death_energy_creates_near_death_memory() {
        let mut world = test_world();
        world.tick = 40;

        // Energy at 5% of max (below 10% threshold).
        let entity = spawn_entity(&mut world, 30.0, 40.0, 5.0, 100.0, 20);

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory.entries[0].kind, MemoryKind::NearDeath);
        assert_eq!(memory.entries[0].importance, NEAR_DEATH_IMPORTANCE);
        assert!(memory.entries[0].emotional_valence < 0.0);
        assert_eq!(memory.entries[0].x, 30.0);
        assert_eq!(memory.entries[0].y, 40.0);
    }

    // -----------------------------------------------------------------------
    // Test 6: Entity above threshold does NOT get NearDeath memory
    // -----------------------------------------------------------------------
    #[test]
    fn entity_above_threshold_no_near_death_memory() {
        let mut world = test_world();
        world.tick = 50;

        // Energy at 50% - well above 10% threshold.
        let entity = spawn_entity(&mut world, 10.0, 10.0, 50.0, 100.0, 20);

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert!(
            memory.is_empty(),
            "entity with 50% energy should not get NearDeath memory"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: Memory eviction when at capacity (add new, lowest-scored evicted)
    // -----------------------------------------------------------------------
    #[test]
    fn memory_eviction_when_at_capacity() {
        let mut world = test_world();
        world.tick = 100;

        // Entity with memory capacity of 3.
        let entity = spawn_entity(&mut world, 50.0, 50.0, 80.0, 100.0, 3);
        let entity_id = entity.to_bits().get();

        // Pre-fill memory with 3 entries of varying quality.
        {
            let mut memory = world.ecs.get::<&mut Memory>(entity).unwrap();
            // Old, low importance, low emotion -> should be evicted.
            memory.add(
                MemoryEntry {
                    tick: 1,
                    kind: MemoryKind::FoundFood,
                    importance: 0.0,
                    emotional_valence: 0.0,
                    x: 0.0,
                    y: 0.0,
                    associated_entity_id: None,
                },
                100,
            );
            // Recent, high importance.
            memory.add(
                MemoryEntry {
                    tick: 98,
                    kind: MemoryKind::NearDeath,
                    importance: 1.0,
                    emotional_valence: -0.9,
                    x: 0.0,
                    y: 0.0,
                    associated_entity_id: None,
                },
                100,
            );
            // Recent, moderate importance.
            memory.add(
                MemoryEntry {
                    tick: 99,
                    kind: MemoryKind::Reproduced,
                    importance: 0.8,
                    emotional_valence: 0.9,
                    x: 0.0,
                    y: 0.0,
                    associated_entity_id: None,
                },
                100,
            );
            assert_eq!(memory.len(), 3);
        }

        // Now trigger a new memory via an event.
        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 10,
            energy_gained: 25.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert_eq!(
            memory.len(),
            3,
            "memory should stay at capacity after eviction"
        );

        // The old FoundFood entry (tick=1, importance=0) should have been evicted.
        let old_food = memory
            .entries
            .iter()
            .find(|e| e.tick == 1 && e.kind == MemoryKind::FoundFood);
        assert!(
            old_food.is_none(),
            "lowest-scored entry (old, unimportant) should have been evicted"
        );

        // The new FoundFood entry (tick=100) should exist.
        let new_food = memory
            .entries
            .iter()
            .find(|e| e.tick == 100 && e.kind == MemoryKind::FoundFood);
        assert!(new_food.is_some(), "new FoundFood memory should be present");
    }

    // -----------------------------------------------------------------------
    // Test 8: Multiple events in same tick create multiple memories
    // -----------------------------------------------------------------------
    #[test]
    fn multiple_events_create_multiple_memories() {
        let mut world = test_world();
        world.tick = 15;

        let entity = spawn_entity(&mut world, 25.0, 35.0, 80.0, 100.0, 20);
        let entity_id = entity.to_bits().get();

        // Entity eats and reproduces in the same tick.
        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 1,
            energy_gained: 20.0,
        });
        world.event_log.push(SimEvent::EntityReproduced {
            parent_id: entity_id,
            offspring_id: 9999,
            x: 25.0,
            y: 35.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert_eq!(memory.len(), 2);

        let kinds: Vec<MemoryKind> = memory.entries.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&MemoryKind::FoundFood));
        assert!(kinds.contains(&MemoryKind::Reproduced));
    }

    // -----------------------------------------------------------------------
    // Test 9: Entity without Memory component is gracefully skipped
    // -----------------------------------------------------------------------
    #[test]
    fn entity_without_memory_component_is_skipped() {
        let mut world = test_world();
        world.tick = 5;

        // Spawn entity with Position and Energy but NO Memory.
        let entity = world.ecs.spawn((
            Position { x: 10.0, y: 10.0, z: 0.0 },
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
        ));
        let entity_id = entity.to_bits().get();

        world.event_log.push(SimEvent::EntityAte {
            entity_id,
            resource_id: 1,
            energy_gained: 20.0,
        });

        // Should not panic.
        run(&mut world);
    }

    // -----------------------------------------------------------------------
    // Test 10: No events and healthy entity produces no memories
    // -----------------------------------------------------------------------
    #[test]
    fn no_events_healthy_entity_no_memories() {
        let mut world = test_world();
        world.tick = 1;

        let entity = spawn_entity(&mut world, 10.0, 10.0, 80.0, 100.0, 20);

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert!(memory.is_empty());
    }

    // -----------------------------------------------------------------------
    // Test 11: NearDeath not duplicated for same tick
    // -----------------------------------------------------------------------
    #[test]
    fn near_death_not_duplicated_same_tick() {
        let mut world = test_world();
        world.tick = 60;

        // Entity near death.
        let entity = spawn_entity(&mut world, 10.0, 10.0, 3.0, 100.0, 20);

        // Run twice in the same tick (simulating the system being called).
        run(&mut world);
        // Manually call check_near_death again.
        check_near_death(&mut world, 60);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        let near_death_count = memory
            .entries
            .iter()
            .filter(|e| e.kind == MemoryKind::NearDeath && e.tick == 60)
            .count();
        assert_eq!(
            near_death_count, 1,
            "should not duplicate NearDeath memory for same tick"
        );
    }

    // -----------------------------------------------------------------------
    // Test 12: Dead entity (energy=0) does not get NearDeath memory
    // -----------------------------------------------------------------------
    #[test]
    fn dead_entity_no_near_death_memory() {
        let mut world = test_world();
        world.tick = 70;

        // Entity with zero energy (already dead).
        let entity = spawn_entity(&mut world, 10.0, 10.0, 0.0, 100.0, 20);

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(entity).unwrap();
        assert!(
            memory.is_empty(),
            "dead entity (energy=0) should not get NearDeath memory"
        );
    }

    // -----------------------------------------------------------------------
    // Test 13: WasAttacked memory has high importance and negative valence
    // -----------------------------------------------------------------------
    #[test]
    fn was_attacked_memory_has_correct_properties() {
        let mut world = test_world();
        world.tick = 25;

        let target = spawn_entity(&mut world, 10.0, 10.0, 80.0, 100.0, 20);
        let attacker = spawn_entity(&mut world, 15.0, 10.0, 80.0, 100.0, 20);
        let target_id = target.to_bits().get();
        let attacker_id = attacker.to_bits().get();

        world.event_log.push(SimEvent::EntityAttacked {
            attacker_id,
            target_id,
            damage: 30.0,
            target_health_remaining: 70.0,
        });

        run(&mut world);

        let memory = world.ecs.get::<&Memory>(target).unwrap();
        let attacked_mem = &memory.entries[0];
        assert_eq!(attacked_mem.kind, MemoryKind::WasAttacked);
        assert!(
            attacked_mem.importance >= 0.8,
            "WasAttacked should have high importance, got {}",
            attacked_mem.importance
        );
        assert!(
            attacked_mem.emotional_valence < -0.5,
            "WasAttacked should have strongly negative valence, got {}",
            attacked_mem.emotional_valence
        );
    }
}

use crate::components::action::Action;
use crate::components::behavior_tree::{
    tick_bt, BtAction, BtContext, BtNode, MemoryContextEntry, PerceivedSignalInfo,
    SocialEntityInfo,
};
use crate::components::drives::Drives;
use crate::components::genome::Genome;
use crate::components::memory::{Memory, MemoryKind};
use crate::components::perception::Perception;
use crate::components::physical::Energy;
use crate::components::social::Social;
use crate::components::spatial::Position;
use crate::components::world_object::Inventory;
use crate::core::world::SimulationWorld;

/// Ticks each entity's behavior tree and produces an `Action` component.
///
/// Builds a `BtContext` from the entity's current state, evaluates the BT,
/// and converts the resulting `BtAction` into a concrete `Action` with
/// world-space coordinates (e.g. MoveTowardResource -> MoveTo closest resource).
pub fn run(world: &mut SimulationWorld) {
    let mut rng = world.rng.tick_rng("decision", world.tick);

    let current_tick = world.tick;

    // Collect entity state for BT evaluation.
    let evaluations: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy, &Drives, &Perception, &BtNode)>()
        .iter()
        .map(|(entity, (pos, energy, drives, perception, bt))| {
            let energy_fraction = if energy.max > 0.0 {
                energy.current / energy.max
            } else {
                0.0
            };

            let closest_res = perception.closest_resource();
            let has_nearby_resource = closest_res.is_some();
            let closest_resource_distance = closest_res
                .map(|r| r.distance)
                .unwrap_or(f64::MAX);

            let closest_ent = perception.closest_entity();
            let has_nearby_entity = closest_ent.is_some();
            let closest_entity_distance = closest_ent
                .map(|e| e.distance)
                .unwrap_or(f64::MAX);

            // Build memory context if Memory component is present.
            let (has_food_memory, food_memory_location, has_threat_memory, threat_memory_location, was_attacked_count, memory_entries) =
                if let Ok(memory) = world.ecs.get::<&Memory>(entity) {
                    build_memory_context(&memory, current_tick)
                } else {
                    (false, None, false, None, 0, vec![])
                };

            // Build social entity info from perception + social relationships.
            let social_entities = build_social_entities(perception, &world.ecs, entity);

            let ctx = BtContext {
                hunger: drives.hunger,
                fear: drives.fear,
                curiosity: drives.curiosity,
                social_need: drives.social_need,
                aggression: drives.aggression,
                reproductive_urge: drives.reproductive_urge,
                energy_fraction,
                has_nearby_resource,
                closest_resource_distance,
                has_nearby_entity,
                closest_entity_distance,
                social_entities,
                has_food_memory,
                food_memory_location,
                has_threat_memory,
                threat_memory_location,
                was_attacked_count,
                current_tick,
                memory_entries,
                perceived_signals: build_signal_context(perception),
                has_nearby_object: perception.closest_object().is_some(),
                nearest_object_id: perception.closest_object().map(|o| o.object_id),
                has_object_in_inventory: world
                    .ecs
                    .get::<&Inventory>(entity)
                    .map(|inv| !inv.items.is_empty())
                    .unwrap_or(false),
                first_inventory_item: world
                    .ecs
                    .get::<&Inventory>(entity)
                    .ok()
                    .and_then(|inv| inv.items.first().copied()),
                can_create_object: world
                    .ecs
                    .get::<&Genome>(entity)
                    .map(|g| energy.current >= g.blueprint.energy_cost)
                    .unwrap_or(false),
            };

            let (_status, bt_action) = tick_bt(bt, &ctx);

            // Convert BtAction to Action with concrete coordinates.
            let action = match bt_action {
                BtAction::MoveTowardResource { speed_factor } => {
                    if let Some(res) = closest_res {
                        Action::MoveTo {
                            x: res.x,
                            y: res.y,
                            speed: speed_factor * 2.0, // base speed * factor
                        }
                    } else {
                        Action::Wander { speed: 1.5 }
                    }
                }
                BtAction::Wander { speed } => Action::Wander { speed },
                BtAction::Eat => Action::Eat,
                BtAction::Rest => Action::Rest,
                BtAction::Attack { force_factor } => {
                    if let Some(ent) = closest_ent {
                        Action::Attack {
                            target_id: ent.entity_id,
                            force: force_factor,
                        }
                    } else {
                        Action::Wander { speed: 1.5 }
                    }
                }
                BtAction::MoveTowardMemory { x, y, speed_factor } => {
                    Action::MoveTo {
                        x,
                        y,
                        speed: speed_factor * 2.0,
                    }
                }
                BtAction::FleeFrom { x: fx, y: fy, speed_factor } => {
                    // Flee: move away from (fx, fy) - compute opposite direction.
                    let dx = pos.x - fx;
                    let dy = pos.y - fy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.01 {
                        Action::MoveDirection {
                            dx: dx / dist,
                            dy: dy / dist,
                            speed: speed_factor * 2.0,
                        }
                    } else {
                        Action::Wander { speed: speed_factor * 2.0 }
                    }
                }
                BtAction::MoveTowardEntity { x, y, speed_factor, .. } => {
                    Action::MoveTo {
                        x,
                        y,
                        speed: speed_factor * 2.0,
                    }
                }
                BtAction::FleeFromEntity { x: fx, y: fy, speed_factor, .. } => {
                    let dx = pos.x - fx;
                    let dy = pos.y - fy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.01 {
                        Action::MoveDirection {
                            dx: dx / dist,
                            dy: dy / dist,
                            speed: speed_factor * 2.0,
                        }
                    } else {
                        Action::Wander { speed: speed_factor * 2.0 }
                    }
                }
                BtAction::CompositionAttempt => Action::CompositionAttempt,
                BtAction::EmitSignal { signal_type } => {
                    Action::EmitSignal { signal_type }
                }
                BtAction::MoveTowardSignal { x, y, speed_factor } => {
                    Action::MoveTowardSignal {
                        x,
                        y,
                        speed: speed_factor * 2.0,
                    }
                }
                BtAction::PickUp { object_id } => Action::PickUp { object_id },
                BtAction::Drop { object_id } => Action::Drop { object_id },
                BtAction::CreateObject => Action::CreateObject,
                BtAction::None => Action::None,
            };

            (entity, action, pos.x, pos.y)
        })
        .collect();

    // Apply actions.
    for (entity, action, _x, _y) in evaluations {
        if let Ok(mut a) = world.ecs.get::<&mut Action>(entity) {
            *a = action;
        }
    }

    // For entities with Action but no BT, set Wander as fallback.
    let no_bt: Vec<_> = world
        .ecs
        .query::<&Action>()
        .without::<&BtNode>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in no_bt {
        if let Ok(mut a) = world.ecs.get::<&mut Action>(entity) {
            let speed: f64 = rand::Rng::gen_range(&mut rng, 1.0..2.0);
            *a = Action::Wander { speed };
        }
    }
}

/// Memory recall window used for BtContext construction.
const MEMORY_RECALL_MAX_AGE: u64 = 500;

/// Build memory-related context fields from an entity's Memory component.
fn build_memory_context(
    memory: &Memory,
    current_tick: u64,
) -> (bool, Option<(f64, f64)>, bool, Option<(f64, f64)>, u32, Vec<MemoryContextEntry>) {
    let food_memories = memory.recall(MemoryKind::FoundFood, MEMORY_RECALL_MAX_AGE, current_tick);
    let has_food_memory = !food_memories.is_empty();
    let food_memory_location = food_memories.first().map(|e| (e.x, e.y));

    let threat_memories = memory.recall(MemoryKind::WasAttacked, MEMORY_RECALL_MAX_AGE, current_tick);
    let has_threat_memory = !threat_memories.is_empty();
    let threat_memory_location = threat_memories.first().map(|e| (e.x, e.y));
    let was_attacked_count = threat_memories.len() as u32;

    let all_memories = memory.recall_all(MEMORY_RECALL_MAX_AGE, current_tick);
    let memory_entries: Vec<MemoryContextEntry> = all_memories
        .iter()
        .map(|e| MemoryContextEntry {
            kind: e.kind,
            x: e.x,
            y: e.y,
            tick: e.tick,
            emotional_valence: e.emotional_valence,
        })
        .collect();

    (has_food_memory, food_memory_location, has_threat_memory, threat_memory_location, was_attacked_count, memory_entries)
}

/// Build signal context info from perceived signals for BT evaluation.
fn build_signal_context(perception: &Perception) -> Vec<PerceivedSignalInfo> {
    perception
        .perceived_signals
        .iter()
        .map(|ps| PerceivedSignalInfo {
            signal_type: ps.signal_type,
            distance: ps.distance,
            direction_x: ps.direction_x,
            direction_y: ps.direction_y,
            strength: ps.strength,
            source_x: ps.source_x,
            source_y: ps.source_y,
        })
        .collect()
}

/// Build social entity info from perceived entities and the Social component.
///
/// For each perceived entity, looks up the relationship score from the
/// perceiver's Social component. Falls back to 0.0 if no Social component exists.
fn build_social_entities(
    perception: &Perception,
    ecs: &hecs::World,
    entity: hecs::Entity,
) -> Vec<SocialEntityInfo> {
    let social = ecs.get::<&Social>(entity).ok();
    perception
        .perceived_entities
        .iter()
        .map(|pe| {
            let relationship = social
                .as_ref()
                .map(|s| s.get_relationship(pe.entity_id))
                .unwrap_or(0.0);
            SocialEntityInfo {
                entity_id: pe.entity_id,
                x: pe.x,
                y: pe.y,
                distance: pe.distance,
                is_kin: pe.is_kin,
                relationship,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::behavior_tree::{default_starter_bt, BtNode};
    use crate::components::drives::Drives;
    use crate::components::perception::{PerceivedResource, Perception};
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn spawn_bt_entity(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        bt: BtNode,
        perception: Perception,
    ) -> hecs::Entity {
        let drives = Drives {
            hunger: 1.0 - (energy_current / energy_max),
            ..Drives::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            drives,
            perception,
            bt,
            Action::None,
        ))
    }

    #[test]
    fn hungry_entity_with_food_seeks_food() {
        let mut world = test_world();
        let perception = Perception {
            perceived_entities: vec![],
            perceived_resources: vec![PerceivedResource {
                resource_index: 0,
                x: 80.0,
                y: 50.0,
                distance: 30.0,
            }],
            perceived_signals: vec![],
            perceived_objects: vec![],
        };
        let e = spawn_bt_entity(&mut world, 20.0, 100.0, default_starter_bt(), perception);

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveTo { x, y, .. } => {
                assert_eq!(*x, 80.0);
                assert_eq!(*y, 50.0);
            }
            Action::Eat => {} // also valid (if already adjacent)
            other => panic!("expected MoveTo or Eat, got {:?}", other),
        }
    }

    #[test]
    fn full_entity_wanders() {
        let mut world = test_world();
        let e = spawn_bt_entity(
            &mut world,
            95.0,
            100.0,
            default_starter_bt(),
            Perception::default(),
        );

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::Wander { speed } => {
                assert_eq!(*speed, 1.5);
            }
            other => panic!("expected Wander, got {:?}", other),
        }
    }

    #[test]
    fn entity_with_rest_bt_rests() {
        let mut world = test_world();
        let bt = BtNode::Rest;
        let e = spawn_bt_entity(&mut world, 50.0, 100.0, bt, Perception::default());

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        assert_eq!(*action, Action::Rest);
    }

    #[test]
    fn social_entity_with_kin_nearby_moves_toward_kin() {
        use crate::components::behavior_tree::social_starter_bt;
        use crate::components::perception::PerceivedEntity;

        let mut world = test_world();
        let perception = Perception {
            perceived_entities: vec![PerceivedEntity {
                entity_id: 999,
                x: 80.0,
                y: 50.0,
                distance: 30.0,
                energy_estimate: 50.0,
                is_kin: true,
            }],
            perceived_resources: vec![],
            perceived_signals: vec![],
            perceived_objects: vec![],
        };
        let drives = Drives {
            social_need: 0.7,
            ..Drives::default()
        };
        let e = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            drives,
            perception,
            social_starter_bt(),
            Action::None,
        ));

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveTo { x, y, .. } => {
                assert_eq!(*x, 80.0);
                assert_eq!(*y, 50.0);
            }
            other => panic!("expected MoveTo toward kin, got {:?}", other),
        }
    }

    #[test]
    fn social_entity_flees_from_negative_relationship() {
        use crate::components::behavior_tree::social_starter_bt;
        use crate::components::perception::PerceivedEntity;

        let mut world = test_world();
        let perception = Perception {
            perceived_entities: vec![PerceivedEntity {
                entity_id: 888,
                x: 80.0,
                y: 50.0,
                distance: 20.0,
                energy_estimate: 50.0,
                is_kin: false,
            }],
            perceived_resources: vec![],
            perceived_signals: vec![],
            perceived_objects: vec![],
        };
        // Give the entity a negative relationship with entity 888.
        let mut social = Social::default();
        social.record_interaction(888, -1.0, None);
        social.record_interaction(888, -1.0, None);
        social.record_interaction(888, -1.0, None);

        let e = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Drives::default(),
            perception,
            social_starter_bt(),
            Action::None,
            social,
        ));

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveDirection { dx, .. } => {
                // Should flee LEFT (away from 80,50)
                assert!(*dx < 0.0, "should move away from threat, got dx={}", dx);
            }
            other => panic!("expected MoveDirection (flee), got {:?}", other),
        }
    }

    #[test]
    fn action_updates_each_tick() {
        let mut world = test_world();
        let e = spawn_bt_entity(
            &mut world,
            95.0,
            100.0,
            default_starter_bt(),
            Perception::default(),
        );

        run(&mut world);
        let action1 = (*world.ecs.get::<&Action>(e).unwrap()).clone();

        // Make entity hungry.
        world.ecs.get::<&mut Energy>(e).unwrap().current = 20.0;
        world.ecs.get::<&mut Drives>(e).unwrap().hunger = 0.8;

        run(&mut world);
        let action2 = (*world.ecs.get::<&Action>(e).unwrap()).clone();

        // First tick should be Wander, second should still be Wander
        // (no food perceived), but action should have been re-evaluated.
        assert_eq!(action1, Action::Wander { speed: 1.5 });
        assert_eq!(action2, Action::Wander { speed: 1.5 });
    }

    // ---- Phase 3.3: Memory-Behavior Integration decision system tests ----

    use crate::components::behavior_tree::memory_enhanced_starter_bt;
    use crate::components::memory::{EvictionWeights, MemoryEntry};

    /// Helper to spawn an entity with memory-enhanced BT and a Memory component.
    fn spawn_memory_entity(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        bt: BtNode,
        perception: Perception,
        memory: Memory,
    ) -> hecs::Entity {
        let drives = Drives {
            hunger: 1.0 - (energy_current / energy_max),
            ..Drives::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            drives,
            perception,
            bt,
            Action::None,
            memory,
        ))
    }

    #[test]
    fn hungry_entity_with_food_memory_moves_toward_remembered_food() {
        let mut world = test_world();
        world.tick = 200;

        let mut memory = Memory::new(10, EvictionWeights::default());
        memory.add(
            MemoryEntry {
                tick: 150,
                kind: MemoryKind::FoundFood,
                importance: 0.8,
                emotional_valence: 0.5,
                x: 120.0,
                y: 80.0,
                associated_entity_id: None,
            },
            150,
        );

        let e = spawn_memory_entity(
            &mut world,
            20.0, // low energy -> high hunger
            100.0,
            memory_enhanced_starter_bt(),
            Perception::default(), // no visible food
            memory,
        );

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveTo { x, y, .. } => {
                assert_eq!(*x, 120.0);
                assert_eq!(*y, 80.0);
            }
            other => panic!(
                "expected MoveTo toward remembered food location, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn entity_without_memory_component_still_works() {
        // Entities without Memory should still function normally.
        let mut world = test_world();
        let e = spawn_bt_entity(
            &mut world,
            20.0,
            100.0,
            memory_enhanced_starter_bt(),
            Perception::default(),
        );

        run(&mut world);

        // No food visible and no memory -> should wander.
        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::Wander { .. } => {}
            other => panic!("expected Wander (no memory, no food), got {:?}", other),
        }
    }

    #[test]
    fn entity_prefers_visible_food_over_memory() {
        // When food is both visible and remembered, the entity should
        // prefer direct perception (first branch in the selector).
        let mut world = test_world();
        world.tick = 200;

        let mut memory = Memory::new(10, EvictionWeights::default());
        memory.add(
            MemoryEntry {
                tick: 100,
                kind: MemoryKind::FoundFood,
                importance: 0.8,
                emotional_valence: 0.5,
                x: 200.0,
                y: 200.0,
                associated_entity_id: None,
            },
            100,
        );

        let perception = Perception {
            perceived_entities: vec![],
            perceived_resources: vec![PerceivedResource {
                resource_index: 0,
                x: 60.0,
                y: 55.0,
                distance: 11.0,
            }],
            perceived_signals: vec![],
            perceived_objects: vec![],
        };

        let e = spawn_memory_entity(
            &mut world,
            20.0,
            100.0,
            memory_enhanced_starter_bt(),
            perception,
            memory,
        );

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::Eat | Action::MoveTo { x: 60.0, .. } => {}
            Action::MoveTo { x, .. } => {
                // Should move toward the visible food (60, 55), not the remembered one (200, 200).
                assert!(
                    (*x - 60.0).abs() < 1.0,
                    "should prefer visible food at 60, got x={}",
                    x
                );
            }
            other => panic!("expected MoveTo toward visible food, got {:?}", other),
        }
    }

    #[test]
    fn fear_drive_boosted_by_was_attacked_memories() {
        // Verify that build_memory_context correctly counts WasAttacked memories.
        let mut memory = Memory::new(10, EvictionWeights::default());
        for i in 0..5 {
            memory.add(
                MemoryEntry {
                    tick: 100 + i,
                    kind: MemoryKind::WasAttacked,
                    importance: 0.9,
                    emotional_valence: -0.8,
                    x: 30.0,
                    y: 40.0,
                    associated_entity_id: Some(42),
                },
                100 + i,
            );
        }
        let (_, _, has_threat, _, was_attacked_count, _) =
            build_memory_context(&memory, 200);
        assert!(has_threat);
        assert_eq!(was_attacked_count, 5);
    }

    #[test]
    fn build_memory_context_with_no_memories() {
        let memory = Memory::new(10, EvictionWeights::default());
        let (has_food, food_loc, has_threat, threat_loc, attacked_count, entries) =
            build_memory_context(&memory, 100);
        assert!(!has_food);
        assert!(food_loc.is_none());
        assert!(!has_threat);
        assert!(threat_loc.is_none());
        assert_eq!(attacked_count, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn build_memory_context_returns_most_recent_food_location() {
        let mut memory = Memory::new(10, EvictionWeights::default());
        memory.add(
            MemoryEntry {
                tick: 50,
                kind: MemoryKind::FoundFood,
                importance: 0.5,
                emotional_valence: 0.3,
                x: 10.0,
                y: 20.0,
                associated_entity_id: None,
            },
            50,
        );
        memory.add(
            MemoryEntry {
                tick: 90,
                kind: MemoryKind::FoundFood,
                importance: 0.7,
                emotional_valence: 0.5,
                x: 80.0,
                y: 90.0,
                associated_entity_id: None,
            },
            90,
        );

        let (has_food, food_loc, _, _, _, _) = build_memory_context(&memory, 100);
        assert!(has_food);
        // recall() returns most recent first, so food_loc should be (80, 90).
        assert_eq!(food_loc, Some((80.0, 90.0)));
    }

    #[test]
    fn flee_from_memory_produces_move_direction_away() {
        use crate::components::behavior_tree::{
            BtNode, MemoryKindFilter,
        };

        let mut world = test_world();
        world.tick = 200;

        // Create a BT that flees from threat memory.
        let bt = BtNode::Sequence(vec![
            BtNode::RecallMemory {
                kind: MemoryKindFilter::AnyThreat,
                max_age: 500,
            },
            BtNode::FleeFromMemory {
                kind: MemoryKindFilter::AnyThreat,
                speed_factor: 2.0,
            },
        ]);

        let mut memory = Memory::new(10, EvictionWeights::default());
        memory.add(
            MemoryEntry {
                tick: 180,
                kind: MemoryKind::WasAttacked,
                importance: 0.9,
                emotional_valence: -0.9,
                x: 80.0, // Threat was to the right (+x direction)
                y: 50.0,
                associated_entity_id: None,
            },
            180,
        );

        let e = spawn_memory_entity(
            &mut world,
            80.0,
            100.0,
            bt,
            Perception::default(),
            memory,
        );

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveDirection { dx, dy, .. } => {
                // Entity at (50,50), threat at (80,50) -> should flee left (negative dx).
                assert!(
                    *dx < 0.0,
                    "should flee away from threat (left), got dx={}",
                    dx
                );
                assert!(
                    dy.abs() < 0.1,
                    "dy should be ~0 (threat is on same y), got dy={}",
                    dy
                );
            }
            other => panic!("expected MoveDirection (flee from memory), got {:?}", other),
        }
    }
}

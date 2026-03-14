//! Object system: handles decay, pickup, drop, creation, and tool use.
//!
//! Phases 6.1-6.4:
//! - Object decay: durability decreases each tick, objects removed at 0
//! - Pickup/Drop: entities interact with nearby ground objects
//! - Creation: entities spend energy to create objects from blueprint
//! - Tool use: equipped tools modify entity combat stats

use crate::components::action::Action;
use crate::components::genome::Genome;
use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::components::world_object::Inventory;
use crate::core::world::SimulationWorld;

/// Maximum distance at which an entity can pick up an object.
const PICKUP_RANGE: f64 = 15.0;

/// Per-use wear applied to an equipped tool when used in combat.
const TOOL_USE_WEAR: f64 = 0.5;

/// Speed penalty multiplier per unit of carried weight.
/// Movement speed is multiplied by (1.0 - total_weight * CARRY_SPEED_PENALTY).
const CARRY_SPEED_PENALTY: f64 = 0.15;

/// Run the object system for one tick.
///
/// Execution order:
/// 1. Decay all objects, remove destroyed ones
/// 2. Process PickUp actions
/// 3. Process Drop actions
/// 4. Process CreateObject actions
pub fn run(world: &mut SimulationWorld) {
    run_decay(world);
    run_pickup(world);
    run_drop(world);
    run_create(world);
}

/// Phase 6.1: Decay all objects. Remove objects with durability <= 0.
/// Also unlink destroyed objects from any inventory that holds them.
fn run_decay(world: &mut SimulationWorld) {
    // Apply decay to all objects.
    for obj in &mut world.objects {
        obj.apply_decay();
    }

    // Collect IDs of destroyed objects.
    let destroyed_ids: Vec<u64> = world
        .objects
        .iter()
        .filter(|o| !o.is_intact())
        .map(|o| o.id)
        .collect();

    // Remove destroyed objects from inventories.
    if !destroyed_ids.is_empty() {
        let entities_with_inv: Vec<hecs::Entity> = world
            .ecs
            .query::<&Inventory>()
            .iter()
            .map(|(e, _)| e)
            .collect();

        for entity in entities_with_inv {
            if let Ok(mut inv) = world.ecs.get::<&mut Inventory>(entity) {
                for &id in &destroyed_ids {
                    inv.remove(id);
                }
            }
        }
    }

    // Remove destroyed objects from the world.
    world.objects.retain(|o| o.is_intact());
}

/// Phase 6.2: Process PickUp actions.
fn run_pickup(world: &mut SimulationWorld) {
    let pickups: Vec<(hecs::Entity, u64, f64, f64)> = world
        .ecs
        .query::<(&Action, &Position)>()
        .iter()
        .filter_map(|(entity, (action, pos))| {
            if let Action::PickUp { object_id } = action {
                Some((entity, *object_id, pos.x, pos.y))
            } else {
                None
            }
        })
        .collect();

    for (entity, object_id, ex, ey) in pickups {
        // Check if entity has inventory with room.
        let has_room = world
            .ecs
            .get::<&Inventory>(entity)
            .map(|inv| inv.has_room())
            .unwrap_or(false);

        if !has_room {
            continue;
        }

        // Check if object exists, is on the ground, and within range.
        let obj_valid = world.objects.iter().any(|o| {
            o.id == object_id && o.is_on_ground() && o.is_intact() && {
                let dx = o.x - ex;
                let dy = o.y - ey;
                (dx * dx + dy * dy).sqrt() <= PICKUP_RANGE
            }
        });

        if !obj_valid {
            continue;
        }

        let entity_id = entity.to_bits().get();

        // Add to inventory.
        if let Ok(mut inv) = world.ecs.get::<&mut Inventory>(entity) {
            if inv.add(object_id) {
                // Mark object as held.
                if let Some(obj) = world.objects.iter_mut().find(|o| o.id == object_id) {
                    obj.held_by = Some(entity_id);
                }
            }
        }
    }
}

/// Phase 6.2: Process Drop actions.
fn run_drop(world: &mut SimulationWorld) {
    let drops: Vec<(hecs::Entity, u64, f64, f64)> = world
        .ecs
        .query::<(&Action, &Position)>()
        .iter()
        .filter_map(|(entity, (action, pos))| {
            if let Action::Drop { object_id } = action {
                Some((entity, *object_id, pos.x, pos.y))
            } else {
                None
            }
        })
        .collect();

    for (entity, object_id, ex, ey) in drops {
        // Remove from inventory.
        let removed = world
            .ecs
            .get::<&mut Inventory>(entity)
            .map(|mut inv| inv.remove(object_id))
            .unwrap_or(false);

        if removed {
            // Update object position and clear held_by.
            if let Some(obj) = world.objects.iter_mut().find(|o| o.id == object_id) {
                obj.x = ex;
                obj.y = ey;
                obj.held_by = None;
            }
        }
    }
}

/// Phase 6.3: Process CreateObject actions.
fn run_create(world: &mut SimulationWorld) {
    let creators: Vec<(hecs::Entity, f64, f64, f64, crate::components::world_object::Blueprint)> = world
        .ecs
        .query::<(&Action, &Position, &Energy, &Genome)>()
        .iter()
        .filter_map(|(entity, (action, pos, energy, genome))| {
            if *action == Action::CreateObject && energy.current >= genome.blueprint.energy_cost {
                Some((entity, pos.x, pos.y, genome.blueprint.energy_cost, genome.blueprint.clone()))
            } else {
                None
            }
        })
        .collect();

    for (entity, x, y, energy_cost, blueprint) in creators {
        let entity_id = entity.to_bits().get();
        let object_id = world.next_object_id;
        world.next_object_id += 1;

        let obj = blueprint.create_object(object_id, x, y, entity_id, world.tick);
        world.objects.push(obj);

        // Deduct energy.
        if let Ok(mut energy) = world.ecs.get::<&mut Energy>(entity) {
            energy.current = (energy.current - energy_cost).max(0.0);
        }
    }
}

/// Phase 6.4: Get the attack bonus from an entity's equipped tool.
/// Returns 0.0 if no tool is equipped or the tool doesn't exist.
pub fn equipped_attack_bonus(world: &SimulationWorld, entity: hecs::Entity) -> f64 {
    let equipped_id = match world.ecs.get::<&Inventory>(entity) {
        Ok(inv) => inv.equipped,
        Err(_) => return 0.0,
    };

    match equipped_id {
        Some(id) => world
            .objects
            .iter()
            .find(|o| o.id == id && o.is_intact())
            .map(|o| o.attack_bonus())
            .unwrap_or(0.0),
        None => 0.0,
    }
}

/// Phase 6.4: Get the defense bonus from an entity's equipped tool.
pub fn equipped_defense_bonus(world: &SimulationWorld, entity: hecs::Entity) -> f64 {
    let equipped_id = match world.ecs.get::<&Inventory>(entity) {
        Ok(inv) => inv.equipped,
        Err(_) => return 0.0,
    };

    match equipped_id {
        Some(id) => world
            .objects
            .iter()
            .find(|o| o.id == id && o.is_intact())
            .map(|o| o.defense_bonus())
            .unwrap_or(0.0),
        None => 0.0,
    }
}

/// Phase 6.4: Apply wear to an entity's equipped tool after use.
/// Returns true if the tool is still intact after wear.
pub fn apply_equipped_wear(world: &mut SimulationWorld, entity: hecs::Entity) -> bool {
    let equipped_id = match world.ecs.get::<&Inventory>(entity) {
        Ok(inv) => inv.equipped,
        Err(_) => return false,
    };

    match equipped_id {
        Some(id) => {
            if let Some(obj) = world.objects.iter_mut().find(|o| o.id == id) {
                obj.apply_use_wear(TOOL_USE_WEAR)
            } else {
                false
            }
        }
        None => false,
    }
}

/// Phase 6.2: Compute speed multiplier based on carried weight.
/// Returns a value in (0.0, 1.0] where lower means slower.
pub fn carry_speed_multiplier(inv: &Inventory, objects: &[crate::components::world_object::WorldObject]) -> f64 {
    let total_weight = inv.total_weight(objects);
    (1.0 - total_weight * CARRY_SPEED_PENALTY).max(0.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::genome::Genome;
    use crate::components::physical::Energy;
    use crate::components::spatial::Position;
    use crate::components::world_object::{Blueprint, Inventory, MaterialProperties, WorldObject};
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn make_ground_object(id: u64, x: f64, y: f64, durability: f64) -> WorldObject {
        WorldObject {
            id,
            x,
            y,
            material: MaterialProperties::default(),
            durability,
            max_durability: durability,
            creator_id: None,
            created_tick: 0,
            held_by: None,
        }
    }

    // -- Phase 6.1: Decay tests --

    #[test]
    fn decay_reduces_durability() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 50.0, 50.0, 100.0));

        run_decay(&mut world);

        assert_eq!(world.objects.len(), 1);
        assert!(world.objects[0].durability < 100.0);
    }

    #[test]
    fn decay_removes_destroyed_objects() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 50.0, 50.0, 0.005));

        run_decay(&mut world);

        assert!(world.objects.is_empty(), "destroyed object should be removed");
    }

    #[test]
    fn decay_removes_destroyed_from_inventory() {
        let mut world = test_world();
        let mut inv = Inventory::new(5);
        inv.add(1);
        let entity = world.ecs.spawn((inv,));

        world.objects.push(make_ground_object(1, 50.0, 50.0, 0.005));

        run_decay(&mut world);

        let inv = world.ecs.get::<&Inventory>(entity).unwrap();
        assert!(!inv.contains(1), "destroyed object should be removed from inventory");
    }

    // -- Phase 6.2: Pickup tests --

    #[test]
    fn pickup_adds_to_inventory() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 52.0, 50.0, 100.0));

        let entity = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::PickUp { object_id: 1 },
            Inventory::new(5),
        ));

        run_pickup(&mut world);

        let inv = world.ecs.get::<&Inventory>(entity).unwrap();
        assert!(inv.contains(1));
        assert_eq!(world.objects[0].held_by, Some(entity.to_bits().get()));
    }

    #[test]
    fn pickup_fails_if_inventory_full() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 52.0, 50.0, 100.0));

        let mut full_inv = Inventory::new(1);
        full_inv.add(99);
        let entity = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::PickUp { object_id: 1 },
            full_inv,
        ));

        run_pickup(&mut world);

        let inv = world.ecs.get::<&Inventory>(entity).unwrap();
        assert!(!inv.contains(1));
        assert!(world.objects[0].is_on_ground());
    }

    #[test]
    fn pickup_fails_if_out_of_range() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 200.0, 200.0, 100.0));

        let entity = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::PickUp { object_id: 1 },
            Inventory::new(5),
        ));

        run_pickup(&mut world);

        let inv = world.ecs.get::<&Inventory>(entity).unwrap();
        assert!(!inv.contains(1));
    }

    // -- Phase 6.2: Drop tests --

    #[test]
    fn drop_removes_from_inventory() {
        let mut world = test_world();
        let mut obj = make_ground_object(1, 0.0, 0.0, 100.0);
        obj.held_by = Some(42);
        world.objects.push(obj);

        let mut inv = Inventory::new(5);
        inv.add(1);
        let entity = world.ecs.spawn((
            Position { x: 75.0, y: 80.0 },
            Action::Drop { object_id: 1 },
            inv,
        ));

        run_drop(&mut world);

        let inv = world.ecs.get::<&Inventory>(entity).unwrap();
        assert!(!inv.contains(1));
        assert!(world.objects[0].is_on_ground());
        assert!((world.objects[0].x - 75.0).abs() < f64::EPSILON);
        assert!((world.objects[0].y - 80.0).abs() < f64::EPSILON);
    }

    // -- Phase 6.3: Creation tests --

    #[test]
    fn create_object_spawns_and_deducts_energy() {
        let mut world = test_world();
        let bp = Blueprint {
            energy_cost: 20.0,
            output_sharpness: 0.8,
            output_hardness: 0.6,
            output_weight: 0.3,
            output_durability: 50.0,
            ..Blueprint::default()
        };
        let genome = Genome {
            blueprint: bp,
            ..Genome::default()
        };
        let entity = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Action::CreateObject,
            genome,
        ));

        run_create(&mut world);

        assert_eq!(world.objects.len(), 1);
        let obj = &world.objects[0];
        assert!(obj.is_intact());
        assert_eq!(obj.creator_id, Some(entity.to_bits().get()));
        assert!((obj.material.sharpness - 0.8).abs() < f64::EPSILON);

        let energy = world.ecs.get::<&Energy>(entity).unwrap();
        assert!((energy.current - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn create_object_fails_if_insufficient_energy() {
        let mut world = test_world();
        let genome = Genome::default(); // energy_cost = 20.0
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: 10.0, // less than 20.0 energy cost
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Action::CreateObject,
            genome,
        ));

        run_create(&mut world);

        assert!(world.objects.is_empty(), "should not create object with insufficient energy");
    }

    // -- Phase 6.4: Tool use tests --

    #[test]
    fn equipped_attack_bonus_returns_bonus() {
        let mut world = test_world();
        let mut obj = make_ground_object(1, 0.0, 0.0, 100.0);
        obj.material.sharpness = 0.8;
        obj.held_by = Some(1); // placeholder
        world.objects.push(obj);

        let mut inv = Inventory::new(5);
        inv.add(1);
        inv.equip(1);
        let entity = world.ecs.spawn((inv,));

        let bonus = equipped_attack_bonus(&world, entity);
        assert!((bonus - 4.0).abs() < f64::EPSILON); // 0.8 * 5.0
    }

    #[test]
    fn equipped_defense_bonus_returns_bonus() {
        let mut world = test_world();
        let mut obj = make_ground_object(1, 0.0, 0.0, 100.0);
        obj.material.hardness = 0.9;
        obj.material.weight = 0.8;
        obj.held_by = Some(1);
        world.objects.push(obj);

        let mut inv = Inventory::new(5);
        inv.add(1);
        inv.equip(1);
        let entity = world.ecs.spawn((inv,));

        let bonus = equipped_defense_bonus(&world, entity);
        let expected = 0.9 * 0.8 * 5.0;
        assert!((bonus - expected).abs() < 1e-10, "expected {}, got {}", expected, bonus);
    }

    #[test]
    fn apply_wear_reduces_durability() {
        let mut world = test_world();
        let mut obj = make_ground_object(1, 0.0, 0.0, 10.0);
        obj.held_by = Some(1);
        world.objects.push(obj);

        let mut inv = Inventory::new(5);
        inv.add(1);
        inv.equip(1);
        let entity = world.ecs.spawn((inv,));

        let intact = apply_equipped_wear(&mut world, entity);
        assert!(intact);
        assert!(world.objects[0].durability < 10.0);
    }

    #[test]
    fn carry_speed_multiplier_reduces_with_weight() {
        let objects = vec![
            WorldObject {
                id: 1,
                x: 0.0,
                y: 0.0,
                material: MaterialProperties {
                    weight: 0.5,
                    ..MaterialProperties::default()
                },
                durability: 10.0,
                max_durability: 10.0,
                creator_id: None,
                created_tick: 0,
                held_by: None,
            },
            WorldObject {
                id: 2,
                x: 0.0,
                y: 0.0,
                material: MaterialProperties {
                    weight: 0.5,
                    ..MaterialProperties::default()
                },
                durability: 10.0,
                max_durability: 10.0,
                creator_id: None,
                created_tick: 0,
                held_by: None,
            },
        ];
        let mut inv = Inventory::new(5);
        inv.add(1);
        inv.add(2);
        let mult = carry_speed_multiplier(&inv, &objects);
        // total weight = 1.0, penalty = 1.0 * 0.15 = 0.15, mult = 0.85
        assert!((mult - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn no_equipped_tool_gives_zero_bonus() {
        let mut world = test_world();
        // Entity without Inventory.
        let entity = world.ecs.spawn(());
        assert_eq!(equipped_attack_bonus(&world, entity), 0.0);
        assert_eq!(equipped_defense_bonus(&world, entity), 0.0);
    }

    // -- Full system run test --

    #[test]
    fn full_run_does_not_panic() {
        let mut world = test_world();
        world.objects.push(make_ground_object(1, 50.0, 50.0, 100.0));
        world.objects.push(make_ground_object(2, 60.0, 60.0, 0.001));

        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Action::PickUp { object_id: 1 },
            Inventory::new(5),
        ));

        run(&mut world);

        // Object 2 should be decayed away.
        assert!(world.objects.iter().all(|o| o.id != 2));
    }
}

use std::collections::{HashMap, HashSet};

use crate::components::social::Social;
use crate::components::spatial::Position;
use crate::components::tribe::{
    Tribe, TribeId, MIN_TRIBE_SIZE, MIN_TRIBE_SURVIVAL_SIZE, TRIBE_FORMATION_RANGE,
    TRIBE_RELATIONSHIP_THRESHOLD,
};
use crate::core::world::SimulationWorld;

/// Tribe formation and maintenance system.
///
/// Runs each tick to:
/// 1. Update territory centroids for existing tribes.
/// 2. Remove dead members from tribes.
/// 3. Dissolve tribes that fall below minimum size.
/// 4. Attempt to form new tribes from unaffiliated entities with mutual positive relationships.
/// 5. Allow unaffiliated entities to join nearby tribes if they have positive relationships with members.
pub fn run(world: &mut SimulationWorld) {
    let current_tick = world.tick;

    // 1. Collect entity data: id, position, social relationships, tribe membership.
    let entity_data: Vec<(u64, f64, f64, HashMap<u64, f64>, Option<u64>)> = world
        .ecs
        .query::<(&Position, &Social, &TribeId)>()
        .iter()
        .map(|(entity, (pos, social, tribe_id))| {
            (
                entity.to_bits().get(),
                pos.x,
                pos.y,
                social.relationships.clone(),
                tribe_id.0,
            )
        })
        .collect();

    // Build lookup maps.
    let positions: HashMap<u64, (f64, f64)> = entity_data
        .iter()
        .map(|(id, x, y, _, _)| (*id, (*x, *y)))
        .collect();

    let alive_ids: HashSet<u64> = entity_data.iter().map(|(id, _, _, _, _)| *id).collect();

    // 2. Remove dead members from all tribes and update centroids.
    cleanup_tribes(world, &alive_ids, &positions);

    // 3. Collect unaffiliated entities.
    let unaffiliated: Vec<(u64, f64, f64, &HashMap<u64, f64>)> = entity_data
        .iter()
        .filter(|(_, _, _, _, tribe)| tribe.is_none())
        .map(|(id, x, y, rels, _)| (*id, *x, *y, rels))
        .collect();

    // 4. Try to recruit unaffiliated entities into existing tribes.
    let recruited = recruit_into_existing_tribes(world, &unaffiliated, &entity_data);

    // 5. Try to form new tribes from remaining unaffiliated entities.
    let still_unaffiliated: Vec<(u64, f64, f64, &HashMap<u64, f64>)> = unaffiliated
        .into_iter()
        .filter(|(id, _, _, _)| !recruited.contains(id))
        .collect();

    form_new_tribes(world, &still_unaffiliated, current_tick);
}

/// Remove dead members from tribes, dissolve tribes below minimum size,
/// and update territory centroids.
fn cleanup_tribes(
    world: &mut SimulationWorld,
    alive_ids: &HashSet<u64>,
    positions: &HashMap<u64, (f64, f64)>,
) {
    let tribe_ids: Vec<u64> = world.tribes.keys().cloned().collect();
    let mut dissolved_tribes = Vec::new();

    for tribe_id in tribe_ids {
        if let Some(tribe) = world.tribes.get_mut(&tribe_id) {
            // Remove dead members.
            tribe.member_ids.retain(|id| alive_ids.contains(id));

            // Dissolve if too small.
            if tribe.member_ids.len() < MIN_TRIBE_SURVIVAL_SIZE {
                dissolved_tribes.push(tribe_id);
                continue;
            }

            // Update centroid.
            let (sum_x, sum_y, count) = tribe.member_ids.iter().fold(
                (0.0, 0.0, 0usize),
                |(sx, sy, c), id| {
                    if let Some((x, y)) = positions.get(id) {
                        (sx + x, sy + y, c + 1)
                    } else {
                        (sx, sy, c)
                    }
                },
            );
            if count > 0 {
                tribe.territory_centroid_x = sum_x / count as f64;
                tribe.territory_centroid_y = sum_y / count as f64;
            }
        }
    }

    // Dissolve tribes and clear member tribe IDs.
    for tribe_id in &dissolved_tribes {
        if let Some(tribe) = world.tribes.remove(tribe_id) {
            for member_id in &tribe.member_ids {
                set_tribe_id_for_entity(world, *member_id, None);
            }
        }
    }
}

/// Try to recruit unaffiliated entities into existing tribes.
/// Returns the set of entity IDs that were recruited.
fn recruit_into_existing_tribes(
    world: &mut SimulationWorld,
    unaffiliated: &[(u64, f64, f64, &HashMap<u64, f64>)],
    _entity_data: &[(u64, f64, f64, HashMap<u64, f64>, Option<u64>)],
) -> HashSet<u64> {
    let mut recruited = HashSet::new();

    // For each unaffiliated entity, check if it has positive relationships
    // with members of an existing tribe and is within range.
    for &(entity_id, ex, ey, relationships) in unaffiliated {
        let mut best_tribe: Option<u64> = None;
        let mut best_avg_rel = 0.0;

        for (tribe_id, tribe) in world.tribes.iter() {
            // Check distance to tribe centroid.
            let dx = ex - tribe.territory_centroid_x;
            let dy = ey - tribe.territory_centroid_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > TRIBE_FORMATION_RANGE {
                continue;
            }

            // Check average relationship with tribe members.
            let mut total_rel = 0.0;
            let mut count = 0;
            for member_id in &tribe.member_ids {
                let rel = relationships.get(member_id).copied().unwrap_or(0.0);
                total_rel += rel;
                count += 1;
            }

            if count > 0 {
                let avg_rel = total_rel / count as f64;
                if avg_rel >= TRIBE_RELATIONSHIP_THRESHOLD && avg_rel > best_avg_rel {
                    best_avg_rel = avg_rel;
                    best_tribe = Some(*tribe_id);
                }
            }
        }

        if let Some(tribe_id) = best_tribe {
            if let Some(tribe) = world.tribes.get_mut(&tribe_id) {
                tribe.member_ids.insert(entity_id);
                set_tribe_id_for_entity(world, entity_id, Some(tribe_id));
                recruited.insert(entity_id);
            }
        }
    }

    recruited
}

/// Form new tribes from unaffiliated entities that have mutual positive relationships.
fn form_new_tribes(
    world: &mut SimulationWorld,
    unaffiliated: &[(u64, f64, f64, &HashMap<u64, f64>)],
    current_tick: u64,
) {
    if unaffiliated.len() < MIN_TRIBE_SIZE {
        return;
    }

    let mut already_assigned: HashSet<u64> = HashSet::new();

    // For each unaffiliated entity, try to find MIN_TRIBE_SIZE nearby entities
    // with mutual positive relationships.
    for i in 0..unaffiliated.len() {
        let (seed_id, sx, sy, seed_rels) = unaffiliated[i];
        if already_assigned.contains(&seed_id) {
            continue;
        }

        // Find nearby candidates with mutual positive relationships.
        let mut candidates: Vec<u64> = Vec::new();
        candidates.push(seed_id);

        for j in 0..unaffiliated.len() {
            if i == j {
                continue;
            }
            let (cand_id, cx, cy, cand_rels) = unaffiliated[j];
            if already_assigned.contains(&cand_id) {
                continue;
            }

            // Check distance.
            let dx = sx - cx;
            let dy = sy - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > TRIBE_FORMATION_RANGE {
                continue;
            }

            // Check mutual relationship: seed->candidate and candidate->seed.
            let seed_to_cand = seed_rels.get(&cand_id).copied().unwrap_or(0.0);
            let cand_to_seed = cand_rels.get(&seed_id).copied().unwrap_or(0.0);

            if seed_to_cand >= TRIBE_RELATIONSHIP_THRESHOLD
                && cand_to_seed >= TRIBE_RELATIONSHIP_THRESHOLD
            {
                // Also check mutual relationships with all existing candidates.
                let mutually_positive = candidates.iter().all(|&existing_id| {
                    if existing_id == seed_id {
                        return true; // already checked
                    }
                    // Find the existing candidate's relationships.
                    let existing_rels = unaffiliated
                        .iter()
                        .find(|(id, _, _, _)| *id == existing_id)
                        .map(|(_, _, _, rels)| *rels);

                    if let Some(existing_rels) = existing_rels {
                        let existing_to_cand =
                            existing_rels.get(&cand_id).copied().unwrap_or(0.0);
                        let cand_to_existing =
                            cand_rels.get(&existing_id).copied().unwrap_or(0.0);
                        existing_to_cand >= TRIBE_RELATIONSHIP_THRESHOLD
                            && cand_to_existing >= TRIBE_RELATIONSHIP_THRESHOLD
                    } else {
                        false
                    }
                });

                if mutually_positive {
                    candidates.push(cand_id);
                }
            }
        }

        // If we found enough candidates, form a tribe.
        if candidates.len() >= MIN_TRIBE_SIZE {
            let tribe_id = world.next_tribe_id;
            world.next_tribe_id += 1;

            // Compute centroid from member positions.
            let (sum_x, sum_y) = candidates.iter().fold((0.0, 0.0), |(sx, sy), id| {
                let pos = unaffiliated
                    .iter()
                    .find(|(eid, _, _, _)| *eid == *id)
                    .map(|(_, x, y, _)| (*x, *y))
                    .unwrap_or((0.0, 0.0));
                (sx + pos.0, sy + pos.1)
            });
            let count = candidates.len() as f64;
            let centroid_x = sum_x / count;
            let centroid_y = sum_y / count;

            let member_set: HashSet<u64> = candidates.iter().cloned().collect();
            let tribe = Tribe::new(tribe_id, member_set.clone(), centroid_x, centroid_y, current_tick);
            world.tribes.insert(tribe_id, tribe);

            for &member_id in &candidates {
                set_tribe_id_for_entity(world, member_id, Some(tribe_id));
                already_assigned.insert(member_id);
            }
        }
    }
}

/// Helper: set the TribeId component on an entity given its raw ID bits.
fn set_tribe_id_for_entity(world: &mut SimulationWorld, entity_id_bits: u64, tribe_id: Option<u64>) {
    if let Some(bits) = std::num::NonZeroU64::new(entity_id_bits) {
        if let Some(entity) = hecs::Entity::from_bits(bits.get()) {
            if world.ecs.contains(entity) {
                if let Ok(mut tid) = world.ecs.get::<&mut TribeId>(entity) {
                    tid.0 = tribe_id;
                }
            }
        }
    }
}

/// Get the tribe ID for an entity, if it has one.
pub fn get_tribe_id(world: &SimulationWorld, entity_id_bits: u64) -> Option<u64> {
    let bits = std::num::NonZeroU64::new(entity_id_bits)?;
    let entity = hecs::Entity::from_bits(bits.get())?;
    if world.ecs.contains(entity) {
        world
            .ecs
            .get::<&TribeId>(entity)
            .ok()
            .and_then(|tid| tid.0)
    } else {
        None
    }
}

/// Check if two entities are in the same tribe.
pub fn are_tribemates(world: &SimulationWorld, a_id: u64, b_id: u64) -> bool {
    match (get_tribe_id(world, a_id), get_tribe_id(world, b_id)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Count how many members of the same tribe as `entity_id` are within `range` of position (x, y).
pub fn count_nearby_allies(
    world: &SimulationWorld,
    entity_id_bits: u64,
    x: f64,
    y: f64,
    range: f64,
) -> usize {
    let tribe_id = match get_tribe_id(world, entity_id_bits) {
        Some(id) => id,
        None => return 0,
    };

    let tribe = match world.tribes.get(&tribe_id) {
        Some(t) => t,
        None => return 0,
    };

    let range_sq = range * range;
    let mut count = 0;

    for &member_id in &tribe.member_ids {
        if member_id == entity_id_bits {
            continue;
        }
        if let Some(bits) = std::num::NonZeroU64::new(member_id) {
            if let Some(entity) = hecs::Entity::from_bits(bits.get()) {
                if let Ok(pos) = world.ecs.get::<&Position>(entity) {
                    let dx = pos.x - x;
                    let dy = pos.y - y;
                    if dx * dx + dy * dy <= range_sq {
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

/// Count how many enemies (members of other tribes) are within `range` of position (x, y).
pub fn count_nearby_enemies(
    world: &SimulationWorld,
    entity_id_bits: u64,
    x: f64,
    y: f64,
    range: f64,
) -> usize {
    let my_tribe_id = get_tribe_id(world, entity_id_bits);
    let range_sq = range * range;
    let mut count = 0;

    for (tribe_id, tribe) in &world.tribes {
        // Skip our own tribe.
        if Some(*tribe_id) == my_tribe_id {
            continue;
        }

        for &member_id in &tribe.member_ids {
            if let Some(bits) = std::num::NonZeroU64::new(member_id) {
                if let Some(entity) = hecs::Entity::from_bits(bits.get()) {
                    if let Ok(pos) = world.ecs.get::<&Position>(entity) {
                        let dx = pos.x - x;
                        let dy = pos.y - y;
                        if dx * dx + dy * dy <= range_sq {
                            count += 1;
                        }
                    }
                }
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::social::Social;
    use crate::components::spatial::Position;
    use crate::components::tribe::TribeId;
    use crate::core::config::SimulationConfig;
    use crate::core::world::SimulationWorld;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Spawn an entity with Position, Social, and TribeId components.
    fn spawn_social_entity(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        social: Social,
    ) -> hecs::Entity {
        world.ecs.spawn((
            Position { x, y },
            social,
            TribeId::default(),
        ))
    }

    #[test]
    fn tribe_forms_from_three_mutual_friends() {
        let mut world = test_world();
        world.tick = 10;

        // Spawn 3 entities with mutual positive relationships.
        let mut social_a = Social::default();
        let mut social_b = Social::default();
        let mut social_c = Social::default();

        // We'll set relationships after spawning so we know entity IDs.
        let a = spawn_social_entity(&mut world, 10.0, 10.0, Social::default());
        let b = spawn_social_entity(&mut world, 15.0, 10.0, Social::default());
        let c = spawn_social_entity(&mut world, 12.0, 15.0, Social::default());

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();

        // Set up mutual positive relationships (> 0.3).
        social_a.relationships.insert(b_id, 0.5);
        social_a.relationships.insert(c_id, 0.5);
        social_b.relationships.insert(a_id, 0.5);
        social_b.relationships.insert(c_id, 0.4);
        social_c.relationships.insert(a_id, 0.6);
        social_c.relationships.insert(b_id, 0.4);

        *world.ecs.get::<&mut Social>(a).unwrap() = social_a;
        *world.ecs.get::<&mut Social>(b).unwrap() = social_b;
        *world.ecs.get::<&mut Social>(c).unwrap() = social_c;

        run(&mut world);

        // All three should now be in the same tribe.
        let tid_a = world.ecs.get::<&TribeId>(a).unwrap().0;
        let tid_b = world.ecs.get::<&TribeId>(b).unwrap().0;
        let tid_c = world.ecs.get::<&TribeId>(c).unwrap().0;

        assert!(tid_a.is_some(), "entity a should be in a tribe");
        assert_eq!(tid_a, tid_b, "a and b should be in the same tribe");
        assert_eq!(tid_a, tid_c, "a and c should be in the same tribe");

        // Tribe should exist in world.tribes.
        let tribe = world.tribes.get(&tid_a.unwrap()).unwrap();
        assert_eq!(tribe.size(), 3);
    }

    #[test]
    fn tribe_does_not_form_with_weak_relationships() {
        let mut world = test_world();
        world.tick = 10;

        let a = spawn_social_entity(&mut world, 10.0, 10.0, Social::default());
        let b = spawn_social_entity(&mut world, 15.0, 10.0, Social::default());
        let c = spawn_social_entity(&mut world, 12.0, 15.0, Social::default());

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();

        // Set relationships below threshold (0.3).
        let mut social_a = Social::default();
        social_a.relationships.insert(b_id, 0.1);
        social_a.relationships.insert(c_id, 0.1);
        let mut social_b = Social::default();
        social_b.relationships.insert(a_id, 0.1);
        social_b.relationships.insert(c_id, 0.1);
        let mut social_c = Social::default();
        social_c.relationships.insert(a_id, 0.1);
        social_c.relationships.insert(b_id, 0.1);

        *world.ecs.get::<&mut Social>(a).unwrap() = social_a;
        *world.ecs.get::<&mut Social>(b).unwrap() = social_b;
        *world.ecs.get::<&mut Social>(c).unwrap() = social_c;

        run(&mut world);

        assert!(world.tribes.is_empty(), "tribe should not form with weak relationships");
    }

    #[test]
    fn tribe_does_not_form_when_too_far_apart() {
        let mut world = test_world();
        world.tick = 10;

        // Entities far apart.
        let a = spawn_social_entity(&mut world, 10.0, 10.0, Social::default());
        let b = spawn_social_entity(&mut world, 200.0, 200.0, Social::default());
        let c = spawn_social_entity(&mut world, 400.0, 400.0, Social::default());

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();

        let mut social_a = Social::default();
        social_a.relationships.insert(b_id, 0.8);
        social_a.relationships.insert(c_id, 0.8);
        let mut social_b = Social::default();
        social_b.relationships.insert(a_id, 0.8);
        social_b.relationships.insert(c_id, 0.8);
        let mut social_c = Social::default();
        social_c.relationships.insert(a_id, 0.8);
        social_c.relationships.insert(b_id, 0.8);

        *world.ecs.get::<&mut Social>(a).unwrap() = social_a;
        *world.ecs.get::<&mut Social>(b).unwrap() = social_b;
        *world.ecs.get::<&mut Social>(c).unwrap() = social_c;

        run(&mut world);

        assert!(world.tribes.is_empty(), "tribe should not form when entities are too far apart");
    }

    #[test]
    fn tribe_dissolves_when_members_removed() {
        let mut world = test_world();
        world.tick = 10;

        // Manually create a tribe with member IDs that don't exist.
        let tribe_id = world.next_tribe_id;
        world.next_tribe_id += 1;

        let members: HashSet<u64> = [999, 888].into_iter().collect();
        let tribe = Tribe::new(tribe_id, members, 50.0, 50.0, 5);
        world.tribes.insert(tribe_id, tribe);

        assert_eq!(world.tribes.len(), 1);

        // Run tribe system - members don't exist as entities, so they'll be cleaned up.
        run(&mut world);

        assert!(world.tribes.is_empty(), "tribe with no alive members should dissolve");
    }

    #[test]
    fn are_tribemates_works() {
        let mut world = test_world();

        let a = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            Social::default(),
            TribeId(Some(42)),
        ));
        let b = world.ecs.spawn((
            Position { x: 20.0, y: 20.0 },
            Social::default(),
            TribeId(Some(42)),
        ));
        let c = world.ecs.spawn((
            Position { x: 30.0, y: 30.0 },
            Social::default(),
            TribeId(Some(99)),
        ));
        let d = world.ecs.spawn((
            Position { x: 40.0, y: 40.0 },
            Social::default(),
            TribeId::default(),
        ));

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();
        let d_id = d.to_bits().get();

        assert!(are_tribemates(&world, a_id, b_id));
        assert!(!are_tribemates(&world, a_id, c_id));
        assert!(!are_tribemates(&world, a_id, d_id));
        assert!(!are_tribemates(&world, d_id, d_id)); // no tribe
    }

    #[test]
    fn count_nearby_allies_counts_tribemates() {
        let mut world = test_world();

        let tribe_id = 1u64;
        let a = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));
        let b = world.ecs.spawn((
            Position { x: 15.0, y: 10.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));
        let c = world.ecs.spawn((
            Position { x: 12.0, y: 12.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));
        // Far away tribemate.
        let d = world.ecs.spawn((
            Position { x: 200.0, y: 200.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();
        let d_id = d.to_bits().get();

        let members: HashSet<u64> = [a_id, b_id, c_id, d_id].into_iter().collect();
        world.tribes.insert(
            tribe_id,
            Tribe::new(tribe_id, members, 0.0, 0.0, 0),
        );

        let allies = count_nearby_allies(&world, a_id, 10.0, 10.0, 20.0);
        assert_eq!(allies, 2, "should count b and c as nearby allies, not d");
    }

    #[test]
    fn count_nearby_enemies_counts_other_tribes() {
        let mut world = test_world();

        let tribe_a = 1u64;
        let tribe_b = 2u64;

        let a = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            Social::default(),
            TribeId(Some(tribe_a)),
        ));
        let b = world.ecs.spawn((
            Position { x: 15.0, y: 10.0 },
            Social::default(),
            TribeId(Some(tribe_b)),
        ));
        let c = world.ecs.spawn((
            Position { x: 12.0, y: 12.0 },
            Social::default(),
            TribeId(Some(tribe_b)),
        ));

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();
        let c_id = c.to_bits().get();

        world.tribes.insert(
            tribe_a,
            Tribe::new(tribe_a, [a_id].into_iter().collect(), 0.0, 0.0, 0),
        );
        world.tribes.insert(
            tribe_b,
            Tribe::new(tribe_b, [b_id, c_id].into_iter().collect(), 0.0, 0.0, 0),
        );

        let enemies = count_nearby_enemies(&world, a_id, 10.0, 10.0, 20.0);
        assert_eq!(enemies, 2, "should count b and c as nearby enemies");
    }

    #[test]
    fn territory_centroid_updates_on_tick() {
        let mut world = test_world();
        world.tick = 5;

        let tribe_id = world.next_tribe_id;
        world.next_tribe_id += 1;

        let a = world.ecs.spawn((
            Position { x: 10.0, y: 10.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));
        let b = world.ecs.spawn((
            Position { x: 30.0, y: 30.0 },
            Social::default(),
            TribeId(Some(tribe_id)),
        ));

        let a_id = a.to_bits().get();
        let b_id = b.to_bits().get();

        let members: HashSet<u64> = [a_id, b_id].into_iter().collect();
        world.tribes.insert(
            tribe_id,
            Tribe::new(tribe_id, members, 0.0, 0.0, 1),
        );

        run(&mut world);

        let tribe = world.tribes.get(&tribe_id).unwrap();
        assert!((tribe.territory_centroid_x - 20.0).abs() < 0.01);
        assert!((tribe.territory_centroid_y - 20.0).abs() < 0.01);
    }
}

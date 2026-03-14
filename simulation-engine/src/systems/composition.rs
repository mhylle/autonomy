use crate::components::action::Action;
use crate::components::composite::{
    assign_role_from_genome, AggregateStats, CompositeBody, CompositeMember,
    CompositeMemberMarker, COMPOSITION_RANGE, DECOMPOSITION_ENERGY_THRESHOLD,
    MAX_COMPOSITE_SIZE, MIN_COMPOSITION_AFFINITY, PARTIAL_DECOMPOSITION_THRESHOLD,
};
use crate::components::genome::Genome;
use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::events::types::SimEvent;

/// Composition system (Phase 4.1-4.3).
///
/// Handles three sub-phases each tick:
/// 1. **Merging**: Find adjacent entity pairs that both chose CompositionAttempt,
///    check compatibility (composition_affinity >= threshold, same species), and
///    create/extend a composite.
/// 2. **Aggregate capability computation**: Update AggregateStats on composites.
/// 3. **Decomposition**: Decompose composites with critically low energy; shed
///    weakest member on moderately low energy.
pub fn run(world: &mut SimulationWorld) {
    run_merging(world);
    run_aggregate_stats(world);
    run_decomposition(world);
}

// ---------------------------------------------------------------------------
// Phase 1: Merging
// ---------------------------------------------------------------------------

/// Find pairs of entities that both chose CompositionAttempt, are close enough,
/// and have compatible genomes. Merge them into composites.
fn run_merging(world: &mut SimulationWorld) {
    let current_tick = world.tick;

    // Collect entities attempting composition, excluding those already in a composite.
    let candidates: Vec<_> = world
        .ecs
        .query::<(&Action, &Position, &Genome)>()
        .without::<&CompositeMemberMarker>()
        .iter()
        .filter(|(_, (action, _, _))| matches!(action, Action::CompositionAttempt))
        .map(|(entity, (_, pos, genome))| {
            (
                entity.to_bits().get(),
                entity,
                pos.x,
                pos.y,
                genome.composition_affinity,
                genome.species_id,
            )
        })
        .collect();

    if candidates.len() < 2 {
        return;
    }

    let mut merged_this_tick = std::collections::HashSet::new();

    // For each candidate pair, check distance and compatibility.
    for i in 0..candidates.len() {
        let (id_a, entity_a, ax, ay, affinity_a, species_a) = &candidates[i];
        if merged_this_tick.contains(id_a) {
            continue;
        }

        for j in (i + 1)..candidates.len() {
            let (id_b, entity_b, bx, by, affinity_b, species_b) = &candidates[j];
            if merged_this_tick.contains(id_b) {
                continue;
            }

            // Distance check.
            let dx = ax - bx;
            let dy = ay - by;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > COMPOSITION_RANGE {
                continue;
            }

            // Compatibility: both must have sufficient affinity and same species.
            if *affinity_a < MIN_COMPOSITION_AFFINITY
                || *affinity_b < MIN_COMPOSITION_AFFINITY
                || species_a != species_b
            {
                continue;
            }

            // Check if entity_a is already a composite leader.
            let a_is_composite = world.ecs.get::<&CompositeBody>(*entity_a).is_ok();
            let b_is_composite = world.ecs.get::<&CompositeBody>(*entity_b).is_ok();

            if a_is_composite && b_is_composite {
                // Both are composites -- skip, don't merge two composites.
                continue;
            }

            if a_is_composite {
                // Add B as a member of A's composite.
                let body_full = world
                    .ecs
                    .get::<&CompositeBody>(*entity_a)
                    .map(|b| b.member_count() >= MAX_COMPOSITE_SIZE)
                    .unwrap_or(true);
                if body_full {
                    continue;
                }

                let role = world
                    .ecs
                    .get::<&Genome>(*entity_b)
                    .map(|g| assign_role_from_genome(&g))
                    .unwrap_or(crate::components::composite::CellRole::Undifferentiated);

                if let Ok(mut body) = world.ecs.get::<&mut CompositeBody>(*entity_a) {
                    body.add_member(*id_b, role);
                }

                let _ = world.ecs.insert_one(
                    *entity_b,
                    CompositeMemberMarker { leader_id: *id_a },
                );

                merged_this_tick.insert(*id_b);

                world.event_log.push(SimEvent::CompositeFormed {
                    leader_id: *id_a,
                    member_id: *id_b,
                    x: *ax,
                    y: *ay,
                });
            } else if b_is_composite {
                // Add A as a member of B's composite.
                let body_full = world
                    .ecs
                    .get::<&CompositeBody>(*entity_b)
                    .map(|b| b.member_count() >= MAX_COMPOSITE_SIZE)
                    .unwrap_or(true);
                if body_full {
                    continue;
                }

                let role = world
                    .ecs
                    .get::<&Genome>(*entity_a)
                    .map(|g| assign_role_from_genome(&g))
                    .unwrap_or(crate::components::composite::CellRole::Undifferentiated);

                if let Ok(mut body) = world.ecs.get::<&mut CompositeBody>(*entity_b) {
                    body.add_member(*id_a, role);
                }

                let _ = world.ecs.insert_one(
                    *entity_a,
                    CompositeMemberMarker { leader_id: *id_b },
                );

                merged_this_tick.insert(*id_a);

                world.event_log.push(SimEvent::CompositeFormed {
                    leader_id: *id_b,
                    member_id: *id_a,
                    x: *bx,
                    y: *by,
                });
            } else {
                // Neither is a composite: A becomes the leader, B becomes a member.
                let role_b = world
                    .ecs
                    .get::<&Genome>(*entity_b)
                    .map(|g| assign_role_from_genome(&g))
                    .unwrap_or(crate::components::composite::CellRole::Undifferentiated);

                let mut body = CompositeBody::new(*id_a, current_tick);
                body.add_member(*id_b, role_b);

                let _ = world.ecs.insert_one(*entity_a, body);
                let _ = world.ecs.insert_one(
                    *entity_b,
                    CompositeMemberMarker { leader_id: *id_a },
                );

                merged_this_tick.insert(*id_a);
                merged_this_tick.insert(*id_b);

                world.event_log.push(SimEvent::CompositeFormed {
                    leader_id: *id_a,
                    member_id: *id_b,
                    x: *ax,
                    y: *ay,
                });
            }

            break; // Entity A already merged this tick.
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Aggregate stats computation
// ---------------------------------------------------------------------------

/// Recompute aggregate stats for all composite entities.
fn run_aggregate_stats(world: &mut SimulationWorld) {
    // Collect composite leaders and their member info.
    let composites: Vec<_> = world
        .ecs
        .query::<&CompositeBody>()
        .iter()
        .map(|(entity, body)| (entity, body.members.clone()))
        .collect();

    for (leader_entity, members) in &composites {
        // Gather trait values from each member.
        let mut role_stats = Vec::new();
        for member in members {
            // Look up the member entity's genome to get the relevant trait value.
            let trait_value = find_member_trait_value(world, member);
            role_stats.push((member.entity_id, member.role, trait_value));
        }

        let stats = crate::components::composite::compute_aggregate_stats(members, &role_stats);

        // Insert or update AggregateStats on the leader.
        if world.ecs.get::<&AggregateStats>(*leader_entity).is_ok() {
            if let Ok(mut existing) = world.ecs.get::<&mut AggregateStats>(*leader_entity) {
                *existing = stats;
            }
        } else {
            let _ = world.ecs.insert_one(*leader_entity, stats);
        }
    }
}

/// Get the trait value relevant to a member's role from its genome.
fn find_member_trait_value(world: &SimulationWorld, member: &CompositeMember) -> f64 {
    use crate::components::composite::CellRole;

    // Find the entity with matching ID bits.
    for (entity, genome) in world.ecs.query::<&Genome>().iter() {
        if entity.to_bits().get() == member.entity_id {
            return match member.role {
                CellRole::Locomotion => genome.max_speed,
                CellRole::Sensing => genome.sensor_range,
                CellRole::Attack => genome.drive_weights.base_aggression * genome.size,
                CellRole::Defense => genome.size,
                CellRole::Digestion => 1.0 - genome.metabolism_rate.min(1.0),
                CellRole::Reproduction => genome.drive_weights.base_reproductive,
                CellRole::Undifferentiated => genome.size,
            };
        }
    }
    0.0
}

// ---------------------------------------------------------------------------
// Phase 3: Decomposition
// ---------------------------------------------------------------------------

/// Check composites for low energy and trigger decomposition.
fn run_decomposition(world: &mut SimulationWorld) {
    // Collect composites needing decomposition.
    let composites: Vec<_> = world
        .ecs
        .query::<(&CompositeBody, &Energy, &Position)>()
        .iter()
        .map(|(entity, (body, energy, pos))| {
            let fraction = if energy.max > 0.0 {
                energy.current / energy.max
            } else {
                0.0
            };
            (
                entity,
                entity.to_bits().get(),
                body.clone(),
                fraction,
                pos.x,
                pos.y,
            )
        })
        .collect();

    for (leader_entity, leader_id, body, energy_fraction, x, y) in composites {
        if body.members.is_empty() {
            // No members left, remove the CompositeBody component.
            let _ = world.ecs.remove_one::<CompositeBody>(leader_entity);
            if world.ecs.get::<&AggregateStats>(leader_entity).is_ok() {
                let _ = world.ecs.remove_one::<AggregateStats>(leader_entity);
            }
            continue;
        }

        if energy_fraction <= DECOMPOSITION_ENERGY_THRESHOLD {
            // Full decomposition: release all members.
            full_decomposition(world, leader_entity, leader_id, &body, x, y);
        } else if energy_fraction <= PARTIAL_DECOMPOSITION_THRESHOLD && body.members.len() > 1 {
            // Partial decomposition: shed the weakest member.
            partial_decomposition(world, leader_entity, leader_id, &body, x, y);
        }
    }
}

/// Release all members from a composite, removing the CompositeBody.
fn full_decomposition(
    world: &mut SimulationWorld,
    leader_entity: hecs::Entity,
    leader_id: u64,
    body: &CompositeBody,
    x: f64,
    y: f64,
) {
    let mut released_ids = Vec::new();

    for member in &body.members {
        release_member(world, member.entity_id, x, y);
        released_ids.push(member.entity_id);
    }

    // Remove CompositeBody and AggregateStats from the leader.
    let _ = world.ecs.remove_one::<CompositeBody>(leader_entity);
    if world.ecs.get::<&AggregateStats>(leader_entity).is_ok() {
        let _ = world.ecs.remove_one::<AggregateStats>(leader_entity);
    }

    world.event_log.push(SimEvent::CompositeDecomposed {
        leader_id,
        released_member_ids: released_ids,
        x,
        y,
    });
}

/// Shed the weakest member from a composite.
///
/// "Weakest" is determined by lowest energy among members.
fn partial_decomposition(
    world: &mut SimulationWorld,
    leader_entity: hecs::Entity,
    leader_id: u64,
    body: &CompositeBody,
    x: f64,
    y: f64,
) {
    // Find the weakest member by energy.
    let weakest = body
        .members
        .iter()
        .filter_map(|m| {
            let energy = find_member_energy(world, m.entity_id);
            Some((m.entity_id, energy))
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((weakest_id, _)) = weakest {
        // Remove from the CompositeBody.
        if let Ok(mut body_mut) = world.ecs.get::<&mut CompositeBody>(leader_entity) {
            body_mut.remove_member(weakest_id);
        }

        release_member(world, weakest_id, x, y);

        world.event_log.push(SimEvent::CompositeDecomposed {
            leader_id,
            released_member_ids: vec![weakest_id],
            x,
            y,
        });
    }
}

/// Remove a member's CompositeMemberMarker and place it near the given position.
fn release_member(world: &mut SimulationWorld, member_id: u64, base_x: f64, base_y: f64) {
    // Find the entity first, collecting to drop the borrow.
    let target: Option<hecs::Entity> = {
        let found: Vec<_> = world
            .ecs
            .query::<&CompositeMemberMarker>()
            .iter()
            .filter(|(entity, _)| entity.to_bits().get() == member_id)
            .map(|(entity, _)| entity)
            .collect();
        found.into_iter().next()
    };

    if let Some(entity) = target {
        let _ = world.ecs.remove_one::<CompositeMemberMarker>(entity);

        // Place the released member near the composite's position.
        if let Ok(mut pos) = world.ecs.get::<&mut Position>(entity) {
            pos.x = (base_x + 5.0).rem_euclid(1000.0);
            pos.y = (base_y + 5.0).rem_euclid(1000.0);
        }
    }
}

/// Find a member's current energy.
fn find_member_energy(world: &SimulationWorld, member_id: u64) -> f64 {
    for (entity, energy) in world.ecs.query::<&Energy>().iter() {
        if entity.to_bits().get() == member_id {
            return energy.current;
        }
    }
    0.0
}

// ---------------------------------------------------------------------------
// Energy distribution (Phase 4.2)
// ---------------------------------------------------------------------------

/// Distribute energy from the leader to members proportionally.
///
/// Called as part of aggregate stats computation. The leader's metabolism
/// cost scales with member count.
pub fn distribute_energy(world: &mut SimulationWorld) {
    let composites: Vec<_> = world
        .ecs
        .query::<(&CompositeBody, &Energy)>()
        .iter()
        .map(|(entity, (body, energy))| {
            (entity, body.members.clone(), energy.current, energy.max)
        })
        .collect();

    for (_leader_entity, members, leader_energy, leader_max) in &composites {
        if members.is_empty() {
            continue;
        }

        let fraction = leader_energy / leader_max;

        // Each member gets a share of the leader's energy state.
        for member in members {
            for (entity, energy) in world.ecs.query::<&mut Energy>().iter() {
                if entity.to_bits().get() == member.entity_id {
                    // Member energy tracks composite health: set to fraction of their max.
                    energy.current = (fraction * energy.max).max(0.0);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::action::Action;
    use crate::components::behavior_tree::default_starter_bt;
    use crate::components::composite::{CellRole, CompositeBody, CompositeMemberMarker};
    use crate::components::drives::Drives;
    use crate::components::genome::Genome;
    use crate::components::identity::Identity;
    use crate::components::memory::Memory;
    use crate::components::perception::Perception;
    use crate::components::physical::{Age, Energy, Health, Size};
    use crate::components::social::Social;
    use crate::components::spatial::{Position, Velocity};
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Spawn a full entity with a given action, position, and composition_affinity.
    fn spawn_entity_for_composition(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        action: Action,
        affinity: f64,
    ) -> hecs::Entity {
        let mut genome = Genome::default();
        genome.composition_affinity = affinity;
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            action,
            genome,
        ))
    }

    #[test]
    fn composition_system_runs_without_panic() {
        let mut world = test_world();
        run(&mut world);
    }

    #[test]
    fn two_adjacent_entities_merge() {
        let mut world = test_world();
        let a = spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        let b = spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        run(&mut world);

        // One should be a composite leader, the other a member.
        let a_is_leader = world.ecs.get::<&CompositeBody>(a).is_ok();
        let b_is_member = world.ecs.get::<&CompositeMemberMarker>(b).is_ok();

        assert!(
            a_is_leader || world.ecs.get::<&CompositeBody>(b).is_ok(),
            "one entity should become a composite leader"
        );
        assert!(
            b_is_member || world.ecs.get::<&CompositeMemberMarker>(a).is_ok(),
            "one entity should become a composite member"
        );
    }

    #[test]
    fn entities_too_far_apart_dont_merge() {
        let mut world = test_world();
        spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        spawn_entity_for_composition(
            &mut world, 200.0, 200.0, Action::CompositionAttempt, 0.5,
        );

        run(&mut world);

        // Neither should have CompositeBody.
        let composite_count = world.ecs.query::<&CompositeBody>().iter().count();
        assert_eq!(composite_count, 0, "distant entities should not merge");
    }

    #[test]
    fn low_affinity_prevents_merge() {
        let mut world = test_world();
        spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.1,
        );
        spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.1,
        );

        run(&mut world);

        let composite_count = world.ecs.query::<&CompositeBody>().iter().count();
        assert_eq!(composite_count, 0, "low affinity should prevent merging");
    }

    #[test]
    fn different_species_dont_merge() {
        let mut world = test_world();
        let a = spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        let b = spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        // Change species_id of B.
        if let Ok(mut genome) = world.ecs.get::<&mut Genome>(b) {
            genome.species_id = 999999;
        }

        run(&mut world);

        let a_is_leader = world.ecs.get::<&CompositeBody>(a).is_ok();
        let b_is_leader = world.ecs.get::<&CompositeBody>(b).is_ok();
        assert!(
            !a_is_leader && !b_is_leader,
            "different species should not merge"
        );
    }

    #[test]
    fn entity_not_attempting_composition_doesnt_merge() {
        let mut world = test_world();
        spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        // B is wandering, not attempting composition.
        spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::Wander { speed: 1.0 }, 0.5,
        );

        run(&mut world);

        let composite_count = world.ecs.query::<&CompositeBody>().iter().count();
        assert_eq!(composite_count, 0, "non-attempting entity should not merge");
    }

    #[test]
    fn composite_member_gets_marker() {
        let mut world = test_world();
        let a = spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        let b = spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        run(&mut world);

        // Verify the member has a marker pointing to the leader.
        let a_is_leader = world.ecs.get::<&CompositeBody>(a).is_ok();
        if a_is_leader {
            let marker = world.ecs.get::<&CompositeMemberMarker>(b).unwrap();
            assert_eq!(marker.leader_id, a.to_bits().get());
        } else {
            let marker = world.ecs.get::<&CompositeMemberMarker>(a).unwrap();
            assert_eq!(marker.leader_id, b.to_bits().get());
        }
    }

    #[test]
    fn aggregate_stats_computed_for_composite() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        // Create a member with high speed -> Locomotion role.
        let mut member_genome = Genome::default();
        member_genome.max_speed = 10.0;
        member_genome.sensor_range = 5.0;
        member_genome.size = 1.0;
        let member = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy::default(),
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            member_genome,
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member.to_bits().get(), CellRole::Locomotion);
        world.ecs.insert_one(leader, body).unwrap();

        // Run the system (only aggregate stats phase matters here).
        run_aggregate_stats(&mut world);

        let stats = world.ecs.get::<&AggregateStats>(leader).unwrap();
        assert_eq!(stats.member_count, 1);
        assert!(stats.speed > 0.0, "speed should be computed from locomotion member");
    }

    #[test]
    fn full_decomposition_on_critical_energy() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 10.0, // 10% of max -> below DECOMPOSITION_ENERGY_THRESHOLD (15%)
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        let member = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy::default(),
            Health::default(),
            Age::default(),
            Size::default(),
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member.to_bits().get(), CellRole::Locomotion);
        world.ecs.insert_one(leader, body).unwrap();

        run(&mut world);

        // Leader should no longer have CompositeBody.
        assert!(
            world.ecs.get::<&CompositeBody>(leader).is_err(),
            "leader should lose CompositeBody after full decomposition"
        );
        // Member should no longer have CompositeMemberMarker.
        assert!(
            world.ecs.get::<&CompositeMemberMarker>(member).is_err(),
            "member should lose marker after full decomposition"
        );
    }

    #[test]
    fn partial_decomposition_sheds_weakest_member() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 25.0, // 25% -> between PARTIAL (30%) and FULL (15%) thresholds
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        // Member 1: strong (high energy)
        let member1 = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        // Member 2: weak (low energy)
        let member2 = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 10.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member1.to_bits().get(), CellRole::Locomotion);
        body.add_member(member2.to_bits().get(), CellRole::Sensing);
        world.ecs.insert_one(leader, body).unwrap();

        run(&mut world);

        // The weaker member (member2) should be released.
        let body = world.ecs.get::<&CompositeBody>(leader).unwrap();
        assert_eq!(
            body.member_count(),
            1,
            "one member should remain after partial decomposition"
        );
        assert!(
            body.has_member(member1.to_bits().get()),
            "the stronger member should remain"
        );
        assert!(
            !body.has_member(member2.to_bits().get()),
            "the weaker member should be released"
        );
        assert!(
            world.ecs.get::<&CompositeMemberMarker>(member2).is_err(),
            "released member should lose marker"
        );
    }

    #[test]
    fn merge_emits_composite_formed_event() {
        let mut world = test_world();
        spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        run(&mut world);

        let formed_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::CompositeFormed { .. }))
            .collect();
        assert_eq!(
            formed_events.len(),
            1,
            "should emit exactly one CompositeFormed event"
        );
    }

    #[test]
    fn full_decomposition_emits_event() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 5.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        let member = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy::default(),
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member.to_bits().get(), CellRole::Defense);
        world.ecs.insert_one(leader, body).unwrap();

        run(&mut world);

        let decomp_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::CompositeDecomposed { .. }))
            .collect();
        assert_eq!(
            decomp_events.len(),
            1,
            "should emit CompositeDecomposed event"
        );
    }

    #[test]
    fn healthy_composite_does_not_decompose() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 80.0, // healthy
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        let member = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy::default(),
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member.to_bits().get(), CellRole::Locomotion);
        world.ecs.insert_one(leader, body).unwrap();

        run(&mut world);

        assert!(
            world.ecs.get::<&CompositeBody>(leader).is_ok(),
            "healthy composite should not decompose"
        );
        assert_eq!(
            world.ecs.get::<&CompositeBody>(leader).unwrap().member_count(),
            1
        );
    }

    #[test]
    fn member_with_marker_cannot_initiate_merge() {
        let mut world = test_world();
        // Create A as a composite leader.
        let a = spawn_entity_for_composition(
            &mut world, 50.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        let a_id = a.to_bits().get();

        // Create B as a member of A.
        let b = spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );
        let _ = world.ecs.insert_one(b, CompositeMemberMarker { leader_id: a_id });

        // Create C as a standalone entity.
        let c = spawn_entity_for_composition(
            &mut world, 56.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        // B already has a marker, so B should not be able to merge with C.
        run_merging(&mut world);

        // C should not become a member of anything through B.
        // But A might merge with C since A is attempting composition.
        // The key test is that B (already a member) is excluded from candidates.
        let c_marker = world.ecs.get::<&CompositeMemberMarker>(c);
        if c_marker.is_ok() {
            // C merged with A (which is fine), check that it points to A.
            assert_eq!(c_marker.unwrap().leader_id, a_id);
        }
    }

    #[test]
    fn max_composite_size_enforced() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 80.0,
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            Identity {
                generation: 0,
                parent_id: None,
                birth_tick: 0,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            Memory::default(),
            default_starter_bt(),
            Action::CompositionAttempt,
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        // Fill composite to max size.
        let mut body = CompositeBody::new(leader_id, 0);
        for i in 0..MAX_COMPOSITE_SIZE {
            let member_genome = Genome::default();
            let member = world.ecs.spawn((
                Position { x: 50.0, y: 50.0, z: 0.0 },
                Velocity::default(),
                Energy::default(),
                member_genome,
                CompositeMemberMarker { leader_id },
            ));
            body.add_member(member.to_bits().get(), CellRole::Undifferentiated);
            let _ = i; // suppress warning
        }
        world.ecs.insert_one(leader, body).unwrap();

        // Now try to merge another entity.
        let extra = spawn_entity_for_composition(
            &mut world, 55.0, 50.0, Action::CompositionAttempt, 0.5,
        );

        run_merging(&mut world);

        // Extra should NOT have been absorbed.
        assert!(
            world.ecs.get::<&CompositeMemberMarker>(extra).is_err(),
            "composite at max size should not accept new members"
        );
    }

    #[test]
    fn energy_distribution_updates_members() {
        let mut world = test_world();
        let leader_genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 50.0, // 50% of max
                max: 100.0,
                metabolism_rate: 0.1,
            },
            Health::default(),
            Age::default(),
            Size::default(),
            leader_genome,
        ));
        let leader_id = leader.to_bits().get();

        let member = world.ecs.spawn((
            Position { x: 50.0, y: 50.0, z: 0.0 },
            Velocity::default(),
            Energy {
                current: 100.0, // starts at full
                max: 80.0,
                metabolism_rate: 0.1,
            },
            Genome::default(),
            CompositeMemberMarker { leader_id },
        ));

        let mut body = CompositeBody::new(leader_id, 0);
        body.add_member(member.to_bits().get(), CellRole::Locomotion);
        world.ecs.insert_one(leader, body).unwrap();

        distribute_energy(&mut world);

        // Member energy should be 50% of its max (80 * 0.5 = 40).
        let member_energy = world.ecs.get::<&Energy>(member).unwrap();
        assert!(
            (member_energy.current - 40.0).abs() < f64::EPSILON,
            "member energy should be fraction of its max, got {}",
            member_energy.current
        );
    }
}

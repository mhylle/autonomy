use crate::components::action::Action;
use crate::components::behavior_tree::BtNode;
use crate::components::bt_ops;
use crate::components::composite::{
    CellRole, CompositeBody, CompositeMember, CompositeMemberMarker, CompositionPattern,
};
use crate::components::drives::Drives;
use crate::components::genome::{mutate, Genome};
use crate::components::identity::Identity;
use crate::components::memory::Memory;
use crate::components::perception::Perception;
use crate::components::physical::{Age, Energy, Health, Size};
use crate::components::social::Social;
use crate::components::spatial::{Position, Velocity};
use crate::core::world::SimulationWorld;
use crate::events::types::SimEvent;
use rand::Rng;

/// Energy threshold fraction for composite reproduction.
const COMPOSITE_REPRODUCTION_THRESHOLD: f64 = 0.7;

/// Minimum number of members a composite must have to reproduce.
const MIN_MEMBERS_FOR_REPRODUCTION: usize = 2;

/// Maximum mutation delta for composition pattern counts.
const PATTERN_MUTATION_DELTA: i8 = 1;

/// Composite reproduction system.
///
/// When a composite organism has enough energy and sufficient members,
/// it reproduces by:
/// 1. Using the leader's genome as the primary template
/// 2. Incorporating contributions from member genomes
/// 3. Copying and mutating the composition pattern
/// 4. Creating new member entities with mutated genomes for the offspring
pub fn run(world: &mut SimulationWorld) {
    let mut rng = world.rng.tick_rng("composite_reproduction", world.tick);
    let current_tick = world.tick;

    // Collect candidate composites: leader entity, position, energy, genome, bt, identity, body.
    let candidates: Vec<_> = world
        .ecs
        .query::<(
            &Position,
            &Energy,
            &Genome,
            &BtNode,
            &Identity,
            &CompositeBody,
        )>()
        .iter()
        .filter(|(_, (_, energy, genome, _, _, body))| {
            energy.current > genome.max_energy * COMPOSITE_REPRODUCTION_THRESHOLD
                && body.member_count() >= MIN_MEMBERS_FOR_REPRODUCTION
        })
        .map(|(entity, (pos, energy, genome, bt, identity, body))| {
            (
                entity,
                pos.clone(),
                energy.current,
                genome.clone(),
                bt.clone(),
                identity.generation,
                body.clone(),
            )
        })
        .collect();

    // Collect member genomes for blending: for each candidate, gather
    // the genomes of its member entities.
    let candidate_member_genomes: Vec<Vec<(CellRole, Genome)>> = candidates
        .iter()
        .map(|(_, _, _, _, _, _, body)| {
            body.members
                .iter()
                .filter_map(|member| {
                    // Try to find the member entity and read its genome.
                    // Members are stored by entity ID bits. We iterate the ECS
                    // to find matching entities. This is safe because we only read.
                    let mut found = None;
                    for (entity, genome) in world.ecs.query::<&Genome>().iter() {
                        if entity.to_bits().get() == member.entity_id {
                            found = Some((member.role, genome.clone()));
                            break;
                        }
                    }
                    found
                })
                .collect()
        })
        .collect();

    for (idx, (parent_entity, parent_pos, parent_energy, parent_genome, parent_bt, parent_gen, parent_body)) in
        candidates.iter().enumerate()
    {
        let member_genomes = &candidate_member_genomes[idx];
        let offspring_energy = parent_energy / 2.0;

        // 1. Create offspring leader genome: leader genome + member contributions.
        let offspring_leader_genome =
            blend_leader_with_members(parent_genome, member_genomes, &mut rng);

        // 2. Derive composition pattern from parent and mutate.
        let parent_pattern = CompositionPattern::from_members(&parent_body.members);
        let offspring_pattern = mutate_composition_pattern(&parent_pattern, &mut rng);

        // 3. Mutate the behavior tree.
        let offspring_bt = bt_ops::mutate_parameters(parent_bt, 0.05, &mut rng);
        let offspring_bt = bt_ops::simplify(&offspring_bt);

        // Reduce parent energy.
        if let Ok(mut energy) = world.ecs.get::<&mut Energy>(*parent_entity) {
            energy.current = offspring_energy;
        }

        // Offspring position with small offset.
        let offset_x: f64 = rng.gen_range(-15.0..15.0);
        let offset_y: f64 = rng.gen_range(-15.0..15.0);
        let offspring_x = (parent_pos.x + offset_x).rem_euclid(world.config.world_width);
        let offspring_y = (parent_pos.y + offset_y).rem_euclid(world.config.world_height);

        let offspring_memory = Memory::new(
            offspring_leader_genome.memory_capacity as usize,
            offspring_leader_genome.eviction_weights.clone(),
        );

        // Spawn offspring leader entity first to get its ID.
        let offspring_entity = world.ecs.spawn((
            Position {
                x: offspring_x,
                y: offspring_y,
            },
            Velocity::default(),
            Energy {
                current: offspring_energy,
                max: offspring_leader_genome.max_energy,
                metabolism_rate: offspring_leader_genome.metabolism_rate,
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
            Age {
                ticks: 0,
                max_lifespan: offspring_leader_genome.max_lifespan,
            },
            Size {
                radius: offspring_leader_genome.size,
            },
            offspring_leader_genome,
            Identity {
                generation: parent_gen + 1,
                parent_id: Some(parent_entity.to_bits().get()),
                birth_tick: current_tick,
            },
            Perception::default(),
            Drives::default(),
            Social::default(),
            offspring_memory,
            offspring_bt,
            Action::default(),
        ));

        let offspring_leader_id = offspring_entity.to_bits().get();

        // 4. Create member entities for the offspring composite.
        let roles = offspring_pattern.member_roles();
        let mut offspring_members = Vec::with_capacity(roles.len());

        for role in &roles {
            // Find a parent member with the same role to use as template.
            let template_genome = member_genomes
                .iter()
                .find(|(r, _)| *r == *role)
                .map(|(_, g)| g)
                .or_else(|| member_genomes.first().map(|(_, g)| g));

            let member_genome = if let Some(template) = template_genome {
                mutate(template, &mut rng)
            } else {
                // No parent members found, create from leader genome with mutation.
                mutate(&parent_genome, &mut rng)
            };

            // Spawn the member entity near the offspring.
            let member_entity = world.ecs.spawn((
                Position {
                    x: offspring_x,
                    y: offspring_y,
                },
                Velocity::default(),
                Energy {
                    current: offspring_energy * 0.3,
                    max: member_genome.max_energy,
                    metabolism_rate: member_genome.metabolism_rate,
                },
                Health {
                    current: 100.0,
                    max: 100.0,
                },
                Age {
                    ticks: 0,
                    max_lifespan: member_genome.max_lifespan,
                },
                Size {
                    radius: member_genome.size,
                },
                member_genome,
                Identity {
                    generation: parent_gen + 1,
                    parent_id: Some(parent_entity.to_bits().get()),
                    birth_tick: current_tick,
                },
                Perception::default(),
                Drives::default(),
                Social::default(),
                Memory::default(),
                Action::default(),
                CompositeMemberMarker {
                    leader_id: offspring_leader_id,
                },
            ));

            offspring_members.push(CompositeMember {
                entity_id: member_entity.to_bits().get(),
                role: *role,
            });
        }

        let member_count = offspring_members.len();

        // Attach the CompositeBody to the offspring leader.
        let mut offspring_body =
            CompositeBody::new(offspring_leader_id, current_tick);
        offspring_body.members = offspring_members;

        world
            .ecs
            .insert_one(offspring_entity, offspring_body)
            .expect("offspring entity should exist");

        world.event_log.push(SimEvent::CompositeReproduced {
            parent_id: parent_entity.to_bits().get(),
            offspring_id: offspring_leader_id,
            member_count,
            x: offspring_x,
            y: offspring_y,
        });
    }
}

/// Blend the leader genome with contributions from member genomes.
///
/// The leader genome provides 60% of each trait, while the average
/// of member genomes contributes 40%. The result is then mutated.
fn blend_leader_with_members(
    leader: &Genome,
    members: &[(CellRole, Genome)],
    rng: &mut rand_chacha::ChaCha8Rng,
) -> Genome {
    use crate::components::drives::DriveWeights;
    use crate::components::genome::compute_species_id;
    use crate::components::memory::EvictionWeights;

    if members.is_empty() {
        return mutate(leader, rng);
    }

    let leader_weight = 0.6;
    let member_weight = 0.4;

    let avg = |leader_val: f64, member_vals: &[f64]| -> f64 {
        let member_avg = member_vals.iter().sum::<f64>() / member_vals.len() as f64;
        leader_val * leader_weight + member_avg * member_weight
    };

    let member_genomes: Vec<&Genome> = members.iter().map(|(_, g)| g).collect();

    let blended = Genome {
        max_energy: avg(
            leader.max_energy,
            &member_genomes.iter().map(|g| g.max_energy).collect::<Vec<_>>(),
        ),
        metabolism_rate: avg(
            leader.metabolism_rate,
            &member_genomes
                .iter()
                .map(|g| g.metabolism_rate)
                .collect::<Vec<_>>(),
        ),
        max_speed: avg(
            leader.max_speed,
            &member_genomes
                .iter()
                .map(|g| g.max_speed)
                .collect::<Vec<_>>(),
        ),
        sensor_range: avg(
            leader.sensor_range,
            &member_genomes
                .iter()
                .map(|g| g.sensor_range)
                .collect::<Vec<_>>(),
        ),
        size: avg(
            leader.size,
            &member_genomes.iter().map(|g| g.size).collect::<Vec<_>>(),
        ),
        max_lifespan: {
            let avg_lifespan = avg(
                leader.max_lifespan as f64,
                &member_genomes
                    .iter()
                    .map(|g| g.max_lifespan as f64)
                    .collect::<Vec<_>>(),
            );
            avg_lifespan.max(100.0) as u64
        },
        mutation_rate: avg(
            leader.mutation_rate,
            &member_genomes
                .iter()
                .map(|g| g.mutation_rate)
                .collect::<Vec<_>>(),
        )
        .clamp(0.001, 0.5),
        drive_weights: DriveWeights {
            base_curiosity: avg(
                leader.drive_weights.base_curiosity,
                &member_genomes
                    .iter()
                    .map(|g| g.drive_weights.base_curiosity)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            base_social_need: avg(
                leader.drive_weights.base_social_need,
                &member_genomes
                    .iter()
                    .map(|g| g.drive_weights.base_social_need)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            base_aggression: avg(
                leader.drive_weights.base_aggression,
                &member_genomes
                    .iter()
                    .map(|g| g.drive_weights.base_aggression)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            base_reproductive: avg(
                leader.drive_weights.base_reproductive,
                &member_genomes
                    .iter()
                    .map(|g| g.drive_weights.base_reproductive)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
        },
        memory_capacity: {
            let avg_cap = avg(
                leader.memory_capacity as f64,
                &member_genomes
                    .iter()
                    .map(|g| g.memory_capacity as f64)
                    .collect::<Vec<_>>(),
            );
            avg_cap.clamp(1.0, 200.0) as u16
        },
        eviction_weights: EvictionWeights {
            recency_weight: avg(
                leader.eviction_weights.recency_weight,
                &member_genomes
                    .iter()
                    .map(|g| g.eviction_weights.recency_weight)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            importance_weight: avg(
                leader.eviction_weights.importance_weight,
                &member_genomes
                    .iter()
                    .map(|g| g.eviction_weights.importance_weight)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            emotional_weight: avg(
                leader.eviction_weights.emotional_weight,
                &member_genomes
                    .iter()
                    .map(|g| g.eviction_weights.emotional_weight)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
            variety_weight: avg(
                leader.eviction_weights.variety_weight,
                &member_genomes
                    .iter()
                    .map(|g| g.eviction_weights.variety_weight)
                    .collect::<Vec<_>>(),
            )
            .clamp(0.0, 1.0),
        },
        composition_affinity: avg(
            leader.composition_affinity,
            &member_genomes
                .iter()
                .map(|g| g.composition_affinity)
                .collect::<Vec<_>>(),
        )
        .clamp(0.0, 1.0),
        blueprint: leader.blueprint.clone(),
        species_id: 0,
    };

    let mut result = mutate(&blended, rng);
    result.species_id = compute_species_id(&result);
    result
}

/// Mutate a composition pattern.
///
/// Each count has a 30% chance to increase or decrease by 1, clamped to 0..10.
fn mutate_composition_pattern(
    pattern: &CompositionPattern,
    rng: &mut rand_chacha::ChaCha8Rng,
) -> CompositionPattern {
    let mut mutate_count = |count: u8| -> u8 {
        if rng.gen::<f64>() < 0.3 {
            let delta: i8 = rng.gen_range(-PATTERN_MUTATION_DELTA..=PATTERN_MUTATION_DELTA);
            (count as i8 + delta).clamp(0, 10) as u8
        } else {
            count
        }
    };

    CompositionPattern {
        sensing_count: mutate_count(pattern.sensing_count),
        locomotion_count: mutate_count(pattern.locomotion_count),
        attack_count: mutate_count(pattern.attack_count),
        defense_count: mutate_count(pattern.defense_count),
        digestion_count: mutate_count(pattern.digestion_count),
        reproduction_count: mutate_count(pattern.reproduction_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::behavior_tree::default_starter_bt;
    use crate::components::composite::CellRole;
    use crate::core::config::SimulationConfig;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Spawn a composite entity with the given energy and member count.
    /// Members are real entities in the ECS with genomes.
    fn make_composite_entity(
        world: &mut SimulationWorld,
        energy: f64,
        member_count: usize,
    ) -> hecs::Entity {
        let genome = Genome::default();
        let leader = world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Velocity::default(),
            Energy {
                current: energy,
                max: genome.max_energy,
                metabolism_rate: genome.metabolism_rate,
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
            Age {
                ticks: 0,
                max_lifespan: genome.max_lifespan,
            },
            Size {
                radius: genome.size,
            },
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
            genome,
        ));

        let leader_id = leader.to_bits().get();
        let roles = [
            CellRole::Sensing,
            CellRole::Locomotion,
            CellRole::Attack,
            CellRole::Defense,
            CellRole::Digestion,
        ];

        let mut body = CompositeBody::new(leader_id, 0);

        for i in 0..member_count {
            let member_genome = Genome::default();
            let member = world.ecs.spawn((
                Position { x: 50.0, y: 50.0 },
                Velocity::default(),
                Energy {
                    current: 50.0,
                    max: member_genome.max_energy,
                    metabolism_rate: member_genome.metabolism_rate,
                },
                Health {
                    current: 100.0,
                    max: 100.0,
                },
                Age {
                    ticks: 0,
                    max_lifespan: member_genome.max_lifespan,
                },
                Size {
                    radius: member_genome.size,
                },
                Identity {
                    generation: 0,
                    parent_id: None,
                    birth_tick: 0,
                },
                Perception::default(),
                Drives::default(),
                Social::default(),
                Memory::default(),
                Action::default(),
                member_genome,
                CompositeMemberMarker {
                    leader_id,
                },
            ));

            body.add_member(member.to_bits().get(), roles[i % roles.len()]);
        }

        world.ecs.insert_one(leader, body).unwrap();
        leader
    }

    #[test]
    fn composite_below_threshold_does_not_reproduce() {
        let mut world = test_world();
        // 3 members but low energy
        make_composite_entity(&mut world, 50.0, 3);

        let initial_count = world.entity_count();
        run(&mut world);
        assert_eq!(world.entity_count(), initial_count);
    }

    #[test]
    fn composite_above_threshold_with_enough_members_reproduces() {
        let mut world = test_world();
        // 3 members + leader = 4 entities, high energy
        make_composite_entity(&mut world, 90.0, 3);

        let initial_count = world.entity_count(); // 4
        run(&mut world);
        assert!(
            world.entity_count() > initial_count,
            "composite should have reproduced: {} vs {}",
            world.entity_count(),
            initial_count
        );
    }

    #[test]
    fn composite_with_too_few_members_does_not_reproduce() {
        let mut world = test_world();
        // 1 member (below MIN_MEMBERS_FOR_REPRODUCTION)
        make_composite_entity(&mut world, 90.0, 1);

        let initial_count = world.entity_count();
        run(&mut world);
        assert_eq!(world.entity_count(), initial_count);
    }

    #[test]
    fn offspring_has_composite_body() {
        let mut world = test_world();
        make_composite_entity(&mut world, 90.0, 3);

        run(&mut world);

        let mut found_offspring_composite = false;
        for (_entity, (identity, _body)) in
            world.ecs.query_mut::<(&Identity, &CompositeBody)>()
        {
            if identity.generation == 1 {
                found_offspring_composite = true;
            }
        }
        assert!(
            found_offspring_composite,
            "offspring should have a CompositeBody component"
        );
    }

    #[test]
    fn offspring_has_members() {
        let mut world = test_world();
        make_composite_entity(&mut world, 90.0, 4);

        run(&mut world);

        for (_entity, (identity, body)) in
            world.ecs.query_mut::<(&Identity, &CompositeBody)>()
        {
            if identity.generation == 1 {
                assert!(
                    body.member_count() > 0,
                    "offspring composite should have members, got {}",
                    body.member_count()
                );
                return;
            }
        }
        panic!("no offspring composite found");
    }

    #[test]
    fn parent_energy_halved_after_composite_reproduction() {
        let mut world = test_world();
        let parent = make_composite_entity(&mut world, 90.0, 3);

        run(&mut world);

        let parent_energy = world.ecs.get::<&Energy>(parent).unwrap();
        assert!(
            (parent_energy.current - 45.0).abs() < f64::EPSILON,
            "parent energy should be halved, got {}",
            parent_energy.current
        );
    }

    #[test]
    fn composite_reproduction_emits_event() {
        let mut world = test_world();
        make_composite_entity(&mut world, 90.0, 3);

        run(&mut world);

        let composite_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::CompositeReproduced { .. }))
            .collect();

        assert_eq!(
            composite_events.len(),
            1,
            "should emit exactly one CompositeReproduced event"
        );
    }

    #[test]
    fn blend_leader_with_members_produces_blended_genome() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let leader = Genome {
            max_energy: 100.0,
            sensor_range: 50.0,
            ..Genome::default()
        };
        let members = vec![(
            CellRole::Sensing,
            Genome {
                max_energy: 200.0,
                sensor_range: 100.0,
                ..Genome::default()
            },
        )];

        let blended = blend_leader_with_members(&leader, &members, &mut rng);

        // Blended should be between leader and member values (before mutation).
        // Leader contributes 60%, member 40%:
        // max_energy: 0.6*100 + 0.4*200 = 140 (before mutation)
        // With mutation, values should be in a reasonable range.
        assert!(
            blended.max_energy > 50.0 && blended.max_energy < 250.0,
            "blended max_energy should be in reasonable range, got {}",
            blended.max_energy
        );
    }

    #[test]
    fn mutate_composition_pattern_stays_in_bounds() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let pattern = CompositionPattern {
            sensing_count: 0,
            locomotion_count: 10,
            attack_count: 5,
            defense_count: 0,
            digestion_count: 10,
            reproduction_count: 0,
        };

        for _ in 0..100 {
            let mutated = mutate_composition_pattern(&pattern, &mut rng);
            assert!(mutated.sensing_count <= 10);
            assert!(mutated.locomotion_count <= 10);
            assert!(mutated.attack_count <= 10);
            assert!(mutated.defense_count <= 10);
            assert!(mutated.digestion_count <= 10);
            assert!(mutated.reproduction_count <= 10);
        }
    }

    #[test]
    fn offspring_members_have_marker() {
        let mut world = test_world();
        make_composite_entity(&mut world, 90.0, 3);

        run(&mut world);

        // Find the offspring composite.
        let mut offspring_leader_id = None;
        for (entity, (identity, _body)) in
            world.ecs.query_mut::<(&Identity, &CompositeBody)>()
        {
            if identity.generation == 1 {
                offspring_leader_id = Some(entity.to_bits().get());
                break;
            }
        }

        let leader_id = offspring_leader_id.expect("should find offspring composite");

        // Check that offspring members have the CompositeMemberMarker.
        let mut markers_found = 0;
        for (_entity, marker) in world.ecs.query_mut::<&CompositeMemberMarker>() {
            if marker.leader_id == leader_id {
                markers_found += 1;
            }
        }

        assert!(
            markers_found > 0,
            "offspring members should have CompositeMemberMarker pointing to offspring leader"
        );
    }

    #[test]
    fn offspring_generation_incremented() {
        let mut world = test_world();
        make_composite_entity(&mut world, 90.0, 3);

        run(&mut world);

        let mut found = false;
        for (_entity, (identity, _body)) in
            world.ecs.query_mut::<(&Identity, &CompositeBody)>()
        {
            if identity.generation == 1 {
                found = true;
                assert!(identity.parent_id.is_some());
            }
        }
        assert!(found, "offspring should have generation=1");
    }

    #[test]
    fn blend_with_empty_members_still_produces_genome() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let leader = Genome::default();
        let blended = blend_leader_with_members(&leader, &[], &mut rng);

        // Should produce a valid mutated genome even with no members.
        assert!(blended.max_energy > 0.0);
        assert!(blended.metabolism_rate > 0.0);
    }

    #[test]
    fn composition_pattern_from_members_roundtrip() {
        let members = vec![
            CompositeMember {
                entity_id: 1,
                role: CellRole::Sensing,
            },
            CompositeMember {
                entity_id: 2,
                role: CellRole::Sensing,
            },
            CompositeMember {
                entity_id: 3,
                role: CellRole::Locomotion,
            },
            CompositeMember {
                entity_id: 4,
                role: CellRole::Attack,
            },
        ];

        let pattern = CompositionPattern::from_members(&members);
        assert_eq!(pattern.sensing_count, 2);
        assert_eq!(pattern.locomotion_count, 1);
        assert_eq!(pattern.attack_count, 1);
        assert_eq!(pattern.defense_count, 0);
    }
}

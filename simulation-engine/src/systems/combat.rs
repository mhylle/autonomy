use crate::components::action::Action;
use crate::components::genome::Genome;
use crate::components::physical::{Energy, Health, Size};
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;
use crate::events::types::{DeathCause, SimEvent};
use crate::systems::tribe::count_nearby_allies;

/// Maximum distance at which an attack can land.
const ATTACK_RANGE: f64 = 15.0;

/// Base damage multiplier (damage = size * force * BASE_DAMAGE).
const BASE_DAMAGE: f64 = 5.0;

/// Fraction of the killed target's remaining energy gained by the attacker.
const KILL_ENERGY_FRACTION: f64 = 0.5;

/// Range within which allies/enemies are counted for morale.
const MORALE_RANGE: f64 = 30.0;

/// Maximum morale bonus from nearby allies (multiplicative on damage).
const MAX_MORALE_BONUS: f64 = 0.5;

/// Morale bonus per nearby ally.
const MORALE_PER_ALLY: f64 = 0.15;

/// Resolves combat actions each tick.
///
/// For each entity with `Action::Attack`, finds the target entity,
/// checks range, computes damage from attacker size * force_factor,
/// applies damage to target health, and handles death + energy gain.
pub fn run(world: &mut SimulationWorld) {
    // 1. Collect all attack intents.
    let attacks: Vec<_> = world
        .ecs
        .query::<(&Action, &Position, &Size)>()
        .iter()
        .filter_map(|(attacker, (action, pos, size))| {
            if let Action::Attack { target_id, force } = action {
                Some((attacker, *target_id, pos.x, pos.y, size.radius, *force))
            } else {
                None
            }
        })
        .collect();

    // 2. Resolve each attack.
    let mut kills: Vec<(hecs::Entity, f64)> = Vec::new(); // (attacker, energy_gained)
    let mut species_kills: Vec<(u64, u64)> = Vec::new(); // (attacker_species, victim_species)

    for (attacker, target_id, ax, ay, attacker_size, force) in &attacks {
        // Find the target entity by its ID bits.
        let target = find_entity_by_id(&world.ecs, *target_id);
        let target = match target {
            Some(t) => t,
            None => continue, // target no longer exists
        };

        // Don't attack yourself.
        if *attacker == target {
            continue;
        }

        // Check range.
        let (tx, ty) = match world.ecs.get::<&Position>(target) {
            Ok(pos) => (pos.x, pos.y),
            Err(_) => continue,
        };
        let dist = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
        if dist > ATTACK_RANGE {
            continue;
        }

        // Compute morale bonus from nearby tribe allies.
        let attacker_id_bits = attacker.to_bits().get();
        let allies = count_nearby_allies(world, attacker_id_bits, *ax, *ay, MORALE_RANGE);
        let morale_bonus = (allies as f64 * MORALE_PER_ALLY).min(MAX_MORALE_BONUS);

        // Compute damage with morale modifier.
        let damage = attacker_size * force * BASE_DAMAGE * (1.0 + morale_bonus);

        // Apply damage to target health.
        let (target_health_remaining, target_energy) = {
            let mut health = match world.ecs.get::<&mut Health>(target) {
                Ok(h) => h,
                Err(_) => continue, // target has no Health component
            };
            health.current = (health.current - damage).max(0.0);
            let remaining = health.current;
            drop(health);

            let energy = world
                .ecs
                .get::<&Energy>(target)
                .map(|e| e.current)
                .unwrap_or(0.0);
            (remaining, energy)
        };

        // Emit attack event.
        world.event_log.push(SimEvent::EntityAttacked {
            attacker_id: attacker.to_bits().get(),
            target_id: *target_id,
            damage,
            target_health_remaining,
        });

        // If target died from combat damage.
        if target_health_remaining <= 0.0 {
            let energy_gained = target_energy * KILL_ENERGY_FRACTION;
            kills.push((*attacker, energy_gained));

            // Record species interaction for the kill matrix.
            let attacker_species = world
                .ecs
                .get::<&Genome>(*attacker)
                .map(|g| g.species_id)
                .unwrap_or(0);
            let victim_species = world
                .ecs
                .get::<&Genome>(target)
                .map(|g| g.species_id)
                .unwrap_or(0);
            species_kills.push((attacker_species, victim_species));

            // Get target position for death event.
            let (dx, dy) = world
                .ecs
                .get::<&Position>(target)
                .map(|p| (p.x, p.y))
                .unwrap_or((0.0, 0.0));

            let target_age = world
                .ecs
                .get::<&crate::components::physical::Age>(target)
                .map(|a| a.ticks)
                .unwrap_or(0);

            world.event_log.push(SimEvent::EntityDied {
                entity_id: *target_id,
                x: dx,
                y: dy,
                age: target_age,
                cause: DeathCause::Combat {
                    killer_id: attacker.to_bits().get(),
                },
            });

            // Set target energy to 0 so cleanup system removes it.
            if let Ok(mut energy) = world.ecs.get::<&mut Energy>(target) {
                energy.current = 0.0;
            }
        }
    }

    // 3. Award energy to attackers that scored kills.
    for (attacker, energy_gained) in kills {
        if let Ok(mut energy) = world.ecs.get::<&mut Energy>(attacker) {
            energy.current = (energy.current + energy_gained).min(energy.max);
        }
    }

    // 4. Update the species interaction kill matrix.
    for (attacker_species, victim_species) in species_kills {
        *world
            .kill_matrix
            .entry((attacker_species, victim_species))
            .or_insert(0) += 1;
    }
}

/// Find an entity by its `to_bits().get()` ID value.
fn find_entity_by_id(ecs: &hecs::World, id: u64) -> Option<hecs::Entity> {
    // Reconstruct the entity from its bits.
    let bits = std::num::NonZeroU64::new(id)?;
    let entity = hecs::Entity::from_bits(bits.get())?;
    if ecs.contains(entity) {
        Some(entity)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::action::Action;
    use crate::components::physical::{Age, Energy, Health, Size};
    use crate::components::spatial::Position;
    use crate::core::config::SimulationConfig;
    use crate::events::types::SimEvent;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    /// Spawn a combat-ready entity and return its hecs::Entity handle.
    fn spawn_combatant(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        health: f64,
        energy: f64,
        size: f64,
    ) -> hecs::Entity {
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Health {
                current: health,
                max: health,
            },
            Energy {
                current: energy,
                max: energy,
                metabolism_rate: 0.1,
            },
            Size { radius: size },
            Age::default(),
            Action::None,
        ))
    }

    #[test]
    fn attack_deals_damage_to_target() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        let attacker = spawn_combatant(&mut world, 55.0, 50.0, 100.0, 80.0, 5.0);

        // Set attacker's action to attack the target.
        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        // damage = 5.0 (size) * 1.0 (force) * 5.0 (BASE_DAMAGE) = 25.0
        assert!(
            (target_health.current - 75.0).abs() < 0.01,
            "target health should be 75.0, got {}",
            target_health.current
        );
    }

    #[test]
    fn attack_out_of_range_does_nothing() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        let attacker = spawn_combatant(&mut world, 200.0, 200.0, 100.0, 80.0, 5.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        assert_eq!(
            target_health.current, 100.0,
            "target should not take damage when out of range"
        );
    }

    #[test]
    fn lethal_attack_sets_target_energy_to_zero() {
        let mut world = test_world();

        // Target with very low health.
        let target = spawn_combatant(&mut world, 50.0, 50.0, 10.0, 60.0, 3.0);
        // Attacker with large size -> high damage.
        let attacker = spawn_combatant(&mut world, 52.0, 50.0, 100.0, 80.0, 10.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        // Target should be marked for cleanup (energy = 0).
        let target_energy = world.ecs.get::<&Energy>(target).unwrap();
        assert_eq!(
            target_energy.current, 0.0,
            "killed target should have 0 energy"
        );

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        assert_eq!(
            target_health.current, 0.0,
            "killed target should have 0 health"
        );
    }

    #[test]
    fn attacker_gains_energy_from_kill() {
        let mut world = test_world();

        // Target with low health but decent energy.
        let target = spawn_combatant(&mut world, 50.0, 50.0, 5.0, 100.0, 3.0);
        // Attacker has 50 current out of 200 max, so it has room to gain.
        let attacker = world.ecs.spawn((
            Position { x: 52.0, y: 50.0, z: 0.0 },
            Health {
                current: 100.0,
                max: 100.0,
            },
            Energy {
                current: 50.0,
                max: 200.0,
                metabolism_rate: 0.1,
            },
            Size { radius: 10.0 },
            Age::default(),
            Action::None,
        ));

        let attacker_energy_before = world.ecs.get::<&Energy>(attacker).unwrap().current;

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let attacker_energy_after = world.ecs.get::<&Energy>(attacker).unwrap().current;
        // Should gain 100.0 * 0.5 = 50.0 energy.
        assert!(
            attacker_energy_after > attacker_energy_before,
            "attacker should gain energy from kill: before={}, after={}",
            attacker_energy_before,
            attacker_energy_after
        );
    }

    #[test]
    fn attacker_energy_does_not_exceed_max() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 5.0, 200.0, 3.0);
        // Attacker nearly full.
        let attacker = spawn_combatant(&mut world, 52.0, 50.0, 100.0, 95.0, 10.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let attacker_energy = world.ecs.get::<&Energy>(attacker).unwrap();
        assert!(
            attacker_energy.current <= attacker_energy.max,
            "energy {} should not exceed max {}",
            attacker_energy.current,
            attacker_energy.max
        );
    }

    #[test]
    fn larger_attacker_deals_more_damage() {
        let mut world = test_world();

        // Two targets with same health.
        let target_a = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        let target_b = spawn_combatant(&mut world, 60.0, 50.0, 100.0, 80.0, 5.0);

        // Small attacker.
        let small = spawn_combatant(&mut world, 51.0, 50.0, 100.0, 80.0, 3.0);
        // Large attacker.
        let large = spawn_combatant(&mut world, 61.0, 50.0, 100.0, 80.0, 10.0);

        let tid_a = target_a.to_bits().get();
        let tid_b = target_b.to_bits().get();
        *world.ecs.get::<&mut Action>(small).unwrap() = Action::Attack {
            target_id: tid_a,
            force: 1.0,
        };
        *world.ecs.get::<&mut Action>(large).unwrap() = Action::Attack {
            target_id: tid_b,
            force: 1.0,
        };

        run(&mut world);

        let health_a = world.ecs.get::<&Health>(target_a).unwrap().current;
        let health_b = world.ecs.get::<&Health>(target_b).unwrap().current;

        // Large attacker deals more damage -> target_b has less health.
        assert!(
            health_b < health_a,
            "larger attacker should deal more damage: small target health={}, large target health={}",
            health_a,
            health_b
        );
    }

    #[test]
    fn higher_force_deals_more_damage() {
        let mut world = test_world();

        let target_a = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        let target_b = spawn_combatant(&mut world, 60.0, 50.0, 100.0, 80.0, 5.0);

        // Same size attackers, different force.
        let attacker_low = spawn_combatant(&mut world, 51.0, 50.0, 100.0, 80.0, 5.0);
        let attacker_high = spawn_combatant(&mut world, 61.0, 50.0, 100.0, 80.0, 5.0);

        let tid_a = target_a.to_bits().get();
        let tid_b = target_b.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker_low).unwrap() = Action::Attack {
            target_id: tid_a,
            force: 0.5,
        };
        *world.ecs.get::<&mut Action>(attacker_high).unwrap() = Action::Attack {
            target_id: tid_b,
            force: 2.0,
        };

        run(&mut world);

        let health_a = world.ecs.get::<&Health>(target_a).unwrap().current;
        let health_b = world.ecs.get::<&Health>(target_b).unwrap().current;

        assert!(
            health_b < health_a,
            "higher force should deal more damage: low_force target health={}, high_force target health={}",
            health_a,
            health_b
        );
    }

    #[test]
    fn attack_emits_entity_attacked_event() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        let attacker = spawn_combatant(&mut world, 55.0, 50.0, 100.0, 80.0, 5.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let attack_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::EntityAttacked { .. }))
            .collect();

        assert_eq!(
            attack_events.len(),
            1,
            "should emit exactly one EntityAttacked event"
        );

        if let SimEvent::EntityAttacked {
            attacker_id,
            target_id: tid,
            damage,
            ..
        } = &attack_events[0]
        {
            assert_eq!(*attacker_id, attacker.to_bits().get());
            assert_eq!(*tid, target_id);
            assert!(*damage > 0.0);
        }
    }

    #[test]
    fn kill_emits_entity_died_combat_event() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 5.0, 80.0, 3.0);
        let attacker = spawn_combatant(&mut world, 52.0, 50.0, 100.0, 80.0, 10.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let death_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::EntityDied { .. }))
            .collect();

        assert_eq!(death_events.len(), 1, "should emit EntityDied on kill");

        if let SimEvent::EntityDied {
            entity_id, cause, ..
        } = &death_events[0]
        {
            assert_eq!(*entity_id, target_id);
            assert!(
                matches!(cause, DeathCause::Combat { .. }),
                "death cause should be Combat"
            );
        }
    }

    #[test]
    fn no_self_attack() {
        let mut world = test_world();

        let entity = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);

        let self_id = entity.to_bits().get();
        *world.ecs.get::<&mut Action>(entity).unwrap() = Action::Attack {
            target_id: self_id,
            force: 1.0,
        };

        run(&mut world);

        let health = world.ecs.get::<&Health>(entity).unwrap();
        assert_eq!(
            health.current, 100.0,
            "entity should not be able to attack itself"
        );
    }

    #[test]
    fn attack_nonexistent_target_is_noop() {
        let mut world = test_world();

        let attacker = spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);

        // Use a bogus target ID.
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id: 999999,
            force: 1.0,
        };

        // Should not panic.
        run(&mut world);

        // No events should be emitted.
        assert!(
            world.event_log.is_empty(),
            "no events should be emitted for nonexistent target"
        );
    }

    #[test]
    fn multiple_attacks_in_same_tick() {
        let mut world = test_world();

        let target = spawn_combatant(&mut world, 50.0, 50.0, 200.0, 100.0, 5.0);
        let attacker_a = spawn_combatant(&mut world, 52.0, 50.0, 100.0, 80.0, 5.0);
        let attacker_b = spawn_combatant(&mut world, 48.0, 50.0, 100.0, 80.0, 5.0);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker_a).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };
        *world.ecs.get::<&mut Action>(attacker_b).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        // Each attacker: 5.0 * 1.0 * 5.0 = 25.0 damage, total 50.0
        assert!(
            (target_health.current - 150.0).abs() < 0.01,
            "target should take damage from both attackers, got {}",
            target_health.current
        );

        let attack_events: Vec<_> = world
            .event_log
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::EntityAttacked { .. }))
            .collect();
        assert_eq!(attack_events.len(), 2, "should emit two attack events");
    }

    #[test]
    fn no_attack_actions_is_noop() {
        let mut world = test_world();

        spawn_combatant(&mut world, 50.0, 50.0, 100.0, 80.0, 5.0);
        spawn_combatant(&mut world, 55.0, 50.0, 100.0, 80.0, 5.0);

        // No attack actions set -- both have Action::None by default.
        run(&mut world);

        assert!(
            world.event_log.is_empty(),
            "no events should be emitted when no attacks occur"
        );
    }

    /// Spawn a combat-ready entity with a Genome (for kill_matrix tracking).
    fn spawn_combatant_with_genome(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        health: f64,
        energy: f64,
        size: f64,
        genome: Genome,
    ) -> hecs::Entity {
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Health {
                current: health,
                max: health,
            },
            Energy {
                current: energy,
                max: energy,
                metabolism_rate: 0.1,
            },
            Size { radius: size },
            Age::default(),
            Action::None,
            genome,
        ))
    }

    #[test]
    fn kill_records_species_in_kill_matrix() {
        use crate::components::genome::{compute_species_id, Genome};

        let mut world = test_world();

        // Create two genomes with different species IDs.
        // Use very different traits to guarantee different species hashes.
        let mut predator_genome = Genome {
            max_speed: 10.0,
            size: 30.0,
            max_energy: 500.0,
            metabolism_rate: 0.5,
            max_lifespan: 10000,
            ..Genome::default()
        };
        predator_genome.species_id = compute_species_id(&predator_genome);

        let prey_genome = Genome::default();

        let predator_species = predator_genome.species_id;
        let prey_species = prey_genome.species_id;

        // Ensure they are actually different species.
        assert_ne!(
            predator_species, prey_species,
            "test genomes should produce different species IDs"
        );

        // Spawn: prey with low health (will die), predator with large size (high damage).
        let prey = spawn_combatant_with_genome(
            &mut world, 50.0, 50.0, 5.0, 80.0, 3.0, prey_genome,
        );
        let predator = spawn_combatant_with_genome(
            &mut world, 52.0, 50.0, 100.0, 80.0, 10.0, predator_genome,
        );

        let prey_id = prey.to_bits().get();
        *world.ecs.get::<&mut Action>(predator).unwrap() = Action::Attack {
            target_id: prey_id,
            force: 1.0,
        };

        // Kill matrix should be empty before combat.
        assert!(world.kill_matrix.is_empty(), "kill matrix should start empty");

        run(&mut world);

        // Kill matrix should now have an entry for (predator_species, prey_species).
        assert!(
            !world.kill_matrix.is_empty(),
            "kill matrix should have entries after a lethal attack"
        );

        let kill_count = world
            .kill_matrix
            .get(&(predator_species, prey_species))
            .copied()
            .unwrap_or(0);

        assert_eq!(
            kill_count, 1,
            "kill matrix should record 1 kill for (predator_species, prey_species)"
        );
    }

    #[test]
    fn non_lethal_attack_does_not_update_kill_matrix() {
        use crate::components::genome::Genome;

        let mut world = test_world();

        let genome_a = Genome::default();
        let genome_b = Genome::default();

        // Target with plenty of health (won't die from one attack).
        let target = spawn_combatant_with_genome(
            &mut world, 50.0, 50.0, 200.0, 80.0, 5.0, genome_a,
        );
        let attacker = spawn_combatant_with_genome(
            &mut world, 52.0, 50.0, 100.0, 80.0, 5.0, genome_b,
        );

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        // No kill occurred, so kill matrix should remain empty.
        assert!(
            world.kill_matrix.is_empty(),
            "kill matrix should not be updated for non-lethal attacks"
        );
    }

    // ---- Phase 5.6: Group combat / morale tests ----

    use crate::components::social::Social;
    use crate::components::tribe::{Tribe, TribeId};
    use std::collections::HashSet;

    /// Spawn a combat-ready entity with a TribeId component.
    fn spawn_tribal_combatant(
        world: &mut SimulationWorld,
        x: f64,
        y: f64,
        health: f64,
        energy: f64,
        size: f64,
        tribe_id: Option<u64>,
    ) -> hecs::Entity {
        world.ecs.spawn((
            Position { x, y, z: 0.0 },
            Health {
                current: health,
                max: health,
            },
            Energy {
                current: energy,
                max: energy,
                metabolism_rate: 0.1,
            },
            Size { radius: size },
            Age::default(),
            Action::None,
            TribeId(tribe_id),
            Social::default(),
        ))
    }

    #[test]
    fn morale_bonus_increases_damage_with_allies_nearby() {
        let mut world = test_world();

        let tribe_id = 1u64;

        // Target with lots of health.
        let target = spawn_tribal_combatant(&mut world, 50.0, 50.0, 500.0, 80.0, 5.0, Some(2));

        // Attacker with allies nearby.
        let attacker = spawn_tribal_combatant(&mut world, 52.0, 50.0, 100.0, 80.0, 5.0, Some(tribe_id));
        let ally1 = spawn_tribal_combatant(&mut world, 54.0, 50.0, 100.0, 80.0, 5.0, Some(tribe_id));
        let ally2 = spawn_tribal_combatant(&mut world, 56.0, 50.0, 100.0, 80.0, 5.0, Some(tribe_id));

        let attacker_id = attacker.to_bits().get();
        let ally1_id = ally1.to_bits().get();
        let ally2_id = ally2.to_bits().get();

        // Register tribe.
        let members: HashSet<u64> = [attacker_id, ally1_id, ally2_id].into_iter().collect();
        world.tribes.insert(tribe_id, Tribe::new(tribe_id, members, 54.0, 50.0, 0));

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        // Base damage = 5.0 * 1.0 * 5.0 = 25.0
        // With 2 allies: morale = 2 * 0.15 = 0.30, damage = 25.0 * 1.30 = 32.5
        let expected_damage = 25.0 * (1.0 + 2.0 * MORALE_PER_ALLY);
        let actual_damage = 500.0 - target_health.current;
        assert!(
            (actual_damage - expected_damage).abs() < 0.01,
            "damage with allies should be {}, got {}",
            expected_damage,
            actual_damage
        );
    }

    #[test]
    fn no_morale_bonus_without_tribe() {
        let mut world = test_world();

        // Target.
        let target = spawn_tribal_combatant(&mut world, 50.0, 50.0, 500.0, 80.0, 5.0, None);

        // Attacker without a tribe.
        let attacker = spawn_tribal_combatant(&mut world, 52.0, 50.0, 100.0, 80.0, 5.0, None);

        // Other entity nearby but no tribe connection.
        spawn_tribal_combatant(&mut world, 54.0, 50.0, 100.0, 80.0, 5.0, None);

        let target_id = target.to_bits().get();
        *world.ecs.get::<&mut Action>(attacker).unwrap() = Action::Attack {
            target_id,
            force: 1.0,
        };

        run(&mut world);

        let target_health = world.ecs.get::<&Health>(target).unwrap();
        // Base damage = 5.0 * 1.0 * 5.0 = 25.0, no morale bonus.
        let actual_damage = 500.0 - target_health.current;
        assert!(
            (actual_damage - 25.0).abs() < 0.01,
            "damage without tribe should be 25.0, got {}",
            actual_damage
        );
    }
}

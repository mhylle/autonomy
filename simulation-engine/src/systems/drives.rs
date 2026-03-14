use crate::components::drives::Drives;
use crate::components::genome::Genome;
use crate::components::memory::{Memory, MemoryKind};
use crate::components::perception::Perception;
use crate::components::physical::{Age, Energy, Size};
use crate::components::social::Social;
use crate::components::spatial::Position;
use crate::components::tribe::TribeId;
use crate::core::world::SimulationWorld;

/// How many ticks an entity must stay near the same position to max curiosity.
const CURIOSITY_SATURATION_TICKS: f64 = 200.0;

/// Radius within which an entity is considered "in the same area" for curiosity.
/// (Will be used when position history tracking is added.)
#[allow(dead_code)]
const SAME_AREA_RADIUS: f64 = 20.0;

/// Number of ticks without positive social contact to reach maximum social need.
const SOCIAL_NEED_SATURATION_TICKS: f64 = 500.0;

/// Max age (in ticks) for WasAttacked memories to contribute to fear.
const MEMORY_FEAR_RECALL_AGE: u64 = 300;

/// Distance from tribe centroid at which territorial pull starts reducing curiosity.
const TERRITORY_PULL_RANGE: f64 = 100.0;

/// Maximum reduction in fear from nearby tribe allies.
const TRIBE_FEAR_REDUCTION: f64 = 0.3;

/// Range for counting allies that reduce fear.
const TRIBE_ALLY_FEAR_RANGE: f64 = 30.0;

/// Computes drive values from entity state each tick.
///
/// - hunger = 1.0 - (energy / max_energy)
/// - fear = 0.0 (no threats yet, expanded in Phase 3.3)
/// - curiosity = f(time_in_same_area, base_curiosity)
/// - social_need = 0.0 (no social system yet)
/// - aggression = 0.0 (no combat system yet)
/// - reproductive_urge = f(energy_surplus, age, base_reproductive)
pub fn run(world: &mut SimulationWorld) {
    let current_tick = world.tick;

    // Collect data needed for drive computation.
    let updates: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy, &Age, &Genome, &Drives, &Perception, &Size, &Social)>()
        .iter()
        .map(|(entity, (pos, energy, age, genome, _drives, perception, _size, social))| {
            let hunger = 1.0 - (energy.current / energy.max).clamp(0.0, 1.0);

            // Fear: f(perceived_threats + count_of_WasAttacked_memories)
            // Stronger nearby entities contribute to perception-based fear.
            let larger_count = perception
                .perceived_entities
                .iter()
                .filter(|pe| pe.energy_estimate > energy.current * 1.2)
                .count() as f64;
            let perception_fear = larger_count * 0.3;

            // Memory-based fear: recent WasAttacked memories contribute.
            let memory_fear = if let Ok(memory) = world.ecs.get::<&Memory>(entity) {
                let was_attacked_count = memory
                    .recall(MemoryKind::WasAttacked, MEMORY_FEAR_RECALL_AGE, current_tick)
                    .len() as f64;
                was_attacked_count * 0.15
            } else {
                0.0
            };

            let fear = (perception_fear + memory_fear).clamp(0.0, 1.0);

            // Curiosity: simple model based on age and base_curiosity.
            let age_factor = (age.ticks as f64 / CURIOSITY_SATURATION_TICKS).min(1.0);
            let curiosity = (genome.drive_weights.base_curiosity * age_factor).clamp(0.0, 1.0);

            // Social need: increases with time since last positive social contact.
            let ticks_since_contact = current_tick.saturating_sub(social.last_positive_contact_tick) as f64;
            let isolation_factor = (ticks_since_contact / SOCIAL_NEED_SATURATION_TICKS).min(1.0);
            let social_need = (isolation_factor * genome.drive_weights.base_social_need).clamp(0.0, 1.0);

            // Aggression: f(hunger, perceived_weakness_of_nearby, base_aggression)
            // Perceived weakness: count of entities with lower energy estimate.
            let weak_nearby = perception
                .perceived_entities
                .iter()
                .filter(|pe| pe.energy_estimate < energy.current * 0.8)
                .count() as f64;
            let weakness_factor = (weak_nearby * 0.25).min(1.0);
            let aggression =
                (hunger * 0.4 + weakness_factor * 0.4 + genome.drive_weights.base_aggression * 0.2)
                    .clamp(0.0, 1.0);

            // Reproductive urge: high when energy surplus is large and entity
            // is mature enough (past 10% of lifespan).
            let energy_surplus = (energy.current / energy.max - 0.5).max(0.0) * 2.0; // 0..1
            let maturity = if age.ticks as f64 > age.max_lifespan as f64 * 0.1 {
                1.0
            } else {
                age.ticks as f64 / (age.max_lifespan as f64 * 0.1)
            };
            let reproductive_urge =
                (energy_surplus * maturity * genome.drive_weights.base_reproductive).clamp(0.0, 1.0);

            // Territory pull: if in a tribe, reduce curiosity when near territory centroid.
            let tribe_id_opt = world.ecs.get::<&TribeId>(entity).ok().and_then(|tid| tid.0);
            let territory_curiosity_reduction = if let Some(tid) = tribe_id_opt {
                if let Some(tribe) = world.tribes.get(&tid) {
                    let dx = pos.x - tribe.territory_centroid_x;
                    let dy = pos.y - tribe.territory_centroid_y;
                    let dist_to_centroid = (dx * dx + dy * dy).sqrt();
                    // When near territory, reduce curiosity (entities prefer staying).
                    // When far from territory, curiosity reduction is 0 (no penalty).
                    if dist_to_centroid < TERRITORY_PULL_RANGE {
                        let closeness = 1.0 - (dist_to_centroid / TERRITORY_PULL_RANGE);
                        closeness * 0.3 // max 0.3 reduction
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let curiosity = (curiosity - territory_curiosity_reduction).clamp(0.0, 1.0);

            // Tribe ally nearby reduces fear (safety in numbers).
            let tribe_fear_reduction = if let Some(_tid) = tribe_id_opt {
                let entity_id_bits = entity.to_bits().get();
                let ally_count = crate::systems::tribe::count_nearby_allies(
                    world, entity_id_bits, pos.x, pos.y, TRIBE_ALLY_FEAR_RANGE,
                );
                (ally_count as f64 * 0.1).min(TRIBE_FEAR_REDUCTION)
            } else {
                0.0
            };
            let fear = (fear - tribe_fear_reduction).clamp(0.0, 1.0);

            (entity, hunger, fear, curiosity, social_need, aggression, reproductive_urge, pos.x, pos.y)
        })
        .collect();

    // Apply drive updates.
    for (entity, hunger, fear, curiosity, social_need, aggression, reproductive_urge, _x, _y) in updates {
        if let Ok(mut drives) = world.ecs.get::<&mut Drives>(entity) {
            drives.hunger = hunger;
            drives.fear = fear;
            drives.curiosity = curiosity;
            drives.social_need = social_need;
            drives.aggression = aggression;
            drives.reproductive_urge = reproductive_urge;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::drives::{DriveWeights, Drives};
    use crate::components::physical::{Age, Size};
    use crate::components::perception::Perception;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn spawn_entity(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        age_ticks: u64,
        max_lifespan: u64,
        drive_weights: DriveWeights,
    ) -> hecs::Entity {
        let genome = Genome {
            max_energy: energy_max,
            max_lifespan,
            drive_weights,
            ..Genome::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            Age {
                ticks: age_ticks,
                max_lifespan,
            },
            genome,
            Drives::default(),
            Perception::default(),
            Size::default(),
            Social::default(),
        ))
    }

    /// Spawn entity with a custom Social component for testing social need drive.
    fn spawn_entity_with_social(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        age_ticks: u64,
        max_lifespan: u64,
        drive_weights: DriveWeights,
        social: Social,
    ) -> hecs::Entity {
        let genome = Genome {
            max_energy: energy_max,
            max_lifespan,
            drive_weights,
            ..Genome::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            Age {
                ticks: age_ticks,
                max_lifespan,
            },
            genome,
            Drives::default(),
            Perception::default(),
            Size::default(),
            social,
        ))
    }

    #[test]
    fn low_energy_produces_high_hunger() {
        let mut world = test_world();
        let e = spawn_entity(&mut world, 20.0, 100.0, 100, 5000, DriveWeights::default());

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.hunger > 0.7,
            "entity with 20/100 energy should have hunger > 0.7, got {}",
            drives.hunger
        );
        assert!(
            (drives.hunger - 0.8).abs() < 0.01,
            "hunger should be ~0.8, got {}",
            drives.hunger
        );
    }

    #[test]
    fn full_energy_produces_zero_hunger() {
        let mut world = test_world();
        let e = spawn_entity(&mut world, 100.0, 100.0, 100, 5000, DriveWeights::default());

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.hunger < 0.01,
            "entity with full energy should have ~0 hunger, got {}",
            drives.hunger
        );
    }

    #[test]
    fn high_energy_mature_entity_has_high_reproductive_urge() {
        let mut world = test_world();
        let weights = DriveWeights {
            base_reproductive: 1.0,
            ..DriveWeights::default()
        };
        let e = spawn_entity(&mut world, 95.0, 100.0, 1000, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.reproductive_urge > 0.5,
            "entity with 95/100 energy and mature age should have high reproductive_urge, got {}",
            drives.reproductive_urge
        );
    }

    #[test]
    fn low_energy_entity_has_low_reproductive_urge() {
        let mut world = test_world();
        let weights = DriveWeights {
            base_reproductive: 1.0,
            ..DriveWeights::default()
        };
        let e = spawn_entity(&mut world, 30.0, 100.0, 1000, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.reproductive_urge < 0.1,
            "entity with 30/100 energy should have low reproductive_urge, got {}",
            drives.reproductive_urge
        );
    }

    #[test]
    fn young_entity_has_low_reproductive_urge() {
        let mut world = test_world();
        let weights = DriveWeights {
            base_reproductive: 1.0,
            ..DriveWeights::default()
        };
        // Age 10, max_lifespan 5000 -> 10/500 = 0.02 maturity
        let e = spawn_entity(&mut world, 95.0, 100.0, 10, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.reproductive_urge < 0.1,
            "very young entity should have low reproductive_urge, got {}",
            drives.reproductive_urge
        );
    }

    #[test]
    fn curiosity_increases_with_age() {
        let mut world = test_world();
        let weights = DriveWeights {
            base_curiosity: 0.8,
            ..DriveWeights::default()
        };

        let young = spawn_entity(&mut world, 80.0, 100.0, 10, 5000, weights.clone());
        let old = spawn_entity(&mut world, 80.0, 100.0, 200, 5000, weights);

        run(&mut world);

        let young_drives = world.ecs.get::<&Drives>(young).unwrap();
        let old_drives = world.ecs.get::<&Drives>(old).unwrap();

        assert!(
            old_drives.curiosity > young_drives.curiosity,
            "older entity should have more curiosity: old={}, young={}",
            old_drives.curiosity,
            young_drives.curiosity
        );
    }

    #[test]
    fn fear_is_zero_when_no_threats() {
        let mut world = test_world();
        let e = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, DriveWeights::default());

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert_eq!(
            drives.fear, 0.0,
            "fear should be 0.0 with no perceived entities"
        );
    }

    #[test]
    fn all_drives_clamped_0_to_1() {
        let mut world = test_world();

        // Extreme values to test clamping.
        let weights = DriveWeights {
            base_curiosity: 1.0,
            base_social_need: 1.0,
            base_aggression: 1.0,
            base_reproductive: 1.0,
        };
        let e = spawn_entity(&mut world, 0.1, 100.0, 5000, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(drives.hunger >= 0.0 && drives.hunger <= 1.0);
        assert!(drives.fear >= 0.0 && drives.fear <= 1.0);
        assert!(drives.curiosity >= 0.0 && drives.curiosity <= 1.0);
        assert!(drives.social_need >= 0.0 && drives.social_need <= 1.0);
        assert!(drives.aggression >= 0.0 && drives.aggression <= 1.0);
        assert!(drives.reproductive_urge >= 0.0 && drives.reproductive_urge <= 1.0);
    }

    #[test]
    fn social_need_increases_with_isolation() {
        let mut world = test_world();
        world.tick = 600; // Well past saturation

        let weights = DriveWeights {
            base_social_need: 1.0,
            ..DriveWeights::default()
        };

        // Entity with no recent positive contact (last_positive_contact_tick = 0).
        let isolated = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, weights.clone());

        // Entity with very recent positive contact.
        let mut recent_social = Social::default();
        recent_social.last_positive_contact_tick = 599; // just 1 tick ago
        let social_entity = spawn_entity_with_social(
            &mut world, 50.0, 100.0, 100, 5000, weights, recent_social,
        );

        run(&mut world);

        let iso_drives = world.ecs.get::<&Drives>(isolated).unwrap();
        let soc_drives = world.ecs.get::<&Drives>(social_entity).unwrap();

        assert!(
            iso_drives.social_need > soc_drives.social_need,
            "isolated entity should have higher social_need ({}) than recently-social entity ({})",
            iso_drives.social_need,
            soc_drives.social_need,
        );
    }

    #[test]
    fn social_need_zero_when_base_social_need_zero() {
        let mut world = test_world();
        world.tick = 1000;

        let weights = DriveWeights {
            base_social_need: 0.0,
            ..DriveWeights::default()
        };
        let e = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.social_need < 0.01,
            "social_need should be ~0 when base_social_need is 0, got {}",
            drives.social_need,
        );
    }

    #[test]
    fn social_need_high_after_long_isolation() {
        let mut world = test_world();
        world.tick = 1000;

        let weights = DriveWeights {
            base_social_need: 1.0,
            ..DriveWeights::default()
        };
        // Last positive contact at tick 0, current tick 1000 -> 1000 ticks isolation.
        let e = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, weights);

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.social_need > 0.9,
            "entity isolated for 1000 ticks with base_social_need=1.0 should have high social_need, got {}",
            drives.social_need,
        );
    }

    #[test]
    fn recent_positive_contact_reduces_social_need() {
        let mut world = test_world();
        world.tick = 100;

        let weights = DriveWeights {
            base_social_need: 1.0,
            ..DriveWeights::default()
        };

        let mut social = Social::default();
        social.last_positive_contact_tick = 95; // 5 ticks ago
        let e = spawn_entity_with_social(
            &mut world, 50.0, 100.0, 100, 5000, weights, social,
        );

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert!(
            drives.social_need < 0.05,
            "entity with very recent positive contact should have low social_need, got {}",
            drives.social_need,
        );
    }

    // ---- Phase 3.3: Memory-boosted fear tests ----

    use crate::components::memory::{EvictionWeights, MemoryEntry};

    fn spawn_entity_with_memory(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        age_ticks: u64,
        max_lifespan: u64,
        drive_weights: DriveWeights,
        memory: crate::components::memory::Memory,
    ) -> hecs::Entity {
        let genome = Genome {
            max_energy: energy_max,
            max_lifespan,
            drive_weights,
            ..Genome::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            Age {
                ticks: age_ticks,
                max_lifespan,
            },
            genome,
            Drives::default(),
            Perception::default(),
            Size::default(),
            Social::default(),
            memory,
        ))
    }

    #[test]
    fn was_attacked_memories_increase_fear() {
        let mut world = test_world();
        world.tick = 200;

        let mut memory = crate::components::memory::Memory::new(10, EvictionWeights::default());
        for i in 0..4 {
            memory.add(
                MemoryEntry {
                    tick: 100 + i * 10,
                    kind: MemoryKind::WasAttacked,
                    importance: 0.9,
                    emotional_valence: -0.8,
                    x: 30.0,
                    y: 40.0,
                    associated_entity_id: Some(42),
                },
                100 + i * 10,
            );
        }

        let e_with_memory = spawn_entity_with_memory(
            &mut world,
            50.0,
            100.0,
            100,
            5000,
            DriveWeights::default(),
            memory,
        );
        let e_without = spawn_entity(
            &mut world,
            50.0,
            100.0,
            100,
            5000,
            DriveWeights::default(),
        );

        run(&mut world);

        let fear_with = world.ecs.get::<&Drives>(e_with_memory).unwrap().fear;
        let fear_without = world.ecs.get::<&Drives>(e_without).unwrap().fear;

        assert!(
            fear_with > fear_without,
            "entity with WasAttacked memories should have higher fear ({}) than without ({})",
            fear_with,
            fear_without,
        );
    }

    #[test]
    fn fear_without_memories_uses_perception_only() {
        let mut world = test_world();
        let e = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, DriveWeights::default());

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        // No perceived threats and no memory -> fear should be 0.
        assert_eq!(
            drives.fear, 0.0,
            "fear should be 0.0 with no threats and no memories"
        );
    }
}

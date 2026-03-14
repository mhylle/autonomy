use crate::components::drives::Drives;
use crate::components::genome::Genome;
use crate::components::physical::{Age, Energy};
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;

/// How many ticks an entity must stay near the same position to max curiosity.
const CURIOSITY_SATURATION_TICKS: f64 = 200.0;

/// Radius within which an entity is considered "in the same area" for curiosity.
/// (Will be used when position history tracking is added.)
#[allow(dead_code)]
const SAME_AREA_RADIUS: f64 = 20.0;

/// Computes drive values from entity state each tick.
///
/// - hunger = 1.0 - (energy / max_energy)
/// - fear = 0.0 (no threats yet, expanded in Phase 3.3)
/// - curiosity = f(time_in_same_area, base_curiosity)
/// - social_need = 0.0 (no social system yet)
/// - aggression = 0.0 (no combat system yet)
/// - reproductive_urge = f(energy_surplus, age, base_reproductive)
pub fn run(world: &mut SimulationWorld) {
    // Collect data needed for drive computation.
    let updates: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy, &Age, &Genome, &Drives)>()
        .iter()
        .map(|(entity, (pos, energy, age, genome, _drives))| {
            let hunger = 1.0 - (energy.current / energy.max).clamp(0.0, 1.0);

            let fear = 0.0; // Phase 3.3

            // Curiosity: simple model based on age and base_curiosity.
            // In a full implementation this would track position history;
            // for now, use age as a proxy for time-in-area (entities that
            // have lived longer in a stable sim have explored less).
            let age_factor = (age.ticks as f64 / CURIOSITY_SATURATION_TICKS).min(1.0);
            let curiosity = (genome.drive_weights.base_curiosity * age_factor).clamp(0.0, 1.0);

            let social_need = 0.0; // Phase 4
            let aggression = 0.0; // Phase 3.6

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
    use crate::components::physical::Age;
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
    fn fear_is_zero() {
        let mut world = test_world();
        let e = spawn_entity(&mut world, 50.0, 100.0, 100, 5000, DriveWeights::default());

        run(&mut world);

        let drives = world.ecs.get::<&Drives>(e).unwrap();
        assert_eq!(drives.fear, 0.0, "fear should be 0.0 until Phase 3.3");
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
}

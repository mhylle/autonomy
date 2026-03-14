use crate::components::physical::{Age, Energy};
use crate::core::world::SimulationWorld;

/// Advances age and drains energy by the metabolism rate.
///
/// Each tick every entity with `Age` and `Energy` grows one tick older
/// and loses energy proportional to its metabolism rate, scaled by the
/// climate's metabolism multiplier. Extreme temperatures (hot or cold)
/// increase energy consumption.
pub fn run(world: &mut SimulationWorld) {
    let climate_multiplier = world.climate.metabolism_multiplier();

    for (_entity, (age, energy)) in world.ecs.query_mut::<(&mut Age, &mut Energy)>() {
        age.ticks += 1;
        energy.current -= energy.metabolism_rate * climate_multiplier;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    #[test]
    fn increments_age_by_one() {
        let mut world = test_world();
        world.ecs.spawn((
            Age {
                ticks: 0,
                max_lifespan: 5000,
            },
            Energy::default(),
        ));

        run(&mut world);

        for (_id, age) in world.ecs.query_mut::<&Age>() {
            assert_eq!(age.ticks, 1);
        }
    }

    #[test]
    fn drains_energy_by_metabolism_rate() {
        let mut world = test_world();
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 100.0,
                max: 100.0,
                metabolism_rate: 0.5,
            },
        ));

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert!((energy.current - 99.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn multiple_ticks_accumulate() {
        let mut world = test_world();
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 50.0,
                max: 100.0,
                metabolism_rate: 1.0,
            },
        ));

        for _ in 0..10 {
            run(&mut world);
        }

        for (_id, (age, energy)) in world.ecs.query_mut::<(&Age, &Energy)>() {
            assert_eq!(age.ticks, 10);
            assert!((energy.current - 40.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn energy_can_go_negative() {
        let mut world = test_world();
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 0.5,
                max: 100.0,
                metabolism_rate: 1.0,
            },
        ));

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            assert!(energy.current < 0.0);
        }
    }

    #[test]
    fn extreme_cold_increases_metabolism() {
        use crate::environment::climate::Climate;

        let mut world = test_world();
        world.climate = Climate {
            temperature: 0.0, // freezing
            ..Climate::default()
        };
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 100.0,
                max: 100.0,
                metabolism_rate: 1.0,
            },
        ));

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            // At 0.0 temp, multiplier = 1.5, so drain = 1.0 * 1.5 = 1.5
            assert!((energy.current - 98.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn extreme_heat_increases_metabolism() {
        use crate::environment::climate::Climate;

        let mut world = test_world();
        world.climate = Climate {
            temperature: 1.0, // scorching
            ..Climate::default()
        };
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 100.0,
                max: 100.0,
                metabolism_rate: 1.0,
            },
        ));

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            // At 1.0 temp, multiplier = 1.5, so drain = 1.0 * 1.5 = 1.5
            assert!((energy.current - 98.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn temperate_climate_no_extra_drain() {
        use crate::environment::climate::Climate;

        let mut world = test_world();
        world.climate = Climate {
            temperature: 0.5, // temperate
            ..Climate::default()
        };
        world.ecs.spawn((
            Age::default(),
            Energy {
                current: 100.0,
                max: 100.0,
                metabolism_rate: 1.0,
            },
        ));

        run(&mut world);

        for (_id, energy) in world.ecs.query_mut::<&Energy>() {
            // At 0.5 temp, multiplier = 1.0, so drain = 1.0 * 1.0 = 1.0
            assert!((energy.current - 99.0).abs() < f64::EPSILON);
        }
    }
}

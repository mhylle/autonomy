use crate::core::world::SimulationWorld;

/// Regrow depleted resources each tick.
///
/// For every resource whose `amount` is below `max_amount`, add
/// `regrowth_rate` scaled by the current climate multiplier (capped
/// at `max_amount`). Resources that regrow above zero are marked as
/// no longer depleted.
///
/// Climate effects on regrowth:
/// - Cold temperatures slow regrowth
/// - Seasonal cycle modulates resource abundance
/// - Drought events drastically reduce regrowth
pub fn run(world: &mut SimulationWorld) {
    let climate_multiplier = world.climate.regrowth_multiplier();

    for resource in &mut world.resources {
        if resource.amount < resource.max_amount {
            let effective_regrowth = resource.regrowth_rate * climate_multiplier;
            resource.amount = (resource.amount + effective_regrowth).min(resource.max_amount);

            if resource.amount > 0.0 {
                resource.depleted = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::environment::climate::{Climate, Season};
    use crate::environment::resources::Resource;

    /// Create a world with neutral climate (multiplier = 1.0) for baseline tests.
    fn world_with_resources(resources: Vec<Resource>) -> SimulationWorld {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        world.resources = resources;
        // Set neutral climate so multiplier is exactly 1.0
        world.climate = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        world
    }

    #[test]
    fn regrowth_increases_amount() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            ..Default::default()
        }]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 10.5);
    }

    #[test]
    fn regrowth_caps_at_max() {
        let mut world = world_with_resources(vec![Resource {
            amount: 49.8,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            ..Default::default()
        }]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 50.0);
    }

    #[test]
    fn regrowth_clears_depleted_flag() {
        let mut world = world_with_resources(vec![Resource {
            amount: 0.0,
            max_amount: 50.0,
            regrowth_rate: 1.0,
            depleted: true,
            ..Default::default()
        }]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 1.0);
        assert!(!world.resources[0].depleted);
    }

    #[test]
    fn full_resource_unchanged() {
        let mut world = world_with_resources(vec![Resource {
            amount: 50.0,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            ..Default::default()
        }]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 50.0);
    }

    #[test]
    fn regrowth_with_zero_rate() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 0.0,
            ..Default::default()
        }]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 10.0);
    }

    #[test]
    fn multiple_resources_all_regrow() {
        let mut world = world_with_resources(vec![
            Resource {
                id: 0,
                amount: 10.0,
                max_amount: 50.0,
                regrowth_rate: 1.0,
                ..Default::default()
            },
            Resource {
                id: 1,
                amount: 0.0,
                max_amount: 50.0,
                regrowth_rate: 2.0,
                depleted: true,
                ..Default::default()
            },
            Resource {
                id: 2,
                amount: 50.0,
                max_amount: 50.0,
                regrowth_rate: 0.5,
                ..Default::default()
            },
        ]);

        run(&mut world);
        assert_eq!(world.resources[0].amount, 11.0);
        assert_eq!(world.resources[1].amount, 2.0);
        assert!(!world.resources[1].depleted);
        assert_eq!(world.resources[2].amount, 50.0);
    }

    #[test]
    fn regrowth_slowed_by_cold_climate() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 1.0,
            ..Default::default()
        }]);
        world.climate = Climate {
            temperature: 0.0, // freezing
            season: Season::Summer,
            drought_active: false,
            drought_ticks_remaining: 0,
        };

        run(&mut world);
        // At freezing temp, regrowth multiplier = 0.3, so effective = 1.0 * 0.3 = 0.3
        assert!((world.resources[0].amount - 10.3).abs() < 0.01);
    }

    #[test]
    fn regrowth_slowed_by_drought() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 1.0,
            ..Default::default()
        }]);
        world.climate = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: true,
            drought_ticks_remaining: 100,
        };

        run(&mut world);
        // Drought multiplier = 0.2, so effective = 1.0 * 0.2 = 0.2
        assert!((world.resources[0].amount - 10.2).abs() < 0.01);
    }

    #[test]
    fn regrowth_boosted_in_spring() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 1.0,
            ..Default::default()
        }]);
        world.climate = Climate {
            temperature: 0.5,
            season: Season::Spring,
            drought_active: false,
            drought_ticks_remaining: 0,
        };

        run(&mut world);
        // Spring multiplier = 1.2, so effective = 1.0 * 1.2 = 1.2
        assert!((world.resources[0].amount - 11.2).abs() < 0.01);
    }

    #[test]
    fn regrowth_very_slow_in_winter_drought() {
        let mut world = world_with_resources(vec![Resource {
            amount: 10.0,
            max_amount: 50.0,
            regrowth_rate: 1.0,
            ..Default::default()
        }]);
        world.climate = Climate {
            temperature: 0.1, // cold
            season: Season::Winter,
            drought_active: true,
            drought_ticks_remaining: 100,
        };

        let multiplier = world.climate.regrowth_multiplier();
        run(&mut world);
        let expected = 10.0 + 1.0 * multiplier;
        assert!(
            (world.resources[0].amount - expected).abs() < 0.01,
            "Expected {}, got {}",
            expected,
            world.resources[0].amount
        );
        // Should be very slow regrowth
        assert!(multiplier < 0.1, "Multiplier should be very low, got {}", multiplier);
    }
}

//! Agriculture system: grows planted farms each tick.
//!
//! Farms grow based on their growth_rate, climate conditions, and tending bonus.
//! Harvested farms are removed from the world.

use crate::core::world::SimulationWorld;

/// Advance all farms' growth by one tick and remove harvested ones.
pub fn run(world: &mut SimulationWorld) {
    let climate_multiplier = world.climate.regrowth_multiplier();
    let current_tick = world.tick;

    for farm in &mut world.farms {
        farm.grow_tick(current_tick, climate_multiplier);
    }

    // Remove harvested farms.
    world.farms.retain(|f| !f.harvested);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::environment::climate::{Climate, Season};
    use crate::environment::structures::Farm;

    fn test_world() -> SimulationWorld {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        // Neutral climate for predictable testing
        world.climate = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        world
    }

    fn test_farm(growth: f64) -> Farm {
        Farm {
            id: 1,
            x: 50.0,
            y: 50.0,
            planter_id: 100,
            growth,
            growth_rate: 0.01,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 0,
            tribe_id: None,
        }
    }

    #[test]
    fn farms_grow_each_tick() {
        let mut world = test_world();
        world.tick = 1000;
        world.farms.push(test_farm(0.0));

        run(&mut world);
        assert!(world.farms[0].growth > 0.0);
    }

    #[test]
    fn harvested_farms_are_removed() {
        let mut world = test_world();
        world.tick = 1000;
        let mut farm = test_farm(1.0);
        farm.harvested = true;
        world.farms.push(farm);

        run(&mut world);
        assert!(world.farms.is_empty());
    }

    #[test]
    fn climate_affects_growth() {
        let mut world = test_world();
        world.tick = 1000;
        world.climate = Climate {
            temperature: 0.5,
            season: Season::Spring,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        world.farms.push(test_farm(0.0));

        run(&mut world);
        let spring_growth = world.farms[0].growth;

        // Reset with drought
        let mut world2 = test_world();
        world2.tick = 1000;
        world2.climate = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: true,
            drought_ticks_remaining: 100,
        };
        world2.farms.push(test_farm(0.0));

        run(&mut world2);
        let drought_growth = world2.farms[0].growth;

        assert!(spring_growth > drought_growth,
            "Spring growth {} should exceed drought growth {}",
            spring_growth, drought_growth);
    }

    #[test]
    fn tended_farm_grows_faster() {
        let mut world = test_world();
        world.tick = 100;

        // Farm tended recently
        let mut tended = test_farm(0.0);
        tended.last_tended_tick = 90; // within TENDING_DURATION
        world.farms.push(tended);

        // Farm not tended
        let untended = test_farm(0.0);
        world.farms.push(untended);

        run(&mut world);

        assert!(world.farms[0].growth > world.farms[1].growth,
            "Tended farm growth {} should exceed untended {}",
            world.farms[0].growth, world.farms[1].growth);
    }

    #[test]
    fn multiple_farms_grow_independently() {
        let mut world = test_world();
        world.tick = 1000;
        world.farms.push(test_farm(0.0));
        world.farms.push(test_farm(0.5));

        run(&mut world);
        // First farm should have grown from 0
        assert!(world.farms[0].growth > 0.0);
        // Second farm should have grown from 0.5
        assert!(world.farms[1].growth > 0.5);
    }
}

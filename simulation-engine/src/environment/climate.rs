use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::core::world::SimulationWorld;

/// Length of one full seasonal cycle in ticks.
const SEASON_CYCLE_TICKS: u64 = 8000;

/// Length of one season in ticks (cycle / 4).
const TICKS_PER_SEASON: u64 = SEASON_CYCLE_TICKS / 4;

/// Maximum magnitude of temperature drift per tick.
const TEMPERATURE_DRIFT_RATE: f64 = 0.002;

/// Probability per tick that a drought event begins.
const DROUGHT_CHANCE_PER_TICK: f64 = 0.0005;

/// Default duration of a drought event in ticks.
const DEFAULT_DROUGHT_DURATION: u64 = 500;

/// Regrowth multiplier during drought conditions.
const DROUGHT_REGROWTH_MULTIPLIER: f64 = 0.2;

/// Minimum regrowth multiplier from cold temperatures.
const MIN_COLD_REGROWTH_MULTIPLIER: f64 = 0.3;

/// Extra metabolism multiplier at extreme temperatures (0.0 or 1.0).
const EXTREME_TEMP_METABOLISM_BONUS: f64 = 1.5;

/// The four seasons, each affecting temperature baseline and resource availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Returns the baseline temperature contribution for this season.
    /// Spring/Autumn are neutral (0.5), Summer is warm (0.7), Winter is cold (0.3).
    pub fn baseline_temperature(self) -> f64 {
        match self {
            Season::Spring => 0.5,
            Season::Summer => 0.7,
            Season::Autumn => 0.5,
            Season::Winter => 0.3,
        }
    }

    /// Returns the resource abundance multiplier for this season.
    pub fn resource_multiplier(self) -> f64 {
        match self {
            Season::Spring => 1.2,
            Season::Summer => 1.0,
            Season::Autumn => 0.8,
            Season::Winter => 0.5,
        }
    }
}

/// Global climate state for the simulation.
///
/// Tracks temperature (0.0 = freezing, 1.0 = scorching, 0.5 = temperate),
/// the current season, and drought conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Climate {
    /// Current global temperature, clamped to [0.0, 1.0].
    /// 0.0 = freezing, 0.5 = temperate, 1.0 = scorching.
    pub temperature: f64,

    /// Current season.
    pub season: Season,

    /// Whether a drought is currently active.
    pub drought_active: bool,

    /// Remaining ticks of the current drought event.
    pub drought_ticks_remaining: u64,
}

impl Default for Climate {
    fn default() -> Self {
        Self {
            temperature: 0.5,
            season: Season::Spring,
            drought_active: false,
            drought_ticks_remaining: 0,
        }
    }
}

impl Climate {
    /// Determine the season from the current tick.
    pub fn season_from_tick(tick: u64) -> Season {
        let phase = tick % SEASON_CYCLE_TICKS;
        match phase / TICKS_PER_SEASON {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// Compute the regrowth multiplier based on current climate conditions.
    ///
    /// Factors in temperature (colder = slower), season, and drought.
    pub fn regrowth_multiplier(&self) -> f64 {
        // Temperature factor: linear from MIN_COLD_REGROWTH_MULTIPLIER at 0.0 to 1.0 at 0.5+
        let temp_factor = if self.temperature < 0.5 {
            MIN_COLD_REGROWTH_MULTIPLIER
                + (1.0 - MIN_COLD_REGROWTH_MULTIPLIER) * (self.temperature / 0.5)
        } else {
            1.0
        };

        let seasonal_factor = self.season.resource_multiplier();

        let drought_factor = if self.drought_active {
            DROUGHT_REGROWTH_MULTIPLIER
        } else {
            1.0
        };

        temp_factor * seasonal_factor * drought_factor
    }

    /// Compute the metabolism multiplier based on temperature extremes.
    ///
    /// Entities consume more energy when temperature is far from temperate (0.5).
    pub fn metabolism_multiplier(&self) -> f64 {
        let distance_from_temperate = (self.temperature - 0.5).abs() * 2.0; // 0.0 to 1.0
        1.0 + distance_from_temperate * (EXTREME_TEMP_METABOLISM_BONUS - 1.0)
    }
}

/// Update the climate state for this tick.
///
/// Advances the season, drifts temperature with a random walk biased toward
/// the seasonal baseline, and manages drought events.
pub fn run(world: &mut SimulationWorld) {
    let tick = world.tick;
    let mut rng = world.rng.tick_rng("climate", tick);

    // Update season
    world.climate.season = Climate::season_from_tick(tick);

    // Temperature drift: random walk biased toward seasonal baseline
    let seasonal_baseline = world.climate.season.baseline_temperature();
    let bias = (seasonal_baseline - world.climate.temperature) * 0.1;
    let drift: f64 = rng.gen_range(-TEMPERATURE_DRIFT_RATE..TEMPERATURE_DRIFT_RATE) + bias;
    world.climate.temperature = (world.climate.temperature + drift).clamp(0.0, 1.0);

    // Drought management
    if world.climate.drought_active {
        if world.climate.drought_ticks_remaining > 0 {
            world.climate.drought_ticks_remaining -= 1;
        }
        if world.climate.drought_ticks_remaining == 0 {
            world.climate.drought_active = false;
        }
    } else {
        // Random chance to start a drought
        let roll: f64 = rng.gen();
        if roll < DROUGHT_CHANCE_PER_TICK {
            world.climate.drought_active = true;
            world.climate.drought_ticks_remaining = DEFAULT_DROUGHT_DURATION;
        }
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
    fn default_climate_is_temperate() {
        let climate = Climate::default();
        assert_eq!(climate.temperature, 0.5);
        assert_eq!(climate.season, Season::Spring);
        assert!(!climate.drought_active);
        assert_eq!(climate.drought_ticks_remaining, 0);
    }

    #[test]
    fn season_from_tick_cycles_correctly() {
        assert_eq!(Climate::season_from_tick(0), Season::Spring);
        assert_eq!(Climate::season_from_tick(1999), Season::Spring);
        assert_eq!(Climate::season_from_tick(2000), Season::Summer);
        assert_eq!(Climate::season_from_tick(3999), Season::Summer);
        assert_eq!(Climate::season_from_tick(4000), Season::Autumn);
        assert_eq!(Climate::season_from_tick(5999), Season::Autumn);
        assert_eq!(Climate::season_from_tick(6000), Season::Winter);
        assert_eq!(Climate::season_from_tick(7999), Season::Winter);
        // Full cycle wraps
        assert_eq!(Climate::season_from_tick(8000), Season::Spring);
        assert_eq!(Climate::season_from_tick(10000), Season::Summer);
    }

    #[test]
    fn temperature_stays_in_bounds_after_many_ticks() {
        let mut world = test_world();
        for _ in 0..5000 {
            world.tick += 1;
            run(&mut world);
        }
        assert!(world.climate.temperature >= 0.0);
        assert!(world.climate.temperature <= 1.0);
    }

    #[test]
    fn season_updates_each_tick() {
        let mut world = test_world();
        // Advance to tick 2000 (should be Summer)
        for _ in 0..2000 {
            world.tick += 1;
            run(&mut world);
        }
        assert_eq!(world.climate.season, Season::Summer);
    }

    #[test]
    fn drought_eventually_ends() {
        let mut world = test_world();
        world.climate.drought_active = true;
        world.climate.drought_ticks_remaining = 10;

        for _ in 0..10 {
            world.tick += 1;
            run(&mut world);
        }
        assert!(!world.climate.drought_active);
        assert_eq!(world.climate.drought_ticks_remaining, 0);
    }

    #[test]
    fn regrowth_multiplier_temperate_no_drought() {
        let climate = Climate {
            temperature: 0.5,
            season: Season::Spring,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        let mult = climate.regrowth_multiplier();
        // At temperate temperature with spring (1.2), no drought
        assert!((mult - 1.2).abs() < 0.01);
    }

    #[test]
    fn regrowth_multiplier_reduced_in_cold() {
        let climate = Climate {
            temperature: 0.0,
            season: Season::Spring,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        let mult = climate.regrowth_multiplier();
        // At freezing: temp_factor = MIN_COLD_REGROWTH_MULTIPLIER (0.3), season = 1.2
        let expected = MIN_COLD_REGROWTH_MULTIPLIER * 1.2;
        assert!((mult - expected).abs() < 0.01);
    }

    #[test]
    fn regrowth_multiplier_reduced_during_drought() {
        let climate = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: true,
            drought_ticks_remaining: 100,
        };
        let mult = climate.regrowth_multiplier();
        // Temperate temp (1.0) * summer (1.0) * drought (0.2)
        assert!((mult - 0.2).abs() < 0.01);
    }

    #[test]
    fn regrowth_multiplier_winter_is_low() {
        let climate = Climate {
            temperature: 0.3,
            season: Season::Winter,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        let mult = climate.regrowth_multiplier();
        // Cold temp factor and winter season factor combine to slow regrowth
        assert!(mult < 0.5, "Winter regrowth multiplier should be < 0.5, got {}", mult);
    }

    #[test]
    fn metabolism_multiplier_temperate_is_one() {
        let climate = Climate {
            temperature: 0.5,
            ..Climate::default()
        };
        assert!((climate.metabolism_multiplier() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn metabolism_multiplier_extreme_cold() {
        let climate = Climate {
            temperature: 0.0,
            ..Climate::default()
        };
        let mult = climate.metabolism_multiplier();
        assert!((mult - EXTREME_TEMP_METABOLISM_BONUS).abs() < f64::EPSILON);
    }

    #[test]
    fn metabolism_multiplier_extreme_hot() {
        let climate = Climate {
            temperature: 1.0,
            ..Climate::default()
        };
        let mult = climate.metabolism_multiplier();
        assert!((mult - EXTREME_TEMP_METABOLISM_BONUS).abs() < f64::EPSILON);
    }

    #[test]
    fn climate_run_is_deterministic() {
        let mut world1 = test_world();
        let mut world2 = test_world();

        for _ in 0..100 {
            world1.tick += 1;
            world2.tick += 1;
            run(&mut world1);
            run(&mut world2);
        }

        assert_eq!(world1.climate.temperature, world2.climate.temperature);
        assert_eq!(world1.climate.season, world2.climate.season);
        assert_eq!(world1.climate.drought_active, world2.climate.drought_active);
    }

    #[test]
    fn drought_reduces_regrowth_severely() {
        let no_drought = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: false,
            drought_ticks_remaining: 0,
        };
        let drought = Climate {
            temperature: 0.5,
            season: Season::Summer,
            drought_active: true,
            drought_ticks_remaining: 100,
        };
        let ratio = drought.regrowth_multiplier() / no_drought.regrowth_multiplier();
        assert!((ratio - DROUGHT_REGROWTH_MULTIPLIER).abs() < 0.01);
    }

    #[test]
    fn temperature_drifts_toward_seasonal_baseline() {
        // Start at 0.0 (freezing) during summer (baseline 0.7)
        // Temperature should drift upward over time
        let mut world = test_world();
        world.climate.temperature = 0.0;
        // Set tick to summer range
        world.tick = 2000;

        for _ in 0..500 {
            world.tick += 1;
            run(&mut world);
        }

        // After 500 ticks biased toward 0.7, temperature should have risen
        assert!(
            world.climate.temperature > 0.1,
            "Temperature should drift toward summer baseline, got {}",
            world.climate.temperature
        );
    }
}

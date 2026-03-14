use hecs::World;

use super::config::SimulationConfig;
use super::rng::SimulationRng;
use crate::environment::climate::Climate;
use crate::environment::resources::Resource;
use crate::environment::spatial_index::SpatialIndex;
use crate::environment::terrain::TerrainGrid;
use crate::events::EventLog;

/// Wraps `hecs::World` with simulation metadata.
///
/// This is the top-level simulation state container. It owns the ECS
/// world, configuration, RNG provider, and tick counter.
pub struct SimulationWorld {
    pub ecs: World,
    pub config: SimulationConfig,
    pub rng: SimulationRng,
    pub tick: u64,
    pub resources: Vec<Resource>,
    pub spatial_index: SpatialIndex,
    pub terrain: TerrainGrid,
    pub climate: Climate,
    pub event_log: EventLog,
    pub paused: bool,
    pub speed_multiplier: f64,
    /// (tick, species_id -> population count) recorded periodically.
    pub species_history: Vec<(u64, std::collections::HashMap<u64, u32>)>,
}

impl SimulationWorld {
    pub fn new(config: SimulationConfig) -> Self {
        let rng = SimulationRng::new(config.seed);
        let spatial_index =
            SpatialIndex::new(config.world_width, config.world_height, 50.0);
        let terrain = TerrainGrid::generate(
            config.world_width,
            config.world_height,
            config.seed,
        );
        Self {
            ecs: World::new(),
            config,
            rng,
            tick: 0,
            resources: Vec::new(),
            spatial_index,
            terrain,
            climate: Climate::default(),
            event_log: EventLog::new(),
            paused: false,
            speed_multiplier: 1.0,
            species_history: Vec::new(),
        }
    }

    pub fn entity_count(&self) -> u32 {
        self.ecs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_world_starts_at_tick_zero() {
        let world = SimulationWorld::new(SimulationConfig::default());
        assert_eq!(world.tick, 0);
    }

    #[test]
    fn new_world_has_no_entities() {
        let world = SimulationWorld::new(SimulationConfig::default());
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn world_uses_config_seed() {
        let mut config = SimulationConfig::default();
        config.seed = 123;
        let world = SimulationWorld::new(config);
        assert_eq!(world.rng.master_seed(), 123);
    }
}

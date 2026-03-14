use serde::{Deserialize, Serialize};

/// Top-level simulation configuration.
///
/// Controls world dimensions, entity counts, tick rate, and the master
/// seed that guarantees deterministic replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub world_width: f64,
    pub world_height: f64,
    pub seed: u64,
    pub initial_entity_count: u32,
    pub tick_rate: u32,
    pub headless: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world_width: 500.0,
            world_height: 500.0,
            seed: 42,
            initial_entity_count: 100,
            tick_rate: 60,
            headless: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = SimulationConfig::default();
        assert!(config.world_width > 0.0);
        assert!(config.world_height > 0.0);
        assert!(config.initial_entity_count > 0);
        assert!(config.tick_rate > 0);
    }

    #[test]
    fn config_serializes_roundtrip() {
        let config = SimulationConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: SimulationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.seed, config.seed);
        assert_eq!(restored.world_width, config.world_width);
    }
}

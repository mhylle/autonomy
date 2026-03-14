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
    /// How often to write a snapshot to disk (0 = disabled).
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: u64,
    /// Directory for snapshot files.
    #[serde(default = "default_snapshot_dir")]
    pub snapshot_dir: String,
    /// Enable 3D simulation features (elevation, caves, vertical movement).
    /// When false (default), all z-coordinates remain 0 and the simulation
    /// behaves as a flat 2D world.
    #[serde(default)]
    pub enable_3d: bool,
    /// Whether chunk-based infinite world is enabled.
    /// When false (default), the existing fixed-world behavior is unchanged.
    #[serde(default)]
    pub enable_chunks: bool,
    /// Size of each chunk in world units (default 256.0).
    /// Only used when `enable_chunks` is true.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: f64,
}

fn default_snapshot_interval() -> u64 {
    1000
}

fn default_snapshot_dir() -> String {
    "snapshots".to_string()
}

fn default_chunk_size() -> f64 {
    256.0
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
            snapshot_interval: default_snapshot_interval(),
            snapshot_dir: default_snapshot_dir(),
            enable_3d: false,
            enable_chunks: false,
            chunk_size: default_chunk_size(),
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
        assert!(!config.enable_3d);
        assert!(!config.enable_chunks);
        assert_eq!(config.chunk_size, 256.0);
    }

    #[test]
    fn config_serializes_roundtrip() {
        let config = SimulationConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: SimulationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.seed, config.seed);
        assert_eq!(restored.world_width, config.world_width);
        assert_eq!(restored.enable_3d, config.enable_3d);
    }

    #[test]
    fn config_deserialize_without_enable_3d_defaults_false() {
        let json = r#"{"world_width":500.0,"world_height":500.0,"seed":42,"initial_entity_count":100,"tick_rate":60,"headless":false,"snapshot_interval":1000,"snapshot_dir":"snapshots"}"#;
        let config: SimulationConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enable_3d);
    }

    #[test]
    fn config_deserialize_without_chunk_fields_defaults() {
        let json = r#"{"world_width":500.0,"world_height":500.0,"seed":42,"initial_entity_count":100,"tick_rate":60,"headless":false}"#;
        let config: SimulationConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enable_chunks);
        assert_eq!(config.chunk_size, 256.0);
    }

    #[test]
    fn config_chunk_roundtrip() {
        let mut config = SimulationConfig::default();
        config.enable_chunks = true;
        config.chunk_size = 512.0;
        let json = serde_json::to_string(&config).unwrap();
        let restored: SimulationConfig = serde_json::from_str(&json).unwrap();
        assert!(restored.enable_chunks);
        assert_eq!(restored.chunk_size, 512.0);
    }
}

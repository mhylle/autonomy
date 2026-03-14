use rand::Rng;

use crate::core::world::SimulationWorld;
use crate::environment::resources::{Resource, ResourceType};
use crate::environment::terrain::TerrainType;

/// Scatter initial resources across the simulation world.
///
/// The number of resources is derived from the world area:
/// `(world_width * world_height / 1250.0) as u32`, which yields roughly
/// 200 resources for a default 500x500 world.
///
/// Resources are placed respecting terrain: their density is scaled by
/// the terrain's `resource_density_multiplier`. No resources spawn on
/// Water cells. Resources on Forest terrain get higher max amounts,
/// while Desert resources get lower amounts.
pub fn scatter_resources(world: &mut SimulationWorld) {
    let width = world.config.world_width;
    let height = world.config.world_height;
    let count = (width * height / 1250.0) as u32;

    let mut rng = world.rng.system_rng("resource_spawning");
    let mut id_counter = 0u64;

    // Attempt to place `count` resources, retrying positions that fall
    // on impassable terrain (Water). Cap retries to avoid infinite loops.
    let max_attempts = count * 3;
    let mut attempts = 0;
    let mut placed = 0;

    while placed < count && attempts < max_attempts {
        attempts += 1;

        let x = rng.gen_range(0.0..width);
        let y = rng.gen_range(0.0..height);

        let terrain = world.terrain.terrain_at(x, y);

        // Skip water cells entirely.
        if terrain == TerrainType::Water {
            continue;
        }

        let density_mult = terrain.resource_density_multiplier();

        // Use density multiplier as a probability gate: low-density
        // terrain will have fewer resources placed on it.
        let roll: f64 = rng.gen();
        if roll > density_mult {
            continue;
        }

        let (max_amount, regrowth_rate) = resource_params_for_terrain(terrain);

        world.resources.push(Resource {
            id: id_counter,
            x,
            y,
            resource_type: ResourceType::Food,
            amount: max_amount,
            max_amount,
            regrowth_rate,
            depleted: false,
        });
        id_counter += 1;
        placed += 1;
    }
}

/// Return (max_amount, regrowth_rate) tuned to the terrain type.
fn resource_params_for_terrain(terrain: TerrainType) -> (f64, f64) {
    match terrain {
        TerrainType::Forest => (75.0, 0.8),    // Berries: abundant, fast regrowth
        TerrainType::Grassland => (50.0, 0.5),  // Grain: moderate
        TerrainType::Mountain => (30.0, 0.3),   // Sparse alpine plants
        TerrainType::Desert => (20.0, 0.1),     // Very sparse, slow regrowth
        TerrainType::Water => (0.0, 0.0),       // Should never be reached
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    #[test]
    fn scatter_creates_some_resources() {
        let config = SimulationConfig::default(); // 500x500
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);
        // Exact count varies due to terrain filtering, but should have some.
        assert!(
            !world.resources.is_empty(),
            "should have placed some resources"
        );
    }

    #[test]
    fn resources_are_within_world_bounds() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        for r in &world.resources {
            assert!(r.x >= 0.0 && r.x < 500.0, "x={} out of bounds", r.x);
            assert!(r.y >= 0.0 && r.y < 500.0, "y={} out of bounds", r.y);
        }
    }

    #[test]
    fn scatter_is_deterministic() {
        let config_a = SimulationConfig::default();
        let config_b = SimulationConfig::default();

        let mut world_a = SimulationWorld::new(config_a);
        let mut world_b = SimulationWorld::new(config_b);

        scatter_resources(&mut world_a);
        scatter_resources(&mut world_b);

        assert_eq!(world_a.resources.len(), world_b.resources.len());
        for (a, b) in world_a.resources.iter().zip(world_b.resources.iter()) {
            assert_eq!(a.x, b.x);
            assert_eq!(a.y, b.y);
        }
    }

    #[test]
    fn no_resources_on_water() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        for r in &world.resources {
            let terrain = world.terrain.terrain_at(r.x, r.y);
            assert_ne!(
                terrain,
                TerrainType::Water,
                "resource at ({}, {}) should not be on water",
                r.x,
                r.y
            );
        }
    }

    #[test]
    fn forest_resources_have_higher_amounts() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        for r in &world.resources {
            let terrain = world.terrain.terrain_at(r.x, r.y);
            if terrain == TerrainType::Forest {
                assert_eq!(r.max_amount, 75.0, "forest resources should have max_amount=75");
            }
        }
    }

    #[test]
    fn desert_resources_have_lower_amounts() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        for r in &world.resources {
            let terrain = world.terrain.terrain_at(r.x, r.y);
            if terrain == TerrainType::Desert {
                assert_eq!(r.max_amount, 20.0, "desert resources should have max_amount=20");
            }
        }
    }

    #[test]
    fn all_resources_are_food() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        for r in &world.resources {
            assert_eq!(r.resource_type, crate::environment::resources::ResourceType::Food);
        }
    }

    #[test]
    fn resources_have_unique_ids() {
        let config = SimulationConfig::default();
        let mut world = SimulationWorld::new(config);
        scatter_resources(&mut world);

        let mut ids: Vec<u64> = world.resources.iter().map(|r| r.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), world.resources.len());
    }

    #[test]
    fn resource_params_match_terrain() {
        assert_eq!(resource_params_for_terrain(TerrainType::Forest), (75.0, 0.8));
        assert_eq!(resource_params_for_terrain(TerrainType::Grassland), (50.0, 0.5));
        assert_eq!(resource_params_for_terrain(TerrainType::Mountain), (30.0, 0.3));
        assert_eq!(resource_params_for_terrain(TerrainType::Desert), (20.0, 0.1));
    }
}

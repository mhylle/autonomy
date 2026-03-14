//! Construction system: advances construction sites and converts completed sites
//! into structures.
//!
//! Each tick, all active construction sites receive one work tick of progress.
//! When a site completes, it is removed and a new Structure is added to the world.
//! New storage containers are also created for StorageBuilding structures.

use crate::core::world::SimulationWorld;
use crate::environment::structures::{Storage, StructureType};

/// Advance all construction sites by one tick, completing any that reach 100%.
pub fn run(world: &mut SimulationWorld) {
    let mut completed_indices: Vec<usize> = Vec::new();

    for (i, site) in world.construction_sites.iter_mut().enumerate() {
        if site.work_tick() {
            completed_indices.push(i);
        }
    }

    // Process completions in reverse order to preserve indices.
    for &i in completed_indices.iter().rev() {
        let site = world.construction_sites.remove(i);
        let is_storage = site.target_type == StructureType::StorageBuilding;
        let structure = site.into_structure();
        let structure_id = structure.id;
        let tribe_id = structure.tribe_id;
        world.structures.push(structure);

        // Auto-create a storage container for storage buildings.
        if is_storage {
            world.storages.push(Storage::new(structure_id, 20, tribe_id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::world_object::MaterialProperties;
    use crate::core::config::SimulationConfig;
    use crate::environment::structures::{ConstructionSite, StructureType};

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    #[test]
    fn construction_progresses_each_tick() {
        let mut world = test_world();
        world.construction_sites.push(ConstructionSite {
            id: 1,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            target_type: StructureType::Wall,
            builder_id: 100,
            progress: 0.0,
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: None,
        });

        run(&mut world);
        // Not yet complete, still in construction_sites
        assert_eq!(world.construction_sites.len(), 1);
        assert!(world.construction_sites[0].progress > 0.0);
        assert!(world.structures.is_empty());
    }

    #[test]
    fn completed_site_becomes_structure() {
        let mut world = test_world();
        world.construction_sites.push(ConstructionSite {
            id: 42,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            target_type: StructureType::Shelter,
            builder_id: 100,
            progress: 0.96, // will complete with one more tick (hardness=0 => +0.05)
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: Some(5),
        });

        run(&mut world);
        assert!(world.construction_sites.is_empty());
        assert_eq!(world.structures.len(), 1);
        assert_eq!(world.structures[0].id, 42);
        assert_eq!(world.structures[0].structure_type, StructureType::Shelter);
        assert_eq!(world.structures[0].tribe_id, Some(5));
    }

    #[test]
    fn storage_building_creates_storage_container() {
        let mut world = test_world();
        world.construction_sites.push(ConstructionSite {
            id: 99,
            x: 50.0,
            y: 50.0,
            width: 15.0,
            height: 15.0,
            target_type: StructureType::StorageBuilding,
            builder_id: 200,
            progress: 0.96,
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: Some(3),
        });

        run(&mut world);
        assert_eq!(world.structures.len(), 1);
        assert_eq!(world.storages.len(), 1);
        assert_eq!(world.storages[0].structure_id, 99);
        assert_eq!(world.storages[0].tribe_id, Some(3));
    }

    #[test]
    fn multiple_sites_process_independently() {
        let mut world = test_world();
        // Site 1: nearly complete
        world.construction_sites.push(ConstructionSite {
            id: 1,
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
            target_type: StructureType::Wall,
            builder_id: 100,
            progress: 0.96,
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: None,
        });
        // Site 2: just started
        world.construction_sites.push(ConstructionSite {
            id: 2,
            x: 50.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
            target_type: StructureType::Wall,
            builder_id: 200,
            progress: 0.0,
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: None,
        });

        run(&mut world);
        assert_eq!(world.structures.len(), 1);
        assert_eq!(world.construction_sites.len(), 1);
        assert_eq!(world.construction_sites[0].id, 2);
    }
}

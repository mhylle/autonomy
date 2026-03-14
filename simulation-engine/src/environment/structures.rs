use serde::{Deserialize, Serialize};

use crate::components::world_object::MaterialProperties;

/// A completed structure in the world (wall, shelter, storage building).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub material: MaterialProperties,
    /// Entity that built (or initiated building) this structure.
    pub builder_id: u64,
    /// Current durability (0.0 = destroyed).
    pub durability: f64,
    /// Maximum durability (derived from material hardness).
    pub max_durability: f64,
    /// What the structure does.
    pub structure_type: StructureType,
    /// Tribe that owns this structure (if any).
    pub tribe_id: Option<u64>,
}

/// The functional type of a structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureType {
    /// Blocks movement through its area.
    Wall,
    /// Reduces damage to entities inside its area.
    Shelter,
    /// Holds objects/resources (see storage system).
    StorageBuilding,
}

impl Structure {
    /// Check if a point is inside this structure's bounding box.
    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x
            && px <= self.x + self.width
            && py >= self.y
            && py <= self.y + self.height
    }

    /// Apply damage to the structure. Returns true if destroyed.
    pub fn take_damage(&mut self, amount: f64) -> bool {
        self.durability = (self.durability - amount).max(0.0);
        self.durability <= 0.0
    }

    /// Whether this structure is still standing.
    pub fn is_intact(&self) -> bool {
        self.durability > 0.0
    }

    /// Damage reduction factor for entities inside a shelter (0.0-1.0).
    /// Based on material hardness and remaining durability ratio.
    pub fn shelter_protection(&self) -> f64 {
        if self.structure_type != StructureType::Shelter || !self.is_intact() {
            return 0.0;
        }
        let durability_ratio = self.durability / self.max_durability;
        self.material.hardness * durability_ratio * 0.5
    }
}

/// A construction site: tracks build progress toward a completed structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructionSite {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub target_type: StructureType,
    /// Builder entity ID.
    pub builder_id: u64,
    /// Accumulated build progress (0.0 to 1.0).
    pub progress: f64,
    /// Material gathered so far (accumulated from contributions).
    pub accumulated_material: MaterialProperties,
    /// Number of material contributions received.
    pub contribution_count: u32,
    /// Tribe that owns this site (if any).
    pub tribe_id: Option<u64>,
}

impl ConstructionSite {
    /// How much progress is added per work tick. Harder materials are
    /// harder to build with, so progress is slower.
    const BASE_PROGRESS_PER_TICK: f64 = 0.05;

    /// Apply one tick of construction work. Returns true if complete.
    pub fn work_tick(&mut self) -> bool {
        let difficulty = 1.0 + self.accumulated_material.hardness;
        self.progress += Self::BASE_PROGRESS_PER_TICK / difficulty;
        self.progress = self.progress.min(1.0);
        self.is_complete()
    }

    /// Whether the construction is finished.
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Contribute material to this construction site.
    pub fn contribute_material(&mut self, material: &MaterialProperties) {
        let n = self.contribution_count as f64;
        let new_n = n + 1.0;
        // Running weighted average
        self.accumulated_material.hardness =
            (self.accumulated_material.hardness * n + material.hardness) / new_n;
        self.accumulated_material.weight =
            (self.accumulated_material.weight * n + material.weight) / new_n;
        self.accumulated_material.sharpness =
            (self.accumulated_material.sharpness * n + material.sharpness) / new_n;
        self.accumulated_material.flexibility =
            (self.accumulated_material.flexibility * n + material.flexibility) / new_n;
        self.accumulated_material.nutritional_value =
            (self.accumulated_material.nutritional_value * n + material.nutritional_value) / new_n;
        self.contribution_count += 1;
    }

    /// Convert a completed construction site into a Structure.
    pub fn into_structure(self) -> Structure {
        let max_durability = 50.0 + self.accumulated_material.hardness * 100.0;
        Structure {
            id: self.id,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            material: self.accumulated_material,
            builder_id: self.builder_id,
            durability: max_durability,
            max_durability,
            structure_type: self.target_type,
            tribe_id: self.tribe_id,
        }
    }
}

/// A planted crop/resource that grows over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Farm {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    /// Entity that planted this.
    pub planter_id: u64,
    /// Growth stage: 0.0 = just planted, 1.0 = mature/harvestable.
    pub growth: f64,
    /// Growth rate per tick (base, before tending bonus).
    pub growth_rate: f64,
    /// How much food this yields when harvested at full maturity.
    pub max_yield: f64,
    /// Whether this farm has been harvested (and thus removed).
    pub harvested: bool,
    /// Tick when last tended (tending boosts growth).
    pub last_tended_tick: u64,
    /// Tribe that owns this farm (if any).
    pub tribe_id: Option<u64>,
}

impl Farm {
    /// Tending bonus multiplier applied for TENDING_DURATION ticks after tending.
    pub const TENDING_BONUS: f64 = 2.0;
    /// How many ticks the tending bonus lasts.
    pub const TENDING_DURATION: u64 = 50;

    /// Advance growth by one tick.
    pub fn grow_tick(&mut self, current_tick: u64, climate_multiplier: f64) {
        if self.harvested || self.growth >= 1.0 {
            return;
        }
        let tending_mult = if current_tick.saturating_sub(self.last_tended_tick) < Self::TENDING_DURATION {
            Self::TENDING_BONUS
        } else {
            1.0
        };
        self.growth += self.growth_rate * climate_multiplier * tending_mult;
        self.growth = self.growth.min(1.0);
    }

    /// Whether the farm is ready to harvest.
    pub fn is_mature(&self) -> bool {
        self.growth >= 1.0 && !self.harvested
    }

    /// Harvest the farm, returning the food yield.
    pub fn harvest(&mut self) -> f64 {
        if !self.is_mature() {
            return 0.0;
        }
        self.harvested = true;
        self.max_yield * self.growth
    }

    /// Tend the farm (record the current tick for the bonus).
    pub fn tend(&mut self, current_tick: u64) {
        self.last_tended_tick = current_tick;
    }
}

/// Storage container associated with a StorageBuilding structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub structure_id: u64,
    /// Object IDs stored here.
    pub items: Vec<u64>,
    /// Maximum items this storage can hold.
    pub capacity: usize,
    /// Food units stored (bulk resource, not individual objects).
    pub food_stored: f64,
    /// Maximum food units.
    pub food_capacity: f64,
    /// Tribe that can access this storage (if any).
    pub tribe_id: Option<u64>,
}

impl Storage {
    pub fn new(structure_id: u64, capacity: usize, tribe_id: Option<u64>) -> Self {
        Self {
            structure_id,
            items: Vec::new(),
            capacity,
            food_stored: 0.0,
            food_capacity: 200.0,
            tribe_id,
        }
    }

    /// Deposit an object. Returns false if full.
    pub fn deposit_item(&mut self, object_id: u64) -> bool {
        if self.items.len() >= self.capacity {
            return false;
        }
        self.items.push(object_id);
        true
    }

    /// Withdraw an object by ID. Returns false if not found.
    pub fn withdraw_item(&mut self, object_id: u64) -> bool {
        if let Some(pos) = self.items.iter().position(|&id| id == object_id) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }

    /// Deposit food. Returns amount actually stored (may be less if near capacity).
    pub fn deposit_food(&mut self, amount: f64) -> f64 {
        let space = self.food_capacity - self.food_stored;
        let actual = amount.min(space).max(0.0);
        self.food_stored += actual;
        actual
    }

    /// Withdraw food. Returns amount actually withdrawn.
    pub fn withdraw_food(&mut self, amount: f64) -> f64 {
        let actual = amount.min(self.food_stored).max(0.0);
        self.food_stored -= actual;
        actual
    }

    /// Whether an entity can access this storage (same tribe or no tribe restriction).
    pub fn can_access(&self, entity_tribe_id: Option<u64>) -> bool {
        match self.tribe_id {
            None => true,
            Some(tid) => entity_tribe_id == Some(tid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_structure(stype: StructureType) -> Structure {
        Structure {
            id: 1,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            material: MaterialProperties {
                hardness: 0.8,
                weight: 0.5,
                sharpness: 0.0,
                flexibility: 0.2,
                nutritional_value: 0.0,
            },
            builder_id: 100,
            durability: 100.0,
            max_durability: 100.0,
            structure_type: stype,
            tribe_id: None,
        }
    }

    #[test]
    fn structure_contains_point_inside() {
        let s = test_structure(StructureType::Wall);
        assert!(s.contains_point(15.0, 15.0));
        assert!(s.contains_point(10.0, 10.0)); // corner
        assert!(s.contains_point(30.0, 30.0)); // opposite corner
    }

    #[test]
    fn structure_does_not_contain_point_outside() {
        let s = test_structure(StructureType::Wall);
        assert!(!s.contains_point(5.0, 15.0));
        assert!(!s.contains_point(35.0, 15.0));
        assert!(!s.contains_point(15.0, 5.0));
        assert!(!s.contains_point(15.0, 35.0));
    }

    #[test]
    fn structure_take_damage() {
        let mut s = test_structure(StructureType::Wall);
        assert!(!s.take_damage(50.0));
        assert_eq!(s.durability, 50.0);
        assert!(s.is_intact());

        assert!(s.take_damage(60.0));
        assert_eq!(s.durability, 0.0);
        assert!(!s.is_intact());
    }

    #[test]
    fn shelter_protection_factor() {
        let shelter = test_structure(StructureType::Shelter);
        let prot = shelter.shelter_protection();
        // hardness=0.8, durability_ratio=1.0, * 0.5 = 0.4
        assert!((prot - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn wall_has_no_shelter_protection() {
        let wall = test_structure(StructureType::Wall);
        assert_eq!(wall.shelter_protection(), 0.0);
    }

    #[test]
    fn destroyed_shelter_has_no_protection() {
        let mut shelter = test_structure(StructureType::Shelter);
        shelter.durability = 0.0;
        assert_eq!(shelter.shelter_protection(), 0.0);
    }

    #[test]
    fn construction_site_progress() {
        let mut site = ConstructionSite {
            id: 1,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            target_type: StructureType::Wall,
            builder_id: 100,
            progress: 0.0,
            accumulated_material: MaterialProperties::default(),
            contribution_count: 1,
            tribe_id: None,
        };
        // With default hardness=0.5, difficulty=1.5, progress=0.05/1.5~0.0333
        assert!(!site.work_tick());
        assert!(site.progress > 0.0);
        assert!(!site.is_complete());
    }

    #[test]
    fn construction_site_completes() {
        let mut site = ConstructionSite {
            id: 1,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            target_type: StructureType::Shelter,
            builder_id: 100,
            progress: 0.95,
            accumulated_material: MaterialProperties {
                hardness: 0.0,
                ..Default::default()
            },
            contribution_count: 1,
            tribe_id: None,
        };
        // With hardness=0, difficulty=1.0, progress += 0.05 -> 1.0
        assert!(site.work_tick());
        assert!(site.is_complete());
    }

    #[test]
    fn construction_site_into_structure() {
        let site = ConstructionSite {
            id: 42,
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
            target_type: StructureType::Shelter,
            builder_id: 200,
            progress: 1.0,
            accumulated_material: MaterialProperties {
                hardness: 0.8,
                weight: 0.3,
                sharpness: 0.1,
                flexibility: 0.3,
                nutritional_value: 0.0,
            },
            contribution_count: 3,
            tribe_id: Some(7),
        };
        let s = site.into_structure();
        assert_eq!(s.id, 42);
        assert_eq!(s.structure_type, StructureType::Shelter);
        assert_eq!(s.builder_id, 200);
        assert_eq!(s.tribe_id, Some(7));
        // max_durability = 50 + 0.8*100 = 130
        assert!((s.max_durability - 130.0).abs() < f64::EPSILON);
        assert_eq!(s.durability, s.max_durability);
    }

    #[test]
    fn construction_contribute_material() {
        let mut site = ConstructionSite {
            id: 1,
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
            target_type: StructureType::Wall,
            builder_id: 1,
            progress: 0.0,
            accumulated_material: MaterialProperties {
                hardness: 1.0,
                weight: 1.0,
                sharpness: 1.0,
                flexibility: 1.0,
                nutritional_value: 0.0,
            },
            contribution_count: 1,
            tribe_id: None,
        };
        site.contribute_material(&MaterialProperties {
            hardness: 0.0,
            weight: 0.0,
            sharpness: 0.0,
            flexibility: 0.0,
            nutritional_value: 0.0,
        });
        assert_eq!(site.contribution_count, 2);
        // Average of 1.0 and 0.0 = 0.5
        assert!((site.accumulated_material.hardness - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn farm_growth_basic() {
        let mut farm = Farm {
            id: 1,
            x: 10.0,
            y: 10.0,
            planter_id: 1,
            growth: 0.0,
            growth_rate: 0.01,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 0,
            tribe_id: None,
        };
        // Without tending (current tick far from last tended)
        farm.grow_tick(1000, 1.0);
        assert!((farm.growth - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn farm_tending_boost() {
        let mut farm = Farm {
            id: 1,
            x: 10.0,
            y: 10.0,
            planter_id: 1,
            growth: 0.0,
            growth_rate: 0.01,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 100,
            tribe_id: None,
        };
        // Tended recently (tick 100, current tick 110, within TENDING_DURATION=50)
        farm.grow_tick(110, 1.0);
        // With tending bonus: 0.01 * 1.0 * 2.0 = 0.02
        assert!((farm.growth - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn farm_harvest_when_mature() {
        let mut farm = Farm {
            id: 1,
            x: 10.0,
            y: 10.0,
            planter_id: 1,
            growth: 1.0,
            growth_rate: 0.01,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 0,
            tribe_id: None,
        };
        assert!(farm.is_mature());
        let yield_amount = farm.harvest();
        assert!((yield_amount - 50.0).abs() < f64::EPSILON);
        assert!(farm.harvested);
        // Can't harvest again
        assert_eq!(farm.harvest(), 0.0);
    }

    #[test]
    fn farm_cannot_harvest_immature() {
        let mut farm = Farm {
            id: 1,
            x: 10.0,
            y: 10.0,
            planter_id: 1,
            growth: 0.5,
            growth_rate: 0.01,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 0,
            tribe_id: None,
        };
        assert!(!farm.is_mature());
        assert_eq!(farm.harvest(), 0.0);
    }

    #[test]
    fn farm_growth_caps_at_one() {
        let mut farm = Farm {
            id: 1,
            x: 10.0,
            y: 10.0,
            planter_id: 1,
            growth: 0.99,
            growth_rate: 0.1,
            max_yield: 50.0,
            harvested: false,
            last_tended_tick: 0,
            tribe_id: None,
        };
        farm.grow_tick(1000, 1.0);
        assert!((farm.growth - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_deposit_withdraw_items() {
        let mut storage = Storage::new(1, 5, None);
        assert!(storage.deposit_item(10));
        assert!(storage.deposit_item(20));
        assert_eq!(storage.items.len(), 2);

        assert!(storage.withdraw_item(10));
        assert!(!storage.withdraw_item(10)); // already removed
        assert_eq!(storage.items.len(), 1);
    }

    #[test]
    fn storage_capacity_limit() {
        let mut storage = Storage::new(1, 2, None);
        assert!(storage.deposit_item(1));
        assert!(storage.deposit_item(2));
        assert!(!storage.deposit_item(3)); // full
    }

    #[test]
    fn storage_food_deposit_withdraw() {
        let mut storage = Storage::new(1, 5, None);
        let deposited = storage.deposit_food(100.0);
        assert!((deposited - 100.0).abs() < f64::EPSILON);
        assert!((storage.food_stored - 100.0).abs() < f64::EPSILON);

        let withdrawn = storage.withdraw_food(30.0);
        assert!((withdrawn - 30.0).abs() < f64::EPSILON);
        assert!((storage.food_stored - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_food_capacity_limit() {
        let mut storage = Storage::new(1, 5, None);
        storage.food_capacity = 50.0;
        let deposited = storage.deposit_food(80.0);
        assert!((deposited - 50.0).abs() < f64::EPSILON);
        assert!((storage.food_stored - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_food_withdraw_limited() {
        let mut storage = Storage::new(1, 5, None);
        storage.food_stored = 10.0;
        let withdrawn = storage.withdraw_food(30.0);
        assert!((withdrawn - 10.0).abs() < f64::EPSILON);
        assert!((storage.food_stored - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_tribe_access() {
        let storage = Storage::new(1, 5, Some(42));
        assert!(storage.can_access(Some(42)));
        assert!(!storage.can_access(Some(99)));
        assert!(!storage.can_access(None));
    }

    #[test]
    fn storage_no_tribe_anyone_can_access() {
        let storage = Storage::new(1, 5, None);
        assert!(storage.can_access(Some(42)));
        assert!(storage.can_access(None));
    }
}

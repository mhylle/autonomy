use serde::{Deserialize, Serialize};

/// Physical material properties of a world object.
///
/// All values are normalized to the 0.0-1.0 range. These properties determine
/// how the object interacts with entities (tool use, combat bonuses, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialProperties {
    /// Resistance to deformation (0 = soft, 1 = diamond-hard).
    pub hardness: f64,
    /// Edge quality (0 = blunt, 1 = razor-sharp).
    pub sharpness: f64,
    /// Mass relative to entity carrying capacity (0 = feather, 1 = boulder).
    pub weight: f64,
    /// Ability to bend without breaking (0 = brittle, 1 = rubber).
    pub flexibility: f64,
    /// Food value when consumed (0 = inedible, 1 = highly nutritious).
    pub nutritional_value: f64,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            hardness: 0.5,
            sharpness: 0.0,
            weight: 0.3,
            flexibility: 0.3,
            nutritional_value: 0.0,
        }
    }
}

impl MaterialProperties {
    /// Compute a weighted average of two material property sets.
    pub fn weighted_average(
        &self,
        other: &MaterialProperties,
        self_weight: f64,
    ) -> MaterialProperties {
        let other_weight = 1.0 - self_weight;
        MaterialProperties {
            hardness: self.hardness * self_weight + other.hardness * other_weight,
            sharpness: self.sharpness * self_weight + other.sharpness * other_weight,
            weight: self.weight * self_weight + other.weight * other_weight,
            flexibility: self.flexibility * self_weight + other.flexibility * other_weight,
            nutritional_value: self.nutritional_value * self_weight
                + other.nutritional_value * other_weight,
        }
    }
}

/// A discrete object in the world that entities can interact with.
///
/// Objects persist across ticks, decay over time, and can be picked up,
/// dropped, equipped, and used as tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldObject {
    /// Unique identifier for this object.
    pub id: u64,
    /// World-space x position (meaningful only when not held by an entity).
    pub x: f64,
    /// World-space y position (meaningful only when not held by an entity).
    pub y: f64,
    /// Physical material properties.
    pub material: MaterialProperties,
    /// Remaining durability (0.0 = destroyed). Decreases each tick via decay
    /// and when the object is used as a tool.
    pub durability: f64,
    /// Maximum durability when first created.
    pub max_durability: f64,
    /// Entity ID of the creator, if any.
    pub creator_id: Option<u64>,
    /// Tick at which this object was created.
    pub created_tick: u64,
    /// Entity ID that currently holds this object (`None` if on the ground).
    pub held_by: Option<u64>,
}

impl WorldObject {
    /// Apply per-tick decay to durability.
    ///
    /// Decay rate is inversely proportional to hardness: softer objects decay faster.
    /// Returns `true` if the object is still intact after decay.
    pub fn apply_decay(&mut self) -> bool {
        let decay_rate = 0.01 * (1.0 - self.material.hardness * 0.8);
        self.durability = (self.durability - decay_rate).max(0.0);
        self.durability > 0.0
    }

    /// Apply wear from tool use. Returns `true` if still intact.
    pub fn apply_use_wear(&mut self, wear_amount: f64) -> bool {
        self.durability = (self.durability - wear_amount).max(0.0);
        self.durability > 0.0
    }

    /// Whether this object is still usable (durability > 0).
    pub fn is_intact(&self) -> bool {
        self.durability > 0.0
    }

    /// Whether this object is on the ground (not held by anyone).
    pub fn is_on_ground(&self) -> bool {
        self.held_by.is_none()
    }

    /// Compute the attack bonus when this object is used as a weapon.
    /// Sharp objects give an attack bonus.
    pub fn attack_bonus(&self) -> f64 {
        self.material.sharpness * 5.0
    }

    /// Compute the defense bonus when this object is used as a shield.
    /// Hard + heavy objects give a defense bonus.
    pub fn defense_bonus(&self) -> f64 {
        self.material.hardness * self.material.weight * 5.0
    }
}

/// Inventory component for entities that can carry objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    /// IDs of objects currently held.
    pub items: Vec<u64>,
    /// Maximum number of objects this entity can carry.
    pub max_capacity: usize,
    /// ID of the currently equipped tool (must also be in `items`).
    pub equipped: Option<u64>,
}

impl Inventory {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            items: Vec::new(),
            max_capacity,
            equipped: None,
        }
    }

    /// Whether the inventory has room for another item.
    pub fn has_room(&self) -> bool {
        self.items.len() < self.max_capacity
    }

    /// Whether the inventory is full.
    pub fn is_full(&self) -> bool {
        !self.has_room()
    }

    /// Add an object ID to the inventory if there is room.
    /// Returns `true` if the object was added.
    pub fn add(&mut self, object_id: u64) -> bool {
        if self.has_room() {
            self.items.push(object_id);
            true
        } else {
            false
        }
    }

    /// Remove an object ID from the inventory.
    /// Returns `true` if the object was found and removed.
    pub fn remove(&mut self, object_id: u64) -> bool {
        if let Some(pos) = self.items.iter().position(|&id| id == object_id) {
            self.items.remove(pos);
            // Unequip if this was the equipped item.
            if self.equipped == Some(object_id) {
                self.equipped = None;
            }
            true
        } else {
            false
        }
    }

    /// Check whether an object is in the inventory.
    pub fn contains(&self, object_id: u64) -> bool {
        self.items.contains(&object_id)
    }

    /// Equip an object that is already in the inventory.
    /// Returns `true` if successfully equipped.
    pub fn equip(&mut self, object_id: u64) -> bool {
        if self.items.contains(&object_id) {
            self.equipped = Some(object_id);
            true
        } else {
            false
        }
    }

    /// Total weight of all carried items (requires access to world objects).
    pub fn total_weight(&self, objects: &[WorldObject]) -> f64 {
        self.items
            .iter()
            .filter_map(|id| objects.iter().find(|o| o.id == *id))
            .map(|o| o.material.weight)
            .sum()
    }
}

/// Blueprint for creating objects. Stored in the genome as simple parameters.
///
/// Defines what kind of resource is needed and what the output looks like.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    /// Minimum energy required to create the object.
    pub energy_cost: f64,
    /// Preference weights for input material selection (what resource traits to prefer).
    pub hardness_preference: f64,
    /// Output sharpness (how sharp the created object is).
    pub output_sharpness: f64,
    /// Output hardness of the created object.
    pub output_hardness: f64,
    /// Output weight of the created object.
    pub output_weight: f64,
    /// Output durability of the created object.
    pub output_durability: f64,
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            energy_cost: 20.0,
            hardness_preference: 0.5,
            output_sharpness: 0.3,
            output_hardness: 0.5,
            output_weight: 0.3,
            output_durability: 50.0,
        }
    }
}

impl Blueprint {
    /// Create a WorldObject from this blueprint at the given position.
    pub fn create_object(
        &self,
        id: u64,
        x: f64,
        y: f64,
        creator_id: u64,
        tick: u64,
    ) -> WorldObject {
        WorldObject {
            id,
            x,
            y,
            material: MaterialProperties {
                hardness: self.output_hardness.clamp(0.0, 1.0),
                sharpness: self.output_sharpness.clamp(0.0, 1.0),
                weight: self.output_weight.clamp(0.0, 1.0),
                flexibility: (1.0 - self.output_hardness).clamp(0.0, 1.0),
                nutritional_value: 0.0,
            },
            durability: self.output_durability.max(1.0),
            max_durability: self.output_durability.max(1.0),
            creator_id: Some(creator_id),
            created_tick: tick,
            held_by: None,
        }
    }
}

/// Perceived nearby object for the perception system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceivedObject {
    /// Object ID.
    pub object_id: u64,
    /// World-space position.
    pub x: f64,
    pub y: f64,
    /// Distance from the perceiving entity.
    pub distance: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_object(id: u64, hardness: f64, durability: f64) -> WorldObject {
        WorldObject {
            id,
            x: 0.0,
            y: 0.0,
            material: MaterialProperties {
                hardness,
                ..MaterialProperties::default()
            },
            durability,
            max_durability: durability,
            creator_id: None,
            created_tick: 0,
            held_by: None,
        }
    }

    #[test]
    fn material_properties_default() {
        let m = MaterialProperties::default();
        assert_eq!(m.hardness, 0.5);
        assert_eq!(m.sharpness, 0.0);
        assert_eq!(m.weight, 0.3);
        assert_eq!(m.flexibility, 0.3);
        assert_eq!(m.nutritional_value, 0.0);
    }

    #[test]
    fn material_weighted_average_equal() {
        let a = MaterialProperties {
            hardness: 1.0,
            weight: 0.8,
            sharpness: 0.0,
            flexibility: 0.0,
            nutritional_value: 0.0,
        };
        let b = MaterialProperties {
            hardness: 0.0,
            weight: 0.0,
            sharpness: 1.0,
            flexibility: 1.0,
            nutritional_value: 1.0,
        };
        let avg = a.weighted_average(&b, 0.5);
        assert!((avg.hardness - 0.5).abs() < f64::EPSILON);
        assert!((avg.weight - 0.4).abs() < f64::EPSILON);
        assert!((avg.sharpness - 0.5).abs() < f64::EPSILON);
        assert!((avg.flexibility - 0.5).abs() < f64::EPSILON);
        assert!((avg.nutritional_value - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn world_object_decay_reduces_durability() {
        let mut obj = make_test_object(1, 0.5, 1.0);
        let intact = obj.apply_decay();
        assert!(obj.durability < 1.0);
        assert!(intact);
    }

    #[test]
    fn world_object_decay_destroys_fragile_object() {
        let mut obj = make_test_object(1, 0.0, 0.005);
        let intact = obj.apply_decay();
        assert!(!intact);
        assert_eq!(obj.durability, 0.0);
    }

    #[test]
    fn hard_objects_decay_slower() {
        let mut soft = make_test_object(1, 0.0, 100.0);
        let mut hard = make_test_object(2, 1.0, 100.0);

        soft.apply_decay();
        hard.apply_decay();

        assert!(
            hard.durability > soft.durability,
            "hard object should decay slower: hard={}, soft={}",
            hard.durability,
            soft.durability
        );
    }

    #[test]
    fn use_wear_reduces_durability() {
        let mut obj = make_test_object(1, 0.5, 10.0);
        let intact = obj.apply_use_wear(3.0);
        assert!(intact);
        assert!((obj.durability - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn use_wear_can_destroy() {
        let mut obj = make_test_object(1, 0.5, 2.0);
        let intact = obj.apply_use_wear(5.0);
        assert!(!intact);
        assert_eq!(obj.durability, 0.0);
    }

    #[test]
    fn attack_bonus_proportional_to_sharpness() {
        let mut sharp = make_test_object(1, 0.5, 10.0);
        sharp.material.sharpness = 0.8;
        let blunt = make_test_object(2, 0.5, 10.0);

        assert!(sharp.attack_bonus() > blunt.attack_bonus());
        assert_eq!(blunt.attack_bonus(), 0.0);
    }

    #[test]
    fn defense_bonus_from_hard_heavy_object() {
        let mut shield = make_test_object(1, 0.9, 10.0);
        shield.material.weight = 0.8;
        assert!(shield.defense_bonus() > 0.0);
    }

    #[test]
    fn inventory_add_and_remove() {
        let mut inv = Inventory::new(3);
        assert!(inv.has_room());
        assert!(inv.add(1));
        assert!(inv.add(2));
        assert!(inv.add(3));
        assert!(!inv.has_room());
        assert!(inv.is_full());
        assert!(!inv.add(4)); // full
        assert!(inv.remove(2));
        assert!(inv.has_room());
        assert_eq!(inv.items, vec![1, 3]);
    }

    #[test]
    fn inventory_contains() {
        let mut inv = Inventory::new(5);
        inv.add(10);
        assert!(inv.contains(10));
        assert!(!inv.contains(99));
    }

    #[test]
    fn inventory_equip_and_unequip_on_remove() {
        let mut inv = Inventory::new(5);
        inv.add(10);
        inv.add(20);
        assert!(inv.equip(10));
        assert_eq!(inv.equipped, Some(10));
        // Cannot equip item not in inventory.
        assert!(!inv.equip(99));
        // Removing equipped item unequips it.
        inv.remove(10);
        assert_eq!(inv.equipped, None);
    }

    #[test]
    fn inventory_total_weight() {
        let objects = vec![
            WorldObject {
                id: 1,
                x: 0.0,
                y: 0.0,
                material: MaterialProperties {
                    weight: 0.3,
                    ..MaterialProperties::default()
                },
                durability: 10.0,
                max_durability: 10.0,
                creator_id: None,
                created_tick: 0,
                held_by: None,
            },
            WorldObject {
                id: 2,
                x: 0.0,
                y: 0.0,
                material: MaterialProperties {
                    weight: 0.7,
                    ..MaterialProperties::default()
                },
                durability: 10.0,
                max_durability: 10.0,
                creator_id: None,
                created_tick: 0,
                held_by: None,
            },
        ];
        let mut inv = Inventory::new(5);
        inv.add(1);
        inv.add(2);
        let total = inv.total_weight(&objects);
        assert!((total - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn blueprint_creates_object() {
        let bp = Blueprint::default();
        let obj = bp.create_object(42, 10.0, 20.0, 99, 100);
        assert_eq!(obj.id, 42);
        assert_eq!(obj.x, 10.0);
        assert_eq!(obj.y, 20.0);
        assert_eq!(obj.creator_id, Some(99));
        assert_eq!(obj.created_tick, 100);
        assert!(obj.durability > 0.0);
        assert!(obj.is_intact());
        assert!(obj.is_on_ground());
    }

    #[test]
    fn blueprint_output_values_clamped() {
        let bp = Blueprint {
            output_sharpness: 1.5,
            output_hardness: -0.5,
            output_weight: 2.0,
            output_durability: 0.0,
            ..Blueprint::default()
        };
        let obj = bp.create_object(1, 0.0, 0.0, 0, 0);
        assert!(obj.material.sharpness <= 1.0);
        assert!(obj.material.hardness >= 0.0);
        assert!(obj.material.weight <= 1.0);
        assert!(obj.durability >= 1.0);
    }

    #[test]
    fn perceived_object_serialization_roundtrip() {
        let po = PerceivedObject {
            object_id: 42,
            x: 10.0,
            y: 20.0,
            distance: 15.0,
        };
        let json = serde_json::to_string(&po).unwrap();
        let d: PerceivedObject = serde_json::from_str(&json).unwrap();
        assert_eq!(d.object_id, 42);
        assert_eq!(d.x, 10.0);
        assert_eq!(d.y, 20.0);
        assert_eq!(d.distance, 15.0);
    }

    #[test]
    fn is_on_ground_changes_with_held_by() {
        let mut obj = make_test_object(1, 0.5, 10.0);
        assert!(obj.is_on_ground());
        obj.held_by = Some(42);
        assert!(!obj.is_on_ground());
    }
}

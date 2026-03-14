//! Crafting system: combine inventory objects to create new ones.
//!
//! When an entity has multiple objects in its inventory, it can combine them
//! into a new object with properties derived from the weighted average of
//! the input materials. The crafting recipe is guided by the entity's genome
//! (Blueprint).

use crate::components::world_object::{MaterialProperties, WorldObject};

/// Result of a crafting attempt.
pub struct CraftResult {
    /// The newly created object.
    pub product: WorldObject,
    /// IDs of the consumed input objects.
    pub consumed_ids: Vec<u64>,
}

/// Attempt to craft a new object from two input objects.
///
/// The output material properties are a weighted average of the two inputs.
/// The output durability is the average of the two input durabilities scaled
/// by the resulting hardness.
///
/// Returns `None` if fewer than 2 objects are provided.
pub fn craft_from_objects(
    inputs: &[&WorldObject],
    new_id: u64,
    crafter_id: u64,
    tick: u64,
) -> Option<CraftResult> {
    if inputs.len() < 2 {
        return None;
    }

    let total = inputs.len() as f64;
    let mut avg = MaterialProperties {
        hardness: 0.0,
        sharpness: 0.0,
        weight: 0.0,
        flexibility: 0.0,
        nutritional_value: 0.0,
    };

    let mut avg_durability = 0.0;
    let consumed_ids: Vec<u64> = inputs.iter().map(|o| o.id).collect();

    for obj in inputs {
        avg.hardness += obj.material.hardness / total;
        avg.sharpness += obj.material.sharpness / total;
        avg.weight += obj.material.weight / total;
        avg.flexibility += obj.material.flexibility / total;
        avg.nutritional_value += obj.material.nutritional_value / total;
        avg_durability += obj.durability / total;
    }

    // Harder combined materials produce more durable results.
    let durability_bonus = 1.0 + avg.hardness * 0.5;
    let result_durability = (avg_durability * durability_bonus).max(1.0);

    let product = WorldObject {
        id: new_id,
        x: 0.0,
        y: 0.0,
        material: avg,
        durability: result_durability,
        max_durability: result_durability,
        creator_id: Some(crafter_id),
        created_tick: tick,
        held_by: Some(crafter_id),
    };

    Some(CraftResult {
        product,
        consumed_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_object(id: u64, hardness: f64, sharpness: f64, durability: f64) -> WorldObject {
        WorldObject {
            id,
            x: 0.0,
            y: 0.0,
            material: MaterialProperties {
                hardness,
                sharpness,
                weight: 0.5,
                flexibility: 0.5,
                nutritional_value: 0.0,
            },
            durability,
            max_durability: durability,
            creator_id: None,
            created_tick: 0,
            held_by: None,
        }
    }

    #[test]
    fn craft_requires_at_least_two_inputs() {
        let obj = make_object(1, 0.5, 0.3, 10.0);
        let result = craft_from_objects(&[&obj], 100, 1, 0);
        assert!(result.is_none());
    }

    #[test]
    fn craft_empty_inputs_returns_none() {
        let result = craft_from_objects(&[], 100, 1, 0);
        assert!(result.is_none());
    }

    #[test]
    fn craft_two_objects_averages_properties() {
        let a = make_object(1, 1.0, 0.0, 20.0);
        let b = make_object(2, 0.0, 1.0, 10.0);

        let result = craft_from_objects(&[&a, &b], 100, 42, 50).unwrap();
        let product = &result.product;

        assert_eq!(product.id, 100);
        assert_eq!(product.creator_id, Some(42));
        assert_eq!(product.created_tick, 50);
        assert!((product.material.hardness - 0.5).abs() < f64::EPSILON);
        assert!((product.material.sharpness - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn craft_consumed_ids_are_correct() {
        let a = make_object(10, 0.5, 0.5, 10.0);
        let b = make_object(20, 0.5, 0.5, 10.0);

        let result = craft_from_objects(&[&a, &b], 100, 1, 0).unwrap();
        assert_eq!(result.consumed_ids, vec![10, 20]);
    }

    #[test]
    fn craft_durability_bonus_from_hardness() {
        // Two hard objects
        let hard_a = make_object(1, 1.0, 0.0, 10.0);
        let hard_b = make_object(2, 1.0, 0.0, 10.0);

        // Two soft objects
        let soft_a = make_object(3, 0.0, 0.0, 10.0);
        let soft_b = make_object(4, 0.0, 0.0, 10.0);

        let hard_result = craft_from_objects(&[&hard_a, &hard_b], 100, 1, 0).unwrap();
        let soft_result = craft_from_objects(&[&soft_a, &soft_b], 200, 1, 0).unwrap();

        assert!(
            hard_result.product.durability > soft_result.product.durability,
            "Hard craft durability {} should exceed soft {}",
            hard_result.product.durability,
            soft_result.product.durability
        );
    }

    #[test]
    fn craft_three_objects() {
        let a = make_object(1, 0.9, 0.0, 30.0);
        let b = make_object(2, 0.3, 0.6, 20.0);
        let c = make_object(3, 0.0, 0.3, 10.0);

        let result = craft_from_objects(&[&a, &b, &c], 100, 1, 0).unwrap();
        let product = &result.product;

        // Average hardness: (0.9 + 0.3 + 0.0) / 3 = 0.4
        assert!((product.material.hardness - 0.4).abs() < 0.01);
        // Average sharpness: (0.0 + 0.6 + 0.3) / 3 = 0.3
        assert!((product.material.sharpness - 0.3).abs() < 0.01);
        assert_eq!(result.consumed_ids.len(), 3);
    }

    #[test]
    fn craft_product_is_held_by_crafter() {
        let a = make_object(1, 0.5, 0.5, 10.0);
        let b = make_object(2, 0.5, 0.5, 10.0);

        let result = craft_from_objects(&[&a, &b], 100, 42, 0).unwrap();
        assert_eq!(result.product.held_by, Some(42));
    }
}

use serde::{Deserialize, Serialize};

use super::world_object::PerceivedObject;

/// A nearby entity as perceived by this entity's sensors.
///
/// Energy estimates are imperfect -- noise increases with distance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceivedEntity {
    /// Raw hecs entity ID bits (decoupled from ECS crate).
    pub entity_id: u64,
    /// Perceived position.
    pub x: f64,
    pub y: f64,
    /// Euclidean distance from the perceiving entity.
    pub distance: f64,
    /// Estimated energy (noisy -- error proportional to distance).
    pub energy_estimate: f64,
    /// Whether this entity shares the same species_id.
    pub is_kin: bool,
}

/// A nearby resource as perceived by this entity's sensors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceivedResource {
    /// Index into the world's resource list.
    pub resource_index: usize,
    /// Resource position.
    pub x: f64,
    pub y: f64,
    /// Euclidean distance from the perceiving entity.
    pub distance: f64,
}

/// A nearby signal perceived by this entity's sensors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceivedSignal {
    /// The signal type identifier.
    pub signal_type: u8,
    /// Euclidean distance from the perceiving entity to the signal source.
    pub distance: f64,
    /// Normalized direction x-component toward the signal source.
    pub direction_x: f64,
    /// Normalized direction y-component toward the signal source.
    pub direction_y: f64,
    /// Effective strength of the signal at the perceiver's location.
    pub strength: f64,
    /// World-space x position of the signal source.
    pub source_x: f64,
    /// World-space y position of the signal source.
    pub source_y: f64,
}

/// Sensory perception of the surrounding world, populated each tick.
///
/// The `sensor_range` is read from the entity's `Genome`. This component
/// stores the results for other systems (wander, drives, BT) to consume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Perception {
    pub perceived_entities: Vec<PerceivedEntity>,
    pub perceived_resources: Vec<PerceivedResource>,
    pub perceived_signals: Vec<PerceivedSignal>,
    /// Nearby world objects on the ground (Phase 6.2).
    pub perceived_objects: Vec<PerceivedObject>,
}

impl Perception {
    /// Clear all perceived data (called at the start of each perception tick).
    pub fn clear(&mut self) {
        self.perceived_entities.clear();
        self.perceived_resources.clear();
        self.perceived_signals.clear();
        self.perceived_objects.clear();
    }

    /// The closest perceived resource, if any.
    pub fn closest_resource(&self) -> Option<&PerceivedResource> {
        self.perceived_resources
            .iter()
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }

    /// The closest perceived entity, if any.
    pub fn closest_entity(&self) -> Option<&PerceivedEntity> {
        self.perceived_entities
            .iter()
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }

    /// The strongest perceived signal of a given type, if any.
    pub fn strongest_signal_of_type(&self, signal_type: u8) -> Option<&PerceivedSignal> {
        self.perceived_signals
            .iter()
            .filter(|s| s.signal_type == signal_type)
            .max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap())
    }

    /// The closest perceived world object on the ground, if any.
    pub fn closest_object(&self) -> Option<&PerceivedObject> {
        self.perceived_objects
            .iter()
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }

    /// Whether any signal of the given type is perceived.
    pub fn has_signal_of_type(&self, signal_type: u8) -> bool {
        self.perceived_signals
            .iter()
            .any(|s| s.signal_type == signal_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_perception_is_empty() {
        let p = Perception::default();
        assert!(p.perceived_entities.is_empty());
        assert!(p.perceived_resources.is_empty());
        assert!(p.perceived_signals.is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut p = Perception {
            perceived_entities: vec![PerceivedEntity {
                entity_id: 1,
                x: 10.0,
                y: 20.0,
                distance: 5.0,
                energy_estimate: 50.0,
                is_kin: false,
            }],
            perceived_resources: vec![PerceivedResource {
                resource_index: 0,
                x: 30.0,
                y: 40.0,
                distance: 10.0,
            }],
            perceived_signals: vec![PerceivedSignal {
                signal_type: 1,
                distance: 5.0,
                direction_x: 1.0,
                direction_y: 0.0,
                strength: 0.8,
                source_x: 35.0,
                source_y: 40.0,
            }],
            perceived_objects: vec![],
        };

        p.clear();
        assert!(p.perceived_entities.is_empty());
        assert!(p.perceived_resources.is_empty());
        assert!(p.perceived_signals.is_empty());
    }

    #[test]
    fn closest_resource_returns_nearest() {
        let p = Perception {
            perceived_entities: vec![],
            perceived_resources: vec![
                PerceivedResource {
                    resource_index: 0,
                    x: 100.0,
                    y: 100.0,
                    distance: 50.0,
                },
                PerceivedResource {
                    resource_index: 1,
                    x: 20.0,
                    y: 20.0,
                    distance: 10.0,
                },
                PerceivedResource {
                    resource_index: 2,
                    x: 60.0,
                    y: 60.0,
                    distance: 30.0,
                },
            ],
            ..Default::default()
        };

        let closest = p.closest_resource().unwrap();
        assert_eq!(closest.resource_index, 1);
        assert_eq!(closest.distance, 10.0);
    }

    #[test]
    fn closest_resource_none_when_empty() {
        let p = Perception::default();
        assert!(p.closest_resource().is_none());
    }

    #[test]
    fn closest_entity_returns_nearest() {
        let p = Perception {
            perceived_entities: vec![
                PerceivedEntity {
                    entity_id: 1,
                    x: 50.0,
                    y: 50.0,
                    distance: 30.0,
                    energy_estimate: 80.0,
                    is_kin: true,
                },
                PerceivedEntity {
                    entity_id: 2,
                    x: 20.0,
                    y: 20.0,
                    distance: 5.0,
                    energy_estimate: 40.0,
                    is_kin: false,
                },
            ],
            ..Default::default()
        };

        let closest = p.closest_entity().unwrap();
        assert_eq!(closest.entity_id, 2);
        assert_eq!(closest.distance, 5.0);
    }

    #[test]
    fn serialization_roundtrip() {
        let p = Perception {
            perceived_entities: vec![PerceivedEntity {
                entity_id: 42,
                x: 10.0,
                y: 20.0,
                distance: 15.0,
                energy_estimate: 75.5,
                is_kin: true,
            }],
            perceived_resources: vec![PerceivedResource {
                resource_index: 3,
                x: 30.0,
                y: 40.0,
                distance: 25.0,
            }],
            ..Default::default()
        };

        let json = serde_json::to_string(&p).unwrap();
        let d: Perception = serde_json::from_str(&json).unwrap();
        assert_eq!(d.perceived_entities.len(), 1);
        assert_eq!(d.perceived_entities[0].entity_id, 42);
        assert_eq!(d.perceived_resources.len(), 1);
        assert_eq!(d.perceived_resources[0].resource_index, 3);
    }

    #[test]
    fn strongest_signal_of_type_returns_strongest() {
        let p = Perception {
            perceived_signals: vec![
                PerceivedSignal {
                    signal_type: 1,
                    distance: 20.0,
                    direction_x: 1.0,
                    direction_y: 0.0,
                    strength: 0.5,
                    source_x: 70.0,
                    source_y: 50.0,
                },
                PerceivedSignal {
                    signal_type: 1,
                    distance: 10.0,
                    direction_x: 0.0,
                    direction_y: 1.0,
                    strength: 0.9,
                    source_x: 50.0,
                    source_y: 60.0,
                },
                PerceivedSignal {
                    signal_type: 2,
                    distance: 5.0,
                    direction_x: -1.0,
                    direction_y: 0.0,
                    strength: 1.0,
                    source_x: 45.0,
                    source_y: 50.0,
                },
            ],
            ..Default::default()
        };

        let strongest = p.strongest_signal_of_type(1).unwrap();
        assert!((strongest.strength - 0.9).abs() < 1e-9);
        assert!(p.strongest_signal_of_type(3).is_none());
    }

    #[test]
    fn has_signal_of_type_works() {
        let p = Perception {
            perceived_signals: vec![PerceivedSignal {
                signal_type: 5,
                distance: 10.0,
                direction_x: 1.0,
                direction_y: 0.0,
                strength: 0.5,
                source_x: 60.0,
                source_y: 50.0,
            }],
            ..Default::default()
        };

        assert!(p.has_signal_of_type(5));
        assert!(!p.has_signal_of_type(0));
    }
}

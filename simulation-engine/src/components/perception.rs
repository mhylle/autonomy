use serde::{Deserialize, Serialize};

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

/// Sensory perception of the surrounding world, populated each tick.
///
/// The `sensor_range` is read from the entity's `Genome`. This component
/// stores the results for other systems (wander, drives, BT) to consume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Perception {
    pub perceived_entities: Vec<PerceivedEntity>,
    pub perceived_resources: Vec<PerceivedResource>,
}

impl Perception {
    /// Clear all perceived data (called at the start of each perception tick).
    pub fn clear(&mut self) {
        self.perceived_entities.clear();
        self.perceived_resources.clear();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_perception_is_empty() {
        let p = Perception::default();
        assert!(p.perceived_entities.is_empty());
        assert!(p.perceived_resources.is_empty());
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
        };

        p.clear();
        assert!(p.perceived_entities.is_empty());
        assert!(p.perceived_resources.is_empty());
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
            perceived_resources: vec![],
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
        };

        let json = serde_json::to_string(&p).unwrap();
        let d: Perception = serde_json::from_str(&json).unwrap();
        assert_eq!(d.perceived_entities.len(), 1);
        assert_eq!(d.perceived_entities[0].entity_id, 42);
        assert_eq!(d.perceived_resources.len(), 1);
        assert_eq!(d.perceived_resources[0].resource_index, 3);
    }
}

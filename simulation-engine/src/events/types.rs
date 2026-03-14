use serde::{Deserialize, Serialize};

/// All simulation events. Each system emits events describing state changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimEvent {
    EntitySpawned {
        entity_id: u64,
        x: f64,
        y: f64,
        generation: u32,
        parent_id: Option<u64>,
    },
    EntityDied {
        entity_id: u64,
        x: f64,
        y: f64,
        age: u64,
        cause: DeathCause,
    },
    EntityMoved {
        entity_id: u64,
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    },
    EntityAte {
        entity_id: u64,
        resource_id: u64,
        energy_gained: f64,
    },
    EntityReproduced {
        parent_id: u64,
        offspring_id: u64,
        x: f64,
        y: f64,
    },
    ResourceDepleted {
        resource_id: u64,
        x: f64,
        y: f64,
    },
    ResourceRegrown {
        resource_id: u64,
        x: f64,
        y: f64,
        new_amount: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    OldAge,
    // Combat (Phase 3.6+)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(event: &SimEvent) {
        let json = serde_json::to_string(event).expect("serialize");
        let back: SimEvent = serde_json::from_str(&json).expect("deserialize");
        // Compare debug representations for equality
        assert_eq!(format!("{:?}", event), format!("{:?}", back));
    }

    #[test]
    fn roundtrip_entity_spawned() {
        roundtrip(&SimEvent::EntitySpawned {
            entity_id: 1,
            x: 10.0,
            y: 20.0,
            generation: 0,
            parent_id: None,
        });
    }

    #[test]
    fn roundtrip_entity_spawned_with_parent() {
        roundtrip(&SimEvent::EntitySpawned {
            entity_id: 2,
            x: 15.0,
            y: 25.0,
            generation: 1,
            parent_id: Some(1),
        });
    }

    #[test]
    fn roundtrip_entity_died_starvation() {
        roundtrip(&SimEvent::EntityDied {
            entity_id: 1,
            x: 10.0,
            y: 20.0,
            age: 100,
            cause: DeathCause::Starvation,
        });
    }

    #[test]
    fn roundtrip_entity_died_old_age() {
        roundtrip(&SimEvent::EntityDied {
            entity_id: 1,
            x: 10.0,
            y: 20.0,
            age: 500,
            cause: DeathCause::OldAge,
        });
    }

    #[test]
    fn roundtrip_entity_moved() {
        roundtrip(&SimEvent::EntityMoved {
            entity_id: 1,
            from_x: 0.0,
            from_y: 0.0,
            to_x: 5.0,
            to_y: 5.0,
        });
    }

    #[test]
    fn roundtrip_entity_ate() {
        roundtrip(&SimEvent::EntityAte {
            entity_id: 1,
            resource_id: 42,
            energy_gained: 15.5,
        });
    }

    #[test]
    fn roundtrip_entity_reproduced() {
        roundtrip(&SimEvent::EntityReproduced {
            parent_id: 1,
            offspring_id: 3,
            x: 10.0,
            y: 20.0,
        });
    }

    #[test]
    fn roundtrip_resource_depleted() {
        roundtrip(&SimEvent::ResourceDepleted {
            resource_id: 42,
            x: 30.0,
            y: 40.0,
        });
    }

    #[test]
    fn roundtrip_resource_regrown() {
        roundtrip(&SimEvent::ResourceRegrown {
            resource_id: 42,
            x: 30.0,
            y: 40.0,
            new_amount: 100.0,
        });
    }
}

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
    EntityAttacked {
        attacker_id: u64,
        target_id: u64,
        damage: f64,
        target_health_remaining: f64,
    },
    CompositeReproduced {
        parent_id: u64,
        offspring_id: u64,
        member_count: usize,
        x: f64,
        y: f64,
    },
    /// Two entities merged to form a composite organism.
    CompositeFormed {
        leader_id: u64,
        member_id: u64,
        x: f64,
        y: f64,
    },
    /// A composite organism decomposed (fully or partially).
    CompositeDecomposed {
        leader_id: u64,
        released_member_ids: Vec<u64>,
        x: f64,
        y: f64,
    },
    /// Two tribes have accumulated enough cross-tribe kills to be considered at war.
    WarDeclared {
        tribe_a_id: u64,
        tribe_b_id: u64,
        tick: u64,
    },
    /// A war between two tribes has ended due to prolonged peace.
    WarEnded {
        tribe_a_id: u64,
        tribe_b_id: u64,
        started_tick: u64,
        ended_tick: u64,
        duration: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    OldAge,
    Combat { killer_id: u64 },
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

    #[test]
    fn roundtrip_entity_attacked() {
        roundtrip(&SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 25.0,
            target_health_remaining: 75.0,
        });
    }

    #[test]
    fn roundtrip_entity_died_combat() {
        roundtrip(&SimEvent::EntityDied {
            entity_id: 2,
            x: 10.0,
            y: 20.0,
            age: 50,
            cause: DeathCause::Combat { killer_id: 1 },
        });
    }

    #[test]
    fn roundtrip_composite_formed() {
        roundtrip(&SimEvent::CompositeFormed {
            leader_id: 1,
            member_id: 2,
            x: 50.0,
            y: 60.0,
        });
    }

    #[test]
    fn roundtrip_composite_decomposed() {
        roundtrip(&SimEvent::CompositeDecomposed {
            leader_id: 1,
            released_member_ids: vec![2, 3],
            x: 50.0,
            y: 60.0,
        });
    }

    #[test]
    fn roundtrip_war_declared() {
        roundtrip(&SimEvent::WarDeclared {
            tribe_a_id: 1,
            tribe_b_id: 2,
            tick: 500,
        });
    }

    #[test]
    fn roundtrip_war_ended() {
        roundtrip(&SimEvent::WarEnded {
            tribe_a_id: 1,
            tribe_b_id: 2,
            started_tick: 500,
            ended_tick: 750,
            duration: 250,
        });
    }
}

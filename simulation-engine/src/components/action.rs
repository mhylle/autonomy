use serde::{Deserialize, Serialize};

/// Action produced by the decision system (behavior tree evaluation).
///
/// Each tick, the decision system sets this on each entity. Downstream
/// systems (movement, feeding) consume it to execute the entity's intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Move toward a specific world-space position.
    MoveTo { x: f64, y: f64, speed: f64 },
    /// Move in a specific direction.
    MoveDirection { dx: f64, dy: f64, speed: f64 },
    /// Attempt to eat the nearest adjacent resource.
    Eat,
    /// Rest (zero velocity).
    Rest,
    /// Wander randomly.
    Wander { speed: f64 },
    /// Attack a nearby entity.
    Attack { target_id: u64, force: f64 },
    /// Flee from a specific world-space position (move away from it).
    FleeFrom { x: f64, y: f64, speed: f64 },
    /// Attempt to merge with a nearby compatible entity.
    CompositionAttempt,
    /// No action this tick.
    None,
}

impl Default for Action {
    fn default() -> Self {
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        assert_eq!(Action::default(), Action::None);
    }

    #[test]
    fn serialization_roundtrip() {
        let actions = vec![
            Action::MoveTo { x: 10.0, y: 20.0, speed: 2.0 },
            Action::MoveDirection { dx: 1.0, dy: 0.0, speed: 1.5 },
            Action::Eat,
            Action::Rest,
            Action::Wander { speed: 1.0 },
            Action::Attack { target_id: 42, force: 0.8 },
            Action::FleeFrom { x: 10.0, y: 20.0, speed: 3.0 },
            Action::CompositionAttempt,
            Action::None,
        ];
        for action in &actions {
            let json = serde_json::to_string(action).unwrap();
            let d: Action = serde_json::from_str(&json).unwrap();
            assert_eq!(&d, action);
        }
    }
}

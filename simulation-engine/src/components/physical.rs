use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Energy {
    pub current: f64,
    pub max: f64,
    pub metabolism_rate: f64,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
            metabolism_rate: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f64,
    pub max: f64,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Age {
    pub ticks: u64,
    pub max_lifespan: u64,
}

impl Default for Age {
    fn default() -> Self {
        Self {
            ticks: 0,
            max_lifespan: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub radius: f64,
}

impl Default for Size {
    fn default() -> Self {
        Self { radius: 5.0 }
    }
}

/// Returns `true` when an entity should be considered dead.
///
/// An entity is dead if its energy is depleted or it has exceeded its lifespan.
pub fn is_dead(energy: &Energy, age: &Age) -> bool {
    energy.current <= 0.0 || age.ticks >= age.max_lifespan
}

/// Returns `true` when an entity should be considered dead, also checking health.
///
/// An entity is dead if health <= 0, energy depleted, or exceeded lifespan.
pub fn is_dead_with_health(energy: &Energy, age: &Age, health: &Health) -> bool {
    health.current <= 0.0 || energy.current <= 0.0 || age.ticks >= age.max_lifespan
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Default value tests ---

    #[test]
    fn energy_default() {
        let e = Energy::default();
        assert_eq!(e.current, 100.0);
        assert_eq!(e.max, 100.0);
        assert_eq!(e.metabolism_rate, 0.1);
    }

    #[test]
    fn health_default() {
        let h = Health::default();
        assert_eq!(h.current, 100.0);
        assert_eq!(h.max, 100.0);
    }

    #[test]
    fn age_default() {
        let a = Age::default();
        assert_eq!(a.ticks, 0);
        assert_eq!(a.max_lifespan, 5000);
    }

    #[test]
    fn size_default() {
        let s = Size::default();
        assert_eq!(s.radius, 5.0);
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn energy_serialization_roundtrip() {
        let e = Energy {
            current: 42.0,
            max: 200.0,
            metabolism_rate: 0.5,
        };
        let json = serde_json::to_string(&e).unwrap();
        let d: Energy = serde_json::from_str(&json).unwrap();
        assert_eq!(d.current, e.current);
        assert_eq!(d.max, e.max);
        assert_eq!(d.metabolism_rate, e.metabolism_rate);
    }

    #[test]
    fn health_serialization_roundtrip() {
        let h = Health {
            current: 55.0,
            max: 80.0,
        };
        let json = serde_json::to_string(&h).unwrap();
        let d: Health = serde_json::from_str(&json).unwrap();
        assert_eq!(d.current, h.current);
        assert_eq!(d.max, h.max);
    }

    #[test]
    fn age_serialization_roundtrip() {
        let a = Age {
            ticks: 1234,
            max_lifespan: 9999,
        };
        let json = serde_json::to_string(&a).unwrap();
        let d: Age = serde_json::from_str(&json).unwrap();
        assert_eq!(d.ticks, a.ticks);
        assert_eq!(d.max_lifespan, a.max_lifespan);
    }

    #[test]
    fn size_serialization_roundtrip() {
        let s = Size { radius: 12.5 };
        let json = serde_json::to_string(&s).unwrap();
        let d: Size = serde_json::from_str(&json).unwrap();
        assert_eq!(d.radius, s.radius);
    }

    // --- is_dead tests ---

    #[test]
    fn alive_entity_is_not_dead() {
        let energy = Energy::default();
        let age = Age::default();
        assert!(!is_dead(&energy, &age));
    }

    #[test]
    fn zero_energy_is_dead() {
        let energy = Energy {
            current: 0.0,
            ..Energy::default()
        };
        let age = Age::default();
        assert!(is_dead(&energy, &age));
    }

    #[test]
    fn negative_energy_is_dead() {
        let energy = Energy {
            current: -5.0,
            ..Energy::default()
        };
        let age = Age::default();
        assert!(is_dead(&energy, &age));
    }

    #[test]
    fn exceeded_lifespan_is_dead() {
        let energy = Energy::default();
        let age = Age {
            ticks: 5000,
            max_lifespan: 5000,
        };
        assert!(is_dead(&energy, &age));
    }

    #[test]
    fn within_lifespan_is_not_dead() {
        let energy = Energy::default();
        let age = Age {
            ticks: 4999,
            max_lifespan: 5000,
        };
        assert!(!is_dead(&energy, &age));
    }
}

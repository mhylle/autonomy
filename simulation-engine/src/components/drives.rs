use serde::{Deserialize, Serialize};

/// Internal motivational states computed from entity state each tick.
///
/// Values range from 0.0 (no drive) to 1.0 (maximum urgency).
/// The behavior tree (Phase 2.3+) will check these drives to decide actions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Drives {
    /// How hungry the entity is: 1.0 - (energy / max_energy).
    pub hunger: f64,
    /// Fear level from perceived threats (0.0 until Phase 3.3).
    pub fear: f64,
    /// Desire to explore new areas.
    pub curiosity: f64,
    /// Need for social interaction (0.0 until social system).
    pub social_need: f64,
    /// Propensity for aggression (0.0 until combat system).
    pub aggression: f64,
    /// Urge to reproduce based on energy surplus and age.
    pub reproductive_urge: f64,
}

/// Genome-encoded base sensitivities for each drive.
///
/// These weights scale the computed drive values, allowing evolution
/// to calibrate how strongly entities respond to internal states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveWeights {
    pub base_curiosity: f64,
    pub base_social_need: f64,
    pub base_aggression: f64,
    pub base_reproductive: f64,
}

impl Default for DriveWeights {
    fn default() -> Self {
        Self {
            base_curiosity: 0.3,
            base_social_need: 0.2,
            base_aggression: 0.1,
            base_reproductive: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_drives_are_zero() {
        let d = Drives::default();
        assert_eq!(d.hunger, 0.0);
        assert_eq!(d.fear, 0.0);
        assert_eq!(d.curiosity, 0.0);
        assert_eq!(d.social_need, 0.0);
        assert_eq!(d.aggression, 0.0);
        assert_eq!(d.reproductive_urge, 0.0);
    }

    #[test]
    fn default_drive_weights() {
        let w = DriveWeights::default();
        assert_eq!(w.base_curiosity, 0.3);
        assert_eq!(w.base_social_need, 0.2);
        assert_eq!(w.base_aggression, 0.1);
        assert_eq!(w.base_reproductive, 0.5);
    }

    #[test]
    fn serialization_roundtrip_drives() {
        let d = Drives {
            hunger: 0.7,
            fear: 0.1,
            curiosity: 0.5,
            social_need: 0.3,
            aggression: 0.2,
            reproductive_urge: 0.8,
        };
        let json = serde_json::to_string(&d).unwrap();
        let r: Drives = serde_json::from_str(&json).unwrap();
        assert_eq!(r.hunger, 0.7);
        assert_eq!(r.reproductive_urge, 0.8);
    }

    #[test]
    fn serialization_roundtrip_weights() {
        let w = DriveWeights {
            base_curiosity: 0.4,
            base_social_need: 0.6,
            base_aggression: 0.9,
            base_reproductive: 0.1,
        };
        let json = serde_json::to_string(&w).unwrap();
        let r: DriveWeights = serde_json::from_str(&json).unwrap();
        assert_eq!(r.base_curiosity, 0.4);
        assert_eq!(r.base_aggression, 0.9);
    }
}

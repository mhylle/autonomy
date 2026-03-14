use serde::{Deserialize, Serialize};

/// Tracks lineage and birth information for an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub generation: u32,
    /// `entity.to_bits().get()` of the parent, `None` for generation-0
    pub parent_id: Option<u64>,
    pub birth_tick: u64,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            generation: 0,
            parent_id: None,
            birth_tick: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let id = Identity::default();
        assert_eq!(id.generation, 0);
        assert!(id.parent_id.is_none());
        assert_eq!(id.birth_tick, 0);
    }

    #[test]
    fn serialization_roundtrip() {
        let id = Identity {
            generation: 3,
            parent_id: Some(42),
            birth_tick: 1500,
        };
        let json = serde_json::to_string(&id).unwrap();
        let d: Identity = serde_json::from_str(&json).unwrap();
        assert_eq!(d.generation, id.generation);
        assert_eq!(d.parent_id, id.parent_id);
        assert_eq!(d.birth_tick, id.birth_tick);
    }

    #[test]
    fn serialization_roundtrip_no_parent() {
        let id = Identity::default();
        let json = serde_json::to_string(&id).unwrap();
        let d: Identity = serde_json::from_str(&json).unwrap();
        assert!(d.parent_id.is_none());
        assert_eq!(d.generation, 0);
        assert_eq!(d.birth_tick, 0);
    }
}

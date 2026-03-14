use serde::{Deserialize, Serialize};

/// The kind of resource available in the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Food,
}

/// A single resource node in the environment.
///
/// Resources are **not** ECS entities. They live in a flat `Vec` on the
/// `SimulationWorld` so they stay lightweight and easy to query spatially.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub resource_type: ResourceType,
    pub amount: f64,
    pub max_amount: f64,
    pub regrowth_rate: f64,
    pub depleted: bool,
}

impl Default for Resource {
    fn default() -> Self {
        Self {
            id: 0,
            x: 0.0,
            y: 0.0,
            resource_type: ResourceType::Food,
            amount: 50.0,
            max_amount: 50.0,
            regrowth_rate: 0.5,
            depleted: false,
        }
    }
}

impl Resource {
    /// Returns `true` when this resource can still be consumed.
    pub fn is_available(&self) -> bool {
        self.amount > 0.0 && !self.depleted
    }

    /// Consume up to `requested` units from this resource.
    ///
    /// Returns the amount actually consumed (which may be less than
    /// requested if the resource is running low).
    pub fn consume(&mut self, requested: f64) -> f64 {
        if self.depleted || self.amount <= 0.0 {
            return 0.0;
        }

        let actual = requested.min(self.amount);
        self.amount -= actual;

        if self.amount <= 0.0 {
            self.amount = 0.0;
            self.depleted = true;
        }

        actual
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_resource_is_food() {
        let r = Resource::default();
        assert_eq!(r.resource_type, ResourceType::Food);
    }

    #[test]
    fn default_resource_has_correct_amounts() {
        let r = Resource::default();
        assert_eq!(r.amount, 50.0);
        assert_eq!(r.max_amount, 50.0);
        assert_eq!(r.regrowth_rate, 0.5);
    }

    #[test]
    fn default_resource_is_available() {
        let r = Resource::default();
        assert!(r.is_available());
    }

    #[test]
    fn depleted_resource_is_not_available() {
        let r = Resource {
            depleted: true,
            ..Default::default()
        };
        assert!(!r.is_available());
    }

    #[test]
    fn empty_resource_is_not_available() {
        let r = Resource {
            amount: 0.0,
            ..Default::default()
        };
        assert!(!r.is_available());
    }

    #[test]
    fn consume_returns_requested_when_enough() {
        let mut r = Resource::default();
        let consumed = r.consume(10.0);
        assert_eq!(consumed, 10.0);
        assert_eq!(r.amount, 40.0);
        assert!(!r.depleted);
    }

    #[test]
    fn consume_returns_remaining_when_not_enough() {
        let mut r = Resource {
            amount: 5.0,
            ..Default::default()
        };
        let consumed = r.consume(10.0);
        assert_eq!(consumed, 5.0);
        assert_eq!(r.amount, 0.0);
        assert!(r.depleted);
    }

    #[test]
    fn consume_from_depleted_returns_zero() {
        let mut r = Resource {
            amount: 0.0,
            depleted: true,
            ..Default::default()
        };
        let consumed = r.consume(10.0);
        assert_eq!(consumed, 0.0);
    }

    #[test]
    fn consume_depletes_when_fully_consumed() {
        let mut r = Resource {
            amount: 10.0,
            ..Default::default()
        };
        let consumed = r.consume(10.0);
        assert_eq!(consumed, 10.0);
        assert!(r.depleted);
        assert!(!r.is_available());
    }

    #[test]
    fn consume_zero_returns_zero() {
        let mut r = Resource::default();
        let consumed = r.consume(0.0);
        assert_eq!(consumed, 0.0);
        assert_eq!(r.amount, 50.0);
    }
}

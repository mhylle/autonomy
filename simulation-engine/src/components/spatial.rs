use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Velocity {
    pub dx: f64,
    pub dy: f64,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { dx: 0.0, dy: 0.0 }
    }
}

/// Euclidean distance between two positions.
pub fn distance(a: &Position, b: &Position) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_default() {
        let p = Position::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
    }

    #[test]
    fn velocity_default() {
        let v = Velocity::default();
        assert_eq!(v.dx, 0.0);
        assert_eq!(v.dy, 0.0);
    }

    #[test]
    fn position_serialization_roundtrip() {
        let p = Position { x: 3.5, y: -7.2 };
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.x, p.x);
        assert_eq!(deserialized.y, p.y);
    }

    #[test]
    fn velocity_serialization_roundtrip() {
        let v = Velocity { dx: 1.0, dy: -2.5 };
        let json = serde_json::to_string(&v).unwrap();
        let deserialized: Velocity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dx, v.dx);
        assert_eq!(deserialized.dy, v.dy);
    }

    #[test]
    fn distance_same_point() {
        let a = Position::default();
        let b = Position::default();
        assert_eq!(distance(&a, &b), 0.0);
    }

    #[test]
    fn distance_known_triangle() {
        let a = Position { x: 0.0, y: 0.0 };
        let b = Position { x: 3.0, y: 4.0 };
        assert!((distance(&a, &b) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distance_is_symmetric() {
        let a = Position { x: 1.0, y: 2.0 };
        let b = Position { x: 4.0, y: 6.0 };
        assert_eq!(distance(&a, &b), distance(&b, &a));
    }
}

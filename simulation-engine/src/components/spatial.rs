use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    /// Vertical coordinate (altitude). Defaults to 0.0 for backward
    /// compatibility with 2D serialized data.
    #[serde(default)]
    pub z: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Velocity {
    pub dx: f64,
    pub dy: f64,
    /// Vertical velocity component. Defaults to 0.0 for backward
    /// compatibility with 2D serialized data.
    #[serde(default)]
    pub dz: f64,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
        }
    }
}

/// 2D Euclidean distance between two positions (ignoring z).
///
/// Use [`distance_3d`] when vertical separation matters.
pub fn distance(a: &Position, b: &Position) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// 3D Euclidean distance between two positions (including z).
pub fn distance_3d(a: &Position, b: &Position) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_default() {
        let p = Position::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
        assert_eq!(p.z, 0.0);
    }

    #[test]
    fn velocity_default() {
        let v = Velocity::default();
        assert_eq!(v.dx, 0.0);
        assert_eq!(v.dy, 0.0);
        assert_eq!(v.dz, 0.0);
    }

    #[test]
    fn position_serialization_roundtrip() {
        let p = Position {
            x: 3.5,
            y: -7.2,
            z: 12.0,
        };
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.x, p.x);
        assert_eq!(deserialized.y, p.y);
        assert_eq!(deserialized.z, p.z);
    }

    #[test]
    fn position_deserialize_without_z_defaults_to_zero() {
        let json = r#"{"x":1.0,"y":2.0}"#;
        let p: Position = serde_json::from_str(json).unwrap();
        assert_eq!(p.x, 1.0);
        assert_eq!(p.y, 2.0);
        assert_eq!(p.z, 0.0);
    }

    #[test]
    fn velocity_serialization_roundtrip() {
        let v = Velocity {
            dx: 1.0,
            dy: -2.5,
            dz: 0.5,
        };
        let json = serde_json::to_string(&v).unwrap();
        let deserialized: Velocity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dx, v.dx);
        assert_eq!(deserialized.dy, v.dy);
        assert_eq!(deserialized.dz, v.dz);
    }

    #[test]
    fn velocity_deserialize_without_dz_defaults_to_zero() {
        let json = r#"{"dx":1.0,"dy":2.0}"#;
        let v: Velocity = serde_json::from_str(json).unwrap();
        assert_eq!(v.dx, 1.0);
        assert_eq!(v.dy, 2.0);
        assert_eq!(v.dz, 0.0);
    }

    #[test]
    fn distance_same_point() {
        let a = Position::default();
        let b = Position::default();
        assert_eq!(distance(&a, &b), 0.0);
    }

    #[test]
    fn distance_known_triangle() {
        let a = Position { x: 0.0, y: 0.0, z: 0.0 };
        let b = Position { x: 3.0, y: 4.0, z: 0.0 };
        assert!((distance(&a, &b) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distance_is_symmetric() {
        let a = Position { x: 1.0, y: 2.0, z: 0.0 };
        let b = Position { x: 4.0, y: 6.0, z: 0.0 };
        assert_eq!(distance(&a, &b), distance(&b, &a));
    }

    #[test]
    fn distance_2d_ignores_z() {
        let a = Position { x: 0.0, y: 0.0, z: 0.0 };
        let b = Position { x: 3.0, y: 4.0, z: 100.0 };
        assert!((distance(&a, &b) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distance_3d_includes_z() {
        let a = Position { x: 0.0, y: 0.0, z: 0.0 };
        let b = Position { x: 2.0, y: 3.0, z: 6.0 };
        // sqrt(4 + 9 + 36) = sqrt(49) = 7.0
        assert!((distance_3d(&a, &b) - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distance_3d_same_point() {
        let a = Position { x: 5.0, y: 10.0, z: 15.0 };
        let b = Position { x: 5.0, y: 10.0, z: 15.0 };
        assert_eq!(distance_3d(&a, &b), 0.0);
    }

    #[test]
    fn distance_3d_is_symmetric() {
        let a = Position { x: 1.0, y: 2.0, z: 3.0 };
        let b = Position { x: 4.0, y: 6.0, z: 8.0 };
        assert_eq!(distance_3d(&a, &b), distance_3d(&b, &a));
    }

    #[test]
    fn distance_3d_equals_2d_when_z_same() {
        let a = Position { x: 1.0, y: 2.0, z: 5.0 };
        let b = Position { x: 4.0, y: 6.0, z: 5.0 };
        assert!((distance(&a, &b) - distance_3d(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn distance_3d_vertical_only() {
        let a = Position { x: 0.0, y: 0.0, z: 0.0 };
        let b = Position { x: 0.0, y: 0.0, z: 10.0 };
        assert!((distance_3d(&a, &b) - 10.0).abs() < f64::EPSILON);
    }
}

//! Signal data structures and management.
//!
//! Signals are emitted by entities and decay over time. They carry a type,
//! position, radius, strength, and decay rate. Other entities can perceive
//! signals within their sensor range to make behavioral decisions.

use serde::{Deserialize, Serialize};

/// A signal emitted by an entity into the environment.
///
/// Signals spread from the emitter's position with a given radius and
/// strength, decaying each tick by `decay_rate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// The entity ID (raw bits) of the emitter.
    pub emitter_id: u64,
    /// Type of signal (semantics are emergent, not hard-coded).
    pub signal_type: u8,
    /// World-space x position where the signal was emitted.
    pub x: f64,
    /// World-space y position where the signal was emitted.
    pub y: f64,
    /// Radius within which the signal can be perceived.
    pub radius: f64,
    /// Current strength of the signal (0.0 to 1.0+).
    pub strength: f64,
    /// How much strength is lost each tick (absolute, not proportional).
    pub decay_rate: f64,
}

impl Signal {
    /// Create a new signal.
    pub fn new(
        emitter_id: u64,
        signal_type: u8,
        x: f64,
        y: f64,
        radius: f64,
        strength: f64,
        decay_rate: f64,
    ) -> Self {
        Self {
            emitter_id,
            signal_type,
            x,
            y,
            radius,
            strength,
            decay_rate,
        }
    }

    /// Returns true if the signal has decayed to zero or below.
    pub fn is_expired(&self) -> bool {
        self.strength <= 0.0
    }

    /// Apply one tick of decay to the signal.
    pub fn decay(&mut self) {
        self.strength -= self.decay_rate;
        if self.strength < 0.0 {
            self.strength = 0.0;
        }
    }

    /// Compute the effective strength at a given distance from the signal origin.
    ///
    /// Strength falls off linearly with distance. Returns 0.0 if outside radius.
    pub fn strength_at_distance(&self, distance: f64) -> f64 {
        if distance >= self.radius || self.strength <= 0.0 {
            return 0.0;
        }
        self.strength * (1.0 - distance / self.radius)
    }
}

/// Default signal parameters used when emitting via BT actions.
pub const DEFAULT_SIGNAL_RADIUS: f64 = 80.0;
/// Default signal strength on emission.
pub const DEFAULT_SIGNAL_STRENGTH: f64 = 1.0;
/// Default signal decay rate per tick.
pub const DEFAULT_SIGNAL_DECAY_RATE: f64 = 0.02;

/// Manages the collection of active signals in the simulation.
///
/// Provides methods to add, decay, query, and clean up signals.
pub struct SignalManager;

impl SignalManager {
    /// Decay all signals by one tick and remove expired ones.
    pub fn tick(signals: &mut Vec<Signal>) {
        for signal in signals.iter_mut() {
            signal.decay();
        }
        signals.retain(|s| !s.is_expired());
    }

    /// Add a new signal to the collection.
    pub fn emit(signals: &mut Vec<Signal>, signal: Signal) {
        signals.push(signal);
    }

    /// Query all signals within a given radius of a point.
    ///
    /// Returns references to signals whose origin is within `query_radius`
    /// of the point (qx, qy) and whose own radius reaches the query point.
    pub fn query_at(signals: &[Signal], qx: f64, qy: f64, query_radius: f64) -> Vec<&Signal> {
        signals
            .iter()
            .filter(|s| {
                let dx = s.x - qx;
                let dy = s.y - qy;
                let dist = (dx * dx + dy * dy).sqrt();
                dist <= query_radius && dist <= s.radius && s.strength > 0.0
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_new_creates_correctly() {
        let s = Signal::new(42, 1, 10.0, 20.0, 50.0, 1.0, 0.05);
        assert_eq!(s.emitter_id, 42);
        assert_eq!(s.signal_type, 1);
        assert_eq!(s.x, 10.0);
        assert_eq!(s.y, 20.0);
        assert_eq!(s.radius, 50.0);
        assert_eq!(s.strength, 1.0);
        assert_eq!(s.decay_rate, 0.05);
    }

    #[test]
    fn signal_is_not_expired_when_strong() {
        let s = Signal::new(1, 0, 0.0, 0.0, 50.0, 1.0, 0.1);
        assert!(!s.is_expired());
    }

    #[test]
    fn signal_is_expired_at_zero_strength() {
        let s = Signal::new(1, 0, 0.0, 0.0, 50.0, 0.0, 0.1);
        assert!(s.is_expired());
    }

    #[test]
    fn signal_decay_reduces_strength() {
        let mut s = Signal::new(1, 0, 0.0, 0.0, 50.0, 1.0, 0.1);
        s.decay();
        assert!((s.strength - 0.9).abs() < 1e-9);
    }

    #[test]
    fn signal_decay_clamps_to_zero() {
        let mut s = Signal::new(1, 0, 0.0, 0.0, 50.0, 0.05, 0.1);
        s.decay();
        assert_eq!(s.strength, 0.0);
    }

    #[test]
    fn signal_strength_at_distance_zero() {
        let s = Signal::new(1, 0, 0.0, 0.0, 100.0, 1.0, 0.01);
        assert!((s.strength_at_distance(0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn signal_strength_at_distance_half_radius() {
        let s = Signal::new(1, 0, 0.0, 0.0, 100.0, 1.0, 0.01);
        assert!((s.strength_at_distance(50.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn signal_strength_at_distance_beyond_radius() {
        let s = Signal::new(1, 0, 0.0, 0.0, 100.0, 1.0, 0.01);
        assert_eq!(s.strength_at_distance(100.0), 0.0);
        assert_eq!(s.strength_at_distance(150.0), 0.0);
    }

    #[test]
    fn signal_manager_tick_decays_and_removes() {
        let mut signals = vec![
            Signal::new(1, 0, 0.0, 0.0, 50.0, 0.05, 0.1), // will expire
            Signal::new(2, 0, 0.0, 0.0, 50.0, 1.0, 0.1),  // will survive
        ];
        SignalManager::tick(&mut signals);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].emitter_id, 2);
    }

    #[test]
    fn signal_manager_emit_adds_signal() {
        let mut signals = Vec::new();
        SignalManager::emit(&mut signals, Signal::new(1, 0, 10.0, 20.0, 50.0, 1.0, 0.1));
        assert_eq!(signals.len(), 1);
    }

    #[test]
    fn signal_manager_query_at_finds_nearby() {
        let signals = vec![
            Signal::new(1, 0, 10.0, 10.0, 50.0, 1.0, 0.01),
            Signal::new(2, 0, 200.0, 200.0, 50.0, 1.0, 0.01),
        ];
        let result = SignalManager::query_at(&signals, 15.0, 15.0, 60.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].emitter_id, 1);
    }

    #[test]
    fn signal_manager_query_at_excludes_out_of_range() {
        let signals = vec![Signal::new(1, 0, 100.0, 100.0, 50.0, 1.0, 0.01)];
        let result = SignalManager::query_at(&signals, 0.0, 0.0, 30.0);
        assert!(result.is_empty());
    }

    #[test]
    fn signal_serialization_roundtrip() {
        let s = Signal::new(42, 3, 10.0, 20.0, 80.0, 0.75, 0.02);
        let json = serde_json::to_string(&s).unwrap();
        let d: Signal = serde_json::from_str(&json).unwrap();
        assert_eq!(s, d);
    }

    #[test]
    fn signal_full_decay_cycle() {
        // Use decay_rate = 0.25 with strength 1.0 -> exactly 4 ticks to expire.
        let mut s = Signal::new(1, 0, 0.0, 0.0, 50.0, 1.0, 0.25);
        let mut ticks = 0;
        while !s.is_expired() {
            s.decay();
            ticks += 1;
        }
        assert_eq!(ticks, 4);
        assert_eq!(s.strength, 0.0);
    }

    #[test]
    fn signal_manager_tick_empty_vec() {
        let mut signals: Vec<Signal> = Vec::new();
        SignalManager::tick(&mut signals);
        assert!(signals.is_empty());
    }
}

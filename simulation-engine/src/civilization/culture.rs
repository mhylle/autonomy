//! Cultural identity metrics.
//!
//! Computes cultural identity for tribes by analyzing common behavioral
//! patterns (BT structure) and signal usage. Cultural distance between
//! tribes can be measured to understand divergence and convergence.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Cultural fingerprint for a tribe.
///
/// Encapsulates the behavioral and communicative patterns that
/// distinguish one tribe from another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CulturalIdentity {
    /// Tribe ID this identity belongs to.
    pub tribe_id: u64,
    /// Hash of common BT node patterns across tribe members.
    pub bt_pattern_hash: u64,
    /// Signal type usage frequencies: signal_type -> count.
    pub signal_usage: HashMap<u8, u64>,
    /// Normalized signal usage vector for distance computation.
    /// Computed from signal_usage, keyed by signal type.
    pub signal_profile: Vec<f64>,
    /// Overall "cultural complexity" score (0.0+).
    pub complexity: f64,
}

impl CulturalIdentity {
    /// Create a new cultural identity for a tribe.
    pub fn new(tribe_id: u64) -> Self {
        Self {
            tribe_id,
            bt_pattern_hash: 0,
            signal_usage: HashMap::new(),
            signal_profile: Vec::new(),
            complexity: 0.0,
        }
    }

    /// Compute the cultural complexity score.
    ///
    /// Based on diversity of signal usage and BT pattern variety.
    pub fn compute_complexity(&mut self) {
        // Shannon entropy of signal usage.
        let total: u64 = self.signal_usage.values().sum();
        if total == 0 {
            self.complexity = 0.0;
            return;
        }
        let total_f = total as f64;
        let entropy: f64 = self.signal_usage.values().map(|&count| {
            if count == 0 {
                return 0.0;
            }
            let p = count as f64 / total_f;
            -p * p.ln()
        }).sum();
        self.complexity = entropy;
    }
}

/// Compute a hash of BT node type patterns for a set of entities.
///
/// Takes a list of BT node type counts per entity and hashes
/// the aggregate pattern. This captures "what kinds of behaviors
/// are common in this tribe" without needing the full BT trees.
pub fn compute_bt_pattern_hash(node_type_counts: &[HashMap<String, u64>]) -> u64 {
    if node_type_counts.is_empty() {
        return 0;
    }

    // Aggregate counts across all members.
    let mut aggregate: HashMap<String, u64> = HashMap::new();
    for counts in node_type_counts {
        for (node_type, count) in counts {
            *aggregate.entry(node_type.clone()).or_insert(0) += count;
        }
    }

    // Sort by key for deterministic hashing.
    let mut sorted: Vec<(&String, &u64)> = aggregate.iter().collect();
    sorted.sort_by_key(|(k, _)| *k);

    // FNV-1a hash.
    let mut hash: u64 = 0xcbf29ce484222325;
    for (key, count) in sorted {
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= *count;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

/// Build a normalized signal usage profile.
///
/// Returns a vector of length `max_signal_types` with normalized
/// frequencies (0.0 to 1.0).
pub fn build_signal_profile(signal_usage: &HashMap<u8, u64>, max_signal_types: usize) -> Vec<f64> {
    let total: u64 = signal_usage.values().sum();
    if total == 0 {
        return vec![0.0; max_signal_types];
    }
    let total_f = total as f64;

    (0..max_signal_types)
        .map(|i| {
            let count = signal_usage.get(&(i as u8)).copied().unwrap_or(0);
            count as f64 / total_f
        })
        .collect()
}

/// Compute cultural distance between two tribes.
///
/// Uses a combination of:
/// - Signal profile distance (Euclidean distance of normalized profiles)
/// - BT pattern hash comparison (0 if same, 1 if different)
///
/// Returns a value in [0.0, 1.0] where 0 = identical cultures, 1 = maximally different.
pub fn cultural_distance(a: &CulturalIdentity, b: &CulturalIdentity) -> f64 {
    // Signal profile distance (Euclidean, normalized).
    let max_len = a.signal_profile.len().max(b.signal_profile.len());
    if max_len == 0 {
        // No signal data -- use BT hash only.
        return if a.bt_pattern_hash == b.bt_pattern_hash {
            0.0
        } else {
            1.0
        };
    }

    let mut sum_sq = 0.0;
    for i in 0..max_len {
        let va = a.signal_profile.get(i).copied().unwrap_or(0.0);
        let vb = b.signal_profile.get(i).copied().unwrap_or(0.0);
        let diff = va - vb;
        sum_sq += diff * diff;
    }
    let signal_dist = sum_sq.sqrt().min(1.0);

    // BT hash component (binary: same or different).
    let bt_dist = if a.bt_pattern_hash == b.bt_pattern_hash {
        0.0
    } else {
        1.0
    };

    // Weighted combination: 70% signal, 30% BT.
    (0.7 * signal_dist + 0.3 * bt_dist).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cultural_identity_new() {
        let ci = CulturalIdentity::new(42);
        assert_eq!(ci.tribe_id, 42);
        assert_eq!(ci.bt_pattern_hash, 0);
        assert!(ci.signal_usage.is_empty());
        assert_eq!(ci.complexity, 0.0);
    }

    #[test]
    fn compute_complexity_empty() {
        let mut ci = CulturalIdentity::new(1);
        ci.compute_complexity();
        assert_eq!(ci.complexity, 0.0);
    }

    #[test]
    fn compute_complexity_single_signal() {
        let mut ci = CulturalIdentity::new(1);
        ci.signal_usage.insert(0, 100);
        ci.compute_complexity();
        // Single signal type -> entropy = 0 (no diversity)
        assert_eq!(ci.complexity, 0.0);
    }

    #[test]
    fn compute_complexity_diverse_signals() {
        let mut ci = CulturalIdentity::new(1);
        ci.signal_usage.insert(0, 50);
        ci.signal_usage.insert(1, 50);
        ci.signal_usage.insert(2, 50);
        ci.compute_complexity();
        // Multiple signal types -> positive entropy
        assert!(ci.complexity > 0.0);
    }

    #[test]
    fn bt_pattern_hash_deterministic() {
        let counts = vec![{
            let mut m = HashMap::new();
            m.insert("Eat".to_string(), 10u64);
            m.insert("Wander".to_string(), 5);
            m
        }];
        let hash1 = compute_bt_pattern_hash(&counts);
        let hash2 = compute_bt_pattern_hash(&counts);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn bt_pattern_hash_different_inputs() {
        let counts1 = vec![{
            let mut m = HashMap::new();
            m.insert("Eat".to_string(), 10u64);
            m
        }];
        let counts2 = vec![{
            let mut m = HashMap::new();
            m.insert("Attack".to_string(), 10u64);
            m
        }];
        let hash1 = compute_bt_pattern_hash(&counts1);
        let hash2 = compute_bt_pattern_hash(&counts2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn bt_pattern_hash_empty() {
        assert_eq!(compute_bt_pattern_hash(&[]), 0);
    }

    #[test]
    fn build_signal_profile_empty() {
        let profile = build_signal_profile(&HashMap::new(), 4);
        assert_eq!(profile, vec![0.0; 4]);
    }

    #[test]
    fn build_signal_profile_normalized() {
        let mut usage = HashMap::new();
        usage.insert(0u8, 30);
        usage.insert(1u8, 70);

        let profile = build_signal_profile(&usage, 4);
        assert!((profile[0] - 0.3).abs() < 0.001);
        assert!((profile[1] - 0.7).abs() < 0.001);
        assert_eq!(profile[2], 0.0);
        assert_eq!(profile[3], 0.0);
    }

    #[test]
    fn cultural_distance_identical() {
        let mut a = CulturalIdentity::new(1);
        a.bt_pattern_hash = 12345;
        a.signal_profile = vec![0.5, 0.3, 0.2];

        let b = a.clone();
        let dist = cultural_distance(&a, &b);
        assert!((dist - 0.0).abs() < 0.001);
    }

    #[test]
    fn cultural_distance_different_bt_same_signals() {
        let mut a = CulturalIdentity::new(1);
        a.bt_pattern_hash = 100;
        a.signal_profile = vec![0.5, 0.3, 0.2];

        let mut b = CulturalIdentity::new(2);
        b.bt_pattern_hash = 200;
        b.signal_profile = vec![0.5, 0.3, 0.2];

        let dist = cultural_distance(&a, &b);
        // Only BT component differs: 0.3 * 1.0 = 0.3
        assert!((dist - 0.3).abs() < 0.001);
    }

    #[test]
    fn cultural_distance_different_signals() {
        let mut a = CulturalIdentity::new(1);
        a.bt_pattern_hash = 100;
        a.signal_profile = vec![1.0, 0.0, 0.0];

        let mut b = CulturalIdentity::new(2);
        b.bt_pattern_hash = 100;
        b.signal_profile = vec![0.0, 0.0, 1.0];

        let dist = cultural_distance(&a, &b);
        // BT same (0.0), signals different
        assert!(dist > 0.0);
    }

    #[test]
    fn cultural_distance_no_data() {
        let a = CulturalIdentity::new(1);
        let b = CulturalIdentity::new(2);
        let dist = cultural_distance(&a, &b);
        // Both have hash 0 -> same
        assert_eq!(dist, 0.0);
    }
}

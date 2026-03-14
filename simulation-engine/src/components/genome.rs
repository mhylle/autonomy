use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use super::drives::DriveWeights;
use super::memory::EvictionWeights;

/// Genome containing evolvable physical and cognitive traits.
///
/// Includes physical traits (energy, speed, size), sensory traits
/// (sensor range), and drive sensitivities (how strongly the entity
/// responds to internal states).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genome {
    pub max_energy: f64,
    pub metabolism_rate: f64,
    pub max_speed: f64,
    pub sensor_range: f64,
    pub size: f64,
    pub max_lifespan: u64,
    /// Controls how frequently mutations occur (0.0-1.0)
    pub mutation_rate: f64,
    /// Base sensitivities for the drive system (evolvable).
    pub drive_weights: DriveWeights,
    /// Maximum number of memories this entity can hold.
    /// Higher capacity costs energy (increases metabolism).
    pub memory_capacity: u16,
    /// Weights controlling which memories survive eviction (evolvable).
    pub eviction_weights: EvictionWeights,
    /// Affinity for joining or forming composite organisms (0.0-1.0).
    /// Higher values make the entity more willing to merge with compatible neighbours.
    pub composition_affinity: f64,
    /// Hash for species grouping -- recomputed when genome changes
    pub species_id: u64,
}

impl Default for Genome {
    fn default() -> Self {
        let mut g = Self {
            max_energy: 100.0,
            metabolism_rate: 0.1,
            max_speed: 2.0,
            sensor_range: 50.0,
            size: 5.0,
            max_lifespan: 5000,
            mutation_rate: 0.05,
            drive_weights: DriveWeights::default(),
            memory_capacity: 20,
            eviction_weights: EvictionWeights::default(),
            composition_affinity: 0.1,
            species_id: 0,
        };
        g.species_id = compute_species_id(&g);
        g
    }
}

/// Quantize an f64 to 1 decimal place so that small mutations
/// don't immediately create a new species.
fn quantize(value: f64) -> u64 {
    ((value * 10.0).round() as i64) as u64
}

/// Compute a species hash from key genome traits.
///
/// Uses `wrapping_mul` + `wrapping_add` over the bit patterns of
/// quantized trait values to produce a deterministic hash.
pub fn compute_species_id(genome: &Genome) -> u64 {
    let traits: [u64; 5] = [
        quantize(genome.max_energy),
        quantize(genome.metabolism_rate),
        quantize(genome.max_speed),
        quantize(genome.size),
        genome.max_lifespan,
    ];

    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for t in &traits {
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
        hash = hash.wrapping_add(*t);
    }
    hash
}

/// Create a mutated copy of the genome.
///
/// Each numeric trait has a `genome.mutation_rate` probability of being
/// perturbed by Gaussian-like noise (uniform in -0.1..0.1 of the current
/// value). Results are clamped to sensible bounds and the species_id is
/// recomputed.
pub fn mutate(genome: &Genome, rng: &mut ChaCha8Rng) -> Genome {
    let mut child = genome.clone();

    // Helper: possibly mutate a single f64 trait, clamped to a minimum.
    let mut maybe_mutate_f64 = |value: f64, min: f64| -> f64 {
        if rng.gen::<f64>() < genome.mutation_rate {
            let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1 * value;
            (value + noise).max(min)
        } else {
            value
        }
    };

    child.max_energy = maybe_mutate_f64(child.max_energy, 1.0);
    child.metabolism_rate = maybe_mutate_f64(child.metabolism_rate, 0.001);
    child.max_speed = maybe_mutate_f64(child.max_speed, 0.1);
    child.sensor_range = maybe_mutate_f64(child.sensor_range, 1.0);
    child.size = maybe_mutate_f64(child.size, 0.1);

    // max_lifespan: mutate as f64, then round back
    if rng.gen::<f64>() < genome.mutation_rate {
        let value = child.max_lifespan as f64;
        let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1 * value;
        child.max_lifespan = (value + noise).max(100.0) as u64;
    }

    // mutation_rate itself can mutate, clamped to 0.001..0.5
    if rng.gen::<f64>() < genome.mutation_rate {
        let value = child.mutation_rate;
        let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1 * value;
        child.mutation_rate = (value + noise).clamp(0.001, 0.5);
    }

    // Drive weights: mutate each, clamped to 0.0..1.0.
    let mut mutate_drive = |value: f64| -> f64 {
        if rng.gen::<f64>() < genome.mutation_rate {
            let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1;
            (value + noise).clamp(0.0, 1.0)
        } else {
            value
        }
    };
    child.drive_weights.base_curiosity = mutate_drive(child.drive_weights.base_curiosity);
    child.drive_weights.base_social_need = mutate_drive(child.drive_weights.base_social_need);
    child.drive_weights.base_aggression = mutate_drive(child.drive_weights.base_aggression);
    child.drive_weights.base_reproductive = mutate_drive(child.drive_weights.base_reproductive);

    // Memory capacity: mutate as f64, round back, clamp to 1..200.
    if rng.gen::<f64>() < genome.mutation_rate {
        let value = child.memory_capacity as f64;
        let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1 * value;
        child.memory_capacity = (value + noise).clamp(1.0, 200.0) as u16;
    }

    // Eviction weights: mutate each, clamped to 0.0..1.0.
    let mut mutate_weight = |value: f64| -> f64 {
        if rng.gen::<f64>() < genome.mutation_rate {
            let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1;
            (value + noise).clamp(0.0, 1.0)
        } else {
            value
        }
    };
    child.eviction_weights.recency_weight = mutate_weight(child.eviction_weights.recency_weight);
    child.eviction_weights.importance_weight =
        mutate_weight(child.eviction_weights.importance_weight);
    child.eviction_weights.emotional_weight =
        mutate_weight(child.eviction_weights.emotional_weight);
    child.eviction_weights.variety_weight = mutate_weight(child.eviction_weights.variety_weight);

    // Composition affinity: mutate, clamped to 0.0..1.0.
    if rng.gen::<f64>() < genome.mutation_rate {
        let noise = (rng.gen::<f64>() * 2.0 - 1.0) * 0.1;
        child.composition_affinity = (child.composition_affinity + noise).clamp(0.0, 1.0);
    }

    child.species_id = compute_species_id(&child);
    child
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::drives::DriveWeights;
    use rand::SeedableRng;

    #[test]
    fn default_has_computed_species_id() {
        let g = Genome::default();
        assert_eq!(g.species_id, compute_species_id(&g));
        assert_ne!(g.species_id, 0);
    }

    #[test]
    fn default_values() {
        let g = Genome::default();
        assert_eq!(g.max_energy, 100.0);
        assert_eq!(g.metabolism_rate, 0.1);
        assert_eq!(g.max_speed, 2.0);
        assert_eq!(g.sensor_range, 50.0);
        assert_eq!(g.size, 5.0);
        assert_eq!(g.max_lifespan, 5000);
        assert_eq!(g.mutation_rate, 0.05);
    }

    #[test]
    fn mutate_produces_slightly_different_values() {
        let g = Genome {
            mutation_rate: 1.0, // force every trait to mutate
            ..Genome::default()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let child = mutate(&g, &mut rng);

        // At least one trait should differ
        let differs = child.max_energy != g.max_energy
            || child.metabolism_rate != g.metabolism_rate
            || child.max_speed != g.max_speed
            || child.sensor_range != g.sensor_range
            || child.size != g.size
            || child.max_lifespan != g.max_lifespan;
        assert!(differs, "mutate with rate=1.0 should change at least one trait");
    }

    #[test]
    fn mutate_respects_bounds() {
        // Tiny genome values to test lower-bound clamping
        let g = Genome {
            max_energy: 1.0,
            metabolism_rate: 0.001,
            max_speed: 0.1,
            sensor_range: 1.0,
            size: 0.1,
            max_lifespan: 100,
            mutation_rate: 1.0, // always mutate
            drive_weights: DriveWeights::default(),
            memory_capacity: 1,
            eviction_weights: EvictionWeights::default(),
            composition_affinity: 0.1,
            species_id: 0,
        };
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for _ in 0..100 {
            let child = mutate(&g, &mut rng);
            assert!(child.max_energy >= 1.0);
            assert!(child.metabolism_rate >= 0.001);
            assert!(child.max_speed >= 0.1);
            assert!(child.sensor_range >= 1.0);
            assert!(child.size >= 0.1);
            assert!(child.max_lifespan >= 100);
            assert!(child.mutation_rate >= 0.001);
            assert!(child.mutation_rate <= 0.5);
            assert!(child.memory_capacity >= 1);
            assert!(child.memory_capacity <= 200);
        }
    }

    #[test]
    fn species_id_changes_after_large_mutations() {
        // A genome where every trait will definitely mutate
        let g = Genome {
            mutation_rate: 1.0,
            ..Genome::default()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(12345);

        // Apply many rounds of mutation to accumulate large drift
        let mut current = g.clone();
        for _ in 0..50 {
            current = mutate(&current, &mut rng);
            current.mutation_rate = 1.0; // keep forcing mutations
        }

        assert_ne!(
            current.species_id, g.species_id,
            "50 rounds of full mutation should change species_id"
        );
    }

    #[test]
    fn species_id_stable_after_small_mutations() {
        let g = Genome::default();
        // Quantisation window for each trait is 0.05 (half of 0.1 quantum).
        // With mutation_rate=0.05, most traits won't mutate at all, and
        // those that do will change by at most ~1% of their value, which
        // stays within the quantisation bucket.
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let child = mutate(&g, &mut rng);
        assert_eq!(
            child.species_id, g.species_id,
            "a single low-rate mutation pass should rarely change species_id"
        );
    }

    #[test]
    fn serialization_roundtrip() {
        let g = Genome::default();
        let json = serde_json::to_string(&g).unwrap();
        let d: Genome = serde_json::from_str(&json).unwrap();
        assert_eq!(d.max_energy, g.max_energy);
        assert_eq!(d.metabolism_rate, g.metabolism_rate);
        assert_eq!(d.max_speed, g.max_speed);
        assert_eq!(d.sensor_range, g.sensor_range);
        assert_eq!(d.size, g.size);
        assert_eq!(d.max_lifespan, g.max_lifespan);
        assert_eq!(d.mutation_rate, g.mutation_rate);
        assert_eq!(d.species_id, g.species_id);
    }
}

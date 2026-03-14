//! NEAT (NeuroEvolution of Augmenting Topologies) stub module.
//!
//! This module provides placeholder types for future NEAT integration.
//! It does NOT implement actual neural networks -- it only defines the
//! interfaces and data structures that would be needed.
//!
//! Future work:
//! - NeatGenome: node genes + connection genes
//! - NeatNetwork: evaluate inputs -> outputs
//! - NeatPopulation: speciation, crossover, mutation
//! - Integration with entity decision-making (replace or augment BTs)

use serde::{Deserialize, Serialize};

/// Placeholder for a NEAT genome.
///
/// In a full implementation, this would contain node genes and
/// connection genes defining a neural network topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeatGenome {
    /// Number of input nodes.
    pub input_count: usize,
    /// Number of output nodes.
    pub output_count: usize,
    /// Placeholder for connection count.
    pub connection_count: usize,
    /// Global innovation number for tracking structural mutations.
    pub innovation_number: u64,
}

impl NeatGenome {
    /// Create a minimal NEAT genome with no hidden nodes.
    pub fn new_minimal(input_count: usize, output_count: usize) -> Self {
        Self {
            input_count,
            output_count,
            connection_count: 0,
            innovation_number: 0,
        }
    }

    /// Placeholder: would return the network complexity.
    pub fn complexity(&self) -> usize {
        self.connection_count
    }
}

/// Placeholder for a NEAT species.
///
/// Groups similar genomes for protected evolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeatSpecies {
    /// Species identifier.
    pub id: u64,
    /// Number of members in this species.
    pub member_count: usize,
    /// Average fitness of species members.
    pub avg_fitness: f64,
}

impl NeatSpecies {
    /// Create a new species.
    pub fn new(id: u64) -> Self {
        Self {
            id,
            member_count: 0,
            avg_fitness: 0.0,
        }
    }
}

/// Placeholder for NEAT configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeatConfig {
    /// Weight for excess genes in compatibility distance.
    pub c1_excess: f64,
    /// Weight for disjoint genes in compatibility distance.
    pub c2_disjoint: f64,
    /// Weight for weight differences in compatibility distance.
    pub c3_weight: f64,
    /// Compatibility threshold for speciation.
    pub compatibility_threshold: f64,
    /// Probability of adding a connection mutation.
    pub add_connection_prob: f64,
    /// Probability of adding a node mutation.
    pub add_node_prob: f64,
}

impl Default for NeatConfig {
    fn default() -> Self {
        Self {
            c1_excess: 1.0,
            c2_disjoint: 1.0,
            c3_weight: 0.4,
            compatibility_threshold: 3.0,
            add_connection_prob: 0.05,
            add_node_prob: 0.03,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neat_genome_minimal() {
        let genome = NeatGenome::new_minimal(5, 3);
        assert_eq!(genome.input_count, 5);
        assert_eq!(genome.output_count, 3);
        assert_eq!(genome.connection_count, 0);
        assert_eq!(genome.complexity(), 0);
    }

    #[test]
    fn neat_species_new() {
        let species = NeatSpecies::new(42);
        assert_eq!(species.id, 42);
        assert_eq!(species.member_count, 0);
        assert_eq!(species.avg_fitness, 0.0);
    }

    #[test]
    fn neat_config_default() {
        let config = NeatConfig::default();
        assert!(config.c1_excess > 0.0);
        assert!(config.compatibility_threshold > 0.0);
        assert!(config.add_connection_prob > 0.0);
        assert!(config.add_node_prob > 0.0);
    }

    #[test]
    fn neat_genome_serialization_roundtrip() {
        let genome = NeatGenome::new_minimal(10, 4);
        let json = serde_json::to_string(&genome).unwrap();
        let restored: NeatGenome = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.input_count, 10);
        assert_eq!(restored.output_count, 4);
    }
}

//! Era 9: Civilization systems.
//!
//! Observer/analyzer systems that detect emergent civilization structures
//! from entity behavior. These systems read simulation state and compute
//! metrics -- they do not modify entity behavior.
//!
//! Subsystems:
//! - Settlement detection from entity clustering
//! - Resource specialization tracking per settlement
//! - Trade route detection from movement patterns
//! - Defense scoring from nearby structures
//! - Hierarchy detection from social connections
//! - Cultural identity metrics from BT patterns and signal usage
//! - NEAT integration stub for future neural network work

pub mod culture;
pub mod defense;
pub mod hierarchy;
pub mod neat;
pub mod settlement;
pub mod trade;

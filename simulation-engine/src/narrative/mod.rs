//! Narrative system: identifies, tracks, and narrates emergent stories.
//!
//! This module is an observer/analyzer layer. It reads simulation events
//! and entity state but never modifies the simulation itself.
//!
//! # Subsystems
//!
//! - **significance** -- scores events and entities by narrative interest
//! - **arcs** -- detects story arcs (Rivalry, Alliance, Extinction, Rise, Fall, Migration)
//! - **biography** -- compiles entity life histories
//! - **tracker** -- central coordinator, event search, auto-tracking

pub mod arcs;
pub mod biography;
pub mod significance;
pub mod tracker;

pub use arcs::{ArcDetector, ArcType, StoryArc};
pub use biography::{Biography, BiographyCompiler, LifePhase};
pub use significance::{entity_interest_score, score_event};
pub use tracker::{EntityStats, EventSearchCriteria, NarrativeTracker, Narrator, StubNarrator, TrackedEntities};

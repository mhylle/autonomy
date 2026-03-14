use serde::{Deserialize, Serialize};

use super::memory::MemoryKind;

/// Result of ticking a behavior tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
}

/// Filter for selecting which nearby entities to consider in social behavior nodes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EntityFilter {
    /// Match any entity.
    Any,
    /// Match only entities with the same species_id (is_kin == true).
    Kin,
    /// Match only entities with a different species_id.
    NonKin,
    /// Match entities with a positive relationship score (> threshold).
    PositiveRelationship,
    /// Match entities with a negative relationship score (< -threshold).
    NegativeRelationship,
}

/// Threshold for considering a relationship "positive" or "negative" in EntityFilter.
pub const RELATIONSHIP_THRESHOLD: f64 = 0.1;

/// Data about a perceived entity enriched with social relationship info.
///
/// Used by the BT to make social decisions without accessing the ECS.
#[derive(Debug, Clone)]
pub struct SocialEntityInfo {
    /// The entity's raw ID bits.
    pub entity_id: u64,
    /// World-space position of the entity.
    pub x: f64,
    pub y: f64,
    /// Distance from the perceiving entity.
    pub distance: f64,
    /// Whether this entity is kin (same species).
    pub is_kin: bool,
    /// Relationship score from the Social component (-1.0 to 1.0, 0.0 if unknown).
    pub relationship: f64,
}

/// Filter for selecting which memory kinds to match in BT memory nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MemoryKindFilter {
    /// Match any food-related memory (FoundFood).
    AnyFood,
    /// Match any threat-related memory (WasAttacked).
    AnyThreat,
    /// Match memories with positive emotional valence.
    AnyPositive,
    /// Match memories with negative emotional valence.
    AnyNegative,
    /// Match a specific MemoryKind.
    Specific(MemoryKind),
}

/// Which drive to check in a CheckDrive condition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DriveKind {
    Hunger,
    Fear,
    Curiosity,
    SocialNeed,
    Aggression,
    ReproductiveUrge,
}

/// Comparison operator for condition nodes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Comparison {
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
}

impl Comparison {
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            Comparison::GreaterThan => value > threshold,
            Comparison::LessThan => value < threshold,
            Comparison::GreaterOrEqual => value >= threshold,
            Comparison::LessOrEqual => value <= threshold,
        }
    }
}

/// Action produced by a behavior tree action node.
///
/// Consumed by movement/feeding/etc systems in Phase 2.4+.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtAction {
    /// Move toward the closest perceived resource.
    MoveTowardResource { speed_factor: f64 },
    /// Wander randomly.
    Wander { speed: f64 },
    /// Attempt to eat the closest adjacent resource.
    Eat,
    /// Rest (zero velocity, slight energy recovery).
    Rest,
    /// Attack the nearest perceived entity with given force multiplier.
    Attack { force_factor: f64 },
    /// Move toward a remembered location (from memory recall).
    MoveTowardMemory { x: f64, y: f64, speed_factor: f64 },
    /// Flee from a position (move away from it).
    FleeFrom { x: f64, y: f64, speed_factor: f64 },
    /// Move toward a filtered entity (social behavior).
    MoveTowardEntity { entity_id: u64, x: f64, y: f64, speed_factor: f64 },
    /// Flee from a filtered entity (social behavior).
    FleeFromEntity { entity_id: u64, x: f64, y: f64, speed_factor: f64 },
    /// Attempt to merge with a nearby compatible entity (Phase 4.1).
    CompositionAttempt,
    /// Emit a signal into the environment (Phase 5.1).
    EmitSignal { signal_type: u8 },
    /// Move toward a perceived signal source (Phase 5.2).
    MoveTowardSignal { x: f64, y: f64, speed_factor: f64 },
    /// No action.
    None,
}

/// Context provided to behavior tree evaluation.
///
/// Contains read-only snapshots of the entity's state so the BT can
/// make decisions without borrowing the ECS world.
pub struct BtContext {
    /// Current drive levels.
    pub hunger: f64,
    pub fear: f64,
    pub curiosity: f64,
    pub social_need: f64,
    pub aggression: f64,
    pub reproductive_urge: f64,
    /// Current energy fraction (current / max).
    pub energy_fraction: f64,
    /// Whether any resources are perceived within sensor range.
    pub has_nearby_resource: bool,
    /// Distance to the closest perceived resource (f64::MAX if none).
    pub closest_resource_distance: f64,
    /// Whether any entities are perceived within sensor range.
    pub has_nearby_entity: bool,
    /// Distance to the closest perceived entity (f64::MAX if none).
    pub closest_entity_distance: f64,

    // -- Social integration fields (Phase 3.5) --
    /// Perceived entities enriched with relationship data for social BT nodes.
    pub social_entities: Vec<SocialEntityInfo>,

    // -- Memory integration fields (Phase 3.3) --
    /// Whether a food memory exists within the recall window.
    pub has_food_memory: bool,
    /// Location of the most recent food memory, if any.
    pub food_memory_location: Option<(f64, f64)>,
    /// Whether a threat memory exists within the recall window.
    pub has_threat_memory: bool,
    /// Location of the most recent threat memory, if any.
    pub threat_memory_location: Option<(f64, f64)>,
    /// Number of WasAttacked memories (used to boost fear drive).
    pub was_attacked_count: u32,
    /// Current simulation tick, used for memory age filtering.
    pub current_tick: u64,
    /// All recalled memory locations with their kinds and valences,
    /// for general-purpose memory filter matching.
    pub memory_entries: Vec<MemoryContextEntry>,

    // -- Signal integration fields (Phase 5.2) --
    /// Perceived signals available for BT condition/action nodes.
    pub perceived_signals: Vec<PerceivedSignalInfo>,
}

/// Lightweight snapshot of a perceived signal for BT evaluation.
#[derive(Debug, Clone)]
pub struct PerceivedSignalInfo {
    pub signal_type: u8,
    pub distance: f64,
    pub direction_x: f64,
    pub direction_y: f64,
    pub strength: f64,
    pub source_x: f64,
    pub source_y: f64,
}

/// A lightweight snapshot of a memory entry for BT evaluation.
#[derive(Debug, Clone)]
pub struct MemoryContextEntry {
    pub kind: MemoryKind,
    pub x: f64,
    pub y: f64,
    pub tick: u64,
    pub emotional_valence: f64,
}

impl BtContext {
    fn drive_value(&self, kind: &DriveKind) -> f64 {
        match kind {
            DriveKind::Hunger => self.hunger,
            DriveKind::Fear => self.fear,
            DriveKind::Curiosity => self.curiosity,
            DriveKind::SocialNeed => self.social_need,
            DriveKind::Aggression => self.aggression,
            DriveKind::ReproductiveUrge => self.reproductive_urge,
        }
    }

    /// Find the closest entity matching the given filter within the specified range.
    pub fn closest_filtered_entity(&self, filter: &EntityFilter, range: f64) -> Option<&SocialEntityInfo> {
        self.social_entities
            .iter()
            .filter(|e| e.distance <= range && entity_filter_matches(filter, e))
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Check whether any entity matching the filter is within range.
    pub fn has_filtered_entity(&self, filter: &EntityFilter, range: f64) -> bool {
        self.closest_filtered_entity(filter, range).is_some()
    }

    /// Check whether any perceived signal of the given type exists.
    pub fn has_signal_of_type(&self, signal_type: u8) -> bool {
        self.perceived_signals
            .iter()
            .any(|s| s.signal_type == signal_type)
    }

    /// Find the strongest perceived signal of a given type.
    pub fn strongest_signal_of_type(&self, signal_type: u8) -> Option<&PerceivedSignalInfo> {
        self.perceived_signals
            .iter()
            .filter(|s| s.signal_type == signal_type)
            .max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Check whether any memory entry matches the given filter within the max age.
    pub fn has_memory_matching(&self, filter: &MemoryKindFilter, max_age: u64) -> bool {
        self.find_matching_memory(filter, max_age).is_some()
    }

    /// Find the most recent memory entry matching the given filter within max age.
    /// Returns the (x, y) location and tick of the best match.
    pub fn find_matching_memory(&self, filter: &MemoryKindFilter, max_age: u64) -> Option<(f64, f64)> {
        let min_tick = self.current_tick.saturating_sub(max_age);
        self.memory_entries
            .iter()
            .filter(|e| e.tick >= min_tick && filter_matches(filter, e))
            .max_by_key(|e| e.tick)
            .map(|e| (e.x, e.y))
    }
}

/// Check whether a social entity matches the given EntityFilter.
fn entity_filter_matches(filter: &EntityFilter, entity: &SocialEntityInfo) -> bool {
    match filter {
        EntityFilter::Any => true,
        EntityFilter::Kin => entity.is_kin,
        EntityFilter::NonKin => !entity.is_kin,
        EntityFilter::PositiveRelationship => entity.relationship > RELATIONSHIP_THRESHOLD,
        EntityFilter::NegativeRelationship => entity.relationship < -RELATIONSHIP_THRESHOLD,
    }
}

/// Check whether a memory context entry matches a filter.
fn filter_matches(filter: &MemoryKindFilter, entry: &MemoryContextEntry) -> bool {
    match filter {
        MemoryKindFilter::AnyFood => entry.kind == MemoryKind::FoundFood,
        MemoryKindFilter::AnyThreat => entry.kind == MemoryKind::WasAttacked,
        MemoryKindFilter::AnyPositive => entry.emotional_valence > 0.0,
        MemoryKindFilter::AnyNegative => entry.emotional_valence < 0.0,
        MemoryKindFilter::Specific(kind) => entry.kind == *kind,
    }
}

/// Behavior tree node.
///
/// Serializable with serde for genome storage and crossover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtNode {
    // -- Control flow --
    /// Run children in order; fail on first failure, succeed if all succeed.
    Sequence(Vec<BtNode>),
    /// Try children in order; succeed on first success, fail if all fail.
    Selector(Vec<BtNode>),
    /// Invert child's result (Success <-> Failure, Running unchanged).
    Inverter(Box<BtNode>),
    /// Always return Success regardless of child's result.
    AlwaysSucceed(Box<BtNode>),

    // -- Condition nodes --
    /// Check a drive against a threshold.
    CheckDrive {
        drive: DriveKind,
        threshold: f64,
        comparison: Comparison,
    },
    /// Check if there is a resource within range.
    NearbyResource { range: f64 },
    /// Check energy fraction against a threshold.
    CheckEnergy {
        threshold: f64,
        comparison: Comparison,
    },

    // -- Condition nodes (combat) --
    /// Check if there is another entity within range.
    NearbyEntity { range: f64 },

    // -- Condition nodes (memory, Phase 3.3) --
    /// Check if a memory matching the filter exists within max_age ticks.
    RecallMemory {
        kind: MemoryKindFilter,
        max_age: u64,
    },

    // -- Condition nodes (social, Phase 3.5) --
    /// Check if there is a nearby entity matching the filter within range.
    NearbyEntityFiltered {
        range: f64,
        filter: EntityFilter,
    },

    // -- Action nodes --
    /// Move toward the closest perceived resource.
    MoveTowardResource { speed_factor: f64 },
    /// Wander randomly.
    Wander { speed: f64 },
    /// Eat the closest adjacent resource.
    Eat,
    /// Rest (no movement).
    Rest,
    /// Attack the nearest perceived entity with given force multiplier.
    Attack { force_factor: f64 },

    // -- Action nodes (social, Phase 3.5) --
    /// Move toward the closest entity matching the filter.
    MoveTowardEntity {
        filter: EntityFilter,
        speed_factor: f64,
    },
    /// Flee from the closest entity matching the filter.
    FleeFromEntity {
        filter: EntityFilter,
        speed_factor: f64,
    },

    // -- Action nodes (memory, Phase 3.3) --
    /// Move toward the location of the most recent matching memory.
    MoveTowardMemory {
        kind: MemoryKindFilter,
        speed_factor: f64,
    },
    /// Flee from the location of the most recent matching memory.
    FleeFromMemory {
        kind: MemoryKindFilter,
        speed_factor: f64,
    },

    // -- Action nodes (composition, Phase 4.1) --
    /// Attempt to merge with a nearby compatible entity.
    CompositionAttempt,

    // -- Condition nodes (signals, Phase 5.2) --
    /// Check if a signal of the given type is perceived.
    DetectSignal { signal_type: u8 },

    // -- Action nodes (signals, Phase 5.1 & 5.2) --
    /// Emit a signal into the environment.
    EmitSignal { signal_type: u8 },
    /// Move toward the strongest perceived signal of a given type.
    MoveTowardSignal { signal_type: u8, speed_factor: f64 },
}

/// Recursively evaluate a behavior tree node given a context.
///
/// Returns the status and the action produced (if any).
pub fn tick_bt(node: &BtNode, ctx: &BtContext) -> (BtStatus, BtAction) {
    match node {
        BtNode::Sequence(children) => {
            let mut last_action = BtAction::None;
            for child in children {
                let (status, action) = tick_bt(child, ctx);
                if action != BtAction::None {
                    last_action = action;
                }
                match status {
                    BtStatus::Failure => return (BtStatus::Failure, BtAction::None),
                    BtStatus::Running => return (BtStatus::Running, last_action),
                    BtStatus::Success => {}
                }
            }
            (BtStatus::Success, last_action)
        }

        BtNode::Selector(children) => {
            for child in children {
                let (status, action) = tick_bt(child, ctx);
                match status {
                    BtStatus::Success => return (BtStatus::Success, action),
                    BtStatus::Running => return (BtStatus::Running, action),
                    BtStatus::Failure => {}
                }
            }
            (BtStatus::Failure, BtAction::None)
        }

        BtNode::Inverter(child) => {
            let (status, action) = tick_bt(child, ctx);
            let inverted = match status {
                BtStatus::Success => BtStatus::Failure,
                BtStatus::Failure => BtStatus::Success,
                BtStatus::Running => BtStatus::Running,
            };
            (inverted, action)
        }

        BtNode::AlwaysSucceed(child) => {
            let (_status, action) = tick_bt(child, ctx);
            (BtStatus::Success, action)
        }

        // Condition nodes
        BtNode::CheckDrive {
            drive,
            threshold,
            comparison,
        } => {
            let value = ctx.drive_value(drive);
            if comparison.evaluate(value, *threshold) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::NearbyResource { range } => {
            if ctx.has_nearby_resource && ctx.closest_resource_distance <= *range {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::NearbyEntity { range } => {
            if ctx.has_nearby_entity && ctx.closest_entity_distance <= *range {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::CheckEnergy {
            threshold,
            comparison,
        } => {
            if comparison.evaluate(ctx.energy_fraction, *threshold) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Action nodes
        BtNode::MoveTowardResource { speed_factor } => {
            if ctx.has_nearby_resource {
                (
                    BtStatus::Success,
                    BtAction::MoveTowardResource {
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::Wander { speed } => (
            BtStatus::Success,
            BtAction::Wander { speed: *speed },
        ),

        BtNode::Eat => {
            if ctx.has_nearby_resource {
                (BtStatus::Success, BtAction::Eat)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::Rest => (BtStatus::Success, BtAction::Rest),

        BtNode::Attack { force_factor } => {
            if ctx.has_nearby_entity {
                (
                    BtStatus::Success,
                    BtAction::Attack {
                        force_factor: *force_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Social condition node (Phase 3.5)
        BtNode::NearbyEntityFiltered { range, filter } => {
            if ctx.has_filtered_entity(filter, *range) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Social action nodes (Phase 3.5)
        BtNode::MoveTowardEntity { filter, speed_factor } => {
            if let Some(ent) = ctx.closest_filtered_entity(filter, f64::MAX) {
                (
                    BtStatus::Success,
                    BtAction::MoveTowardEntity {
                        entity_id: ent.entity_id,
                        x: ent.x,
                        y: ent.y,
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::FleeFromEntity { filter, speed_factor } => {
            if let Some(ent) = ctx.closest_filtered_entity(filter, f64::MAX) {
                (
                    BtStatus::Success,
                    BtAction::FleeFromEntity {
                        entity_id: ent.entity_id,
                        x: ent.x,
                        y: ent.y,
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Memory condition node (Phase 3.3)
        BtNode::RecallMemory { kind, max_age } => {
            if ctx.has_memory_matching(kind, *max_age) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Memory action nodes (Phase 3.3)
        BtNode::MoveTowardMemory { kind, speed_factor } => {
            // Use a generous max_age (u64::MAX) -- the RecallMemory condition
            // already filtered by age, so the action just needs the location.
            if let Some((x, y)) = ctx.find_matching_memory(kind, u64::MAX) {
                (
                    BtStatus::Success,
                    BtAction::MoveTowardMemory {
                        x,
                        y,
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::FleeFromMemory { kind, speed_factor } => {
            if let Some((x, y)) = ctx.find_matching_memory(kind, u64::MAX) {
                (
                    BtStatus::Success,
                    BtAction::FleeFrom {
                        x,
                        y,
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Composition action node (Phase 4.1)
        BtNode::CompositionAttempt => {
            // Always succeeds -- the composition system checks compatibility.
            if ctx.has_nearby_entity {
                (BtStatus::Success, BtAction::CompositionAttempt)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Signal condition node (Phase 5.2)
        BtNode::DetectSignal { signal_type } => {
            if ctx.has_signal_of_type(*signal_type) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Signal action nodes (Phase 5.1 & 5.2)
        BtNode::EmitSignal { signal_type } => {
            (BtStatus::Success, BtAction::EmitSignal { signal_type: *signal_type })
        }

        BtNode::MoveTowardSignal { signal_type, speed_factor } => {
            if let Some(sig) = ctx.strongest_signal_of_type(*signal_type) {
                (
                    BtStatus::Success,
                    BtAction::MoveTowardSignal {
                        x: sig.source_x,
                        y: sig.source_y,
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }
    }
}

/// Build the default starter behavior tree.
///
/// Logic: Selector(Sequence(CheckHungry, NearbyFood, MoveToFood, Eat), Wander)
pub fn default_starter_bt() -> BtNode {
    BtNode::Selector(vec![
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.3,
                comparison: Comparison::GreaterThan,
            },
            BtNode::NearbyResource { range: 50.0 },
            BtNode::MoveTowardResource { speed_factor: 1.0 },
            BtNode::Eat,
        ]),
        BtNode::Wander { speed: 1.5 },
    ])
}

/// Build a memory-enhanced starter behavior tree.
///
/// Logic:
///   Selector(
///     Sequence(CheckHungry, NearbyFood, MoveToFood, Eat),           // direct feeding
///     Sequence(CheckHungry, RecallFood, MoveTowardFoodMemory),       // memory-guided feeding
///     Sequence(CheckFear, RecallThreat, FleeFromThreatMemory),       // flee from remembered threat
///     Wander
///   )
pub fn memory_enhanced_starter_bt() -> BtNode {
    BtNode::Selector(vec![
        // Priority 1: If hungry and food visible, go eat it directly.
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.3,
                comparison: Comparison::GreaterThan,
            },
            BtNode::NearbyResource { range: 50.0 },
            BtNode::MoveTowardResource { speed_factor: 1.0 },
            BtNode::Eat,
        ]),
        // Priority 2: If hungry and remember food location, move toward it.
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.3,
                comparison: Comparison::GreaterThan,
            },
            BtNode::RecallMemory {
                kind: MemoryKindFilter::AnyFood,
                max_age: 500,
            },
            BtNode::MoveTowardMemory {
                kind: MemoryKindFilter::AnyFood,
                speed_factor: 0.8,
            },
        ]),
        // Priority 3: If afraid and remember threat, flee from it.
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Fear,
                threshold: 0.4,
                comparison: Comparison::GreaterThan,
            },
            BtNode::RecallMemory {
                kind: MemoryKindFilter::AnyThreat,
                max_age: 200,
            },
            BtNode::FleeFromMemory {
                kind: MemoryKindFilter::AnyThreat,
                speed_factor: 1.5,
            },
        ]),
        // Fallback: wander.
        BtNode::Wander { speed: 1.5 },
    ])
}

/// Build a social-aware starter behavior tree.
///
/// Logic:
///   Selector(
///     Sequence(CheckHungry, NearbyFood, MoveToFood, Eat),
///     Sequence(CheckSocialNeed, NearbyKin, MoveTowardKin),
///     Sequence(NearbyNegative, FleeFromNegative),
///     Wander
///   )
pub fn social_starter_bt() -> BtNode {
    BtNode::Selector(vec![
        // Priority 1: If hungry and food visible, go eat it directly.
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.3,
                comparison: Comparison::GreaterThan,
            },
            BtNode::NearbyResource { range: 50.0 },
            BtNode::MoveTowardResource { speed_factor: 1.0 },
            BtNode::Eat,
        ]),
        // Priority 2: If social need is high and kin nearby, move toward kin.
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::SocialNeed,
                threshold: 0.4,
                comparison: Comparison::GreaterThan,
            },
            BtNode::NearbyEntityFiltered {
                range: 80.0,
                filter: EntityFilter::Kin,
            },
            BtNode::MoveTowardEntity {
                filter: EntityFilter::Kin,
                speed_factor: 0.8,
            },
        ]),
        // Priority 3: If negative relationship entity nearby, flee.
        BtNode::Sequence(vec![
            BtNode::NearbyEntityFiltered {
                range: 40.0,
                filter: EntityFilter::NegativeRelationship,
            },
            BtNode::FleeFromEntity {
                filter: EntityFilter::NegativeRelationship,
                speed_factor: 1.5,
            },
        ]),
        // Fallback: wander.
        BtNode::Wander { speed: 1.5 },
    ])
}

/// Count the total number of nodes in a BT.
pub fn node_count(node: &BtNode) -> usize {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            1 + children.iter().map(node_count).sum::<usize>()
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => 1 + node_count(child),
        _ => 1, // Leaf nodes
    }
}

/// Maximum depth of a BT.
pub fn depth(node: &BtNode) -> usize {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            1 + children.iter().map(depth).max().unwrap_or(0)
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => 1 + depth(child),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hungry_with_food_ctx() -> BtContext {
        BtContext {
            hunger: 0.8,
            fear: 0.0,
            curiosity: 0.2,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.0,
            energy_fraction: 0.2,
            has_nearby_resource: true,
            closest_resource_distance: 15.0,
            has_nearby_entity: false,
            closest_entity_distance: f64::MAX,
            social_entities: vec![],
            has_food_memory: false,
            food_memory_location: None,
            has_threat_memory: false,
            threat_memory_location: None,
            was_attacked_count: 0,
            current_tick: 100,
            memory_entries: vec![],
            perceived_signals: vec![],
        }
    }

    fn full_no_food_ctx() -> BtContext {
        BtContext {
            hunger: 0.1,
            fear: 0.0,
            curiosity: 0.3,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.5,
            energy_fraction: 0.9,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            has_nearby_entity: false,
            closest_entity_distance: f64::MAX,
            social_entities: vec![],
            has_food_memory: false,
            food_memory_location: None,
            has_threat_memory: false,
            threat_memory_location: None,
            was_attacked_count: 0,
            current_tick: 100,
            memory_entries: vec![],
            perceived_signals: vec![],
        }
    }

    #[test]
    fn sequence_succeeds_when_all_succeed() {
        let bt = BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::Eat,
        ]);
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn sequence_fails_on_first_failure() {
        let bt = BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.9, // hunger is 0.8, so this fails
                comparison: Comparison::GreaterThan,
            },
            BtNode::Eat,
        ]);
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn selector_succeeds_on_first_success() {
        let bt = BtNode::Selector(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Fear,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::Wander { speed: 1.0 },
        ]);
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.0 });
    }

    #[test]
    fn selector_fails_when_all_fail() {
        let bt = BtNode::Selector(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Fear,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::CheckDrive {
                drive: DriveKind::Aggression,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
        ]);
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn inverter_flips_success_to_failure() {
        let bt = BtNode::Inverter(Box::new(BtNode::Rest));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn inverter_flips_failure_to_success() {
        let bt = BtNode::Inverter(Box::new(BtNode::CheckDrive {
            drive: DriveKind::Fear,
            threshold: 0.9,
            comparison: Comparison::GreaterThan,
        }));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn always_succeed_wraps_failure() {
        let bt = BtNode::AlwaysSucceed(Box::new(BtNode::CheckDrive {
            drive: DriveKind::Fear,
            threshold: 0.9,
            comparison: Comparison::GreaterThan,
        }));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn check_drive_hunger() {
        let bt = BtNode::CheckDrive {
            drive: DriveKind::Hunger,
            threshold: 0.5,
            comparison: Comparison::GreaterThan,
        };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 0.8 > 0.5
    }

    #[test]
    fn check_energy_low() {
        let bt = BtNode::CheckEnergy {
            threshold: 0.3,
            comparison: Comparison::LessThan,
        };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 0.2 < 0.3
    }

    #[test]
    fn nearby_resource_in_range() {
        let bt = BtNode::NearbyResource { range: 50.0 };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 15.0 <= 50.0
    }

    #[test]
    fn nearby_resource_out_of_range() {
        let bt = BtNode::NearbyResource { range: 10.0 };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure); // 15.0 > 10.0
    }

    #[test]
    fn move_toward_resource_with_food() {
        let bt = BtNode::MoveTowardResource { speed_factor: 2.0 };
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::MoveTowardResource { speed_factor: 2.0 }
        );
    }

    #[test]
    fn move_toward_resource_no_food() {
        let bt = BtNode::MoveTowardResource { speed_factor: 2.0 };
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn eat_with_food() {
        let bt = BtNode::Eat;
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn eat_no_food() {
        let bt = BtNode::Eat;
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn default_bt_hungry_with_food_seeks_food() {
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        // The sequence should produce MoveTowardResource then Eat;
        // the last action in the sequence wins.
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn default_bt_not_hungry_wanders() {
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.5 });
    }

    #[test]
    fn default_bt_hungry_no_food_wanders() {
        let ctx = BtContext {
            hunger: 0.8,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            ..hungry_with_food_ctx()
        };
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.5 });
    }

    #[test]
    fn serialization_roundtrip() {
        let bt = default_starter_bt();
        let json = serde_json::to_string(&bt).unwrap();
        let deserialized: BtNode = serde_json::from_str(&json).unwrap();
        assert_eq!(bt, deserialized);
    }

    #[test]
    fn node_count_leaf() {
        assert_eq!(node_count(&BtNode::Eat), 1);
        assert_eq!(node_count(&BtNode::Rest), 1);
    }

    #[test]
    fn node_count_default_bt() {
        let bt = default_starter_bt();
        assert_eq!(node_count(&bt), 7); // Selector(Sequence(4 leaves), Wander) = 1+1+4+1
    }

    #[test]
    fn depth_leaf() {
        assert_eq!(depth(&BtNode::Eat), 1);
    }

    #[test]
    fn depth_default_bt() {
        let bt = default_starter_bt();
        assert_eq!(depth(&bt), 3); // Selector -> Sequence -> leaf
    }

    #[test]
    fn comparison_evaluate() {
        assert!(Comparison::GreaterThan.evaluate(0.5, 0.3));
        assert!(!Comparison::GreaterThan.evaluate(0.3, 0.5));
        assert!(Comparison::LessThan.evaluate(0.3, 0.5));
        assert!(!Comparison::LessThan.evaluate(0.5, 0.3));
        assert!(Comparison::GreaterOrEqual.evaluate(0.5, 0.5));
        assert!(Comparison::LessOrEqual.evaluate(0.5, 0.5));
    }

    // ---- Phase 3.3: Memory--Behavior Integration tests ----

    fn ctx_with_food_memory() -> BtContext {
        BtContext {
            hunger: 0.8,
            fear: 0.0,
            curiosity: 0.2,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.0,
            energy_fraction: 0.2,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            has_nearby_entity: false,
            closest_entity_distance: f64::MAX,
            social_entities: vec![],
            has_food_memory: true,
            food_memory_location: Some((120.0, 80.0)),
            has_threat_memory: false,
            threat_memory_location: None,
            was_attacked_count: 0,
            current_tick: 200,
            memory_entries: vec![MemoryContextEntry {
                kind: MemoryKind::FoundFood,
                x: 120.0,
                y: 80.0,
                tick: 150,
                emotional_valence: 0.5,
            }],
            perceived_signals: vec![],
        }
    }

    fn ctx_with_threat_memory() -> BtContext {
        BtContext {
            hunger: 0.2,
            fear: 0.7,
            curiosity: 0.1,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.0,
            energy_fraction: 0.8,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            has_nearby_entity: false,
            closest_entity_distance: f64::MAX,
            social_entities: vec![],
            has_food_memory: false,
            food_memory_location: None,
            has_threat_memory: true,
            threat_memory_location: Some((30.0, 40.0)),
            was_attacked_count: 3,
            current_tick: 200,
            memory_entries: vec![MemoryContextEntry {
                kind: MemoryKind::WasAttacked,
                x: 30.0,
                y: 40.0,
                tick: 180,
                emotional_valence: -0.9,
            }],
            perceived_signals: vec![],
        }
    }

    #[test]
    fn recall_memory_succeeds_when_food_memory_exists() {
        let bt = BtNode::RecallMemory {
            kind: MemoryKindFilter::AnyFood,
            max_age: 100,
        };
        let (status, action) = tick_bt(&bt, &ctx_with_food_memory());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::None);
    }

    #[test]
    fn recall_memory_fails_when_no_matching_memory() {
        let bt = BtNode::RecallMemory {
            kind: MemoryKindFilter::AnyFood,
            max_age: 100,
        };
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn recall_memory_respects_max_age() {
        let bt = BtNode::RecallMemory {
            kind: MemoryKindFilter::AnyFood,
            max_age: 30,
        };
        let (status, _) = tick_bt(&bt, &ctx_with_food_memory());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn move_toward_memory_succeeds_with_food_memory() {
        let bt = BtNode::MoveTowardMemory {
            kind: MemoryKindFilter::AnyFood,
            speed_factor: 1.0,
        };
        let (status, action) = tick_bt(&bt, &ctx_with_food_memory());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::MoveTowardMemory {
                x: 120.0,
                y: 80.0,
                speed_factor: 1.0,
            }
        );
    }

    #[test]
    fn move_toward_memory_fails_without_memory() {
        let bt = BtNode::MoveTowardMemory {
            kind: MemoryKindFilter::AnyFood,
            speed_factor: 1.0,
        };
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn flee_from_memory_succeeds_with_threat() {
        let bt = BtNode::FleeFromMemory {
            kind: MemoryKindFilter::AnyThreat,
            speed_factor: 2.0,
        };
        let (status, action) = tick_bt(&bt, &ctx_with_threat_memory());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::FleeFrom {
                x: 30.0,
                y: 40.0,
                speed_factor: 2.0,
            }
        );
    }

    #[test]
    fn flee_from_memory_fails_without_threat() {
        let bt = BtNode::FleeFromMemory {
            kind: MemoryKindFilter::AnyThreat,
            speed_factor: 2.0,
        };
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn memory_enhanced_bt_hungry_uses_memory() {
        let bt = memory_enhanced_starter_bt();
        let ctx = ctx_with_food_memory();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::MoveTowardMemory {
                x: 120.0,
                y: 80.0,
                speed_factor: 0.8,
            }
        );
    }

    #[test]
    fn memory_enhanced_bt_afraid_flees() {
        let bt = memory_enhanced_starter_bt();
        let ctx = ctx_with_threat_memory();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::FleeFrom {
                x: 30.0,
                y: 40.0,
                speed_factor: 1.5,
            }
        );
    }

    #[test]
    fn memory_kind_filter_any_positive() {
        let ctx = BtContext {
            memory_entries: vec![MemoryContextEntry {
                kind: MemoryKind::Reproduced,
                x: 50.0,
                y: 60.0,
                tick: 90,
                emotional_valence: 0.8,
            }],
            current_tick: 100,
            ..full_no_food_ctx()
        };
        assert!(ctx.has_memory_matching(&MemoryKindFilter::AnyPositive, 100));
        assert!(!ctx.has_memory_matching(&MemoryKindFilter::AnyNegative, 100));
    }

    #[test]
    fn memory_kind_filter_specific() {
        let ctx = BtContext {
            memory_entries: vec![MemoryContextEntry {
                kind: MemoryKind::NearDeath,
                x: 10.0,
                y: 20.0,
                tick: 95,
                emotional_valence: -1.0,
            }],
            current_tick: 100,
            ..full_no_food_ctx()
        };
        assert!(ctx.has_memory_matching(
            &MemoryKindFilter::Specific(MemoryKind::NearDeath),
            100
        ));
        assert!(!ctx.has_memory_matching(
            &MemoryKindFilter::Specific(MemoryKind::FoundFood),
            100
        ));
    }

    #[test]
    fn find_matching_memory_returns_most_recent() {
        let ctx = BtContext {
            memory_entries: vec![
                MemoryContextEntry {
                    kind: MemoryKind::FoundFood,
                    x: 10.0,
                    y: 20.0,
                    tick: 50,
                    emotional_valence: 0.3,
                },
                MemoryContextEntry {
                    kind: MemoryKind::FoundFood,
                    x: 80.0,
                    y: 90.0,
                    tick: 90,
                    emotional_valence: 0.5,
                },
            ],
            current_tick: 100,
            ..full_no_food_ctx()
        };
        let loc = ctx.find_matching_memory(&MemoryKindFilter::AnyFood, u64::MAX);
        assert_eq!(loc, Some((80.0, 90.0)));
    }

    #[test]
    fn serialization_roundtrip_memory_nodes() {
        let bt = memory_enhanced_starter_bt();
        let json = serde_json::to_string(&bt).unwrap();
        let deserialized: BtNode = serde_json::from_str(&json).unwrap();
        assert_eq!(bt, deserialized);
    }

    // ---- Phase 3.5: Social behavior tests ----

    fn make_social_entity(
        entity_id: u64,
        x: f64,
        y: f64,
        distance: f64,
        is_kin: bool,
        relationship: f64,
    ) -> SocialEntityInfo {
        SocialEntityInfo {
            entity_id,
            x,
            y,
            distance,
            is_kin,
            relationship,
        }
    }

    fn base_ctx() -> BtContext {
        BtContext {
            hunger: 0.0,
            fear: 0.0,
            curiosity: 0.0,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.0,
            energy_fraction: 0.5,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            has_nearby_entity: false,
            closest_entity_distance: f64::MAX,
            social_entities: vec![],
            has_food_memory: false,
            food_memory_location: None,
            has_threat_memory: false,
            threat_memory_location: None,
            was_attacked_count: 0,
            current_tick: 100,
            memory_entries: vec![],
            perceived_signals: vec![],
        }
    }

    #[test]
    fn nearby_entity_filtered_kin_succeeds_when_kin_present() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 10.0, true, 0.0)],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::NearbyEntityFiltered {
            range: 50.0,
            filter: EntityFilter::Kin,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn nearby_entity_filtered_kin_fails_when_only_non_kin() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 10.0, false, 0.0)],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::NearbyEntityFiltered {
            range: 50.0,
            filter: EntityFilter::Kin,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn nearby_entity_filtered_positive_relationship() {
        let ctx = BtContext {
            social_entities: vec![
                make_social_entity(1, 60.0, 50.0, 10.0, true, 0.5),
                make_social_entity(2, 70.0, 50.0, 20.0, false, -0.5),
            ],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::NearbyEntityFiltered {
            range: 50.0,
            filter: EntityFilter::PositiveRelationship,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn nearby_entity_filtered_negative_relationship() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 10.0, false, -0.5)],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::NearbyEntityFiltered {
            range: 50.0,
            filter: EntityFilter::NegativeRelationship,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn nearby_entity_filtered_out_of_range_fails() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 100.0, true, 0.5)],
            has_nearby_entity: true,
            closest_entity_distance: 100.0,
            ..base_ctx()
        };
        let bt = BtNode::NearbyEntityFiltered {
            range: 50.0,
            filter: EntityFilter::Kin,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn move_toward_entity_produces_correct_action() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(42, 80.0, 60.0, 30.0, true, 0.5)],
            has_nearby_entity: true,
            closest_entity_distance: 30.0,
            ..base_ctx()
        };
        let bt = BtNode::MoveTowardEntity {
            filter: EntityFilter::Kin,
            speed_factor: 1.5,
        };
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::MoveTowardEntity {
                entity_id: 42,
                x: 80.0,
                y: 60.0,
                speed_factor: 1.5,
            }
        );
    }

    #[test]
    fn move_toward_entity_fails_when_no_match() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 10.0, false, 0.0)],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::MoveTowardEntity {
            filter: EntityFilter::Kin,
            speed_factor: 1.0,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn flee_from_entity_produces_correct_action() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(99, 30.0, 40.0, 15.0, false, -0.8)],
            has_nearby_entity: true,
            closest_entity_distance: 15.0,
            ..base_ctx()
        };
        let bt = BtNode::FleeFromEntity {
            filter: EntityFilter::NegativeRelationship,
            speed_factor: 2.0,
        };
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::FleeFromEntity {
                entity_id: 99,
                x: 30.0,
                y: 40.0,
                speed_factor: 2.0,
            }
        );
    }

    #[test]
    fn flee_from_entity_fails_when_no_negative() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(1, 60.0, 50.0, 10.0, true, 0.5)],
            has_nearby_entity: true,
            closest_entity_distance: 10.0,
            ..base_ctx()
        };
        let bt = BtNode::FleeFromEntity {
            filter: EntityFilter::NegativeRelationship,
            speed_factor: 1.5,
        };
        let (status, _) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn entity_filter_any_matches_all() {
        let kin = make_social_entity(1, 0.0, 0.0, 5.0, true, 0.5);
        let non_kin = make_social_entity(2, 0.0, 0.0, 5.0, false, -0.5);
        assert!(entity_filter_matches(&EntityFilter::Any, &kin));
        assert!(entity_filter_matches(&EntityFilter::Any, &non_kin));
    }

    #[test]
    fn entity_filter_kin_only_matches_kin() {
        let kin = make_social_entity(1, 0.0, 0.0, 5.0, true, 0.0);
        let non_kin = make_social_entity(2, 0.0, 0.0, 5.0, false, 0.0);
        assert!(entity_filter_matches(&EntityFilter::Kin, &kin));
        assert!(!entity_filter_matches(&EntityFilter::Kin, &non_kin));
    }

    #[test]
    fn entity_filter_positive_uses_threshold() {
        let slightly_positive = make_social_entity(1, 0.0, 0.0, 5.0, true, 0.05);
        let above_threshold = make_social_entity(2, 0.0, 0.0, 5.0, true, 0.2);
        assert!(!entity_filter_matches(
            &EntityFilter::PositiveRelationship,
            &slightly_positive
        ));
        assert!(entity_filter_matches(
            &EntityFilter::PositiveRelationship,
            &above_threshold
        ));
    }

    #[test]
    fn entity_filter_negative_uses_threshold() {
        let slightly_negative = make_social_entity(1, 0.0, 0.0, 5.0, false, -0.05);
        let below_threshold = make_social_entity(2, 0.0, 0.0, 5.0, false, -0.3);
        assert!(!entity_filter_matches(
            &EntityFilter::NegativeRelationship,
            &slightly_negative
        ));
        assert!(entity_filter_matches(
            &EntityFilter::NegativeRelationship,
            &below_threshold
        ));
    }

    #[test]
    fn social_starter_bt_seeks_kin_when_social_need_high() {
        let ctx = BtContext {
            social_need: 0.6,
            social_entities: vec![make_social_entity(10, 70.0, 50.0, 20.0, true, 0.3)],
            has_nearby_entity: true,
            closest_entity_distance: 20.0,
            ..base_ctx()
        };
        let bt = social_starter_bt();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        match action {
            BtAction::MoveTowardEntity { entity_id, x, y, .. } => {
                assert_eq!(entity_id, 10);
                assert_eq!(x, 70.0);
                assert_eq!(y, 50.0);
            }
            other => panic!("expected MoveTowardEntity, got {:?}", other),
        }
    }

    #[test]
    fn social_starter_bt_flees_negative_entity() {
        let ctx = BtContext {
            social_entities: vec![make_social_entity(20, 30.0, 30.0, 15.0, false, -0.6)],
            has_nearby_entity: true,
            closest_entity_distance: 15.0,
            ..base_ctx()
        };
        let bt = social_starter_bt();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        match action {
            BtAction::FleeFromEntity { entity_id, .. } => {
                assert_eq!(entity_id, 20);
            }
            other => panic!("expected FleeFromEntity, got {:?}", other),
        }
    }

    #[test]
    fn closest_filtered_entity_picks_nearest() {
        let ctx = BtContext {
            social_entities: vec![
                make_social_entity(1, 100.0, 50.0, 50.0, true, 0.5),
                make_social_entity(2, 60.0, 50.0, 10.0, true, 0.3),
                make_social_entity(3, 80.0, 50.0, 30.0, true, 0.7),
            ],
            ..base_ctx()
        };
        let closest = ctx.closest_filtered_entity(&EntityFilter::PositiveRelationship, f64::MAX).unwrap();
        assert_eq!(closest.entity_id, 2, "should pick the nearest entity");
    }

    #[test]
    fn serialization_roundtrip_social_nodes() {
        let bt = social_starter_bt();
        let json = serde_json::to_string(&bt).unwrap();
        let deserialized: BtNode = serde_json::from_str(&json).unwrap();
        assert_eq!(bt, deserialized);
    }

    #[test]
    fn serialization_roundtrip_entity_filter() {
        let filters = vec![
            EntityFilter::Any,
            EntityFilter::Kin,
            EntityFilter::NonKin,
            EntityFilter::PositiveRelationship,
            EntityFilter::NegativeRelationship,
        ];
        for filter in &filters {
            let json = serde_json::to_string(filter).unwrap();
            let d: EntityFilter = serde_json::from_str(&json).unwrap();
            assert_eq!(&d, filter);
        }
    }
}

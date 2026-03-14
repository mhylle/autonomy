use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::arcs::{ArcDetector, StoryArc};
use super::biography::{Biography, BiographyCompiler};
use super::significance;
use crate::events::types::SimEvent;

/// Maximum number of ticks of attack data to keep in arc detection.
const ARC_DETECTION_WINDOW: u64 = 1000;

/// How often (in ticks) to prune old biography and arc data.
const PRUNE_INTERVAL: u64 = 500;

/// Maximum number of dead entity biographies to retain.
const MAX_DEAD_BIOGRAPHIES: usize = 200;

/// How often (in ticks) to check alliances from relationship data.
const ALLIANCE_CHECK_INTERVAL: u64 = 50;

/// Entity IDs being tracked as narratively interesting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackedEntities {
    /// Manually bookmarked entities.
    pub bookmarked: HashSet<u64>,
    /// Auto-tracked entities (by interest score).
    pub auto_tracked: HashSet<u64>,
}

impl TrackedEntities {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bookmark an entity for tracking.
    pub fn bookmark(&mut self, entity_id: u64) {
        self.bookmarked.insert(entity_id);
    }

    /// Remove a bookmark.
    pub fn unbookmark(&mut self, entity_id: u64) {
        self.bookmarked.remove(&entity_id);
    }

    /// Check if an entity is tracked (either bookmarked or auto-tracked).
    pub fn is_tracked(&self, entity_id: u64) -> bool {
        self.bookmarked.contains(&entity_id) || self.auto_tracked.contains(&entity_id)
    }

    /// All tracked entity IDs.
    pub fn all_tracked(&self) -> HashSet<u64> {
        self.bookmarked.union(&self.auto_tracked).cloned().collect()
    }
}

/// Search criteria for querying the event history.
#[derive(Debug, Clone, Default)]
pub struct EventSearchCriteria {
    /// Filter by entity ID involvement.
    pub entity_id: Option<u64>,
    /// Filter by tick range (inclusive).
    pub tick_range: Option<(u64, u64)>,
    /// Minimum significance score.
    pub min_significance: Option<f64>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

/// A scored event for search results.
#[derive(Debug, Clone)]
pub struct ScoredEvent {
    pub tick: u64,
    pub event: SimEvent,
    pub significance: f64,
}

/// Central narrative tracking system.
///
/// This is an observer/analyzer layer that reads simulation events and entity
/// state but never modifies the simulation itself. It tracks interesting entities,
/// detects story arcs, compiles biographies, and provides event search.
#[derive(Debug, Clone)]
pub struct NarrativeTracker {
    pub tracked: TrackedEntities,
    arc_detector: ArcDetector,
    biography_compiler: BiographyCompiler,
    /// Significant events with their tick and score, for search.
    event_history: Vec<ScoredEvent>,
    /// Maximum number of events to keep in history.
    max_history_size: usize,
}

impl Default for NarrativeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl NarrativeTracker {
    pub fn new() -> Self {
        Self {
            tracked: TrackedEntities::new(),
            arc_detector: ArcDetector::new(),
            biography_compiler: BiographyCompiler::new(),
            event_history: Vec::new(),
            max_history_size: 10_000,
        }
    }

    /// Process a tick's events through all narrative subsystems.
    ///
    /// This is the main entry point called from the tick pipeline.
    pub fn process_tick(
        &mut self,
        events: &[SimEvent],
        tick: u64,
        species_populations: &HashMap<u64, u32>,
        entity_stats: &[EntityStats],
    ) {
        // 1. Score and store significant events
        for event in events {
            let sig = significance::score_event(event);
            if sig >= significance::SIGNIFICANCE_THRESHOLD {
                self.event_history.push(ScoredEvent {
                    tick,
                    event: event.clone(),
                    significance: sig,
                });
            }
        }

        // 2. Update arc detection
        self.arc_detector.process_events(events, tick);
        self.arc_detector
            .update_species_populations(species_populations, tick);

        // 3. Update biographies
        self.biography_compiler.process_events(events, tick);

        // 4. Auto-track interesting entities
        self.update_auto_tracking(entity_stats);

        // 5. Periodic maintenance
        if tick % PRUNE_INTERVAL == 0 {
            self.prune(tick);
        }

        // 6. Check alliances from entity stats
        if tick % ALLIANCE_CHECK_INTERVAL == 0 {
            self.check_alliances_from_stats(entity_stats, tick);
        }
    }

    /// Search event history by criteria.
    pub fn search_events(&self, criteria: &EventSearchCriteria) -> Vec<&ScoredEvent> {
        let mut results: Vec<&ScoredEvent> = self
            .event_history
            .iter()
            .filter(|se| {
                // Tick range filter
                if let Some((start, end)) = criteria.tick_range {
                    if se.tick < start || se.tick > end {
                        return false;
                    }
                }

                // Significance filter
                if let Some(min_sig) = criteria.min_significance {
                    if se.significance < min_sig {
                        return false;
                    }
                }

                // Entity filter
                if let Some(entity_id) = criteria.entity_id {
                    if !event_involves_entity(&se.event, entity_id) {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Sort by significance descending
        results.sort_by(|a, b| {
            b.significance
                .partial_cmp(&a.significance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(limit) = criteria.limit {
            results.truncate(limit);
        }

        results
    }

    /// Get all detected story arcs.
    pub fn arcs(&self) -> &[StoryArc] {
        self.arc_detector.arcs()
    }

    /// Get only active story arcs.
    pub fn active_arcs(&self) -> Vec<&StoryArc> {
        self.arc_detector.active_arcs()
    }

    /// Get an entity's biography.
    pub fn biography(&self, entity_id: u64) -> Option<&Biography> {
        self.biography_compiler.get(entity_id)
    }

    /// Get the top entities by legacy score.
    pub fn top_legacies(&self, limit: usize) -> Vec<(&u64, &Biography)> {
        self.biography_compiler.top_legacies(limit)
    }

    /// Get the number of significant events in history.
    pub fn event_history_len(&self) -> usize {
        self.event_history.len()
    }

    /// Update relationship data for biography tracking.
    pub fn update_entity_relationships(&mut self, entity_id: u64, relationships: &[(u64, f64)]) {
        self.biography_compiler
            .update_relationships(entity_id, relationships);
    }

    // --- Private helpers ---

    fn update_auto_tracking(&mut self, entity_stats: &[EntityStats]) {
        self.tracked.auto_tracked.clear();
        for stats in entity_stats {
            let interest = significance::entity_interest_score(
                stats.age,
                stats.offspring_count,
                stats.kill_count,
                stats.distance_traveled,
                stats.relationship_count,
            );
            if interest >= significance::ENTITY_INTEREST_THRESHOLD {
                self.tracked.auto_tracked.insert(stats.entity_id);
            }
        }
    }

    fn check_alliances_from_stats(&mut self, entity_stats: &[EntityStats], tick: u64) {
        for stats in entity_stats {
            for &(other_id, valence) in &stats.relationships {
                // Only check each pair once (lower id reports)
                if stats.entity_id < other_id {
                    self.arc_detector
                        .check_alliance(stats.entity_id, other_id, valence > 0.3, tick);
                }
            }
        }
    }

    fn prune(&mut self, tick: u64) {
        // Prune arc detection attack records
        self.arc_detector.prune_old_records(tick, ARC_DETECTION_WINDOW);

        // Prune event history
        if self.event_history.len() > self.max_history_size {
            let excess = self.event_history.len() - self.max_history_size;
            self.event_history.drain(0..excess);
        }

        // Prune dead biographies
        self.biography_compiler.prune(MAX_DEAD_BIOGRAPHIES);
    }
}

/// Summary statistics for an entity, used by the narrative tracker.
///
/// This is a read-only snapshot provided by the tick pipeline;
/// the narrative system never modifies simulation state.
#[derive(Debug, Clone)]
pub struct EntityStats {
    pub entity_id: u64,
    pub age: u64,
    pub offspring_count: u32,
    pub kill_count: u32,
    pub distance_traveled: f64,
    pub relationship_count: u32,
    pub relationships: Vec<(u64, f64)>,
    pub species_id: u64,
}

/// Check if an event involves a specific entity.
fn event_involves_entity(event: &SimEvent, entity_id: u64) -> bool {
    match event {
        SimEvent::EntitySpawned {
            entity_id: eid,
            parent_id,
            ..
        } => *eid == entity_id || *parent_id == Some(entity_id),
        SimEvent::EntityDied {
            entity_id: eid, ..
        } => *eid == entity_id,
        SimEvent::EntityMoved {
            entity_id: eid, ..
        } => *eid == entity_id,
        SimEvent::EntityAte {
            entity_id: eid, ..
        } => *eid == entity_id,
        SimEvent::EntityReproduced {
            parent_id,
            offspring_id,
            ..
        } => *parent_id == entity_id || *offspring_id == entity_id,
        SimEvent::ResourceDepleted { .. } => false,
        SimEvent::ResourceRegrown { .. } => false,
        SimEvent::EntityAttacked {
            attacker_id,
            target_id,
            ..
        } => *attacker_id == entity_id || *target_id == entity_id,
        SimEvent::CompositeReproduced {
            parent_id,
            offspring_id,
            ..
        } => *parent_id == entity_id || *offspring_id == entity_id,
        SimEvent::CompositeFormed {
            leader_id,
            member_id,
            ..
        } => *leader_id == entity_id || *member_id == entity_id,
        SimEvent::CompositeDecomposed {
            leader_id,
            released_member_ids,
            ..
        } => *leader_id == entity_id || released_member_ids.contains(&entity_id),
        SimEvent::WarDeclared { .. } | SimEvent::WarEnded { .. } => false,
    }
}

/// Stub interface for LLM-based narration (Phase 7.5).
///
/// This trait defines the contract for converting narrative data into
/// natural language descriptions. The actual LLM integration is deferred.
pub trait Narrator {
    /// Generate a natural language description of a story arc.
    fn narrate_arc(&self, arc: &StoryArc) -> String;

    /// Generate a natural language biography summary.
    fn narrate_biography(&self, biography: &Biography) -> String;

    /// Generate a description for the current simulation state.
    fn narrate_current_state(
        &self,
        active_arcs: &[&StoryArc],
        tracked_entities: &TrackedEntities,
    ) -> String;
}

/// Stub narrator that returns placeholder text.
pub struct StubNarrator;

impl Narrator for StubNarrator {
    fn narrate_arc(&self, arc: &StoryArc) -> String {
        format!(
            "[Stub] {:?} arc involving {:?}, started at tick {}",
            arc.arc_type, arc.protagonists, arc.start_tick
        )
    }

    fn narrate_biography(&self, biography: &Biography) -> String {
        format!(
            "[Stub] Entity {} born at tick {}, {} offspring, {} kills",
            biography.entity_id,
            biography.birth_tick,
            biography.offspring_count,
            biography.kill_count
        )
    }

    fn narrate_current_state(
        &self,
        active_arcs: &[&StoryArc],
        tracked_entities: &TrackedEntities,
    ) -> String {
        format!(
            "[Stub] {} active arcs, {} tracked entities",
            active_arcs.len(),
            tracked_entities.all_tracked().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::DeathCause;
    use crate::narrative::ArcType;

    fn make_spawn(id: u64) -> SimEvent {
        SimEvent::EntitySpawned {
            entity_id: id,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        }
    }

    fn make_attack(attacker: u64, target: u64) -> SimEvent {
        SimEvent::EntityAttacked {
            attacker_id: attacker,
            target_id: target,
            damage: 10.0,
            target_health_remaining: 90.0,
        }
    }

    #[test]
    fn tracker_processes_events_and_stores_significant() {
        let mut tracker = NarrativeTracker::new();
        let events = vec![
            make_spawn(1),
            SimEvent::EntityDied {
                entity_id: 2,
                x: 0.0,
                y: 0.0,
                age: 100,
                cause: DeathCause::Combat { killer_id: 1 },
            },
        ];
        tracker.process_tick(&events, 1, &HashMap::new(), &[]);
        // Combat death is significant, spawn with parent_id=None has score 0.2 < 0.25
        assert!(tracker.event_history_len() >= 1);
    }

    #[test]
    fn event_search_by_entity() {
        let mut tracker = NarrativeTracker::new();
        let events = vec![
            SimEvent::EntityDied {
                entity_id: 1,
                x: 0.0,
                y: 0.0,
                age: 100,
                cause: DeathCause::Combat { killer_id: 2 },
            },
            SimEvent::EntityReproduced {
                parent_id: 3,
                offspring_id: 4,
                x: 0.0,
                y: 0.0,
            },
        ];
        tracker.process_tick(&events, 10, &HashMap::new(), &[]);

        let results = tracker.search_events(&EventSearchCriteria {
            entity_id: Some(1),
            ..Default::default()
        });
        assert!(!results.is_empty());
        for r in &results {
            assert!(event_involves_entity(&r.event, 1));
        }
    }

    #[test]
    fn event_search_by_tick_range() {
        let mut tracker = NarrativeTracker::new();
        for tick in 1..=10 {
            let events = vec![SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 100 + tick,
                x: 0.0,
                y: 0.0,
            }];
            tracker.process_tick(&events, tick, &HashMap::new(), &[]);
        }

        let results = tracker.search_events(&EventSearchCriteria {
            tick_range: Some((5, 8)),
            ..Default::default()
        });
        for r in &results {
            assert!(r.tick >= 5 && r.tick <= 8);
        }
    }

    #[test]
    fn event_search_with_limit() {
        let mut tracker = NarrativeTracker::new();
        for tick in 1..=20 {
            let events = vec![SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 100 + tick,
                x: 0.0,
                y: 0.0,
            }];
            tracker.process_tick(&events, tick, &HashMap::new(), &[]);
        }

        let results = tracker.search_events(&EventSearchCriteria {
            limit: Some(5),
            ..Default::default()
        });
        assert!(results.len() <= 5);
    }

    #[test]
    fn tracked_entities_bookmark() {
        let mut tracked = TrackedEntities::new();
        tracked.bookmark(42);
        assert!(tracked.is_tracked(42));
        assert!(!tracked.is_tracked(99));

        tracked.unbookmark(42);
        assert!(!tracked.is_tracked(42));
    }

    #[test]
    fn auto_tracking_updates_from_stats() {
        let mut tracker = NarrativeTracker::new();
        let stats = vec![EntityStats {
            entity_id: 1,
            age: 3000,
            offspring_count: 8,
            kill_count: 5,
            distance_traveled: 5000.0,
            relationship_count: 5,
            relationships: vec![],
            species_id: 100,
        }];
        tracker.process_tick(&[], 1, &HashMap::new(), &stats);
        assert!(tracker.tracked.is_tracked(1));
    }

    #[test]
    fn stub_narrator_returns_placeholder() {
        let narrator = StubNarrator;

        let arc = StoryArc {
            arc_type: ArcType::Rivalry,
            protagonists: vec![1, 2],
            start_tick: 0,
            end_tick: None,
            tension_curve: vec![],
            key_event_ticks: vec![],
            active: true,
        };
        let text = narrator.narrate_arc(&arc);
        assert!(text.contains("[Stub]"));

        let bio = Biography::new(1, 0);
        let text = narrator.narrate_biography(&bio);
        assert!(text.contains("[Stub]"));
    }

    #[test]
    fn event_involves_entity_checks_all_variants() {
        assert!(event_involves_entity(&make_spawn(1), 1));
        assert!(!event_involves_entity(&make_spawn(1), 2));
        assert!(event_involves_entity(&make_attack(1, 2), 1));
        assert!(event_involves_entity(&make_attack(1, 2), 2));
        assert!(!event_involves_entity(&make_attack(1, 2), 3));

        let reproduced = SimEvent::EntityReproduced {
            parent_id: 1,
            offspring_id: 2,
            x: 0.0,
            y: 0.0,
        };
        assert!(event_involves_entity(&reproduced, 1));
        assert!(event_involves_entity(&reproduced, 2));
        assert!(!event_involves_entity(&reproduced, 3));

        // Resource events never involve entities
        let resource = SimEvent::ResourceDepleted {
            resource_id: 1,
            x: 0.0,
            y: 0.0,
        };
        assert!(!event_involves_entity(&resource, 1));
    }

    #[test]
    fn narrative_tracker_default_works() {
        let tracker = NarrativeTracker::default();
        assert_eq!(tracker.event_history_len(), 0);
        assert!(tracker.arcs().is_empty());
    }
}

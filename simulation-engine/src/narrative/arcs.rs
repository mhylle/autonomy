use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::events::types::{DeathCause, SimEvent};

/// Types of narrative arcs that can be detected from event patterns.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArcType {
    /// Two entities repeatedly attacking each other.
    Rivalry,
    /// Two entities maintaining positive relationships over time.
    Alliance,
    /// A species population dropping to zero.
    Extinction,
    /// An entity accumulating many offspring and kills -- a dominant force.
    Rise,
    /// A previously dominant entity dying.
    Fall,
    /// An entity traveling a large cumulative distance.
    Migration,
}

/// A detected story arc with its participants and progression data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryArc {
    pub arc_type: ArcType,
    /// Primary entities involved in this arc.
    pub protagonists: Vec<u64>,
    /// Tick when this arc was first detected.
    pub start_tick: u64,
    /// Tick when this arc ended (None if ongoing).
    pub end_tick: Option<u64>,
    /// Tension curve: (tick, tension_value) pairs in `[0.0, 1.0]`.
    pub tension_curve: Vec<(u64, f64)>,
    /// Key event ticks that define this arc.
    pub key_event_ticks: Vec<u64>,
    /// Whether this arc is still active.
    pub active: bool,
}

impl StoryArc {
    fn new(arc_type: ArcType, protagonists: Vec<u64>, start_tick: u64) -> Self {
        Self {
            arc_type,
            protagonists,
            start_tick,
            end_tick: None,
            tension_curve: vec![(start_tick, 0.3)],
            key_event_ticks: vec![start_tick],
            active: true,
        }
    }

    /// Add a tension data point to this arc.
    pub fn add_tension(&mut self, tick: u64, tension: f64) {
        self.tension_curve.push((tick, tension.clamp(0.0, 1.0)));
    }

    /// Mark this arc as completed.
    pub fn complete(&mut self, tick: u64) {
        self.end_tick = Some(tick);
        self.active = false;
        self.tension_curve.push((tick, 0.0));
    }

    /// Current tension level (last recorded value).
    pub fn current_tension(&self) -> f64 {
        self.tension_curve.last().map(|(_, t)| *t).unwrap_or(0.0)
    }
}

/// Tracks intermediate state needed for arc detection.
#[derive(Debug, Clone, Default)]
pub struct ArcDetector {
    /// (attacker, target) -> list of ticks when attacks occurred.
    attack_pairs: HashMap<(u64, u64), Vec<u64>>,
    /// entity_id -> cumulative offspring count.
    offspring_counts: HashMap<u64, u32>,
    /// entity_id -> cumulative kill count.
    kill_counts: HashMap<u64, u32>,
    /// entity_id -> cumulative distance traveled.
    distances: HashMap<u64, f64>,
    /// species_id -> last known population count.
    species_populations: HashMap<u64, u32>,
    /// (entity_a, entity_b) -> tick when positive relationship was first noted.
    alliance_starts: HashMap<(u64, u64), u64>,
    /// Detected arcs.
    arcs: Vec<StoryArc>,
    /// Set of entity pairs already in a Rivalry arc (to avoid duplicates).
    active_rivalries: HashMap<(u64, u64), usize>,
    /// Set of entity pairs already in an Alliance arc.
    active_alliances: HashMap<(u64, u64), usize>,
    /// Entities with active Rise arcs.
    active_rises: HashMap<u64, usize>,
}

/// Minimum mutual attacks to declare a rivalry.
const RIVALRY_THRESHOLD: usize = 3;
/// Minimum ticks of positive relationship to declare an alliance.
const ALLIANCE_DURATION_THRESHOLD: u64 = 500;
/// Minimum offspring + kills to detect a Rise arc.
const RISE_THRESHOLD: u32 = 5;
/// Minimum cumulative distance to detect a Migration arc.
const MIGRATION_DISTANCE_THRESHOLD: f64 = 5000.0;

impl ArcDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a batch of events from a single tick and detect new arcs.
    ///
    /// Returns newly detected arcs from this tick.
    pub fn process_events(&mut self, events: &[SimEvent], tick: u64) -> Vec<StoryArc> {
        let mut new_arcs = Vec::new();

        for event in events {
            match event {
                SimEvent::EntityAttacked {
                    attacker_id,
                    target_id,
                    ..
                } => {
                    self.record_attack(*attacker_id, *target_id, tick);
                    if let Some(arc) = self.check_rivalry(*attacker_id, *target_id, tick) {
                        new_arcs.push(arc);
                    }
                }
                SimEvent::EntityReproduced {
                    parent_id,
                    offspring_id: _,
                    ..
                } => {
                    *self.offspring_counts.entry(*parent_id).or_insert(0) += 1;
                    if let Some(arc) = self.check_rise(*parent_id, tick) {
                        new_arcs.push(arc);
                    }
                }
                SimEvent::EntityDied {
                    entity_id, cause, ..
                } => {
                    // Check for Fall arc (death of a dominant entity)
                    if let Some(arc) = self.check_fall(*entity_id, tick) {
                        new_arcs.push(arc);
                    }

                    // Track kills
                    if let DeathCause::Combat { killer_id } = cause {
                        *self.kill_counts.entry(*killer_id).or_insert(0) += 1;
                        if let Some(arc) = self.check_rise(*killer_id, tick) {
                            new_arcs.push(arc);
                        }
                    }

                    // Complete any active arcs involving this entity
                    self.on_entity_death(*entity_id, tick);
                }
                SimEvent::EntityMoved {
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                } => {
                    let dx = to_x - from_x;
                    let dy = to_y - from_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    *self.distances.entry(*entity_id).or_insert(0.0) += dist;
                    if let Some(arc) = self.check_migration(*entity_id, tick) {
                        new_arcs.push(arc);
                    }
                }
                _ => {}
            }
        }

        // Store newly detected arcs
        for arc in &new_arcs {
            self.arcs.push(arc.clone());
        }

        new_arcs
    }

    /// Update species population counts, detecting extinctions.
    pub fn update_species_populations(
        &mut self,
        populations: &HashMap<u64, u32>,
        tick: u64,
    ) -> Vec<StoryArc> {
        let mut new_arcs = Vec::new();

        for (species_id, &count) in populations {
            let prev = self.species_populations.get(species_id).copied().unwrap_or(0);
            if prev > 0 && count == 0 {
                // Species went extinct
                let arc = StoryArc {
                    arc_type: ArcType::Extinction,
                    protagonists: vec![*species_id],
                    start_tick: tick,
                    end_tick: Some(tick),
                    tension_curve: vec![(tick, 1.0), (tick, 0.0)],
                    key_event_ticks: vec![tick],
                    active: false,
                };
                new_arcs.push(arc.clone());
                self.arcs.push(arc);
            }
        }

        // Update stored populations
        self.species_populations = populations.clone();

        new_arcs
    }

    /// Check for alliance formation based on positive relationship duration.
    pub fn check_alliance(
        &mut self,
        entity_a: u64,
        entity_b: u64,
        relationship_positive: bool,
        tick: u64,
    ) -> Option<StoryArc> {
        let key = normalize_pair(entity_a, entity_b);

        if relationship_positive {
            let start = *self.alliance_starts.entry(key).or_insert(tick);
            let duration = tick.saturating_sub(start);

            if duration >= ALLIANCE_DURATION_THRESHOLD && !self.active_alliances.contains_key(&key)
            {
                let arc = StoryArc::new(ArcType::Alliance, vec![key.0, key.1], start);
                let idx = self.arcs.len();
                self.active_alliances.insert(key, idx);
                return Some(arc);
            }
        } else {
            // Relationship turned negative -- remove alliance tracking
            self.alliance_starts.remove(&key);
            if let Some(&idx) = self.active_alliances.get(&key) {
                if let Some(arc) = self.arcs.get_mut(idx) {
                    arc.complete(tick);
                }
                self.active_alliances.remove(&key);
            }
        }

        None
    }

    /// All detected arcs (active and completed).
    pub fn arcs(&self) -> &[StoryArc] {
        &self.arcs
    }

    /// Only active (ongoing) arcs.
    pub fn active_arcs(&self) -> Vec<&StoryArc> {
        self.arcs.iter().filter(|a| a.active).collect()
    }

    /// Prune old attack records to keep memory bounded.
    /// Removes attack records older than `max_age` ticks from the current tick.
    pub fn prune_old_records(&mut self, current_tick: u64, max_age: u64) {
        let cutoff = current_tick.saturating_sub(max_age);
        for ticks in self.attack_pairs.values_mut() {
            ticks.retain(|&t| t >= cutoff);
        }
        self.attack_pairs.retain(|_, ticks| !ticks.is_empty());
    }

    // --- Private helpers ---

    fn record_attack(&mut self, attacker: u64, target: u64, tick: u64) {
        self.attack_pairs
            .entry((attacker, target))
            .or_default()
            .push(tick);
    }

    fn check_rivalry(&mut self, attacker: u64, target: u64, tick: u64) -> Option<StoryArc> {
        let key = normalize_pair(attacker, target);

        if self.active_rivalries.contains_key(&key) {
            // Update tension on existing rivalry
            let idx = self.active_rivalries[&key];
            let total_attacks = self.mutual_attack_count(key.0, key.1);
            let tension = (total_attacks as f64 / 10.0).min(1.0);
            if let Some(arc) = self.arcs.get_mut(idx) {
                arc.add_tension(tick, tension);
                arc.key_event_ticks.push(tick);
            }
            return None;
        }

        let total = self.mutual_attack_count(key.0, key.1);
        if total >= RIVALRY_THRESHOLD {
            let first_tick = self.earliest_attack_tick(key.0, key.1).unwrap_or(tick);
            let arc = StoryArc::new(ArcType::Rivalry, vec![key.0, key.1], first_tick);
            let idx = self.arcs.len(); // will be pushed in process_events
            self.active_rivalries.insert(key, idx);
            Some(arc)
        } else {
            None
        }
    }

    fn mutual_attack_count(&self, a: u64, b: u64) -> usize {
        let ab = self.attack_pairs.get(&(a, b)).map_or(0, |v| v.len());
        let ba = self.attack_pairs.get(&(b, a)).map_or(0, |v| v.len());
        ab + ba
    }

    fn earliest_attack_tick(&self, a: u64, b: u64) -> Option<u64> {
        let ab = self.attack_pairs.get(&(a, b)).and_then(|v| v.first().copied());
        let ba = self.attack_pairs.get(&(b, a)).and_then(|v| v.first().copied());
        match (ab, ba) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        }
    }

    fn check_rise(&mut self, entity_id: u64, tick: u64) -> Option<StoryArc> {
        if self.active_rises.contains_key(&entity_id) {
            // Update tension
            let idx = self.active_rises[&entity_id];
            let dominance = self.entity_dominance(entity_id);
            let tension = (dominance as f64 / 15.0).min(1.0);
            if let Some(arc) = self.arcs.get_mut(idx) {
                arc.add_tension(tick, tension);
            }
            return None;
        }

        let dominance = self.entity_dominance(entity_id);
        if dominance >= RISE_THRESHOLD {
            let arc = StoryArc::new(ArcType::Rise, vec![entity_id], tick);
            let idx = self.arcs.len();
            self.active_rises.insert(entity_id, idx);
            Some(arc)
        } else {
            None
        }
    }

    fn entity_dominance(&self, entity_id: u64) -> u32 {
        let offspring = self.offspring_counts.get(&entity_id).copied().unwrap_or(0);
        let kills = self.kill_counts.get(&entity_id).copied().unwrap_or(0);
        offspring + kills
    }

    fn check_fall(&mut self, entity_id: u64, tick: u64) -> Option<StoryArc> {
        let dominance = self.entity_dominance(entity_id);
        if dominance >= RISE_THRESHOLD {
            let mut arc = StoryArc::new(ArcType::Fall, vec![entity_id], tick);
            arc.add_tension(tick, 1.0);
            arc.complete(tick);
            // Also complete any active Rise arc for this entity
            if let Some(&idx) = self.active_rises.get(&entity_id) {
                if let Some(rise_arc) = self.arcs.get_mut(idx) {
                    rise_arc.complete(tick);
                }
                self.active_rises.remove(&entity_id);
            }
            Some(arc)
        } else {
            None
        }
    }

    fn check_migration(&mut self, entity_id: u64, _tick: u64) -> Option<StoryArc> {
        let distance = self.distances.get(&entity_id).copied().unwrap_or(0.0);
        if distance >= MIGRATION_DISTANCE_THRESHOLD {
            // Only emit once; remove from tracking
            self.distances.remove(&entity_id);
            let mut arc = StoryArc::new(ArcType::Migration, vec![entity_id], _tick);
            arc.add_tension(_tick, 0.5);
            arc.complete(_tick);
            Some(arc)
        } else {
            None
        }
    }

    fn on_entity_death(&mut self, entity_id: u64, tick: u64) {
        // Complete active rivalries
        let rivalry_keys: Vec<(u64, u64)> = self
            .active_rivalries
            .keys()
            .filter(|(a, b)| *a == entity_id || *b == entity_id)
            .cloned()
            .collect();
        for key in rivalry_keys {
            if let Some(&idx) = self.active_rivalries.get(&key) {
                if let Some(arc) = self.arcs.get_mut(idx) {
                    arc.complete(tick);
                }
            }
            self.active_rivalries.remove(&key);
        }

        // Complete active alliances
        let alliance_keys: Vec<(u64, u64)> = self
            .active_alliances
            .keys()
            .filter(|(a, b)| *a == entity_id || *b == entity_id)
            .cloned()
            .collect();
        for key in alliance_keys {
            if let Some(&idx) = self.active_alliances.get(&key) {
                if let Some(arc) = self.arcs.get_mut(idx) {
                    arc.complete(tick);
                }
            }
            self.active_alliances.remove(&key);
        }

        // Clean up tracking data
        self.distances.remove(&entity_id);
    }
}

/// Normalize a pair of entity IDs so (a, b) and (b, a) map to the same key.
fn normalize_pair(a: u64, b: u64) -> (u64, u64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rivalry_detected_after_mutual_attacks() {
        let mut detector = ArcDetector::new();

        // Entity 1 attacks entity 2 twice
        let events1 = vec![SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 10.0,
            target_health_remaining: 90.0,
        }];
        assert!(detector.process_events(&events1, 1).is_empty());
        assert!(detector.process_events(&events1, 2).is_empty());

        // Entity 2 attacks entity 1 once -- total mutual attacks = 3
        let events2 = vec![SimEvent::EntityAttacked {
            attacker_id: 2,
            target_id: 1,
            damage: 10.0,
            target_health_remaining: 90.0,
        }];
        let new_arcs = detector.process_events(&events2, 3);
        assert_eq!(new_arcs.len(), 1);
        assert_eq!(new_arcs[0].arc_type, ArcType::Rivalry);
        assert!(new_arcs[0].protagonists.contains(&1));
        assert!(new_arcs[0].protagonists.contains(&2));
    }

    #[test]
    fn no_rivalry_with_insufficient_attacks() {
        let mut detector = ArcDetector::new();

        let events = vec![SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 10.0,
            target_health_remaining: 90.0,
        }];
        let new_arcs = detector.process_events(&events, 1);
        assert!(new_arcs.is_empty());
    }

    #[test]
    fn extinction_detected_when_species_reaches_zero() {
        let mut detector = ArcDetector::new();

        let mut pop = HashMap::new();
        pop.insert(100, 5);
        detector.update_species_populations(&pop, 10);

        pop.insert(100, 0);
        let new_arcs = detector.update_species_populations(&pop, 20);
        assert_eq!(new_arcs.len(), 1);
        assert_eq!(new_arcs[0].arc_type, ArcType::Extinction);
        assert!(!new_arcs[0].active);
    }

    #[test]
    fn rise_detected_after_enough_offspring_and_kills() {
        let mut detector = ArcDetector::new();

        // Entity 1 reproduces 5 times
        for tick in 1..=5 {
            let events = vec![SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 100 + tick,
                x: 0.0,
                y: 0.0,
            }];
            let arcs = detector.process_events(&events, tick);
            if tick == 5 {
                assert_eq!(arcs.len(), 1, "Rise should be detected at tick 5");
                assert_eq!(arcs[0].arc_type, ArcType::Rise);
            }
        }
    }

    #[test]
    fn fall_detected_on_dominant_entity_death() {
        let mut detector = ArcDetector::new();

        // Build up dominance first
        for tick in 1..=5 {
            let events = vec![SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 100 + tick,
                x: 0.0,
                y: 0.0,
            }];
            detector.process_events(&events, tick);
        }

        // Entity 1 dies
        let death = vec![SimEvent::EntityDied {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            age: 1000,
            cause: DeathCause::Starvation,
        }];
        let arcs = detector.process_events(&death, 10);
        let fall_arcs: Vec<_> = arcs.iter().filter(|a| a.arc_type == ArcType::Fall).collect();
        assert_eq!(fall_arcs.len(), 1, "Fall arc should be detected");
        assert!(!fall_arcs[0].active);
    }

    #[test]
    fn alliance_detected_after_sustained_positive_relationship() {
        let mut detector = ArcDetector::new();

        // Report positive relationship for 500+ ticks
        for tick in 0..=500 {
            let result = detector.check_alliance(1, 2, true, tick);
            if tick == 500 {
                assert!(result.is_some(), "Alliance should be detected at tick 500");
                let arc = result.unwrap();
                assert_eq!(arc.arc_type, ArcType::Alliance);
            }
        }
    }

    #[test]
    fn alliance_broken_by_negative_relationship() {
        let mut detector = ArcDetector::new();

        // Start positive
        for tick in 0..200 {
            detector.check_alliance(1, 2, true, tick);
        }

        // Relationship turns negative
        detector.check_alliance(1, 2, false, 200);

        // Resume positive -- counter should restart
        for tick in 201..700 {
            let result = detector.check_alliance(1, 2, true, tick);
            if tick < 701 {
                // 201 + 500 = 701, so at tick 700 duration = 499 which is not yet 500
                assert!(result.is_none(), "Alliance should not re-form too early at tick {}", tick);
            }
        }
    }

    #[test]
    fn prune_old_records_removes_stale_attacks() {
        let mut detector = ArcDetector::new();

        let events = vec![SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 10.0,
            target_health_remaining: 90.0,
        }];
        detector.process_events(&events, 100);

        detector.prune_old_records(2000, 1000);
        // Attack at tick 100 is older than 2000 - 1000 = 1000, so should be pruned
        assert_eq!(detector.mutual_attack_count(1, 2), 0);
    }

    #[test]
    fn story_arc_tension_updates() {
        let mut arc = StoryArc::new(ArcType::Rivalry, vec![1, 2], 0);
        assert!((arc.current_tension() - 0.3).abs() < f64::EPSILON);

        arc.add_tension(10, 0.7);
        assert!((arc.current_tension() - 0.7).abs() < f64::EPSILON);

        arc.complete(20);
        assert!(!arc.active);
        assert!((arc.current_tension() - 0.0).abs() < f64::EPSILON);
        assert_eq!(arc.end_tick, Some(20));
    }

    #[test]
    fn normalize_pair_is_symmetric() {
        assert_eq!(normalize_pair(5, 3), normalize_pair(3, 5));
        assert_eq!(normalize_pair(1, 1), (1, 1));
    }

    #[test]
    fn active_arcs_filters_correctly() {
        let mut detector = ArcDetector::new();

        // Create a rivalry (active)
        for tick in 1..=3 {
            let events = vec![SimEvent::EntityAttacked {
                attacker_id: 1,
                target_id: 2,
                damage: 10.0,
                target_health_remaining: 90.0,
            }];
            detector.process_events(&events, tick);
        }

        // Create an extinction (completed)
        let mut pop = HashMap::new();
        pop.insert(100, 5);
        detector.update_species_populations(&pop, 10);
        pop.insert(100, 0);
        detector.update_species_populations(&pop, 20);

        let active = detector.active_arcs();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].arc_type, ArcType::Rivalry);
    }
}

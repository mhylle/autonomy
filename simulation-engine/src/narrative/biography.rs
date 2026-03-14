use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::events::types::{DeathCause, SimEvent};
use super::significance;

/// A life phase classification for biography generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifePhase {
    /// Early life -- first 20% of lifespan.
    Youth,
    /// Prime years -- 20% to 60%.
    Prime,
    /// Later years -- 60% to 100%.
    Elder,
}

/// A single significant event in an entity's biography.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiographyEvent {
    pub tick: u64,
    pub description: BiographyEventKind,
    pub significance: f64,
}

/// Kinds of biographical events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiographyEventKind {
    Born { parent_id: Option<u64> },
    Reproduced { offspring_id: u64 },
    Killed { victim_id: u64 },
    WasKilledBy { killer_id: u64 },
    DiedOfOldAge,
    DiedOfStarvation,
    FormedComposite { partner_id: u64 },
    JoinedComposite { leader_id: u64 },
    CompositeDecomposed,
}

/// A relationship summary for the biography.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipSummary {
    pub entity_id: u64,
    /// Positive = ally, negative = enemy.
    pub final_valence: f64,
    /// Number of interactions recorded.
    pub interaction_count: u32,
}

/// Complete biography data for a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biography {
    pub entity_id: u64,
    pub birth_tick: u64,
    pub death_tick: Option<u64>,
    /// Chronological list of significant events.
    pub events: Vec<BiographyEvent>,
    /// Key relationships this entity had.
    pub relationships: Vec<RelationshipSummary>,
    /// Number of offspring produced.
    pub offspring_count: u32,
    /// Number of kills.
    pub kill_count: u32,
    /// Total distance traveled.
    pub distance_traveled: f64,
}

impl Biography {
    pub fn new(entity_id: u64, birth_tick: u64) -> Self {
        Self {
            entity_id,
            birth_tick,
            death_tick: None,
            events: Vec::new(),
            relationships: Vec::new(),
            offspring_count: 0,
            kill_count: 0,
            distance_traveled: 0.0,
        }
    }

    /// Record a significant event in this entity's biography.
    pub fn record_event(&mut self, tick: u64, kind: BiographyEventKind, sig: f64) {
        self.events.push(BiographyEvent {
            tick,
            description: kind,
            significance: sig,
        });
    }

    /// Determine the life phase at a given tick, based on expected lifespan.
    pub fn life_phase_at(&self, tick: u64, max_lifespan: u64) -> LifePhase {
        let age = tick.saturating_sub(self.birth_tick);
        let fraction = if max_lifespan > 0 {
            age as f64 / max_lifespan as f64
        } else {
            0.0
        };

        if fraction < 0.2 {
            LifePhase::Youth
        } else if fraction < 0.6 {
            LifePhase::Prime
        } else {
            LifePhase::Elder
        }
    }

    /// Is this entity still alive (no death recorded)?
    pub fn is_alive(&self) -> bool {
        self.death_tick.is_none()
    }

    /// Total lifespan in ticks (None if still alive).
    pub fn lifespan(&self) -> Option<u64> {
        self.death_tick.map(|dt| dt - self.birth_tick)
    }

    /// Compute a legacy score based on lifetime achievements.
    pub fn legacy_score(&self) -> f64 {
        let offspring_factor = (self.offspring_count as f64 / 10.0).min(1.0);
        let kill_factor = (self.kill_count as f64 / 5.0).min(1.0);
        let travel_factor = (self.distance_traveled / 10000.0).min(1.0);
        let event_factor = (self.events.len() as f64 / 20.0).min(1.0);
        let relationship_factor = (self.relationships.len() as f64 / 10.0).min(1.0);

        offspring_factor * 0.25
            + kill_factor * 0.25
            + travel_factor * 0.10
            + event_factor * 0.20
            + relationship_factor * 0.20
    }
}

/// Compiler that builds biographies from simulation events.
#[derive(Debug, Clone, Default)]
pub struct BiographyCompiler {
    biographies: HashMap<u64, Biography>,
}

impl BiographyCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process events from a tick, updating entity biographies.
    ///
    /// Spawns and deaths are always tracked (lifecycle events), regardless of
    /// significance score. Other events are filtered by the significance
    /// threshold.
    pub fn process_events(&mut self, events: &[SimEvent], tick: u64) {
        for event in events {
            let sig = significance::score_event(event);

            // Always track lifecycle events (spawn, death) and movement distance.
            // Filter other events by significance.
            let is_lifecycle = matches!(
                event,
                SimEvent::EntitySpawned { .. } | SimEvent::EntityDied { .. }
            );

            if !is_lifecycle && !significance::is_significant(event) {
                // Still track distance even for insignificant events
                if let SimEvent::EntityMoved {
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                } = event
                {
                    let dx = to_x - from_x;
                    let dy = to_y - from_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if let Some(bio) = self.biographies.get_mut(entity_id) {
                        bio.distance_traveled += dist;
                    }
                }
                continue;
            }

            match event {
                SimEvent::EntitySpawned {
                    entity_id,
                    parent_id,
                    ..
                } => {
                    let mut bio = Biography::new(*entity_id, tick);
                    bio.record_event(
                        tick,
                        BiographyEventKind::Born {
                            parent_id: *parent_id,
                        },
                        sig,
                    );
                    self.biographies.insert(*entity_id, bio);
                }
                SimEvent::EntityReproduced {
                    parent_id,
                    offspring_id,
                    ..
                } => {
                    if let Some(bio) = self.biographies.get_mut(parent_id) {
                        bio.offspring_count += 1;
                        bio.record_event(
                            tick,
                            BiographyEventKind::Reproduced {
                                offspring_id: *offspring_id,
                            },
                            sig,
                        );
                    }
                }
                SimEvent::EntityDied {
                    entity_id,
                    cause,
                    ..
                } => {
                    if let Some(bio) = self.biographies.get_mut(entity_id) {
                        bio.death_tick = Some(tick);
                        let kind = match cause {
                            DeathCause::Combat { killer_id } => {
                                BiographyEventKind::WasKilledBy {
                                    killer_id: *killer_id,
                                }
                            }
                            DeathCause::OldAge => BiographyEventKind::DiedOfOldAge,
                            DeathCause::Starvation => BiographyEventKind::DiedOfStarvation,
                        };
                        bio.record_event(tick, kind, sig);
                    }
                    // Record the kill on the killer's biography
                    if let DeathCause::Combat { killer_id } = cause {
                        if let Some(killer_bio) = self.biographies.get_mut(killer_id) {
                            killer_bio.kill_count += 1;
                            killer_bio.record_event(
                                tick,
                                BiographyEventKind::Killed {
                                    victim_id: *entity_id,
                                },
                                sig,
                            );
                        }
                    }
                }
                SimEvent::EntityAttacked { .. } => {
                    // Attacks are tracked through kills, don't duplicate
                }
                SimEvent::CompositeFormed {
                    leader_id,
                    member_id,
                    ..
                } => {
                    if let Some(bio) = self.biographies.get_mut(leader_id) {
                        bio.record_event(
                            tick,
                            BiographyEventKind::FormedComposite {
                                partner_id: *member_id,
                            },
                            sig,
                        );
                    }
                    if let Some(bio) = self.biographies.get_mut(member_id) {
                        bio.record_event(
                            tick,
                            BiographyEventKind::JoinedComposite {
                                leader_id: *leader_id,
                            },
                            sig,
                        );
                    }
                }
                SimEvent::CompositeDecomposed {
                    leader_id,
                    released_member_ids,
                    ..
                } => {
                    if let Some(bio) = self.biographies.get_mut(leader_id) {
                        bio.record_event(tick, BiographyEventKind::CompositeDecomposed, sig);
                    }
                    for mid in released_member_ids {
                        if let Some(bio) = self.biographies.get_mut(mid) {
                            bio.record_event(tick, BiographyEventKind::CompositeDecomposed, sig);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Update relationship summaries for entities from current social data.
    pub fn update_relationships(&mut self, entity_id: u64, relationships: &[(u64, f64)]) {
        if let Some(bio) = self.biographies.get_mut(&entity_id) {
            bio.relationships = relationships
                .iter()
                .map(|&(other_id, valence)| RelationshipSummary {
                    entity_id: other_id,
                    final_valence: valence,
                    interaction_count: 1, // simplified; real count not available here
                })
                .collect();
        }
    }

    /// Get a biography by entity ID.
    pub fn get(&self, entity_id: u64) -> Option<&Biography> {
        self.biographies.get(&entity_id)
    }

    /// Get all biographies.
    pub fn all(&self) -> &HashMap<u64, Biography> {
        &self.biographies
    }

    /// Get biographies sorted by legacy score (highest first).
    pub fn top_legacies(&self, limit: usize) -> Vec<(&u64, &Biography)> {
        let mut entries: Vec<_> = self.biographies.iter().collect();
        entries.sort_by(|(_, a), (_, b)| {
            b.legacy_score()
                .partial_cmp(&a.legacy_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries.truncate(limit);
        entries
    }

    /// Prune biographies for long-dead entities to bound memory.
    /// Keeps the top `keep_count` by legacy score, plus all alive entities.
    pub fn prune(&mut self, keep_count: usize) {
        if self.biographies.len() <= keep_count {
            return;
        }

        let mut entries: Vec<(u64, f64, bool)> = self
            .biographies
            .iter()
            .map(|(&id, bio)| (id, bio.legacy_score(), bio.is_alive()))
            .collect();

        // Sort by: alive first, then by legacy score descending
        entries.sort_by(|(_, score_a, alive_a), (_, score_b, alive_b)| {
            alive_b
                .cmp(alive_a)
                .then(score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal))
        });

        let keep_ids: std::collections::HashSet<u64> =
            entries.iter().take(keep_count).map(|(id, _, _)| *id).collect();

        self.biographies.retain(|id, bio| bio.is_alive() || keep_ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biography_tracks_birth() {
        let mut compiler = BiographyCompiler::new();
        let events = vec![SimEvent::EntitySpawned {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        }];
        compiler.process_events(&events, 0);

        let bio = compiler.get(1).expect("biography should exist");
        assert_eq!(bio.entity_id, 1);
        assert_eq!(bio.birth_tick, 0);
        assert!(bio.is_alive());
    }

    #[test]
    fn biography_tracks_reproduction() {
        let mut compiler = BiographyCompiler::new();
        // Create entity first
        compiler.process_events(
            &[SimEvent::EntitySpawned {
                entity_id: 1,
                x: 0.0,
                y: 0.0,
                generation: 0,
                parent_id: None,
            }],
            0,
        );

        // Record reproduction
        compiler.process_events(
            &[SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 2,
                x: 0.0,
                y: 0.0,
            }],
            100,
        );

        let bio = compiler.get(1).unwrap();
        assert_eq!(bio.offspring_count, 1);
    }

    #[test]
    fn biography_tracks_death() {
        let mut compiler = BiographyCompiler::new();
        compiler.process_events(
            &[SimEvent::EntitySpawned {
                entity_id: 1,
                x: 0.0,
                y: 0.0,
                generation: 0,
                parent_id: None,
            }],
            0,
        );
        compiler.process_events(
            &[SimEvent::EntityDied {
                entity_id: 1,
                x: 0.0,
                y: 0.0,
                age: 500,
                cause: DeathCause::OldAge,
            }],
            500,
        );

        let bio = compiler.get(1).unwrap();
        assert!(!bio.is_alive());
        assert_eq!(bio.death_tick, Some(500));
        assert_eq!(bio.lifespan(), Some(500));
    }

    #[test]
    fn biography_tracks_kills() {
        let mut compiler = BiographyCompiler::new();
        // Spawn both entities
        compiler.process_events(
            &[
                SimEvent::EntitySpawned {
                    entity_id: 1,
                    x: 0.0,
                    y: 0.0,
                    generation: 0,
                    parent_id: None,
                },
                SimEvent::EntitySpawned {
                    entity_id: 2,
                    x: 0.0,
                    y: 0.0,
                    generation: 0,
                    parent_id: None,
                },
            ],
            0,
        );
        // Entity 1 kills entity 2
        compiler.process_events(
            &[SimEvent::EntityDied {
                entity_id: 2,
                x: 0.0,
                y: 0.0,
                age: 100,
                cause: DeathCause::Combat { killer_id: 1 },
            }],
            100,
        );

        let killer_bio = compiler.get(1).unwrap();
        assert_eq!(killer_bio.kill_count, 1);

        let victim_bio = compiler.get(2).unwrap();
        assert!(!victim_bio.is_alive());
    }

    #[test]
    fn life_phase_classification() {
        let bio = Biography::new(1, 0);

        assert_eq!(bio.life_phase_at(50, 1000), LifePhase::Youth);
        assert_eq!(bio.life_phase_at(300, 1000), LifePhase::Prime);
        assert_eq!(bio.life_phase_at(700, 1000), LifePhase::Elder);
    }

    #[test]
    fn legacy_score_increases_with_achievements() {
        let empty = Biography::new(1, 0);
        let empty_score = empty.legacy_score();

        let mut active = Biography::new(2, 0);
        active.offspring_count = 5;
        active.kill_count = 3;
        active.distance_traveled = 5000.0;
        let active_score = active.legacy_score();

        assert!(
            active_score > empty_score,
            "active entity should have higher legacy score: {} vs {}",
            active_score,
            empty_score
        );
    }

    #[test]
    fn top_legacies_returns_sorted() {
        let mut compiler = BiographyCompiler::new();

        let mut bio1 = Biography::new(1, 0);
        bio1.offspring_count = 10;
        bio1.kill_count = 5;
        compiler.biographies.insert(1, bio1);

        let bio2 = Biography::new(2, 0);
        compiler.biographies.insert(2, bio2);

        let mut bio3 = Biography::new(3, 0);
        bio3.offspring_count = 3;
        compiler.biographies.insert(3, bio3);

        let top = compiler.top_legacies(2);
        assert_eq!(top.len(), 2);
        assert_eq!(*top[0].0, 1); // highest legacy score
    }

    #[test]
    fn composite_events_recorded_in_biography() {
        let mut compiler = BiographyCompiler::new();
        compiler.process_events(
            &[
                SimEvent::EntitySpawned {
                    entity_id: 1,
                    x: 0.0,
                    y: 0.0,
                    generation: 0,
                    parent_id: None,
                },
                SimEvent::EntitySpawned {
                    entity_id: 2,
                    x: 0.0,
                    y: 0.0,
                    generation: 0,
                    parent_id: None,
                },
            ],
            0,
        );

        compiler.process_events(
            &[SimEvent::CompositeFormed {
                leader_id: 1,
                member_id: 2,
                x: 0.0,
                y: 0.0,
            }],
            50,
        );

        let bio1 = compiler.get(1).unwrap();
        // Should have born + composite formed = 2 events
        assert!(bio1.events.len() >= 2);

        let bio2 = compiler.get(2).unwrap();
        // Should have born + joined composite = 2 events
        assert!(bio2.events.len() >= 2);
    }
}

use crate::events::types::{DeathCause, SimEvent};

/// Significance score for a simulation event.
///
/// Higher values indicate more narratively interesting events.
/// Scores are in `[0.0, 1.0]` range.
pub fn score_event(event: &SimEvent) -> f64 {
    match event {
        // Births are moderately interesting
        SimEvent::EntitySpawned { parent_id, .. } => {
            if parent_id.is_some() {
                0.3 // offspring birth
            } else {
                0.2 // spontaneous spawn
            }
        }
        // Deaths are significant -- combat kills most of all
        SimEvent::EntityDied { age, cause, .. } => match cause {
            DeathCause::Combat { .. } => 0.8,
            DeathCause::OldAge => {
                // Long-lived entities dying is more notable
                if *age > 3000 {
                    0.6
                } else {
                    0.3
                }
            }
            DeathCause::Starvation => 0.4,
        },
        // Movement is low interest
        SimEvent::EntityMoved { .. } => 0.05,
        // Eating is routine
        SimEvent::EntityAte { .. } => 0.05,
        // Reproduction is meaningful
        SimEvent::EntityReproduced { .. } => 0.4,
        // Resource events are low interest
        SimEvent::ResourceDepleted { .. } => 0.1,
        SimEvent::ResourceRegrown { .. } => 0.02,
        // Attacks are notable
        SimEvent::EntityAttacked {
            target_health_remaining,
            damage,
            ..
        } => {
            // Near-kill attacks are more exciting
            if *target_health_remaining <= 0.0 {
                0.7
            } else if *damage > 30.0 {
                0.5
            } else {
                0.3
            }
        }
        // Composite events are rare and interesting
        SimEvent::CompositeReproduced { .. } => 0.6,
        SimEvent::CompositeFormed { .. } => 0.7,
        SimEvent::CompositeDecomposed { .. } => 0.5,
    }
}

/// Minimum significance threshold for an event to be worth tracking.
pub const SIGNIFICANCE_THRESHOLD: f64 = 0.25;

/// Returns true if the event is significant enough to track.
pub fn is_significant(event: &SimEvent) -> bool {
    score_event(event) >= SIGNIFICANCE_THRESHOLD
}

/// Interest score for an entity based on its life stats.
///
/// Takes counts of notable activities and returns a score in `[0.0, 1.0]`.
pub fn entity_interest_score(
    age: u64,
    offspring_count: u32,
    kill_count: u32,
    distance_traveled: f64,
    relationship_count: u32,
) -> f64 {
    // Normalize each factor to [0, 1] with diminishing returns
    let age_factor = (age as f64 / 5000.0).min(1.0);
    let offspring_factor = (offspring_count as f64 / 10.0).min(1.0);
    let kill_factor = (kill_count as f64 / 5.0).min(1.0);
    let travel_factor = (distance_traveled / 10000.0).min(1.0);
    let social_factor = (relationship_count as f64 / 10.0).min(1.0);

    // Weighted sum
    let raw = age_factor * 0.15
        + offspring_factor * 0.25
        + kill_factor * 0.30
        + travel_factor * 0.10
        + social_factor * 0.20;

    raw.min(1.0)
}

/// Threshold above which an entity is considered "interesting" enough to auto-track.
pub const ENTITY_INTEREST_THRESHOLD: f64 = 0.3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_death_is_highly_significant() {
        let event = SimEvent::EntityDied {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            age: 100,
            cause: DeathCause::Combat { killer_id: 2 },
        };
        let score = score_event(&event);
        assert!(score >= 0.7, "combat death should score >= 0.7, got {}", score);
        assert!(is_significant(&event));
    }

    #[test]
    fn movement_is_low_significance() {
        let event = SimEvent::EntityMoved {
            entity_id: 1,
            from_x: 0.0,
            from_y: 0.0,
            to_x: 1.0,
            to_y: 1.0,
        };
        let score = score_event(&event);
        assert!(score < SIGNIFICANCE_THRESHOLD, "movement should be below threshold, got {}", score);
        assert!(!is_significant(&event));
    }

    #[test]
    fn reproduction_is_significant() {
        let event = SimEvent::EntityReproduced {
            parent_id: 1,
            offspring_id: 2,
            x: 0.0,
            y: 0.0,
        };
        assert!(is_significant(&event));
    }

    #[test]
    fn old_age_death_of_elder_is_notable() {
        let event = SimEvent::EntityDied {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            age: 4000,
            cause: DeathCause::OldAge,
        };
        let score = score_event(&event);
        assert!(score >= 0.5, "elder death should be notable, got {}", score);
    }

    #[test]
    fn resource_regrown_is_not_significant() {
        let event = SimEvent::ResourceRegrown {
            resource_id: 1,
            x: 0.0,
            y: 0.0,
            new_amount: 50.0,
        };
        assert!(!is_significant(&event));
    }

    #[test]
    fn composite_formed_is_significant() {
        let event = SimEvent::CompositeFormed {
            leader_id: 1,
            member_id: 2,
            x: 0.0,
            y: 0.0,
        };
        assert!(is_significant(&event));
        assert!(score_event(&event) >= 0.6);
    }

    #[test]
    fn entity_interest_no_activity_is_low() {
        let score = entity_interest_score(0, 0, 0, 0.0, 0);
        assert!(score < ENTITY_INTEREST_THRESHOLD, "inactive entity should have low interest, got {}", score);
    }

    #[test]
    fn entity_interest_prolific_killer_is_high() {
        let score = entity_interest_score(2000, 5, 10, 5000.0, 5);
        assert!(score >= ENTITY_INTEREST_THRESHOLD, "prolific entity should have high interest, got {}", score);
    }

    #[test]
    fn entity_interest_caps_at_one() {
        let score = entity_interest_score(100_000, 100, 100, 1_000_000.0, 100);
        assert!(score <= 1.0, "entity interest should cap at 1.0, got {}", score);
    }

    #[test]
    fn near_kill_attack_is_more_significant() {
        let near_kill = SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 50.0,
            target_health_remaining: 0.0,
        };
        let light_hit = SimEvent::EntityAttacked {
            attacker_id: 1,
            target_id: 2,
            damage: 5.0,
            target_health_remaining: 80.0,
        };
        assert!(score_event(&near_kill) > score_event(&light_hit));
    }

    #[test]
    fn spawned_with_parent_more_interesting_than_spontaneous() {
        let with_parent = SimEvent::EntitySpawned {
            entity_id: 2,
            x: 0.0,
            y: 0.0,
            generation: 1,
            parent_id: Some(1),
        };
        let spontaneous = SimEvent::EntitySpawned {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        };
        assert!(score_event(&with_parent) > score_event(&spontaneous));
    }
}

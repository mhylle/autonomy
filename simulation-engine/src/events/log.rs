use super::types::SimEvent;
use serde::{Deserialize, Serialize};

/// Per-tick event buffer. Events are appended during a tick and
/// can be drained at the end for persistence or network streaming.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventLog {
    events: Vec<SimEvent>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: SimEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[SimEvent] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Drain all events, clearing the log and returning ownership.
    pub fn drain(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.events)
    }

    /// Clear all events without returning them.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Summary statistics for a single tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSummary {
    pub tick: u64,
    pub event_count: usize,
    pub entity_count: u32,
    pub births: usize,
    pub deaths: usize,
    pub feedings: usize,
}

impl TickSummary {
    pub fn from_events(tick: u64, entity_count: u32, events: &[SimEvent]) -> Self {
        let mut births = 0;
        let mut deaths = 0;
        let mut feedings = 0;
        for event in events {
            match event {
                SimEvent::EntitySpawned { .. } | SimEvent::EntityReproduced { .. } => births += 1,
                SimEvent::EntityDied { .. } => deaths += 1,
                SimEvent::EntityAte { .. } => feedings += 1,
                _ => {}
            }
        }
        Self {
            tick,
            event_count: events.len(),
            entity_count,
            births,
            deaths,
            feedings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::DeathCause;

    #[test]
    fn new_log_is_empty() {
        let log = EventLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn push_increments_len() {
        let mut log = EventLog::new();
        log.push(SimEvent::EntitySpawned {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        });
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn drain_returns_events_and_clears() {
        let mut log = EventLog::new();
        log.push(SimEvent::EntitySpawned {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        });
        log.push(SimEvent::EntityMoved {
            entity_id: 1,
            from_x: 0.0,
            from_y: 0.0,
            to_x: 5.0,
            to_y: 5.0,
        });
        let drained = log.drain();
        assert_eq!(drained.len(), 2);
        assert!(log.is_empty());
    }

    #[test]
    fn clear_removes_all_events() {
        let mut log = EventLog::new();
        log.push(SimEvent::EntitySpawned {
            entity_id: 1,
            x: 0.0,
            y: 0.0,
            generation: 0,
            parent_id: None,
        });
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn events_returns_slice() {
        let mut log = EventLog::new();
        log.push(SimEvent::ResourceDepleted {
            resource_id: 10,
            x: 1.0,
            y: 2.0,
        });
        let events = log.events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn tick_summary_counts_births_deaths_feedings() {
        let events = vec![
            SimEvent::EntitySpawned {
                entity_id: 1,
                x: 0.0,
                y: 0.0,
                generation: 0,
                parent_id: None,
            },
            SimEvent::EntitySpawned {
                entity_id: 2,
                x: 1.0,
                y: 1.0,
                generation: 0,
                parent_id: None,
            },
            SimEvent::EntityDied {
                entity_id: 3,
                x: 5.0,
                y: 5.0,
                age: 100,
                cause: DeathCause::Starvation,
            },
            SimEvent::EntityAte {
                entity_id: 1,
                resource_id: 10,
                energy_gained: 20.0,
            },
            SimEvent::EntityAte {
                entity_id: 2,
                resource_id: 11,
                energy_gained: 15.0,
            },
            SimEvent::EntityMoved {
                entity_id: 1,
                from_x: 0.0,
                from_y: 0.0,
                to_x: 1.0,
                to_y: 1.0,
            },
            SimEvent::EntityReproduced {
                parent_id: 1,
                offspring_id: 4,
                x: 0.5,
                y: 0.5,
            },
        ];

        let summary = TickSummary::from_events(42, 3, &events);
        assert_eq!(summary.tick, 42);
        assert_eq!(summary.event_count, 7);
        assert_eq!(summary.entity_count, 3);
        assert_eq!(summary.births, 3); // 2 spawned + 1 reproduced
        assert_eq!(summary.deaths, 1);
        assert_eq!(summary.feedings, 2);
    }

    #[test]
    fn tick_summary_empty_events() {
        let summary = TickSummary::from_events(0, 0, &[]);
        assert_eq!(summary.tick, 0);
        assert_eq!(summary.event_count, 0);
        assert_eq!(summary.births, 0);
        assert_eq!(summary.deaths, 0);
        assert_eq!(summary.feedings, 0);
    }
}

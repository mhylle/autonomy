//! Trade route detection and tracking.
//!
//! Trade routes are detected by observing entity movement patterns between
//! settlements. When entities from the same tribe carry resources between
//! settlement areas, this constitutes a trade route.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A detected trade route between two settlements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRoute {
    /// Settlement ID of the origin.
    pub from_settlement: u64,
    /// Settlement ID of the destination.
    pub to_settlement: u64,
    /// Resource type being traded (if known).
    pub resource_type: Option<String>,
    /// Cumulative volume of resources transported.
    pub volume: u64,
    /// Number of entity trips observed on this route.
    pub trip_count: u64,
    /// Last tick this route was active.
    pub last_active_tick: u64,
}

impl TradeRoute {
    /// Create a new trade route.
    pub fn new(from_settlement: u64, to_settlement: u64, tick: u64) -> Self {
        Self {
            from_settlement,
            to_settlement,
            resource_type: None,
            volume: 0,
            trip_count: 0,
            last_active_tick: tick,
        }
    }

    /// Record a trip along this route.
    pub fn record_trip(&mut self, resource_type: Option<&str>, amount: u64, tick: u64) {
        self.trip_count += 1;
        self.volume += amount;
        self.last_active_tick = tick;
        if let Some(rt) = resource_type {
            self.resource_type = Some(rt.to_string());
        }
    }

    /// Whether this route has been active recently.
    pub fn is_active(&self, current_tick: u64, max_age: u64) -> bool {
        current_tick.saturating_sub(self.last_active_tick) <= max_age
    }

    /// Canonical key for this route (sorted pair of settlement IDs).
    pub fn route_key(&self) -> (u64, u64) {
        let a = self.from_settlement.min(self.to_settlement);
        let b = self.from_settlement.max(self.to_settlement);
        (a, b)
    }
}

/// Record of an entity's position for movement tracking.
#[derive(Debug, Clone)]
pub struct EntityMovementRecord {
    pub entity_id: u64,
    /// Settlement ID the entity was last near (if any).
    pub last_settlement_id: Option<u64>,
    /// Whether the entity is carrying items.
    pub carrying_items: bool,
}

/// Detect trade routes from entity movement between settlements.
///
/// When an entity moves from one settlement area to another while carrying
/// items, this counts as a trade trip.
pub fn detect_trade_trips(
    movement_records: &mut HashMap<u64, EntityMovementRecord>,
    entity_positions: &[(u64, f64, f64, bool)], // (entity_id, x, y, carrying_items)
    settlement_positions: &[(u64, f64, f64, f64)], // (settlement_id, x, y, radius)
    trade_routes: &mut HashMap<(u64, u64), TradeRoute>,
    current_tick: u64,
) {
    for &(entity_id, ex, ey, carrying) in entity_positions {
        // Find which settlement this entity is near.
        let current_settlement = settlement_positions.iter().find_map(|&(sid, sx, sy, radius)| {
            let dx = ex - sx;
            let dy = ey - sy;
            if (dx * dx + dy * dy).sqrt() <= radius {
                Some(sid)
            } else {
                None
            }
        });

        let record = movement_records.entry(entity_id).or_insert(EntityMovementRecord {
            entity_id,
            last_settlement_id: None,
            carrying_items: false,
        });

        if let (Some(prev_sid), Some(curr_sid)) = (record.last_settlement_id, current_settlement) {
            if prev_sid != curr_sid && record.carrying_items {
                // Entity moved from one settlement to another while carrying items.
                let key = {
                    let a = prev_sid.min(curr_sid);
                    let b = prev_sid.max(curr_sid);
                    (a, b)
                };

                let route = trade_routes
                    .entry(key)
                    .or_insert_with(|| TradeRoute::new(prev_sid, curr_sid, current_tick));
                route.record_trip(None, 1, current_tick);
            }
        }

        record.last_settlement_id = current_settlement;
        record.carrying_items = carrying;
    }
}

/// Prune trade routes that have been inactive for too long.
pub fn prune_inactive_routes(
    trade_routes: &mut HashMap<(u64, u64), TradeRoute>,
    current_tick: u64,
    max_inactive_ticks: u64,
) {
    trade_routes.retain(|_, route| route.is_active(current_tick, max_inactive_ticks));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_route_new_defaults() {
        let route = TradeRoute::new(1, 2, 100);
        assert_eq!(route.from_settlement, 1);
        assert_eq!(route.to_settlement, 2);
        assert_eq!(route.volume, 0);
        assert_eq!(route.trip_count, 0);
        assert!(route.resource_type.is_none());
    }

    #[test]
    fn trade_route_record_trip() {
        let mut route = TradeRoute::new(1, 2, 100);
        route.record_trip(Some("food"), 5, 110);
        assert_eq!(route.trip_count, 1);
        assert_eq!(route.volume, 5);
        assert_eq!(route.resource_type.as_deref(), Some("food"));
        assert_eq!(route.last_active_tick, 110);
    }

    #[test]
    fn trade_route_is_active() {
        let route = TradeRoute::new(1, 2, 100);
        assert!(route.is_active(150, 100)); // within max_age
        assert!(!route.is_active(250, 100)); // beyond max_age
    }

    #[test]
    fn trade_route_key_is_sorted() {
        let route1 = TradeRoute::new(5, 2, 0);
        let route2 = TradeRoute::new(2, 5, 0);
        assert_eq!(route1.route_key(), (2, 5));
        assert_eq!(route2.route_key(), (2, 5));
    }

    #[test]
    fn detect_trade_trip_between_settlements() {
        let mut records = HashMap::new();
        let mut routes = HashMap::new();

        // Settlement 1 at (10, 10), Settlement 2 at (200, 200)
        let settlements = vec![
            (1u64, 10.0, 10.0, 50.0),
            (2u64, 200.0, 200.0, 50.0),
        ];

        // Entity starts near settlement 1 carrying items.
        let positions1 = vec![(100u64, 10.0, 10.0, true)];
        detect_trade_trips(&mut records, &positions1, &settlements, &mut routes, 100);

        // Entity moves to settlement 2 still carrying items.
        let positions2 = vec![(100u64, 200.0, 200.0, true)];
        detect_trade_trips(&mut records, &positions2, &settlements, &mut routes, 110);

        assert_eq!(routes.len(), 1);
        let route = routes.values().next().unwrap();
        assert_eq!(route.trip_count, 1);
    }

    #[test]
    fn no_trade_without_carrying() {
        let mut records = HashMap::new();
        let mut routes = HashMap::new();

        let settlements = vec![
            (1u64, 10.0, 10.0, 50.0),
            (2u64, 200.0, 200.0, 50.0),
        ];

        // Entity near settlement 1, NOT carrying.
        let positions1 = vec![(100u64, 10.0, 10.0, false)];
        detect_trade_trips(&mut records, &positions1, &settlements, &mut routes, 100);

        // Entity moves to settlement 2, still not carrying.
        let positions2 = vec![(100u64, 200.0, 200.0, false)];
        detect_trade_trips(&mut records, &positions2, &settlements, &mut routes, 110);

        assert!(routes.is_empty());
    }

    #[test]
    fn prune_removes_old_routes() {
        let mut routes = HashMap::new();
        routes.insert((1, 2), TradeRoute::new(1, 2, 50));
        routes.insert((3, 4), TradeRoute::new(3, 4, 150));

        prune_inactive_routes(&mut routes, 200, 100);

        assert_eq!(routes.len(), 1);
        assert!(routes.contains_key(&(3, 4)));
    }
}

use std::collections::HashMap;

use crate::components::{Age, Energy, Genome, Health, Identity, Position, Size};
use crate::core::world::SimulationWorld;
use crate::net::protocol::autonomy::{EntityState, ResourceState, TickDelta, Vec2};
use crate::net::server::ViewportBounds;

/// Buffer zone (in world units) added around the viewport for filtering.
/// Entities within this buffer outside the viewport are still included to
/// prevent popping when the camera pans.
const VIEWPORT_BUFFER: f64 = 200.0;

/// Detail level determines how many fields are sent per entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// Position + speciesId only (zoomed far out, > 2x).
    Minimal,
    /// Adds energy, health, size (normal zoom, 0.5x..2x).
    Standard,
    /// Everything — all fields (zoomed in, < 0.5x).
    Detailed,
}

impl DetailLevel {
    /// Determine detail level from zoom factor.
    /// Lower zoom values mean the camera is closer (zoomed in).
    /// Higher zoom values mean the camera is farther out (zoomed out).
    ///
    /// zoom < 0.5  => Detailed (zoomed in)
    /// 0.5..2.0    => Standard (normal)
    /// zoom >= 2.0 => Minimal  (zoomed out)
    ///
    /// Note: In the viewer, zoom > 1 means zoomed IN (things look bigger).
    /// The viewer's zoom semantics: higher = more zoomed in.
    /// So we invert the logic: high viewer-zoom = detailed, low = minimal.
    pub fn from_zoom(zoom: f32) -> Self {
        if zoom >= 2.0 {
            // Zoomed in — show everything
            DetailLevel::Detailed
        } else if zoom <= 0.5 {
            // Zoomed far out — minimal data
            DetailLevel::Minimal
        } else {
            DetailLevel::Standard
        }
    }
}

/// Apply detail-level filtering to an EntityState, zeroing out fields
/// that are not needed at the given detail level.
fn apply_detail_level(state: EntityState, level: DetailLevel) -> EntityState {
    match level {
        DetailLevel::Detailed => state,
        DetailLevel::Standard => EntityState {
            // Keep position, species, energy, health, size
            // Zero out age/lifespan/generation
            age: 0,
            max_lifespan: 0,
            generation: 0,
            ..state
        },
        DetailLevel::Minimal => EntityState {
            // Keep only position + species_id
            id: state.id,
            position: state.position,
            species_id: state.species_id,
            energy: 0.0,
            max_energy: 0.0,
            health: 0.0,
            max_health: 0.0,
            age: 0,
            max_lifespan: 0,
            size: 0.0,
            generation: 0,
        },
    }
}

/// Check if a point (x, y) is within the viewport bounds expanded by the buffer zone.
fn in_viewport(x: f64, y: f64, vp: &ViewportBounds) -> bool {
    let left = vp.x - VIEWPORT_BUFFER;
    let right = vp.x + vp.width + VIEWPORT_BUFFER;
    let top = vp.y - VIEWPORT_BUFFER;
    let bottom = vp.y + vp.height + VIEWPORT_BUFFER;
    x >= left && x <= right && y >= top && y <= bottom
}

/// Snapshot of entity state for diff comparison.
#[derive(Debug, Clone)]
struct EntitySnapshot {
    x: f64,
    y: f64,
    energy: f64,
    health: f64,
    age: u64,
    size: f64,
}

impl EntitySnapshot {
    fn from_state(state: &EntityState) -> Self {
        let (x, y) = state.position.as_ref().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
        Self {
            x,
            y,
            energy: state.energy,
            health: state.health,
            age: state.age,
            size: state.size,
        }
    }

    /// Returns true if the state has changed enough to warrant sending an update.
    fn differs_from(&self, other: &EntitySnapshot) -> bool {
        const POS_EPSILON: f64 = 0.01;
        const VAL_EPSILON: f64 = 0.01;

        (self.x - other.x).abs() > POS_EPSILON
            || (self.y - other.y).abs() > POS_EPSILON
            || (self.energy - other.energy).abs() > VAL_EPSILON
            || (self.health - other.health).abs() > VAL_EPSILON
            || self.age != other.age
            || (self.size - other.size).abs() > VAL_EPSILON
    }
}

/// Tracks previous tick state for computing deltas.
pub struct DiffEngine {
    prev_entities: HashMap<u64, EntitySnapshot>,
    prev_resources: HashMap<u64, ResourceState>,
}

impl DiffEngine {
    pub fn new() -> Self {
        Self {
            prev_entities: HashMap::new(),
            prev_resources: HashMap::new(),
        }
    }

    /// Compute a proper TickDelta by comparing current world state to previous.
    ///
    /// Returns:
    /// - `spawned`: entities that exist now but didn't last tick
    /// - `updated`: entities that exist in both ticks but have changed
    /// - `died`: entity IDs that existed last tick but not now
    /// - `resource_changes`: resources whose state changed
    pub fn compute_delta(&mut self, world: &SimulationWorld) -> TickDelta {
        self.compute_delta_with_viewport(world, &ViewportBounds::default())
    }

    /// Compute a TickDelta filtered by the given viewport bounds.
    ///
    /// Only entities within the viewport (plus a buffer zone) are included
    /// in `spawned` and `updated`. Dead entities are always reported so the
    /// client can clean them up. Resources within the viewport are included.
    ///
    /// Detail levels based on zoom:
    /// - Minimal (zoom <= 0.5): position + speciesId only
    /// - Standard (0.5..2.0): + energy/health/size
    /// - Detailed (zoom >= 2.0): everything
    pub fn compute_delta_with_viewport(
        &mut self,
        world: &SimulationWorld,
        viewport: &ViewportBounds,
    ) -> TickDelta {
        let detail = DetailLevel::from_zoom(viewport.zoom);
        let mut spawned = Vec::new();
        let mut updated = Vec::new();
        let mut current_ids = HashMap::new();

        // Build current entity states.
        for (entity, (pos, energy, health, age, size, genome, identity)) in world
            .ecs
            .query::<(&Position, &Energy, &Health, &Age, &Size, &Genome, &Identity)>()
            .iter()
        {
            let id = entity.to_bits().get();
            let state = EntityState {
                id,
                position: Some(Vec2 { x: pos.x, y: pos.y }),
                energy: energy.current,
                max_energy: energy.max,
                health: health.current,
                max_health: health.max,
                age: age.ticks,
                max_lifespan: age.max_lifespan,
                size: size.radius,
                species_id: genome.species_id,
                generation: identity.generation,
            };
            let snapshot = EntitySnapshot::from_state(&state);

            // Always track all entities for accurate diff (prev_entities must be complete).
            // But only include visible entities in the outgoing delta.
            let visible = in_viewport(pos.x, pos.y, viewport);

            if let Some(prev) = self.prev_entities.get(&id) {
                if snapshot.differs_from(prev) && visible {
                    updated.push(apply_detail_level(state, detail));
                }
            } else if visible {
                spawned.push(apply_detail_level(state, detail));
            }

            current_ids.insert(id, snapshot);
        }

        // Find died entities — always report all deaths so client can clean up.
        let died: Vec<u64> = self
            .prev_entities
            .keys()
            .filter(|id| !current_ids.contains_key(id))
            .copied()
            .collect();

        // Compute resource changes — filtered by viewport.
        let mut resource_changes = Vec::new();
        let mut current_resources = HashMap::new();

        for r in &world.resources {
            let state = ResourceState {
                id: r.id,
                position: Some(Vec2 { x: r.x, y: r.y }),
                resource_type: format!("{:?}", r.resource_type),
                amount: r.amount,
                max_amount: r.max_amount,
            };

            let visible = in_viewport(r.x, r.y, viewport);
            let changed = match self.prev_resources.get(&r.id) {
                Some(prev) => (prev.amount - r.amount).abs() > 0.01,
                None => true,
            };

            if changed && visible {
                resource_changes.push(state.clone());
            }

            current_resources.insert(r.id, state);
        }

        // Update previous state for next tick.
        self.prev_entities = current_ids;
        self.prev_resources = current_resources;

        TickDelta {
            tick: world.tick,
            spawned,
            updated,
            died,
            resource_changes,
            entity_count: world.entity_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;
    use crate::core::spawning::spawn_initial_population;
    use crate::core::tick;

    fn test_world(count: u32) -> SimulationWorld {
        let config = SimulationConfig {
            initial_entity_count: count,
            ..SimulationConfig::default()
        };
        let mut world = SimulationWorld::new(config);
        spawn_initial_population(&mut world);
        world
    }

    #[test]
    fn first_tick_all_entities_spawned() {
        let world = test_world(5);
        let mut diff = DiffEngine::new();
        let delta = diff.compute_delta(&world);

        assert_eq!(delta.spawned.len(), 5);
        assert!(delta.updated.is_empty());
        assert!(delta.died.is_empty());
    }

    #[test]
    fn second_tick_only_changes_sent() {
        let mut world = test_world(5);

        // Lower energy to prevent reproduction on first tick.
        for (_entity, energy) in world.ecs.query_mut::<&mut Energy>() {
            energy.current = 50.0;
        }

        let mut diff = DiffEngine::new();

        // First delta: all spawned.
        diff.compute_delta(&world);

        // Run a tick to cause movement.
        tick::tick(&mut world);

        let delta = diff.compute_delta(&world);

        // After movement, original entities should appear in `updated`, not `spawned`.
        assert!(
            !delta.updated.is_empty(),
            "moved entities should appear in updated"
        );
    }

    #[test]
    fn died_entities_detected() {
        let mut world = test_world(3);
        let mut diff = DiffEngine::new();

        diff.compute_delta(&world);

        // Kill all entities by draining energy.
        for (_entity, energy) in world.ecs.query_mut::<&mut Energy>() {
            energy.current = -1.0;
        }
        // Run cleanup to remove dead entities.
        crate::systems::cleanup::run(&mut world);

        let delta = diff.compute_delta(&world);
        assert_eq!(delta.died.len(), 3, "all 3 entities should be dead");
        assert!(delta.spawned.is_empty());
    }

    #[test]
    fn unchanged_entities_not_sent() {
        let world = test_world(3);
        let mut diff = DiffEngine::new();

        diff.compute_delta(&world);

        // No tick run, no changes.
        let delta = diff.compute_delta(&world);
        assert!(delta.spawned.is_empty());
        assert!(delta.updated.is_empty());
        assert!(delta.died.is_empty());
    }

    #[test]
    fn resource_changes_detected() {
        let mut world = test_world(0);
        let mut diff = DiffEngine::new();

        // Add resources.
        crate::environment::spawning::scatter_resources(&mut world);

        // First delta: all resources are new.
        let delta = diff.compute_delta(&world);
        assert!(!delta.resource_changes.is_empty());

        // No changes: should be empty.
        let delta = diff.compute_delta(&world);
        assert!(delta.resource_changes.is_empty());

        // Deplete a resource.
        if !world.resources.is_empty() {
            world.resources[0].amount -= 10.0;
            let delta = diff.compute_delta(&world);
            assert!(!delta.resource_changes.is_empty());
        }
    }

    // --- Viewport filtering tests ---

    #[test]
    fn viewport_filters_entities_outside_bounds() {
        use crate::components::Position;

        let mut world = test_world(5);
        // Move all entities to a known position far outside the viewport.
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 900.0;
            pos.y = 900.0;
        }

        let mut diff = DiffEngine::new();
        // Viewport covers only 0..100 x 0..100, entities are at 900,900.
        // Buffer is 200 units, so effective range is -200..300.
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0,
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        // Entities at 900,900 are outside viewport+buffer (max 300), so none should be spawned.
        assert!(
            delta.spawned.is_empty(),
            "entities outside viewport should be filtered: got {} spawned",
            delta.spawned.len()
        );
    }

    #[test]
    fn viewport_includes_entities_inside_bounds() {
        use crate::components::Position;

        let mut world = test_world(3);
        // Move all entities inside the viewport.
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 50.0;
            pos.y = 50.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0,
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        assert_eq!(
            delta.spawned.len(),
            3,
            "entities inside viewport should be included"
        );
    }

    #[test]
    fn viewport_buffer_zone_includes_nearby_entities() {
        use crate::components::Position;

        let mut world = test_world(2);
        // Place entities just outside the viewport but within the buffer zone.
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 250.0; // viewport edge at 100, buffer extends to 300
            pos.y = 50.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0,
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        assert_eq!(
            delta.spawned.len(),
            2,
            "entities within buffer zone should be included"
        );
    }

    #[test]
    fn died_entities_reported_regardless_of_viewport() {
        use crate::components::Position;

        let mut world = test_world(3);
        // Place entities far outside viewport.
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 900.0;
            pos.y = 900.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0,
        };

        // First tick: entities not in viewport (filtered out of spawned),
        // but they're still tracked internally.
        diff.compute_delta_with_viewport(&world, &vp);

        // Kill all entities.
        for (_entity, energy) in world.ecs.query_mut::<&mut Energy>() {
            energy.current = -1.0;
        }
        crate::systems::cleanup::run(&mut world);

        let delta = diff.compute_delta_with_viewport(&world, &vp);
        // Deaths should always be reported so client can clean up.
        assert_eq!(
            delta.died.len(),
            3,
            "died entities should always be reported"
        );
    }

    // --- Detail level tests ---

    #[test]
    fn detail_level_from_zoom() {
        // High zoom (zoomed in) = Detailed
        assert_eq!(DetailLevel::from_zoom(3.0), DetailLevel::Detailed);
        assert_eq!(DetailLevel::from_zoom(2.0), DetailLevel::Detailed);

        // Normal zoom = Standard
        assert_eq!(DetailLevel::from_zoom(1.0), DetailLevel::Standard);
        assert_eq!(DetailLevel::from_zoom(0.6), DetailLevel::Standard);

        // Low zoom (zoomed out) = Minimal
        assert_eq!(DetailLevel::from_zoom(0.5), DetailLevel::Minimal);
        assert_eq!(DetailLevel::from_zoom(0.2), DetailLevel::Minimal);
    }

    #[test]
    fn minimal_detail_strips_fields() {
        use crate::components::Position;

        let mut world = test_world(1);
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 50.0;
            pos.y = 50.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 0.3, // Minimal detail
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        assert_eq!(delta.spawned.len(), 1);
        let e = &delta.spawned[0];
        // Minimal: only position and species_id
        assert!(e.position.is_some());
        assert!(e.species_id > 0);
        // Other fields should be zeroed
        assert_eq!(e.energy, 0.0);
        assert_eq!(e.max_energy, 0.0);
        assert_eq!(e.health, 0.0);
        assert_eq!(e.max_health, 0.0);
        assert_eq!(e.age, 0);
        assert_eq!(e.size, 0.0);
        assert_eq!(e.generation, 0);
    }

    #[test]
    fn standard_detail_keeps_energy_health_size() {
        use crate::components::Position;

        let mut world = test_world(1);
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 50.0;
            pos.y = 50.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 1.0, // Standard detail
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        assert_eq!(delta.spawned.len(), 1);
        let e = &delta.spawned[0];
        assert!(e.position.is_some());
        assert!(e.species_id > 0);
        // Standard keeps energy, health, size
        assert!(e.energy > 0.0);
        assert!(e.health > 0.0);
        // But zeroes age/generation
        assert_eq!(e.age, 0);
        assert_eq!(e.generation, 0);
    }

    #[test]
    fn detailed_level_keeps_all_fields() {
        use crate::components::Position;

        let mut world = test_world(1);
        for (_entity, pos) in world.ecs.query_mut::<&mut Position>() {
            pos.x = 50.0;
            pos.y = 50.0;
        }

        let mut diff = DiffEngine::new();
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            zoom: 3.0, // Detailed
        };
        let delta = diff.compute_delta_with_viewport(&world, &vp);

        assert_eq!(delta.spawned.len(), 1);
        let e = &delta.spawned[0];
        assert!(e.position.is_some());
        assert!(e.energy > 0.0);
        assert!(e.health > 0.0);
        // Detailed: age starts at 0 for new entities, but max_lifespan should be set
        assert!(e.max_lifespan > 0);
        assert!(e.species_id > 0);
    }

    #[test]
    fn in_viewport_logic() {
        let vp = ViewportBounds {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            zoom: 1.0,
        };
        // Inside viewport
        assert!(in_viewport(150.0, 150.0, &vp));
        // Inside buffer zone (left of viewport but within 200 units)
        assert!(in_viewport(0.0, 150.0, &vp));
        // Outside buffer zone
        assert!(!in_viewport(-200.0, 150.0, &vp));
        // Top-left corner of buffer
        assert!(in_viewport(-99.0, -99.0, &vp));
        // Beyond bottom-right + buffer
        assert!(!in_viewport(600.0, 600.0, &vp));
    }
}

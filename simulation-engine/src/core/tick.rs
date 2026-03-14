use std::time::Instant;
use tracing::trace;

use super::perf::PerformanceStats;
use super::world::SimulationWorld;
use crate::net::server::ViewportBounds;
use crate::systems;

/// Advance the simulation by one tick.
///
/// Runs all systems in deterministic order. Systems are added as the
/// project progresses through implementation phases.
pub fn tick(world: &mut SimulationWorld) {
    tick_with_perf(world, &mut None, &ViewportBounds::default());
}

/// Advance the simulation by one tick with optional performance tracking
/// and LOD-based system skipping.
///
/// When `perf_stats` is Some and enabled, each system's execution time
/// is measured and recorded. The `viewport` is used to compute LOD levels
/// so that distant entities can skip expensive systems.
pub fn tick_with_perf(
    world: &mut SimulationWorld,
    perf_stats: &mut Option<PerformanceStats>,
    viewport: &ViewportBounds,
) {
    let tick_start = Instant::now();
    world.tick += 1;
    world.event_log.clear();
    trace!(tick = world.tick, "tick start");

    let perf_enabled = perf_stats.as_ref().map_or(false, |s| s.enabled);

    // Compute LOD assignments for all entities with positions.
    // We store them on the world so systems can query them.
    if perf_enabled {
        let start = Instant::now();
        compute_lod_assignments(world, viewport);
        if let Some(ref mut stats) = perf_stats {
            stats.record_system("lod_compute", start.elapsed());
        }
    } else {
        compute_lod_assignments(world, viewport);
    }

    // Deterministic system execution order:
    // Macro to time a system call.
    macro_rules! run_system {
        ($name:expr, $call:expr) => {
            if perf_enabled {
                let start = Instant::now();
                $call;
                if let Some(ref mut stats) = perf_stats {
                    stats.record_system($name, start.elapsed());
                }
            } else {
                $call;
            }
        };
    }

    run_system!("climate", crate::environment::climate::run(world));
    run_system!("regrowth", crate::environment::regrowth::run(world));
    run_system!("spatial_rebuild", systems::spatial_rebuild::run(world));
    run_system!("perception", systems::perception::run(world));
    run_system!("drives", systems::drives::run(world));
    run_system!("decision", systems::decision::run(world));
    run_system!("signals", systems::signals::run(world));
    run_system!("wander", systems::wander::run(world));
    run_system!("movement", systems::movement::run(world));
    run_system!("feeding", systems::feeding::run(world));
    run_system!("combat", systems::combat::run(world));
    run_system!("war", systems::war::run(world));
    run_system!("reproduction", systems::reproduction::run(world));
    run_system!("composition", systems::composition::run(world));
    run_system!(
        "composite_repro",
        systems::composite_reproduction::run(world)
    );
    run_system!("memory", systems::memory::run(world));
    run_system!("tribe", systems::tribe::run(world));
    run_system!(
        "cultural_transmission",
        systems::cultural_transmission::run(world)
    );
    run_system!("construction", systems::construction::run(world));
    run_system!("objects", systems::objects::run(world));
    run_system!("agriculture", systems::agriculture::run(world));
    run_system!("aging", systems::aging::run(world));
    run_system!("cleanup", systems::cleanup::run(world));
    run_system!("narrative", run_narrative(world));
    run_system!("civilization", systems::settlement::run(world));

    // Record overall tick timing (only if perf tracking is enabled).
    if perf_enabled {
        if let Some(ref mut stats) = perf_stats {
            let tick_duration = tick_start.elapsed();
            stats.record_tick(tick_duration);
            stats.last_entity_count = world.entity_count();
        }
    }

    trace!(
        tick = world.tick,
        entities = world.entity_count(),
        "tick end"
    );
}

/// Run the narrative observer system.
///
/// Gathers entity stats and species populations from the ECS world, then
/// feeds them to the narrative tracker along with this tick's events.
/// This is a read-only observer -- it never modifies simulation state.
fn run_narrative(world: &mut SimulationWorld) {
    use crate::components::{Genome, Social};
    use crate::components::physical::Age;
    use crate::narrative::EntityStats;

    // Gather species populations from the ECS.
    let mut species_populations = std::collections::HashMap::new();
    for (_, genome) in world.ecs.query::<&Genome>().iter() {
        *species_populations.entry(genome.species_id).or_insert(0u32) += 1;
    }

    // Gather per-entity stats for interest scoring.
    // We use a lightweight query; the narrative tracker stores its own
    // cumulative counters for offspring/kills/distance internally.
    let entity_stats: Vec<EntityStats> = world
        .ecs
        .query::<(&Age, &Genome, Option<&Social>)>()
        .iter()
        .map(|(entity, (age, genome, social))| {
            let eid = entity.to_bits().get();
            let (relationship_count, relationships) = match social {
                Some(s) => (
                    s.relationships.len() as u32,
                    s.relationships.iter().map(|(&id, &v)| (id, v)).collect(),
                ),
                None => (0, Vec::new()),
            };
            EntityStats {
                entity_id: eid,
                age: age.ticks,
                offspring_count: 0, // tracked internally by biography compiler
                kill_count: 0,      // tracked internally by biography compiler
                distance_traveled: 0.0, // tracked internally by biography compiler
                relationship_count,
                relationships,
                species_id: genome.species_id,
            }
        })
        .collect();

    let events = world.event_log.events().to_vec();
    let tick = world.tick;

    world
        .narrative_tracker
        .process_tick(&events, tick, &species_populations, &entity_stats);
}

/// Compute LOD assignments for all entities based on viewport distance.
///
/// Stores results in `world.lod_assignments` so individual systems can
/// check an entity's LOD level and skip work as appropriate.
fn compute_lod_assignments(
    world: &mut SimulationWorld,
    viewport: &ViewportBounds,
) {
    world.lod_assignments.clear();

    let positions: Vec<(u64, f64, f64)> = world
        .ecs
        .query::<&crate::components::spatial::Position>()
        .iter()
        .map(|(entity, pos)| (entity.to_bits().get(), pos.x, pos.y))
        .collect();

    for (id_bits, x, y) in positions {
        let lod = super::lod::compute_lod(x, y, viewport);
        world.lod_assignments.insert(id_bits, lod);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::SimulationConfig;

    #[test]
    fn tick_increments_counter() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        assert_eq!(world.tick, 0);
        tick(&mut world);
        assert_eq!(world.tick, 1);
        tick(&mut world);
        assert_eq!(world.tick, 2);
    }

    #[test]
    fn multiple_ticks_run_without_panic() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        for _ in 0..100 {
            tick(&mut world);
        }
        assert_eq!(world.tick, 100);
    }

    #[test]
    fn tick_with_perf_records_timing() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut stats = Some(PerformanceStats::new(true));
        let vp = ViewportBounds::default();

        for _ in 0..5 {
            tick_with_perf(&mut world, &mut stats, &vp);
        }

        let stats = stats.unwrap();
        assert_eq!(stats.tick_timing.count, 5);
        assert!(stats.systems.contains_key("perception"));
        assert!(stats.systems.contains_key("drives"));
        assert!(stats.systems.contains_key("aging"));
    }

    #[test]
    fn tick_with_perf_disabled_still_works() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut stats = Some(PerformanceStats::new(false));
        let vp = ViewportBounds::default();

        tick_with_perf(&mut world, &mut stats, &vp);
        assert_eq!(world.tick, 1);

        let stats = stats.unwrap();
        assert_eq!(stats.tick_timing.count, 0);
        assert!(stats.systems.is_empty());
    }

    #[test]
    fn tick_with_none_stats_works() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        let mut stats: Option<PerformanceStats> = None;
        let vp = ViewportBounds::default();

        tick_with_perf(&mut world, &mut stats, &vp);
        assert_eq!(world.tick, 1);
    }

    #[test]
    fn lod_assignments_populated_after_tick() {
        let mut world = SimulationWorld::new(SimulationConfig::default());
        crate::core::spawning::spawn_initial_population(&mut world);

        let mut stats = Some(PerformanceStats::new(true));
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
            zoom: 1.0,
        };

        tick_with_perf(&mut world, &mut stats, &vp);

        // Should have LOD assignments for entities with positions.
        assert!(
            !world.lod_assignments.is_empty(),
            "LOD assignments should be populated after tick"
        );
    }
}

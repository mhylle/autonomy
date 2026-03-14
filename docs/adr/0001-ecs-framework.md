# ADR-0001: ECS Framework Selection

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Use `hecs` as the Entity-Component-System framework

## Context

The simulation engine needs an ECS framework for managing thousands of entities with varying component sets. The framework must support:
- Headless simulation (no rendering pipeline)
- Deterministic tick-based execution
- High cache performance for linear iteration over components
- Serialization of all components
- Dynamic component addition/removal (entities gain/lose capabilities)

Candidates evaluated: `bevy_ecs`, `hecs`, `shipyard`, `legion`, `specs`.

## Decision

Use `hecs` (v0.10).

## Rationale

- **Minimal and focused**: ~2k lines, zero opinions about scheduling or rendering. We write our own tick loop, which is essential for deterministic simulation.
- **Fast**: Archetype-based storage gives excellent cache locality for bulk iteration. Benchmarks show hecs is competitive with or faster than bevy_ecs for linear iteration patterns.
- **No framework tax**: bevy_ecs brings Bevy's App/Plugin/Schedule abstractions designed around a game engine. We don't need a game engine -- we need a data store with fast queries.
- **Full control**: No scheduler means we control exactly when and in what order systems execute. Critical for determinism and event sourcing.
- **Stable API**: Well-documented, mature, minimal surface area.

## Alternatives Considered

| Framework | Rejected Because |
|-----------|-----------------|
| `bevy_ecs` | Designed for Bevy's rendering pipeline. Async integration is "not friendly" (confirmed by Bevy maintainers). Overhead not justified for headless simulation. |
| `shipyard` | SparseSet storage is faster for random access but we primarily do bulk linear iteration. Less ecosystem adoption. Could be a future migration if perf requires it. |
| `legion` | No longer maintained. |
| `specs` | Legacy design, slower than alternatives. |

## Consequences

- We write our own system scheduler (deterministic ordered execution)
- We handle parallelism ourselves if needed (Phase 8)
- If we ever build a native Bevy-based renderer, we'll need a sync layer between hecs and bevy_ecs worlds
- Components are plain Rust structs with `#[derive(Serialize, Deserialize)]` -- no framework traits required

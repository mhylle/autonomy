# ADR-0004: Tick-Based Deterministic Simulation with Event Sourcing

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Use a tick-based simulation loop with deterministic execution and event sourcing for all state changes

## Context

The simulation must support:
- Reproducible runs (same seed → same outcome)
- Replay and time-travel (scrub to any tick, replay history)
- Historical queries (what happened to entity X? when did species Y go extinct?)
- Decoupled visualization (render at any speed, independent of simulation speed)
- "What-if" experiments (fork from a snapshot, change parameters)

## Decision

1. **Tick-based**: Simulation advances in discrete ticks. Each tick runs all systems in a fixed order.
2. **Deterministic**: Every system uses a seeded PRNG (ChaCha8). Same seed = identical simulation.
3. **Event sourcing**: Every state change produces an immutable event appended to the event log. Events are the source of truth for history.
4. **Periodic snapshots**: Full world state serialized every N ticks. Enables fast seek without replaying from tick 0.

## Rationale

**Why ticks over continuous time?**
- Determinism is trivial with discrete steps, nearly impossible with variable-timestep physics
- Event ordering is unambiguous within a tick
- Replay is exact (apply the same events in the same order)
- Decoupled from rendering frame rate

**Why event sourcing?**
- Complete audit trail of everything that happened
- Replay from any snapshot by re-applying events
- Natural fit for narrative generation (stories are sequences of events)
- Enables historical analytics without maintaining separate analytics stores

**Why ChaCha8 for RNG?**
- Cryptographically-derived RNG with excellent statistical properties
- `SeedableRng` trait enables deterministic seeding
- Fast enough for simulation use (not as fast as Xoshiro but more robust)
- Consistent across platforms (unlike some hardware-dependent RNGs)

## Tick Pipeline

Systems execute in this fixed order every tick:

```
environment → perception → drives → decision → movement → feeding → combat
→ reproduction → composition → memory → aging → cleanup → event_emit
→ snapshot → network_sync
```

Each system reads components, writes components, and may append events to a per-tick event buffer. The event buffer is flushed to the event log after all systems complete.

## Snapshot Strategy

- Full snapshot every 1000 ticks (configurable)
- Snapshot includes: all entities with all components, environment state, RNG state, tick counter
- Serialized with `serde` + `bincode` for speed
- Seek to tick T: load nearest snapshot before T, replay events from snapshot tick to T

## Consequences

- Simulation speed is measured in ticks/second, not frames/second
- All randomness must go through the seeded RNG -- no `rand::thread_rng()`, no system clock
- Floating-point operations may differ across CPU architectures -- determinism is guaranteed only on the same platform
- Event log grows linearly with simulation time -- need storage management for long runs
- Network sync happens at tick rate, not frame rate -- viewer interpolates between ticks for smooth animation

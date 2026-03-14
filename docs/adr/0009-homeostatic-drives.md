# ADR-0009: Homeostatic Drive System for Emergent Emotions

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Use a homeostatic drive model where emotions and complex motivations emerge from simple internal state variables and their deviations from setpoints

## Context

The simulation needs entities with motivations that drive behavior. The user's vision is that complex drives (love, hate, desire) emerge from simpler ones rather than being hard-coded. We need a system where:
- Simple entities have basic drives (hunger, fear)
- Complex entities develop richer motivational states
- Emotional states emerge from the interaction of drives, memory, and social contact
- No emotion is explicitly programmed

## Decision

Implement a homeostatic reinforcement system based on internal state variables:

1. Each entity has an internal state vector: `[energy, health, safety, social_contact, novelty, ...]`
2. Each variable has a genome-encoded setpoint (ideal value)
3. **Drive intensity** = deviation from setpoint: `|current - setpoint|`
4. **Reward signal** = drive reduction: `drive_before - drive_after`
5. Behavior tree condition nodes check drives against evolved thresholds
6. Memory records which actions/entities/locations led to drive satisfaction

## Rationale

The homeostatic model (Keramati & Gutkin, 2014) produces four emergent behavioral properties without any separate emotion module:

1. **Dose sensitivity**: Larger food rewards reduce hunger drive more → entities prefer richer food sources
2. **Deprivation modulation**: Greater hunger amplifies food reward value → starving entities take bigger risks
3. **Drive competition**: A sufficiently hungry entity ignores mating opportunities → priority emerges naturally
4. **Risk aversion**: Entities prefer frequent small satisfactions over rare large ones

### How complex emotions emerge

**Attachment**: Entity A consistently reduces entity B's social_need drive → B's memory associates A with positive valence → B's behavior tree includes "seek entities with positive memory valence" → B seeks A. This is not programmed love; it's conditioned association.

**Fear**: Entity's health dropped sharply near entity X → memory records high-negative-valence event → future perception of X triggers avoidance behavior via memory recall in BT.

**Curiosity**: Novelty drive increases when entity stays in the same area too long → BT node "CheckDrive(curiosity, threshold)" triggers exploration behavior.

**Grief**: Bonded entity (positive memory associations) dies → social_need drive spikes with no resolution target → entity exhibits searching behavior or drive-driven distress.

None of these are coded as named emotions. They emerge from the drive + memory + behavior tree interaction.

## Design

```rust
struct Drives {
    hunger: f32,           // 1.0 - (energy / max_energy)
    fear: f32,             // f(perceived_threats, health_ratio, threat_memories)
    curiosity: f32,        // f(time_in_area, base_curiosity)
    social_need: f32,      // f(time_since_contact, base_social_need)
    aggression: f32,       // f(hunger, perceived_weakness, base_aggression)
    reproductive_urge: f32,// f(energy_surplus, age, base_reproductive)
}

struct DriveWeights {
    base_hunger_sensitivity: f32,  // genome: how quickly hunger rises
    base_fear_sensitivity: f32,    // genome: how reactive to threats
    base_curiosity: f32,           // genome: baseline exploration drive
    base_social_need: f32,         // genome: how much social contact matters
    base_aggression: f32,          // genome: baseline hostility
    base_reproductive_urge: f32,   // genome: reproduction priority
}
```

Drive weights are part of the genome and evolve. An entity whose genome produces high `base_curiosity` will explore more, finding new food sources but also encountering more danger. Natural selection determines which drive calibrations survive in each environment.

## Consequences

- Drives are recomputed every tick by the `drives_system` -- no persistent "emotion state"
- Memory is essential for drives like fear and social_need (which depend on past experiences)
- The behavior tree must be expressive enough to act on drive levels (condition nodes with thresholds)
- Viewer should display drive levels as a radar chart or bar graph for selected entities
- Drive evolution may produce surprising personality types -- highly curious, highly aggressive, highly social -- this is the point
- Future phases can add new drives (territory, status, aesthetic preference) by adding variables to the state vector

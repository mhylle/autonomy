# ADR-0008: Entity Composition Model

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Entities compose into multi-cellular organisms using a parent-child entity model with role-based specialization

## Context

A core vision of Autonomy is that simple single-cell entities can merge into complex multi-cellular organisms with emergent specialization. The composition model must:
- Allow entities to merge and split dynamically
- Support role-based specialization (sensing, locomotion, attack, defense, digestion, reproduction)
- Aggregate capabilities from member entities
- Be representable in the ECS without special-casing
- Allow the composite's behavior to emerge from its members rather than being hard-coded

## Decision

Use a parent-child entity model:

1. When entities merge, a new **composite entity** is spawned with a `CompositeBody` component
2. Member entities become **children** -- they lose independent `Position` (move with parent) but retain `Genome`, `Memory`, and other identity components
3. Each member is assigned a `CellRole` based on genome traits
4. The composite's capabilities (speed, sensor range, attack power, etc.) are aggregated from its members
5. The composite uses the leader member's behavior tree for decisions
6. If the composite dies or decomposes, members can survive independently

## Rationale

### Why parent-child over merging into one entity?

| Approach | Pros | Cons |
|----------|------|------|
| Parent-child (chosen) | Members retain identity and genome. Decomposition is natural. Member diversity drives specialization. | More complex queries. More entities in the world. |
| Merge into single entity | Simpler model. One genome per organism. | Loses member diversity. Decomposition requires re-creating entities. No path to emergent specialization. |

The parent-child model is essential for the long-term vision: a composite organism's capabilities should depend on *which specific cells* compose it, not on a merged average.

### Role assignment

Roles are assigned based on genome traits of member entities:

```
Locomotion:    highest max_speed
Sensing:       highest sensor_range
Attack:        highest aggression + size
Defense:       highest health + size
Digestion:     highest metabolism efficiency
Reproduction:  highest reproductive capacity
```

Over evolutionary time, entities that specialize in specific roles become better members of composites. A "sensor cell" evolves large sensor_range at the cost of other traits. This emergent specialization mirrors biological cell differentiation.

### Composition triggers

All of the following can trigger composition:
- **Proximity + compatibility**: Adjacent entities with compatible genomes (composition_compatibility vector)
- **Intentional**: BT action node `CompositionAttempt` -- entity actively seeks to merge
- **Environmental pressure**: Resource scarcity or predation pressure increases composition attempts

## Data Model

```rust
struct CompositeBody {
    members: Vec<Entity>,
    roles: Vec<CellRole>,
    energy_distribution: EnergyDistribution,
    leader: Entity,  // whose BT drives decisions
}

enum CellRole {
    Locomotion, Sensing, Attack, Defense, Digestion, Reproduction, Undifferentiated
}

enum EnergyDistribution {
    Equal,
    Proportional,    // by role importance
    Priority(CellRole),
}
```

## Consequences

- ECS queries must handle composite vs. simple entities (check for `CompositeBody` component)
- Perception system aggregates sensor ranges from Sensing-role members
- Movement system uses Locomotion-role members' speed
- Rendering must show composites as clusters, with drill-down to members
- Reproduction of composites is complex: does the whole organism reproduce, or individual cells? (Design decision deferred to Phase 5 implementation)
- Energy distribution within the composite is an evolutionary pressure -- composites that misallocate energy die

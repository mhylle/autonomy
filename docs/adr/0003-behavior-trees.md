# ADR-0003: Custom Behavior Tree Engine

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Build a custom behavior tree engine rather than using an existing crate

## Context

Each entity's decision-making is driven by a behavior tree (BT). The BT must be:
1. **Serializable** -- stored as part of the genome, saved/loaded with simulation state
2. **Evolvable** -- subjected to genetic programming operators (subtree crossover, structural mutation)
3. **Inspectable** -- sent to the viewer for visualization and debugging
4. **Parameterized** -- nodes carry evolved floating-point parameters (thresholds, probabilities, speed factors)

Existing Rust BT crates evaluated: `bonsai-bt`, `behavior-tree`, `behaviortree`, `behavior-tree-lite`.

## Decision

Build a custom BT engine. The core is a `BtNode` enum (tree of nodes) with a tick function. Estimated ~300 lines for the core engine.

## Rationale

No existing crate supports all four requirements simultaneously:

- `bonsai-bt`: Callback-based tick model. BTs are constructed in code, not serializable data. Cannot be stored in a genome or evolved.
- `behavior-tree`: Bare implementation. No serialization support, no parameterized nodes.
- `behaviortree`: Minimal, no_std compatible, but not designed for evolution or inspection.

A custom engine gives us:
- `BtNode` as a plain enum -- trivially serializable with serde, trivially clonable for crossover
- Node parameters as struct fields -- directly mutable for mutation
- Full control over which node types exist (we add new node types as we add simulation features)
- Tree traversal for visualization (send BT structure + current execution state to viewer)

## Design

```rust
enum BtNode {
    // Control flow
    Sequence(Vec<BtNode>),
    Selector(Vec<BtNode>),
    Inverter(Box<BtNode>),

    // Conditions (parameterized)
    CheckDrive { drive: DriveType, threshold: f32, comparison: Comparison },
    NearbyEntity { max_distance: f32, filter: EntityFilter },
    // ...

    // Actions (parameterized)
    MoveTowardEntity { filter: EntityFilter, speed_factor: f32 },
    Eat,
    Attack { force_factor: f32 },
    // ...
}
```

Evolution operators work directly on this tree structure:
- **Subtree crossover**: Select random node in each parent, swap subtrees
- **Parameter mutation**: Gaussian noise on any `f32` parameter
- **Structural mutation**: Replace a random node with a newly generated subtree
- **Simplification**: Remove redundant wrappers (Sequence with one child → child)

## Consequences

- We own the BT engine code -- full responsibility for correctness and performance
- Adding new node types is straightforward (add enum variant, implement tick logic, add to evolution operators)
- The BT representation is the genome's "behavioral DNA" -- the most important data structure in the system
- No external BT crate dependency to manage or work around

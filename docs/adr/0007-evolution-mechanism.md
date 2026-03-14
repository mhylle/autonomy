# ADR-0007: Continuous Evolution via Genetic Programming

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Evolution operates continuously through reproduction using genetic programming on behavior trees and trait mutation on physical parameters

## Context

The simulation needs entities to evolve over time -- not just parameter tuning, but structural evolution of decision-making. The user's vision is that behavior trees themselves restructure across generations, producing genuinely novel behaviors that weren't anticipated at design time.

Key requirements:
- Evolution must be continuous (no discrete "generations")
- Behavior tree structure must evolve, not just parameters
- Physical traits must co-evolve with behavioral traits
- The system must avoid converging to a single "winner" strategy (boring equilibrium)

## Decision

1. **Continuous evolution**: Evolution happens at reproduction time. No global generation counter.
2. **Genetic programming** for behavior trees: subtree crossover, structural mutation, parameter mutation
3. **Trait mutation** for physical parameters: Gaussian noise on floats, bounded to valid ranges
4. **Mutation rates are themselves evolvable**: the genome includes its own mutation rate
5. **Novelty pressure** (Phase 3+): introduce novelty-based selection to prevent convergence

## Rationale

### Why GP over NEAT for behavior trees?

| Approach | Pros | Cons |
|----------|------|------|
| GP on BTs | Interpretable trees, natural for sequential decision-making, easy to visualize and debug | Prone to bloat, crossover can produce nonsensical trees |
| NEAT (neural nets) | Proven for topology evolution, smooth parameter spaces | Opaque decision-making, hard to visualize "why" an entity acted |
| Hybrid | Best of both | Complexity |

**Decision**: Start with GP on behavior trees (interpretable, debuggable, narratable). Add NEAT as neural modules within BT leaf nodes in Phase 8 for more nuanced decision-making. The hybrid approach gives us interpretable high-level decisions with learned low-level parameter tuning.

### Why continuous over generational?

In nature, evolution doesn't happen in synchronized generations. Continuous evolution:
- Allows different species to evolve at different rates (fast reproducers evolve faster)
- Creates overlapping populations with different fitness levels (more realistic ecology)
- Avoids artificial synchronization points

### Avoiding equilibrium

Five mechanisms prevent boring convergence:
1. **Coevolution**: Predator-prey arms races ensure neither side can plateau
2. **Environmental change**: Climate drift, seasonal resource variation, random events
3. **Evolvable mutation rates**: Populations in stable environments evolve lower mutation rates; stressed populations evolve higher rates
4. **Species protection**: Entities are clustered by genome similarity. New species get time to optimize before competing globally (inspired by NEAT's speciation).
5. **Novelty search** (Phase 3+): Optionally reward behavioral novelty alongside fitness

## Genetic Operators

**Subtree crossover** (primary diversity operator):
- Select random node in parent A's BT, random node in parent B's BT
- Swap subtrees
- Depth limit prevents runaway growth

**Parameter mutation** (fine-tuning):
- Each `f32` parameter has a small chance of Gaussian perturbation
- Magnitude controlled by genome's mutation_rate

**Structural mutation** (novelty):
- Insert: wrap a random node in a new control node with a random sibling
- Delete: replace a random control node with one of its children
- Replace: swap a random leaf node for a different leaf type

**Simplification** (bloat control):
- Sequence/Selector with one child → unwrap
- Inverter(Inverter(x)) → x
- Applied probabilistically after crossover

## Consequences

- Behavior trees will grow in complexity over evolutionary time
- Bloat control is essential -- without it, trees grow indefinitely
- The viewer needs a BT visualizer that can show tree structure and highlight active nodes
- Species tracking requires a genome similarity metric (hash of BT structure + key trait values)
- "Interesting" evolved behaviors may be hard to predict -- this is the point, but also means testing is harder
- Deterministic RNG ensures evolution is reproducible given the same seed

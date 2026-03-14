# ADR-0006: Bounded Entity Memory with Evolved Eviction

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Entities have fixed-capacity memory with genome-encoded eviction weights

## Context

Entities need to remember past experiences (food locations, threats, social interactions) to make informed decisions. However:
- Thousands of entities with unbounded memory would exhaust RAM
- Per-entity heap allocation degrades cache performance in ECS
- Memory management is an interesting evolutionary pressure -- entities that remember the right things survive longer

## Decision

Each entity has a `Memory` component with:
- A fixed-capacity buffer (capacity set by genome, typically 10-50 entries)
- Genome-encoded eviction weights that determine which memories are discarded when full
- Memory entries are plain structs (tick, kind, importance, emotional valence, location, associated entity)

## Rationale

### Fixed capacity from genome

Memory capacity costs energy (larger brain → higher metabolism). This creates a trade-off: more memory helps survival but costs more energy. Natural selection finds the optimal balance for each ecological niche.

### Evolved eviction strategy

The eviction scoring function uses weights from the genome:

```
score = recency_weight × recency(age)
      + importance_weight × importance
      + emotional_weight × |valence|
```

Different survival strategies favor different eviction strategies:
- Forager: high importance_weight (remember rich food sources)
- Prey: high emotional_weight (remember terrifying predator encounters)
- Explorer: high recency_weight (keep recent map, forget old locations)

The eviction strategy itself evolves. This is a novel form of evolutionary pressure -- natural selection for metacognition.

### Fixed-size arrays for performance

Using `ArrayVec<MemoryEntry, MAX_CAPACITY>` or similar bounded types avoids per-entity heap allocation. With 10,000 entities × 50 memories × ~64 bytes per entry = ~32MB total. Manageable and cache-friendly.

## Alternatives Considered

| Approach | Rejected Because |
|----------|-----------------|
| Unbounded Vec | Memory grows without limit. 100k entities × unbounded = OOM risk. |
| LRU cache | One-size-fits-all eviction. Removes the evolutionary pressure dimension. |
| No memory | Entities can't learn from experience. Dramatically limits emergent behavior. |
| Neural memory (LSTM/attention) | Too expensive to run per entity per tick at scale. Better for Phase 8 composite organisms. |

## Consequences

- Memory capacity is a heritable trait subject to natural selection
- Entities with poor eviction strategies (e.g., keeping irrelevant memories) waste capacity and make worse decisions
- The behavior tree must include `RecallMemory` condition nodes that query the memory buffer
- Memory entries must be compact -- no strings, no variable-length fields
- The viewer needs a memory inspector UI to show what an entity remembers

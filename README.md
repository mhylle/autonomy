# Autonomy

An artificial life simulation platform where civilizations emerge from simple cellular entities.

## Vision

Autonomy simulates a world of autonomous entities that eat, reproduce, evolve, remember, communicate, and compose into increasingly complex organisms. No behavior is hard-coded beyond the simplest drives. Everything else -- cooperation, aggression, culture, settlement, trade -- emerges from environmental pressure and natural selection.

The end goal: zoom in to see an individual entity's life story, memories, and lineage. Zoom out to watch civilizations rise and fall.

## Architecture

```
┌────────────────────┐              ┌─────────────────────────┐
│  Simulation Engine │   events     │    WebSocket Server     │
│  (Rust / hecs ECS) │──────────────▶  (tokio + axum)        │
│                    │   commands   │                         │
│  Tick-based,       │◀──────────────                        │
│  deterministic     │  (crossbeam) │                         │
└────────────────────┘              └────────┬────────────────┘
                                             │ WebSocket + Protobuf
                                    ┌────────▼────────────────┐
                                    │   Browser Viewer(s)     │
                                    │   (React + PixiJS)      │
                                    └─────────────────────────┘
```

- **Simulation Engine** (Rust): Tick-based, deterministic ECS simulation with event sourcing. Handles entity behavior, evolution, environment, and physics.
- **Viewer** (TypeScript/React): Browser-based visualization with PixiJS for rendering thousands of entities. Pan, zoom, select, interact.
- **Shared Protocol** (Protobuf): Language-neutral schema defining the wire protocol between engine and viewer.

## Key Concepts

### Entities
Autonomous agents with drives (hunger, fear, curiosity, social need), perception, and behavior trees that define their decision-making. Simple entities are single cells. Complex entities are composite organisms made of multiple cells with specialized roles.

### Behavior Trees
Each entity's decision-making is encoded as a behavior tree stored in its genome. Behavior trees evolve via genetic programming -- subtree crossover, structural mutation, and parameter tuning across generations. The decision architecture itself is subject to natural selection.

### Memory
Entities have bounded memory with a capacity determined by their genome. When memory is full, an eviction strategy (also encoded in the genome) decides which memories to discard. Entities that evolve better memory management -- remembering food locations, threats, and allies -- survive longer. Memory management is evolutionary pressure.

### Composition
Entities can merge into composite organisms. Members take on specialized roles (locomotion, sensing, attack, defense, digestion, reproduction). The composite gains aggregate capabilities but requires internal coordination. If the composite dies, members may survive independently.

### Evolution
No separate "generations." Evolution is continuous. When entities reproduce, their genomes (including behavior trees) undergo crossover and mutation. Environmental pressure -- food scarcity, predation, climate change -- drives natural selection. Over time, behavior trees restructure, new strategies emerge, and species diverge.

### Emergent Storytelling
The system tracks interesting entities and detects narrative arcs (rivalries, alliances, migrations, extinctions). An optional LLM layer can narrate these arcs as prose. Every event is logged for replay and historical analysis.

## Project Structure

```
autonomy/
├── simulation-engine/    # Rust: ECS simulation, behavior trees, evolution
├── viewer/               # TypeScript: React + PixiJS browser visualization
├── shared/proto/         # Protobuf schema definitions
├── docs/                 # Solution description, ADRs
│   ├── solution.md
│   └── adr/
└── PLAN.md               # Phased implementation plan
```

## Implementation Phases

1. **Primordial Soup** -- Basic entities, food, reproduction, browser visualization
2. **Behavior and Decision** -- Custom behavior tree engine, drives, perception
3. **Evolution and Diversity** -- Genetic programming, species divergence
4. **Memory and Social Behavior** -- Bounded memory, kin recognition
5. **Composition and Multicellularity** -- Entity merging, cell specialization
6. **Communication and Culture** -- Evolved signals, tribes
7. **Narrative and Intelligence** -- Story arc detection, LLM narration
8. **Civilization Scale** -- 100k+ entities, multi-threaded, settlements

See [PLAN.md](PLAN.md) for full implementation details.

## Documentation

- [Solution Description](docs/solution.md) -- Detailed architecture and design
- [Architecture Decision Records](docs/adr/) -- Key technical decisions and rationale

## License

TBD

# Autonomy -- Solution Description

## 1. Problem Statement

Existing artificial life simulations either achieve scale without meaningful agent complexity (cellular automata, particle systems) or achieve rich agent behavior without scale (complex ABM frameworks). None provide a platform where:

- Thousands of entities with individual behavior, memory, and lineage coexist
- Entity behavior evolves structurally (not just parameter tuning) over generations
- Simple entities compose into complex multi-cellular organisms with emergent specialization
- The system produces observable, narratable emergent stories
- Users can observe and interact with the simulation in real-time through a browser

Autonomy aims to bridge this gap: a scalable simulation engine with deep per-entity complexity, real-time browser visualization, and emergent narrative generation.

## 2. System Overview

Autonomy is a three-component system:

| Component | Language | Purpose |
|-----------|----------|---------|
| Simulation Engine | Rust | Deterministic tick-based simulation of entities, environment, evolution |
| Viewer | TypeScript (React + PixiJS) | Real-time browser visualization, interaction, and observation |
| Shared Protocol | Protobuf | Wire protocol between engine and viewer |

The simulation engine runs as a standalone process. One or more browser viewers connect via WebSocket. The engine streams tick deltas; viewers send commands (kill entity, spawn entity, modify terrain).

## 3. Simulation Engine

### 3.1 Entity-Component-System Architecture

The engine uses an ECS architecture (via the `hecs` crate) where:

- **Entities** are lightweight identifiers (integer IDs)
- **Components** are plain data structs attached to entities (position, energy, genome, memory, etc.)
- **Systems** are functions that query entities by component sets and process them each tick

This design enables:
- Adding new behaviors by adding new components and systems without modifying existing code
- High cache performance through contiguous memory layout (archetype-based storage)
- Straightforward serialization of all entity state

### 3.2 Component Architecture

Components are organized in layers of increasing complexity, introduced across implementation phases:

**Layer 1 -- Physical** (Phase 1)
- `Position` / `Velocity` -- spatial coordinates and movement vector
- `Energy` -- current/max energy, metabolism rate
- `Health` -- current/max hit points
- `Age` -- ticks alive, max lifespan
- `Size` -- collision radius
- `Identity` -- generation count, parent references, birth tick, species ID

**Layer 2 -- Cognitive** (Phase 2-3)
- `Perception` -- sensor range, perceived entities and resources (populated each tick)
- `Drives` -- hunger, fear, curiosity, social need, aggression, reproductive urge
- `BehaviorTree` -- the entity's decision-making tree (from genome)
- `Genome` -- all heritable traits including behavior tree genes, physical traits, cognitive traits, mutation rates

**Layer 3 -- Social** (Phase 4)
- `Memory` -- bounded ring buffer of experiences with evolved eviction weights
- `Social` -- relationship scores with other entities, tribe membership

**Layer 4 -- Compositional** (Phase 5+)
- `CompositeBody` -- links to member entities, role assignments, energy distribution
- `Signal` -- emitted signals for communication

### 3.3 Tick Pipeline

Each tick executes systems in a fixed, deterministic order:

```
1. environment     -- resource regrowth, climate update
2. perception      -- spatial query, populate what each entity sees
3. drives          -- update drive levels from internal state
4. decision        -- tick behavior trees, produce action for each entity
5. movement        -- resolve movement, collision detection
6. feeding         -- consume resources, gain energy
7. combat          -- resolve attacks, apply damage
8. reproduction    -- spawn offspring with crossover + mutation
9. composition     -- entity merging/splitting
10. memory         -- record experiences, evict if over capacity
11. aging          -- increment age, apply metabolism, check lifespan
12. cleanup        -- despawn dead entities
13. event_emit     -- write all events to the event log
14. snapshot       -- periodic full-state snapshot
15. network_sync   -- compute deltas, push to connected viewers
```

Determinism is guaranteed by:
- Seeded PRNG (ChaCha8) per system, derived from a master seed
- Fixed system execution order
- No floating-point non-determinism (single-platform guarantee)

### 3.4 Behavior Tree Engine

Each entity's decision-making is a tree of nodes:

- **Control flow nodes**: Sequence (run children in order, fail on first failure), Selector (try children, succeed on first success), Inverter, RepeatWhile
- **Condition nodes**: Check drives, perceive nearby entities/resources, check energy, recall memories, random chance
- **Action nodes**: Move toward/away from targets, eat, attack, reproduce, rest, signal, attempt composition

The tree is ticked once per simulation tick. The first action node that succeeds determines the entity's action for that tick. The action is stored as a temporary `Action` component, consumed by subsequent systems (movement, feeding, combat, etc.).

**Evolution**: Behavior trees are stored in the genome and subjected to genetic operators:
- *Subtree crossover*: swap random subtrees between two parents
- *Parameter mutation*: tweak thresholds, probabilities, speed factors
- *Structural mutation*: insert, delete, or replace nodes
- *Simplification*: remove redundant nodes (e.g., Sequence with one child)

### 3.5 Homeostatic Drive System

Drives are computed from internal state, not hard-coded:

```
hunger       = 1.0 - (energy.current / energy.max)
fear         = f(perceived_threats, health, memory_of_attacks)
curiosity    = f(time_in_same_area, genome.base_curiosity)
social_need  = f(time_since_social_contact, genome.base_social_need)
aggression   = f(hunger, perceived_weakness_of_others, genome.base_aggression)
reproductive = f(energy_surplus, age, genome.base_reproductive_urge)
```

Behavior tree condition nodes check these drives against evolved thresholds. An entity whose genome sets a low hunger-threshold for seeking food will eat sooner; one with high aggression base will fight more. Natural selection determines which drive calibrations survive.

**Emergent emotions**: When entities develop memory and social tracking (Phase 4+), higher-order states emerge naturally:
- *Attachment*: entity A consistently reduces entity B's social_need drive → B's memory associates A with positive valence → B seeks A
- *Rivalry*: repeated negative interactions between two entities create mutual high-threat memories → avoidance or escalating aggression
- *Grief*: a bonded entity dies → social_need spikes with no resolution target

These are not programmed. They fall out of the drive + memory + behavior tree interaction.

### 3.6 Memory System

Each entity has a bounded memory (capacity from genome, typically 10-50 entries). Each entry records:
- When it happened (tick)
- What happened (found food, was attacked, reproduced, etc.)
- Self-assessed importance score
- Emotional valence (-1.0 to +1.0)
- Where it happened
- Who was involved

When memory is full, a scoring function determines which memory to evict:

```
score = recency_weight * recency(tick)
      + importance_weight * importance
      + emotional_weight * |valence|
```

The weights are part of the genome. Entities that evolve to remember food locations over irrelevant encounters find food more efficiently. Entities that remember threats avoid predators. The memory management strategy is itself subject to natural selection.

### 3.7 Entity Composition

When two or more compatible entities are adjacent and conditions are met (compatible genomes, environmental pressure, intentional via behavior tree), they can merge:

1. A new composite entity spawns with a `CompositeBody` component
2. Member entities lose independent positions (move with the composite)
3. Members retain their genomes and memories
4. Each member is assigned a role based on genome traits: locomotion, sensing, attack, defense, digestion, reproduction
5. The composite's capabilities are aggregated from its members

The composite entity uses the "leader" member's behavior tree for high-level decisions, augmented by member capabilities. If the composite dies or resources are scarce, it can decompose -- members survive independently.

Over evolutionary time, entities that compose effectively gain survival advantages. Specialization emerges: some entities evolve toward being good "sensor cells," others toward "muscle cells."

### 3.8 Environment

The world is a 2D continuous space (expandable to 3D in future) with:

- **Terrain**: Noise-generated (Perlin/Simplex) biomes -- grassland, desert, water, forest. Affects movement speed and resource density.
- **Resources**: Food nodes that deplete when consumed and regrow over time. Different types in different biomes.
- **Climate**: Temperature and seasonal cycles. Long-term drift creates environmental pressure.
- **Spatial index**: Grid-based spatial hash for O(n*k) proximity queries instead of O(n^2).

### 3.9 Event Sourcing

Every state change produces an immutable event appended to the event log:
- EntitySpawned, EntityDied, EntityMoved, EntityAte, EntityAttacked
- EntityReproduced, EntitiesComposed, EntityDecomposed
- ResourceSpawned, ResourceDepleted, EnvironmentChanged

Periodic snapshots (every N ticks) capture full world state. To replay to any tick: load nearest preceding snapshot, replay events forward. This enables:
- Full history browsing
- "What-if" experiments (fork from a snapshot, change parameters)
- Narrative generation from event sequences
- Debugging (reproduce any bug by replaying to the problematic tick)

## 4. Viewer

### 4.1 Rendering

PixiJS v8 (WebGL-backed) renders entities, terrain, and resources. At different zoom levels:

- **Zoomed out**: Entities as colored dots (color = species), terrain as tiled background. Minimal data per entity.
- **Mid zoom**: Entity shapes with size reflecting actual size, energy bars, action indicators.
- **Zoomed in**: Full detail -- sensor ranges, movement paths, relationship lines, behavior state.

The viewer subscribes to a viewport region. The engine only streams entities visible in that viewport (plus a buffer zone), reducing bandwidth.

### 4.2 Interaction

Users can interact with the simulation through the viewer:

- **Observe**: Click any entity to see its stats, memory, lineage, behavior tree, and relationships
- **Control**: Take direct control of an entity (override its behavior tree with manual commands)
- **God mode**: Kill entities, spawn entities, modify terrain, add/remove resources, trigger climate events
- **Time control**: Play/pause, adjust simulation speed, scrub through history

### 4.3 Narrative Panel

The viewer displays detected story arcs:

- Entities automatically tracked when they meet interest criteria (long-lived, many offspring, many kills, extensive travel)
- Arc patterns detected from event sequences: rivalry, alliance, migration, extinction, rise-to-power
- Entity "biographies" showing life story as a sequence of significant events
- Optional LLM-generated prose narration of ongoing arcs

## 5. Communication Protocol

The engine and viewer communicate via WebSocket using Protocol Buffer messages:

**Server → Client**:
- `WorldSnapshot`: Full state dump (on connect or seek)
- `TickDelta`: Per-tick changes -- entity spawns/moves/updates/deaths, resource changes, events
- `EntityDetail`: Detailed info for a selected entity (memory, drives, BT state, lineage)
- `NarrativeUpdate`: Story arc events

**Client → Server**:
- `SubscribeViewport`: Camera position and zoom level (determines what the server streams)
- `RequestEntityDetail`: Request full details for a specific entity
- `SimulationCommand`: Kill, spawn, modify terrain, pause/resume
- `SetSpeed`: Adjust simulation tick rate
- `SeekTick`: Jump to a specific tick (triggers snapshot load + replay)

Delta streaming sends only changed entities each tick. At high zoom levels, only position + color is sent. At low zoom levels, full state including drives and memory.

## 6. Evolution and Natural Selection

Evolution operates continuously through reproduction:

1. Entity reaches energy threshold → initiates reproduction
2. If another compatible entity is adjacent → sexual reproduction (crossover of both genomes)
3. Otherwise → asexual reproduction (clone with mutation)
4. Offspring genome undergoes mutation (rates controlled by parent's genome)
5. Mutation can alter: physical traits, drive sensitivities, memory capacity, memory eviction weights, behavior tree structure, behavior tree parameters, mutation rates themselves

**Selection pressure** comes from the environment:
- Food scarcity rewards efficient foraging behavior
- Predation rewards threat detection and avoidance (or combat ability)
- Climate change rewards adaptability and migration
- Resource heterogeneity rewards specialization and (eventually) trade

**Species divergence** occurs naturally. Entities are clustered into species by genome similarity (hash of behavior tree structure + key trait values). When mutation accumulates enough difference, a new species is born. Population charts track species over time.

## 7. Scalability Strategy

**Phase 1-6** (up to ~10,000 entities):
- Single-threaded simulation
- Spatial hash for proximity queries
- Delta streaming to viewers
- In-memory event log with periodic snapshot to disk

**Phase 7-8** (10,000-100,000+ entities):
- Multi-threaded simulation: world partitioned into regions, each processed by a separate thread
- LOD simulation: distant regions simulate at lower fidelity (skip perception, simplified behavior)
- Compressed event log storage
- Multiple simultaneous viewer connections
- Save/load full simulation state

## 8. Design Principles

1. **Environmental pressure, not hard-coded behavior**: Never code cooperation, fear, or culture. Create conditions where those behaviors have survival advantages. Let evolution find them.

2. **Determinism**: Same seed = same simulation. Every run is reproducible. Every bug is reproducible.

3. **Event sourcing**: The event log is the source of truth. Any question about history can be answered by replaying events.

4. **Separation of simulation and presentation**: The engine knows nothing about rendering. Multiple viewers can connect. The simulation runs headless if needed.

5. **Composition over inheritance**: ECS components, not class hierarchies. New behaviors are new components + systems, not modifications to existing code.

6. **"Keep components simple, remember the complex solution"**: Each phase delivers working software. Each component's interface is designed for the final vision even when current usage is simple.

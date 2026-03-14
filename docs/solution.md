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

### 3.8 Communication and Signals

Entities can emit signals into the world. Signals have a type (a u8 integer), position, radius, and decay rate. Crucially, signals have **no inherent meaning**. Signal meanings emerge through co-evolution of emitters and receivers:

- An entity that emits signal type 3 when finding food, combined with entities that respond to type 3 by approaching → both survive better → signal 3 becomes "food here"
- Alarm calls, mating calls, territorial warnings -- all emerge from evolutionary pressure, not design

This is the foundation of proto-language. Isolated populations develop different signal conventions (cultural divergence).

### 3.9 Tool Use and Construction

Objects are entities with `MaterialProperties` -- continuous floats (hardness, sharpness, flexibility, density, flammability, nutritional value, durability, insulation). There are **no predefined tool types**. A "weapon" is any object with high sharpness and hardness. A "basket" is any object with high flexibility and low density.

**Capability modifiers** are computed from material properties:
```
attack_bonus     = sharpness * hardness * sqrt(length) * sqrt(mass)
defense_bonus    = hardness * width * density
carry_bonus      = flexibility * width * (1.0 - density)
harvest_bonus    = sharpness * hardness
```

**Blueprints** (recipes for creating objects) are encoded in the genome and evolve via GP, alongside behavior trees. They are also transmittable culturally:
- **Genetic blueprints**: inherited, evolve slowly across generations
- **Learned blueprints**: acquired by observation or teaching, spread culturally within a single generation, can be forgotten

Cultural transmission is faster than genetic evolution. This creates a technology acceleration effect where one innovative entity can teach an entire tribe.

**Structures** are objects placed in the world that affect the spatial index:
- Hard materials in a line → wall (blocks movement)
- Insulating materials forming enclosure → shelter (reduces environmental damage)
- Planted resources → farm (increased food production)
- Hard flat materials in a path → road (movement speed bonus)

Structures are not special types. They emerge from the spatial arrangement of objects with specific material properties.

**Agriculture**: Entities can plant resources (the `PlantedBy` component). Planted resources grow over time, grow faster when tended, and produce higher yield than wild resources. Farming knowledge spreads culturally.

**Construction sites**: Complex structures require multi-tick, multi-entity building. A `ConstructionSite` component tracks required materials, deposited materials, work progress, and contributors. This naturally produces collaborative construction and division of labor.

### 3.10 Environment

The world is a 2D continuous space (expandable to 3D in later eras) with:

- **Terrain**: Noise-generated (Perlin/Simplex) biomes -- grassland, desert, water, forest, mountain. Affects movement speed and resource density.
- **Resources**: Food and material nodes that deplete when consumed and regrow over time. Different types in different biomes. Resources have material properties (wood is flexible and light, stone is hard and heavy).
- **Climate**: Temperature and seasonal cycles. Long-term drift creates environmental pressure. Periodic events: droughts, resource scarcity.
- **Spatial index**: Grid-based spatial hash for O(n*k) proximity queries instead of O(n^2). Separate structure grid for placed constructions.
- **Structures**: Persistent constructions that modify the spatial index (block movement, provide shelter, grow food).

### 3.11 Event Sourcing

Every state change produces an immutable event appended to the event log:
- EntitySpawned, EntityDied, EntityMoved, EntityAte, EntityAttacked
- EntityReproduced, EntitiesComposed, EntityDecomposed
- ResourceSpawned, ResourceDepleted, EnvironmentChanged
- ObjectPickedUp, ObjectCrafted, ToolEquipped, StructurePlaced, StructureDestroyed
- KnowledgeTransferred, ResourcePlanted, ResourceHarvested
- ConstructionSiteStarted, ConstructionSiteWorked, ConstructionSiteCompleted

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

## 6. Tribes, Territory, and Society

### 6.1 Group Formation

Entities that maintain positive social relationships above a threshold for sustained periods form **tribes** (TribeId component). Tribes are not designed -- they emerge from social dynamics. Tribe-level data includes member list, territory centroid, population, and collective resources.

### 6.2 Territory

Territory is defined by where tribe members spend most time. Territorial behavior emerges: entities prefer staying near their tribe and show increased aggression toward non-members near territory boundaries. Tribes can fight tribes -- group coordination where members preferentially attack the same target.

### 6.3 Cultural Transmission

Knowledge spreads beyond genetics through **observation and teaching**:
- An entity observes a tribe member's successful action → adds to own memory
- Learned behaviors (food locations, construction techniques, signal meanings) spread within a tribe faster than genetic evolution
- Isolated groups develop different cultural practices (different signal meanings, different construction techniques)

This creates **dual inheritance**: genetic evolution (slow, reliable) + cultural evolution (fast, lossy). The interaction produces a Baldwin effect where cultural innovations create selection pressure that eventually embeds them in genomes.

## 7. Narrative Engine

### 7.1 Event Significance

Not all events are equally interesting. A significance function scores events:
```
significance = state_change_magnitude × rarity × protagonist_relevance × irreversibility × relationship_involvement
```

Events above a threshold enter the narrative buffer. The system auto-tracks "interesting" entities: longest-lived, most offspring, most kills, greatest territory, longest migration.

### 7.2 Arc Detection

Pattern matching on event sequences identifies narrative arcs:
- **Rivalry**: Repeated combat between same pair
- **Alliance**: Sustained cooperation
- **Migration**: Group movement across biomes
- **Extinction**: Species population → 0
- **Rise**: Entity or tribe gaining territory/population
- **Fall**: Losing territory/population

Each arc has protagonists, events, and a tension curve. Multiple arcs tracked concurrently.

### 7.3 LLM Narration

An optional integration with Claude API or similar LLM. Feed narrative arc events to the LLM, receive prose descriptions. Configurable styles: documentary, epic, journal, clinical. Narration cache prevents re-narrating events.

## 8. Civilization

### 8.1 Settlements

Emergent settlements: clusters of structures with persistent population are detected as settlements. Settlements have: population, structures, territory, resources, and a name derived from founding entities.

### 8.2 Trade

Different biomes produce different resources → comparative advantage. Entities carry surplus resources between settlements. Trade routes emerge as frequently traveled paths. Value estimation from memory: entities learn what's scarce elsewhere.

### 8.3 Emergent Hierarchy

Leadership is not assigned. The entity with highest social influence (most relationships, most respect) becomes the de facto leader. Specialization of labor: entities with different BTs fill different roles (farmer, builder, warrior, scout). Succession occurs naturally when leaders die.

## 9. 3D World (Future Era)

The simulation expands from 2D to 3D in 12 phases:
- 3D coordinate system (Position3D, backward-compatible with 2D at Z=0)
- Height-map terrain with elevation affecting movement, visibility, temperature
- Underground caves (3D noise with threshold → cave volumes)
- Three.js renderer replacing PixiJS (InstancedMesh for 100k+ entities)
- 3D spatial index (grid hash or octree)
- Water bodies: floating, swimming, underwater ecology
- Aerial movement: flight as evolved genome trait, gliding, aerial predation
- 3D construction: multi-story structures, bridges, cave dwellings
- Day/night cycle, lighting, weather visualization

## 10. Open World (Future Era)

Infinite, procedurally generated world via chunks:
- Chunk-based world: fixed-size squares, states: Active (full sim), Dormant (statistical), Unloaded (on disk)
- World seed: single seed generates entire infinite world deterministically
- Chunk activation: entity presence or viewer proximity
- Long-distance migration: species spread, colonization of uninhabited chunks
- Multi-civilization encounters: first contact events, communication barriers (different signal "languages"), conflict or cooperation
- Seamless zoom: world overview → continent → settlement → individual entity

## 11. Evolution and Natural Selection

Evolution operates continuously through reproduction:

1. Entity reaches energy threshold → initiates reproduction
2. If another compatible entity is adjacent → sexual reproduction (crossover of both genomes)
3. Otherwise → asexual reproduction (clone with mutation)
4. Offspring genome undergoes mutation (rates controlled by parent's genome)
5. Mutation can alter: physical traits, drive sensitivities, memory capacity, memory eviction weights, behavior tree structure, behavior tree parameters, construction blueprints, mutation rates themselves

**Selection pressure** comes from the environment:
- Food scarcity rewards efficient foraging behavior
- Predation rewards threat detection and avoidance (or combat ability)
- Climate change rewards adaptability and migration
- Resource heterogeneity rewards specialization and (eventually) trade

**Species divergence** occurs naturally. Entities are clustered into species by genome similarity (hash of behavior tree structure + key trait values). When mutation accumulates enough difference, a new species is born. Population charts track species over time.

## 12. Scalability Strategy

**Era 1-5** (up to ~10,000 entities):
- Single-threaded simulation
- Spatial hash for proximity queries
- Delta streaming to viewers
- In-memory event log with periodic snapshot to disk

**Era 8+** (10,000-100,000+ entities):
- Multi-threaded simulation: world partitioned into regions, each processed by a separate thread (rayon)
- LOD simulation: distant regions simulate at lower fidelity (skip perception, simplified BT)
- Compressed event log storage
- Multiple simultaneous viewer connections
- Save/load full simulation state (bincode + zstd compression)

**Era 11** (infinite world):
- Chunk-based world with Active/Dormant/Unloaded states
- Statistical simulation for dormant chunks (population-level birth/death rates)
- Disk-backed chunk storage
- Chunk activation on entity migration or viewer pan

## 13. Expanded Tick Pipeline

The full tick pipeline with all subsystems (later eras add to this):

```
 1. environment        -- resource regrowth, climate, structure decay, farm growth
 2. perception         -- spatial query: entities, resources, objects, structures, signals
 3. drives             -- hunger, fear, curiosity, social_need, aggression, reproductive, constructive
 4. decision           -- tick behavior trees → produce Action component
 5. movement           -- resolve movement, collision, structure blocking
 6. feeding            -- consume resources, harvest farms
 7. combat             -- resolve attacks (with tool modifiers), apply damage
 8. construction       -- resolve build/craft/tool actions
 9. inventory          -- pickup, drop, equip
10. structure          -- place structures, update effects, register in spatial index
11. knowledge          -- observation learning, teaching, blueprint transfer
12. reproduction       -- genome crossover + mutation, spawn offspring
13. composition        -- entity merging/splitting
14. memory             -- record experiences, evict memories
15. aging              -- age, metabolism cost, natural death, object decay
16. cleanup            -- despawn dead entities, broken objects, decayed structures
17. event_emit         -- flush events to log
18. snapshot            -- periodic full-state snapshot
19. network_sync       -- compute deltas, push to WS subscribers
```

## 14. Implementation Status (as of March 2026)

**All 574 roadmap tasks complete.** The simulation engine and viewer are fully implemented across all 11 eras.

| Component | Status |
|-----------|--------|
| Rust simulation engine | ✅ Complete — 776 unit tests, 22 integration tests |
| TypeScript/React viewer | ✅ Complete — all overlays, panels, and charts implemented |
| Protobuf protocol | ✅ Extended — streams entity, tribe, signal, war, settlement, trade, cultural data |
| Documentation | ✅ Complete — roadmap, ADRs, solution description |

Key systems implemented:
- **Behavior trees** with 30+ node types, genetic operators (crossover, mutation, simplification)
- **Memory system** with 9 memory kinds and eviction scoring
- **Social system** with exponential-weighted relationship tracking
- **Signal system** (emergent communication) with decay and LOD-aware perception
- **Tribe/territory** detection from social graph clustering
- **War system** with rolling kill history, WarDeclared/WarEnded events
- **Composition** (multi-cellular organisms) with role specialization
- **Cultural transmission** of behavior patterns and signal usage
- **Civilization** layer: settlements, trade routes, cultural identity, hierarchy
- **Narrative** system: significance scoring, story arc detection, biography compilation
- **LOD/performance** system with rayon parallel perception and viewport streaming
- **Snapshot/replay** with bincode + zstd compression
- **3D coordinate support** with backward-compatible z-axis extension
- **Chunk manager** stub for infinite world generation (Era 11)

## 15. Design Principles

1. **Environmental pressure, not hard-coded behavior**: Never code cooperation, fear, or culture. Create conditions where those behaviors have survival advantages. Let evolution find them.

2. **Determinism**: Same seed = same simulation. Every run is reproducible. Every bug is reproducible.

3. **Event sourcing**: The event log is the source of truth. Any question about history can be answered by replaying events.

4. **Separation of simulation and presentation**: The engine knows nothing about rendering. Multiple viewers can connect. The simulation runs headless if needed.

5. **Composition over inheritance**: ECS components, not class hierarchies. New behaviors are new components + systems, not modifications to existing code.

6. **"Keep components simple, remember the complex solution"**: Each phase delivers working software. Each component's interface is designed for the final vision even when current usage is simple.

# Autonomy: Emergent Life Simulation Platform

**Context**: Build an artificial life simulation where civilizations emerge from simple cellular entities. Rust backend simulation engine with browser-based visualization via WebSocket. ECS architecture with evolvable behavior trees, bounded entity memory, and entity composition into multi-cellular organisms.

---

## Architecture

```
Simulation Thread                    Async Runtime (tokio)
┌────────────────────┐              ┌─────────────────────────┐
│                    │   events     │                         │
│  tick loop:        │──────────────▶  WebSocket Server       │
│    run systems     │  (crossbeam) │    per-client sessions  │
│    produce events  │              │    viewport filtering   │
│    update world    │   commands   │    delta computation    │
│                    │◀──────────────  command handling       │
│                    │  (crossbeam) │                         │
└────────────────────┘              └────────┬────────────────┘
                                             │ WebSocket
                                    ┌────────▼────────────────┐
                                    │  Browser Viewer(s)      │
                                    │    PixiJS rendering     │
                                    │    React UI             │
                                    └─────────────────────────┘
```

### Key Technology Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| ECS | `hecs` | Minimal, fast, no rendering baggage. Full control over tick loop for deterministic simulation. |
| Wire Protocol | Protobuf (`prost` + `ts-proto`) | Language-neutral schema generates both Rust and TypeScript types from single `.proto` source. |
| Behavior Trees | Custom (~300 LOC) | Must be serializable, evolvable via GP, and inspectable. No existing crate supports all three. |
| WebSocket | `tokio-tungstenite` | Async, battle-tested, integrates with tokio runtime. |
| 2D Rendering | PixiJS v8 | WebGL-backed, handles 100k+ sprites. Migration path to Three.js for future 3D. |
| Terrain | `noise` crate | Perlin/Simplex for deterministic terrain generation. |
| Deterministic RNG | `rand_chacha` | Seeded ChaCha8 RNG per system for reproducible simulation. |
| Snapshots | `serde` + `bincode` | Fast binary serialization for save/load, no cross-language need. |

---

## Monorepo Structure

```
autonomy/
├── Cargo.toml                    # Workspace root
├── justfile                      # Task runner
├── shared/
│   └── proto/
│       ├── world.proto           # World state, tick deltas, entity snapshots
│       ├── events.proto          # Event types (born, died, ate, moved, etc.)
│       └── commands.proto        # Client->Server commands (kill, spawn, control)
├── simulation-engine/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── core/
│       │   ├── world.rs          # SimulationWorld: wraps hecs::World + metadata
│       │   ├── tick.rs           # Tick loop, system orchestration
│       │   ├── config.rs         # Simulation configuration
│       │   └── rng.rs            # Deterministic seeded RNG
│       ├── components/
│       │   ├── spatial.rs        # Position, Velocity
│       │   ├── physical.rs       # Energy, Health, Size, Age
│       │   ├── genome.rs         # Genome: traits + behavior tree genes
│       │   ├── behavior.rs       # BehaviorTree component
│       │   ├── memory.rs         # Bounded memory with evolved eviction
│       │   ├── perception.rs     # SensorRange, PerceivedEntities
│       │   ├── social.rs         # Relationships, TribeId
│       │   ├── drives.rs         # Hunger, Fear, Curiosity, SocialNeed, Aggression
│       │   ├── composition.rs    # CompositeBody, CellRole
│       │   └── identity.rs       # Lineage, BirthTick, Generation
│       ├── systems/              # One file per system, executed in deterministic order
│       │   ├── perception.rs     # Spatial query -> populate PerceivedEntities
│       │   ├── decision.rs       # Tick behavior trees -> produce Actions
│       │   ├── movement.rs       # Apply movement, collision
│       │   ├── feeding.rs        # Consume resources, transfer energy
│       │   ├── combat.rs         # Damage resolution
│       │   ├── reproduction.rs   # Genome crossover, spawn offspring
│       │   ├── aging.rs          # Age, metabolism, natural death
│       │   ├── memory.rs         # Record experiences, evict memories
│       │   ├── evolution.rs      # Mutation, selection pressure
│       │   ├── composition.rs    # Entity merging/splitting
│       │   ├── environment.rs    # Resource regrowth, climate
│       │   └── cleanup.rs        # Despawn dead entities
│       ├── behavior_tree/
│       │   ├── node.rs           # BtNode enum, BtStatus, tick engine
│       │   ├── actions.rs        # Leaf action nodes
│       │   ├── conditions.rs     # Condition nodes
│       │   └── evolution.rs      # GP crossover, mutation, simplification
│       ├── environment/
│       │   ├── terrain.rs        # Noise-generated terrain
│       │   ├── resources.rs      # Resource nodes
│       │   └── spatial_index.rs  # Grid-based spatial hash for O(n*k) queries
│       ├── events/
│       │   ├── types.rs          # SimEvent enum
│       │   ├── log.rs            # Append-only per-tick event buffer
│       │   └── snapshot.rs       # Periodic full-state snapshots
│       └── net/
│           ├── server.rs         # tokio WebSocket server
│           ├── protocol.rs       # Protobuf encode/decode
│           └── commands.rs       # Handle viewer commands
└── viewer/
    ├── package.json
    ├── vite.config.ts
    └── src/
        ├── App.tsx
        ├── net/
        │   ├── websocket.ts      # WS client with reconnection
        │   ├── protocol.ts       # Generated protobuf types
        │   └── state-store.ts    # Zustand store from tick deltas
        ├── renderer/
        │   ├── WorldRenderer.ts  # PixiJS setup, camera, zoom
        │   ├── EntityLayer.ts    # Entity sprites
        │   ├── TerrainLayer.ts   # Terrain tiles
        │   └── ResourceLayer.ts  # Resource dots
        └── ui/
            ├── HUD.tsx           # Tick, population, FPS
            ├── EntityPanel.tsx   # Selected entity details
            ├── TimelineBar.tsx   # Play/pause/speed
            └── ControlPanel.tsx  # God-mode controls
```

---

## Tick Pipeline (Deterministic Order)

```
 1. environment_system   → resource regrowth, climate
 2. perception_system    → spatial query, populate PerceivedEntities
 3. drives_system        → update drives from state (energy→hunger, etc.)
 4. decision_system      → tick behavior trees → produce Action component
 5. movement_system      → resolve movement, collision
 6. feeding_system       → eat resources, transfer energy
 7. combat_system        → resolve attacks, apply damage
 8. reproduction_system  → spawn offspring with crossover+mutation
 9. composition_system   → entity merging/splitting
10. memory_system        → record experiences, evict oldest/least-important
11. aging_system         → increment age, metabolism cost, natural death
12. cleanup_system       → despawn dead, emit death events
13. event_emit_system    → write events to EventLog
14. snapshot_system       → every N ticks, full state snapshot
15. network_sync_system  → compute deltas, push to WS subscribers
```

---

## Core Data Model

### Entity Components (simplified for Phase 1, expanded in later phases)

**Phase 1 components**: Position, Velocity, Energy, Health, Age, Size, SimpleGenome, Identity
**Phase 2 adds**: Perception, Drives, BehaviorTree
**Phase 3 adds**: full Genome (with BT genes), Species tracking
**Phase 4 adds**: Memory (bounded, with evolved eviction weights), Social relationships
**Phase 5 adds**: CompositeBody, CellRole

### Behavior Tree Node (the unit of evolution)

```
BtNode = Sequence([BtNode]) | Selector([BtNode]) | Inverter(BtNode)
       | CheckDrive(drive, threshold) | NearbyEntity(range, filter)
       | NearbyResource(range, type) | CheckEnergy(threshold)
       | RecallMemory(kind, max_age) | RandomChance(probability)
       | MoveTowardEntity(filter, speed) | MoveTowardResource(type, speed)
       | FleeFrom(filter, speed) | Wander(speed)
       | Eat | Attack(force) | Reproduce | Rest | Signal(type)
       | CompositionAttempt
```

Evolution operators: subtree crossover, parameter mutation, structural mutation (insert/delete/replace nodes), simplification.

### Memory System

Each entity has a bounded memory (capacity from genome, typically 10-50 entries). Each memory entry has: tick, kind (found food, was attacked, reproduced, etc.), importance score, emotional valence, location, associated entity.

**Eviction** uses genome-evolved weights: `recency_weight * recency_score + importance_weight * importance + emotional_weight * |valence|`. Entities that evolve better memory management strategies survive longer. The memory strategy itself is subject to natural selection.

### Entity Composition

When entities merge: new entity spawns with `CompositeBody` linking members. Members lose independent position, keep genomes/memories. Members take on roles (locomotion, sensing, attack, defense, digestion, reproduction). Composite dies → members may survive independently.

---

## Implementation Roadmap

The full detailed roadmap with task-level breakdowns is in [docs/roadmap.md](docs/roadmap.md) -- 11 Eras, 60+ Phases, hundreds of concrete tasks.

### Summary

### Phase 1: "Scaffolding"
**Goal**: Project structure, build pipeline, basic ECS world.

Cargo workspace setup, hecs world wrapper, tick loop skeleton, basic components (Position, Velocity, Energy), deterministic seeded RNG, simulation config, basic CLI with clap. No entities yet -- just the infrastructure that everything builds on.

### Phase 2: "Primordial Soup"
**Goal**: A living world. Entities exist, move, consume energy, and die.

Spawn entities with Position + Velocity + Energy + Health + Age + Size. Hardcoded behavior (random wander). Flat 2D world with randomly scattered food resources that regrow. Spatial index (grid hash). Feeding system (eat adjacent food). Aging system (metabolism drains energy, lifespan limit). Cleanup system (despawn dead). Event log (in-memory). Basic entity lifecycle: spawn → wander → eat → age → die.

### Phase 3: "First Life"
**Goal**: Entities reproduce. Populations fluctuate. Traits mutate.

Simple genome (physical traits only: max_energy, metabolism_rate, max_speed, sensor_range, size, lifespan). Asexual reproduction (when energy > threshold, split with mutated genome). Identity component (generation, parents, birth tick). Species ID via genome hash. Population dynamics: birth rate vs. death rate, carrying capacity from food supply. First emergent behavior: entities with better metabolism/speed genes dominate.

### Phase 4: "The Viewer"
**Goal**: See the simulation in a browser. Real-time.

Protobuf schema (world.proto). WebSocket server (tokio + axum + tokio-tungstenite). Stream full world state each tick. Vite + React + PixiJS viewer. Render entities as colored circles (color = species hash). Render food as green dots. Pan/zoom with mouse. HUD: tick counter, entity count, FPS. Click entity to see stats panel. Play/pause/speed control.

### Phase 5: "Senses"
**Goal**: Entities perceive the world. They move toward food instead of wandering randomly.

Perception component (sensor_range, perceived entities/resources). Perception system: spatial query → populate what each entity sees. Hardcoded behavior upgraded: move toward nearest visible food when hungry, wander when not. Delta streaming (only send changes, not full state). Viewport subscription (server only sends visible entities).

### Phase 6: "Behavior Trees"
**Goal**: Replace hardcoded behavior with the BT engine. Entities make real decisions.

Custom BT engine: BtNode enum, tick function, BtStatus. Initial action nodes: MoveTowardResource, Wander, Eat, Rest. Initial condition nodes: CheckDrive(hunger), NearbyResource. Drives component: hunger (computed from energy ratio). BehaviorTree component wrapping a BtNode tree. Decision system: tick BT → produce Action component. Hand-designed starter BT: `Selector(Sequence(CheckHungry, NearbyFood, MoveToFood, Eat), Wander)`.

### Phase 7: "Evolution Begins"
**Goal**: Behavior trees are part of the genome and evolve via GP.

Genome now includes BtNode as behavioral DNA. Sexual reproduction: two-parent crossover of genomes. BT crossover operator (swap random subtrees). BT parameter mutation (Gaussian noise on thresholds/speeds). BT structural mutation (insert/delete/replace nodes). Bloat control (depth limit, simplification). Mutation rates as genome parameters (evolvable). First truly emergent behaviors appear.

### Phase 8: "Diverse World"
**Goal**: Terrain, biomes, and environmental pressure drive speciation.

Noise-generated terrain (Perlin/Simplex) with biomes: grassland, desert, water, forest. Terrain affects movement speed and resource density. Different resource types in different biomes. Species tracking and clustering. Viewer: species color coding, population-over-time charts, terrain rendering, minimap. Environmental pressure events: periodic resource scarcity, droughts.

### Phase 9: "Memory"
**Goal**: Entities remember. Behavior changes based on past experience.

Memory component: bounded ring buffer with genome-encoded capacity. Memory entry types: FoundFood, WasAttacked, Encountered, NearDeath, Migrated. Memory formation system: significant events → memory entries. Memory eviction with evolved weights (recency, importance, emotional valence). New BT nodes: RecallMemory condition, MoveTowardMemory action. Entities return to remembered food locations, avoid remembered threats.

### Phase 10: "Social World"
**Goal**: Entities recognize each other. Relationships form.

Social component: relationship scores with other entities. Kin recognition (shared genome similarity). Positive/negative interaction tracking. Social need drive. BT nodes for social behavior: seek kin, avoid enemies. Viewer: relationship graph, lineage tree. Replay system: snapshots to disk, load snapshot and replay forward.

### Phase 11: "Combat and Competition"
**Goal**: Entities fight. Predator-prey dynamics emerge.

Combat system: attack action, damage resolution, death by killing. Aggression drive. Fear drive (from perceived threats + attack memories). BT nodes: Attack, FleeFrom. Coevolutionary arms race: prey evolve better evasion, predators evolve better pursuit. Entity size/strength trade-offs. Viewer: combat events, kill counts, predator/prey species visualization.

### Phase 12: "Composition"
**Goal**: Entities merge into multi-cellular organisms.

CompositeBody component with member tracking. Composition triggers: proximity + compatible genomes + BT action. CellRole assignment based on genome traits. Aggregate capabilities (combined speed, sensing, attack). Energy distribution within composites. Decomposition on death or resource scarcity. Viewer: composite visualization (cluster rendering), member drill-down.

### Phase 13: "Specialization"
**Goal**: Cell differentiation emerges. Composite organisms develop structure.

Role optimization through evolution: some lineages evolve toward Sensing specialization, others toward Locomotion. Composite reproduction: entire organism reproduces (not just individual cells). Composite behavior tree: leader's BT augmented by member capabilities. Performance optimization: batch processing systems, spatial index tuning. Persistent event log to disk.

### Phase 14: "Signals and Communication"
**Goal**: Entities emit and perceive signals. Proto-language emerges.

Signal system: entities emit signals (type = evolved integer, no inherent meaning). Signal perception within range. BT nodes: Signal(type) action, DetectSignal(type) condition. Evolved signal meanings: entities that associate useful responses with signals survive better. Alarm calls, food signals, mating signals -- all emergent. Viewer: signal visualization as expanding rings.

### Phase 15: "Tribes and Territory"
**Goal**: Persistent groups form. Territory emerges.

TribeId component: entities that maintain positive social bonds form tribes. Tribe-level statistics (population, territory, resources). Territorial behavior: prefer to stay near tribe, defend territory from outsiders. Group-level combat: tribes can fight tribes. Teaching: entities observe successful actions by tribe members and add to memory. Viewer: tribe territories, tribal boundaries.

### Phase 16: "Narrative Engine"
**Goal**: The system identifies and tells stories.

Narrative tracker: auto-identify interesting entities (longest-lived, most offspring, most kills, most territory, greatest migration). Arc detector: pattern match on event sequences (rivalry, alliance, migration, extinction, rise-to-power). Story arc data structure with protagonist, events, tension curve. Entity biography: chronological life story from event log. Viewer: narrative panel, entity biography page, "follow this story" mode.

### Phase 17: "History and Replay"
**Goal**: Full history browsing. Time travel.

Timeline scrubbing: drag through simulation history. Seek to any tick (load snapshot, replay forward). Bookmark system: save interesting moments. "What-if" forking: branch from any snapshot with modified parameters. Historical analytics: population graphs, species charts, resource levels over time. Event search: find all events involving entity X, or all events of type Y.

### Phase 18: "LLM Narration"
**Goal**: AI-generated prose narration of emergent stories.

LLM integration (Claude API or similar): feed narrative arc events to LLM, receive prose descriptions. Real-time narration of ongoing arcs. Historical narration: generate prose summary of an entity's life or a civilization's history. Configurable narration style (documentary, epic, journal). Viewer: narration panel with generated text alongside simulation events.

### Phase 19: "Scale"
**Goal**: 100k+ entities. Multi-threaded simulation.

Multi-threaded tick pipeline: partition world into regions, process in parallel. Thread-safe spatial index. LOD simulation: distant regions simulate at lower fidelity (fewer perception queries, simplified BT). Save/load full simulation state to disk. Multiple simultaneous viewer connections. Performance profiling and optimization.

### Phase 20: "Civilization"
**Goal**: Settlements, trade, and emergent civilization.

Emergent settlements: groups of entities that stay in one location become settlements. Resource specialization: different biomes produce different resources → trade incentives. Technology analogue: composite organisms with evolved specialized capabilities. NEAT integration: neural network weights within BT decision nodes for more nuanced behavior. Historical records: civilization timelines, rise and fall detection. Viewer: civilization-scale map view, zoom from world overview to individual entity.

### Phase 21: "3D World"
**Goal**: Expand from 2D to 3D.

3D position and movement. 3D terrain generation. Three.js renderer (replacing PixiJS). 3D spatial index. Volumetric environment: underground, surface, aerial. New movement modes: swimming, climbing, flying (emergent from evolved traits). Viewer: 3D camera controls, cross-section views.

### Phases 23-30: Tool Use & Construction (Era 6)
Object system with material properties, tool creation/use, structures, crafting, agriculture, storage, infrastructure. No predefined tool types -- tools emerge from material property combinations.

### Phases 31-35: Narrative (Era 7)
Event significance scoring, arc detection, entity biographies, history browser, LLM narration.

### Phases 36-40: Scale (Era 8)
Performance profiling, multi-threaded simulation, LOD, save/load, multiple viewers.

### Phases 41-47: Civilization (Era 9)
Settlements, resource specialization, trade, defense/warfare, emergent hierarchy, cultural identity, NEAT integration.

### Phases 48-59: The Third Dimension (Era 10)
3D coordinates, height maps, caves, Three.js renderer, 3D entities, camera system, spatial index, water, flight, lighting, 3D construction, performance optimization.

### Phases 60-65: Open World (Era 11)
Chunk system, procedural generation, chunk loading, long-distance migration, multi-civilization encounters, world-scale analytics.

---

## Key Design Principles

1. **Determinism**: Seeded RNG per system. Same seed = same simulation. Enables replay, debugging, "what-if" comparison.

2. **Event Sourcing**: Event log is source of truth for history. Snapshots are optimization checkpoints. Any query can be answered by replaying events.

3. **Never Hard-Code Emergence**: Don't code cooperation, fear, or culture. Create environmental conditions that reward those behaviors. Let natural selection do the design work.

4. **Separation of Sim and Presentation**: Engine produces events + responds to commands. Viewer is pure consumer. Multiple viewers can connect.

5. **Component Simplicity**: Plain structs, no methods. All logic in systems. Trivially serializable and inspectable.

6. **"Remember the Complex Solution"**: Each phase's interfaces are designed for the final vision. Phase 1's `Genome` struct has room for BT genes even before BTs exist. Phase 1's event system can handle composition events even before composition exists.

---

## Verification

After Phase 1 implementation:
```bash
# Start simulation engine
cd simulation-engine && cargo run -- --seed 42 --world-size 500 --initial-entities 100 &

# Start viewer
cd viewer && npm run dev

# Open browser at localhost:5173 and verify:
# - Entities visible as colored circles moving around
# - Food dots appear and get consumed
# - Population fluctuates (births and deaths)
# - Click entity shows stats panel
# - Play/pause works
```

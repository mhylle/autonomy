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

## Implementation Phases

### Phase 1: "Primordial Soup"
**Goal**: Living world you can watch. Entities wander, eat, reproduce, die.

**Engine**: Project scaffold, hecs world, tick loop, basic components (Position, Velocity, Energy, Health, Age), hardcoded behavior (move toward food when hungry, wander otherwise), flat 2D world with random food that regrows, spatial index, asexual reproduction with trait mutation, event log, WebSocket server streaming world state, deterministic seeded RNG.

**Viewer**: Vite + React + PixiJS, WS client, render entities as colored circles + food as green dots, pan/zoom, HUD (tick/population/FPS), click entity for stats, play/pause.

**Shared**: Initial protobuf schema, code generation script.

### Phase 2: "Behavior and Decision"
**Goal**: Replace hardcoded behavior with behavior trees. Entities make decisions.

Custom BT engine, Perception + Drives + BehaviorTree components, hand-designed starter BT (check hunger→seek food→eat, else wander), genome includes BT, delta streaming (not full state), viewport subscription.

### Phase 3: "Evolution and Diversity"
**Goal**: Behavior trees evolve via GP. Species diverge. Surprising behaviors emerge.

BT crossover + mutation operators, sexual reproduction, species tracking via genome hash, terrain with noise-generated biomes, environmental pressure events (resource scarcity), viewer: species colors, population charts, BT visualizer.

### Phase 4: "Memory and Social Behavior"
**Goal**: Entities remember and recognize each other. Social behaviors emerge.

Memory component with bounded eviction, memory-influenced BT nodes, social relationship tracking, kin recognition, viewer: memory inspector, lineage tree, relationship graph, replay from snapshots.

### Phase 5: "Composition and Multicellularity"
**Goal**: Entities merge into composite organisms. Specialization emerges.

CompositeBody component, cell roles + differentiation, energy distribution within composites, decomposition on death, viewer: composite visualization, performance optimization (batch processing), persistent event log to disk.

### Phase 6: "Communication and Culture"
**Goal**: Entities communicate via signals. Proto-culture emerges.

Signal system (evolved signal types with no inherent meaning), signal perception, group/tribe formation, teaching (observe successful actions), viewer: signal visualization, tribe territories.

### Phase 7: "Narrative and Intelligence"
**Goal**: The system tells stories. Interesting entities get narrative arcs.

Auto-identify interesting entities, arc detection (rivalry, alliance, exodus), structured narrative events, optional LLM narration, viewer: narrative panel, entity biography, timeline scrubbing, bookmarks.

### Phase 8: "Civilization Scale"
**Goal**: 100k+ entities. Settlements. Trade. History.

Multi-threaded simulation (region partitioning), LOD simulation, emergent settlements, resource specialization (trade incentives), NEAT integration for neural decision weights, historical records, save/load, multiple simultaneous viewers.

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

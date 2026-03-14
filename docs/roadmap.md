# Autonomy -- Detailed Roadmap

Hierarchical breakdown: **Eras** → **Phases** → **Tasks**

- **Era**: Major capability milestone. A sentence describing what the simulation can do after this era.
- **Phase**: A deliverable unit of work. Results in a working, testable increment.
- **Task**: A concrete implementation step within a phase.

---

## Era 1: Foundation

*After this era: entities exist in a 2D world, wander, eat, and die. You can watch it in a browser.*

### Phase 1.1: Project Scaffolding

**Goal**: Build infrastructure. Nothing runs yet.

- [x] Cargo workspace with `simulation-engine` crate
- [x] `hecs` dependency, `SimulationWorld` wrapper struct
- [x] Tick loop skeleton (`fn tick(&mut self)` that does nothing yet)
- [x] Deterministic RNG module (`rand_chacha`, seeded, per-system RNG derivation)
- [x] `SimulationConfig` struct (world size, tick rate, seed, initial entity count)
- [x] CLI entry point with `clap` (seed, config file, headless mode flag)
- [x] Logging setup (`tracing` + `tracing-subscriber`)
- [x] Basic test harness: spawn empty world, run 100 ticks, assert no crash

### Phase 1.2: Primordial Soup

**Goal**: Entities exist, move, consume energy, and die. No reproduction yet.

- [x] Components: `Position`, `Velocity`, `Energy`, `Health`, `Age`, `Size`
- [x] `spawn_entity()` function: create entity with random position and default components
- [x] Movement system: apply velocity to position, basic boundary wrapping/clamping
- [x] Aging system: increment age, drain energy by metabolism rate each tick
- [x] Death check: entity dies when energy ≤ 0 or age > max_lifespan
- [x] Cleanup system: despawn dead entities, log death events
- [x] Hardcoded behavior: random wander (pick random direction, move)
- [x] Test: spawn 50 entities, run 1000 ticks, verify all eventually die (no food yet)

### Phase 1.3: Resources and Feeding

**Goal**: Food exists. Entities that find it survive longer.

- [x] Resource data structure: position, type, amount, regrowth rate
- [x] Environment module: scatter initial resources across world
- [x] Resource regrowth system: depleted resources regenerate over time
- [x] Feeding system: entity adjacent to food → consume → gain energy
- [x] Hardcoded behavior upgrade: move toward nearest food (simple distance check, no perception system yet)
- [x] Spatial index: grid-based spatial hash for proximity queries
- [x] Test: spawn entities + food, verify energy increases when feeding, verify resources deplete and regrow

### Phase 1.4: Reproduction

**Goal**: Entities reproduce. Populations self-sustain.

- [x] Simple genome struct: max_energy, metabolism_rate, max_speed, size, lifespan (physical traits only)
- [x] Identity component: generation count, parent IDs, birth tick
- [x] Asexual reproduction system: when energy > threshold → split into two with halved energy
- [x] Trait mutation: offspring genome = parent genome + Gaussian noise per trait
- [x] Species ID: hash of genome traits for grouping
- [x] Population dynamics: verify carrying capacity emerges from food supply
- [x] Test: seed=42, run 5000 ticks, assert population stabilizes, assert offspring have mutated traits

### Phase 1.5: Event System

**Goal**: Every state change is logged. Foundation for replay, narrative, and debugging.

- [x] `SimEvent` enum: EntitySpawned, EntityDied, EntityMoved, EntityAte, EntityReproduced, ResourceSpawned, ResourceDepleted
- [x] `EventLog`: per-tick event buffer, append-only
- [x] Each system emits events (movement emits EntityMoved, feeding emits EntityAte, etc.)
- [x] Event serialization with serde
- [x] `TickSummary` struct: tick number, event count, population count, avg energy
- [x] Test: run simulation, verify event log captures all births/deaths/feedings correctly

### Phase 1.6: Wire Protocol

**Goal**: Protobuf schema and code generation pipeline.

- [x] `shared/proto/world.proto`: WorldSnapshot, EntityState, ResourceState, TerrainData
- [x] `shared/proto/events.proto`: EventProto with all event types
- [x] `shared/proto/commands.proto`: ClientMessage, SimulationCommand
- [x] `prost-build` integration in simulation-engine's `build.rs` (using protox pure-Rust compiler)
- [x] `ts-proto` or `protobuf-ts` setup for TypeScript generation (deferred to viewer phase)
- [x] Code generation script (`tools/proto-gen.sh`) (deferred to viewer phase)
- [x] Test: generate types, compile Rust, verify schemas compile

### Phase 1.7: WebSocket Server

**Goal**: Simulation state accessible from the network.

- [x] Tokio async runtime alongside simulation thread
- [x] `crossbeam-channel` for simulation→server event passing (using tokio broadcast channel)
- [x] `axum` + `tokio-tungstenite` WebSocket server
- [x] ClientSession struct: WebSocket sender, viewport, detail level (ServerState with broadcast)
- [x] On connect: send WorldSnapshot (full state)
- [x] Each tick: encode TickDelta as protobuf, broadcast to all clients
- [x] Handle client disconnect gracefully
- [x] Test: bridge unit tests verify snapshot/delta encoding

### Phase 1.8: Basic Viewer

**Goal**: See the simulation in a browser.

- [x] Vite + React project scaffold (`viewer/`)
- [x] PixiJS v8 setup: Application, stage, viewport
- [x] WebSocket client with reconnection logic
- [x] Protobuf decode: parse WorldSnapshot and TickDelta messages
- [x] Zustand store: maintain client-side world state from deltas
- [x] Entity rendering: colored circles (color derived from species hash)
- [x] Resource rendering: green dots for food
- [x] Camera: pan (drag) and zoom (scroll wheel)
- [x] Test: start engine + viewer, verify entities visible and moving (Playwright verified at 10x for 60s+)

### Phase 1.9: Viewer Interaction

**Goal**: Observe and control the simulation from the browser.

- [x] HUD component: tick counter, entity count, FPS counter
- [x] Entity selection: click entity → highlight, show in panel
- [x] EntityPanel component: display position, energy, health, age, generation, species
- [x] Play/pause button: send PauseSimulation/ResumeSimulation commands
- [x] Speed control: 0.5x, 1x, 2x, 5x, 10x tick rate
- [x] Server: handle incoming commands via crossbeam channel to simulation thread
- [x] Test: pause simulation, verify ticks stop; resume, verify ticks continue (Playwright verified)

---

## Era 2: Intelligence

*After this era: entities make decisions via behavior trees that evolve over generations. Different species with different strategies emerge.*

### Phase 2.1: Perception System

**Goal**: Entities can see the world around them.

- [x] Perception component: sensor_range, Vec<PerceivedEntity>, Vec<PerceivedResource>
- [x] PerceivedEntity struct: entity ID, position, distance, energy estimate, is_kin flag
- [x] Perception system: use spatial index to find entities/resources within sensor_range
- [x] Imperfect sensing: energy estimate has noise proportional to distance
- [x] Sensor range as genome trait (evolvable)
- [x] Replace direct food-seeking with perception-based food-seeking
- [x] Test: entity with sensor_range=50 only perceives entities within 50 units

### Phase 2.2: Drives

**Goal**: Internal motivational states computed from entity state.

- [x] Drives component: hunger, fear, curiosity, social_need, aggression, reproductive_urge
- [x] DriveWeights in genome: base sensitivity for each drive
- [x] Drives system: compute each drive from entity state each tick
  - [x] hunger = 1.0 - (energy / max_energy)
  - [x] fear = 0.0 (no threats yet, expanded in Phase 3.3)
  - [x] curiosity = f(ticks_in_same_area, base_curiosity)
  - [x] reproductive_urge = f(energy_surplus, age, base_reproductive)
- [x] Test: entity with low energy has high hunger; entity with high energy has high reproductive_urge

### Phase 2.3: Behavior Tree Engine

**Goal**: Custom BT engine that can drive entity decisions.

- [x] `BtNode` enum with control flow: Sequence, Selector, Inverter, AlwaysSucceed
- [x] `BtStatus` enum: Success, Failure, Running
- [x] `BtContext` struct: reference to entity's components and perceived world
- [x] `tick_bt()` function: recursively evaluate BtNode tree, return BtStatus
- [x] Initial condition nodes: CheckDrive(drive, threshold, comparison), NearbyResource(range, type), CheckEnergy(threshold)
- [x] Initial action nodes: MoveTowardResource(type, speed_factor), Wander(speed, direction_change_rate), Eat, Rest
- [x] BtNode serialization with serde (critical for genome storage)
- [x] Test: hand-build a BT, tick it with mock context, verify correct action selection

### Phase 2.4: BT-Driven Behavior

**Goal**: Entities use behavior trees for all decisions.

- [x] BehaviorTree component: wraps a BtNode root
- [x] Action enum: MoveTo, MoveDirection, Eat, Rest, Wander, None
- [x] Decision system: tick each entity's BT → produce Action component
- [x] Movement system: consume Action::MoveTo/MoveDirection, apply movement
- [x] Feeding system: consume Action::Eat, attempt to eat adjacent food
- [x] Default starter BT: `Selector(Sequence(CheckHungry, NearbyFood, MoveToFood, Eat), Wander)`
- [x] Remove all hardcoded behavior logic
- [x] Genome includes BtNode (behavioral DNA)
- [x] Test: entity with starter BT seeks food when hungry, wanders when not

### Phase 2.5: Delta Streaming

**Goal**: Network efficiency -- only send changes, not full state.

- [x] TickDelta message: lists of spawned/moved/updated/died entities + resource changes
- [x] Diff computation: compare current tick state to previous, emit only changes
- [x] Viewer: apply deltas to Zustand store incrementally
- [x] Viewport subscription: client sends SubscribeViewport with camera bounds
- [x] Server filters: only include entities within viewport + buffer zone
- [x] Detail levels: Minimal (position+color at zoom-out), Standard (+ energy/health/size), Detailed (+ everything)
- [ ] Test: connect viewer, verify bandwidth decreases vs. full-state streaming

### Phase 2.6: Evolution -- Genetic Operators

**Goal**: Behavior trees can be crossed over and mutated.

- [x] BT crossover: select random subtree from each parent, swap
- [x] BT parameter mutation: Gaussian noise on f32 fields (thresholds, speeds, probabilities)
- [x] BT structural mutation: insert new node, delete node, replace node type
- [x] Random subtree generation: grow random BT up to max depth
- [x] BT simplification: remove redundant wrappers, collapse single-child sequences
- [x] Node count function: measure tree complexity
- [x] Depth limit: reject trees deeper than MAX_DEPTH
- [x] Test: crossover 1000 random tree pairs, assert all results are valid; mutate 1000 trees, assert all valid

### Phase 2.7: Evolution -- Natural Selection

**Goal**: Behavior trees evolve. Species diverge. Surprising behaviors emerge.

- [x] Sexual reproduction: two-parent genome crossover (BT + traits)
- [x] Reproduction requires: energy > threshold + compatible partner adjacent
- [x] Offspring genome: BT from crossover + trait averaging + mutation
- [x] Mutation rates as genome parameters (evolvable mutation rates)
- [x] Fitness is implicit: entities that survive and reproduce pass on their genes
- [x] Species tracking: cluster by genome similarity hash
- [x] Species history: track population count per species per tick
- [x] Test: run 10,000 ticks, verify multiple distinct species emerge, verify BTs have diversified

### Phase 2.8: Terrain and Biomes

**Goal**: The world has geography. Different regions favor different strategies.

- [x] Terrain grid: cell-based, each cell has a TerrainType (grassland, desert, water, forest, mountain)
- [x] Noise-based generation: `noise` crate, Perlin/Simplex, seeded
- [x] Terrain effects: movement speed multiplier, resource density multiplier per terrain type
- [x] Water: impassable (for now), creates natural barriers
- [x] Resource type diversity: berries in forest, grain in grassland, nothing in desert
- [x] Terrain rendering in viewer: tile-based background layer
- [x] Minimap: small overview of entire world with entity dots
- [x] Test: verify entities avoid water, verify different species dominate different biomes

### Phase 2.9: Environmental Pressure

**Goal**: The environment changes. Populations must adapt or die.

- [x] Seasonal cycle: resource abundance oscillates over configurable period
- [x] Drought events: random periods where resource regrowth stops in a region
- [x] Climate drift: slow long-term shift in biome boundaries
- [x] Viewer: population-over-time chart, species diversity chart
- [x] Viewer: BT visualizer for selected entity (tree structure with active node highlighted)
- [x] Test: trigger drought, verify population drops then recovers; verify species that can't adapt go extinct

---

## Era 3: Cognition

*After this era: entities remember, recognize each other, form relationships, fight, and flee. Social dynamics emerge.*

### Phase 3.1: Memory -- Data Structure

**Goal**: Entities have a memory buffer.

- [ ] MemoryEntry struct: tick, MemoryKind, importance, emotional_valence, location, associated_entity
- [ ] MemoryKind enum: FoundFood, WasAttacked, AttackedOther, Reproduced, Encountered, EnvironmentChange, NearDeath, Migrated
- [ ] Memory component: Vec<MemoryEntry> with capacity field, EvictionWeights
- [ ] EvictionWeights in genome: recency_weight, importance_weight, emotional_weight, variety_weight
- [ ] Memory capacity as genome trait (costs energy: higher capacity = higher metabolism)
- [ ] Test: create memory, add entries, verify capacity enforced

### Phase 3.2: Memory -- Formation and Eviction

**Goal**: Entities form memories from experiences and manage capacity.

- [ ] Memory formation system: after each tick, significant events become memories
  - [ ] Eating → FoundFood memory (importance based on energy gained)
  - [ ] Being attacked → WasAttacked memory (high importance, negative valence)
  - [ ] Reproducing → Reproduced memory (positive valence)
  - [ ] Near death (energy < 10%) → NearDeath memory (high importance)
- [ ] Memory eviction: when over capacity, score each memory, evict lowest
- [ ] Eviction scoring: weighted sum of recency, importance, |emotional_valence|
- [ ] Test: fill memory to capacity, add new entry, verify lowest-scored entry evicted

### Phase 3.3: Memory -- Behavior Integration

**Goal**: Entities use memories to make better decisions.

- [ ] New BT condition: RecallMemory(kind_filter, max_age_ticks) -- checks if relevant memory exists
- [ ] New BT action: MoveTowardMemory(kind_filter) -- move toward location of matching memory
- [ ] Example evolved BT: "if hungry AND recall food location → move to remembered food"
- [ ] Fear drive updated: f(perceived_threats + count_of_WasAttacked_memories)
- [ ] Viewer: memory inspector panel for selected entity (list of memories with details)
- [ ] Test: entity finds food, moves away, returns to remembered food location

### Phase 3.4: Social Relationships

**Goal**: Entities track interactions with specific other entities.

- [ ] Social component: HashMap<EntityId, RelationshipScore>
- [ ] Relationship score: running average of interaction valence (-1.0 to 1.0)
- [ ] Positive interactions: reproduction, proximity without conflict, shared food area
- [ ] Negative interactions: combat, resource competition
- [ ] Kin recognition: detect shared genome similarity via species_id comparison
- [ ] Social need drive: increases with time since positive social contact
- [ ] Test: two entities that reproduce together develop positive relationship score

### Phase 3.5: Social Behavior

**Goal**: Social dynamics drive behavior.

- [ ] New BT condition: NearbyEntity with filter options: Kin, NonKin, Positive (relationship > threshold), Negative
- [ ] New BT action: MoveTowardEntity(filter, speed_factor)
- [ ] New BT action: FleeFrom(filter, speed_factor)
- [ ] Entities seek kin when social_need is high
- [ ] Entities avoid entities with negative relationship scores
- [ ] Viewer: relationship graph (nodes = entities, edges = relationships, color = valence)
- [ ] Viewer: lineage tree for selected entity (ancestry visualization)
- [ ] Test: entities with positive relationships cluster together; entities with negative relationships avoid each other

### Phase 3.6: Combat System

**Goal**: Entities can fight. Predator-prey dynamics.

- [ ] New BT action: Attack(force_factor) -- deal damage to adjacent entity
- [ ] Combat system: resolve attacks, apply damage to target health
- [ ] Death by combat: health ≤ 0 → die, attacker gains energy from kill
- [ ] Aggression drive: f(hunger, perceived_weakness_of_nearby, base_aggression)
- [ ] Fear drive enhanced: high when perceiving larger/stronger entities
- [ ] Entity size/strength trade-offs: larger = more damage + more health, but higher metabolism
- [ ] Combat memory: WasAttacked and AttackedOther memory entries
- [ ] Test: aggressive entity attacks weaker neighbor, gains energy; weak entity flees strong one

### Phase 3.7: Coevolutionary Dynamics

**Goal**: Arms races between predators and prey.

- [ ] Verify predator species emerge (high aggression, large size, pursuit BTs)
- [ ] Verify prey species emerge (high fear, fast speed, evasion BTs)
- [ ] Verify arms race: as prey evolve speed, predators evolve pursuit strategies
- [ ] Viewer: predator/prey species visualization, kill event indicators
- [ ] Species interaction matrix: which species eat which
- [ ] Test: run 50,000 ticks, verify predator and prey populations oscillate (Lotka-Volterra-like dynamics)

### Phase 3.8: Replay System

**Goal**: Save and replay simulation history.

- [ ] Snapshot serialization: full world state → bincode → disk file
- [ ] Snapshot schedule: every N ticks (configurable)
- [ ] Snapshot loading: deserialize → reconstruct hecs World
- [ ] Event log persistence: append events to disk file between snapshots
- [ ] Replay: load snapshot, replay events forward to target tick
- [ ] CLI: `--replay <snapshot-file> --to-tick <N>` mode
- [ ] Test: run 10,000 ticks, save snapshot at 5000, load and replay to 10,000, verify identical state

---

## Era 4: Organism

*After this era: entities merge into multi-cellular organisms with specialized cells. Composite organisms are more capable than individuals.*

### Phase 4.1: Composition -- Basic Merging

**Goal**: Two entities can merge into a composite entity.

- [ ] CompositeBody component: member list, leader entity ID
- [ ] Composition compatibility: genome trait (composition_affinity, compatibility_vector)
- [ ] New BT action: CompositionAttempt -- try to merge with adjacent compatible entity
- [ ] Merge mechanics: new entity spawns with CompositeBody, members lose Position (move with parent)
- [ ] Members retain: Genome, Memory, Identity (but not independent movement)
- [ ] Composite entity inherits leader's BehaviorTree
- [ ] Test: two compatible entities merge, verify composite entity created, verify members linked

### Phase 4.2: Composition -- Aggregate Capabilities

**Goal**: Composites are stronger than individuals.

- [ ] CellRole enum: Locomotion, Sensing, Attack, Defense, Digestion, Reproduction, Undifferentiated
- [ ] Role assignment: based on member genome traits (fastest → Locomotion, best sensors → Sensing, etc.)
- [ ] Aggregate speed: sum of Locomotion members' speed contributions
- [ ] Aggregate sensor range: max of Sensing members' ranges
- [ ] Aggregate attack: sum of Attack members' force
- [ ] Aggregate health: sum of all members' health
- [ ] Energy distribution: Equal, Proportional, or Priority (genome-controlled)
- [ ] Test: composite of 3 entities is faster, sees farther, and hits harder than any individual member

### Phase 4.3: Decomposition

**Goal**: Composites can break apart.

- [ ] Decomposition triggers: composite dies, energy critically low, structural mutation
- [ ] On decomposition: members regain Position (placed near composite's position), regain independent movement
- [ ] Members retain memories from composite period
- [ ] Partial decomposition: shed weakest member to save energy
- [ ] Natural death of member: remove from composite, reduce capabilities
- [ ] Test: starve a composite, verify it decomposes, verify members survive independently

### Phase 4.4: Composite Reproduction

**Goal**: Composite organisms reproduce as a unit.

- [ ] Composite reproduction: leader genome + contributions from members
- [ ] Offspring: new composite with copied member composition pattern
- [ ] Member genomes in offspring: mutated copies of parent's members
- [ ] Composition pattern as genome trait: "this organism type has N members in these roles"
- [ ] Viewer: composite rendering (cluster of circles bound together), member drill-down panel
- [ ] Test: composite reproduces, offspring has similar structure

### Phase 4.5: Emergent Specialization

**Goal**: Evolution produces specialized cell types.

- [ ] Verify over 50,000+ ticks: some lineages evolve toward sensing specialization (large sensor_range, small size)
- [ ] Verify: some lineages evolve toward locomotion specialization (high speed, low energy cost)
- [ ] Verify: some lineages evolve toward attack specialization (high aggression, large size)
- [ ] Role optimization: members in composites evolve toward their assigned role
- [ ] Performance: batch processing for composite systems, spatial index tuning
- [ ] Persistent event log: events written to disk, not just in-memory
- [ ] Test: run long evolution, verify measurable trait divergence between cell types

---

## Era 5: Society

*After this era: entities communicate, form tribes, control territory, and transmit knowledge to others.*

### Phase 5.1: Signal Emission

**Goal**: Entities can emit signals into the world.

- [ ] Signal struct: emitter entity, signal_type (u8, no inherent meaning), position, radius, strength, decay_rate
- [ ] New BT action: Signal(type) -- emit a signal
- [ ] Signal storage: world-level Vec<Signal>, decaying each tick
- [ ] Signal visualization in viewer: expanding rings, color coded by type
- [ ] Test: entity emits signal, signal exists in world, signal decays over time

### Phase 5.2: Signal Perception

**Goal**: Entities can detect signals.

- [ ] Perception system extended: detect signals within sensor_range
- [ ] PerceivedSignal struct: type, distance, direction, strength
- [ ] New BT condition: DetectSignal(type) -- true if signal of type is perceived
- [ ] New BT action: MoveTowardSignal(type) / FleeFromSignal(type)
- [ ] Test: entity emits food signal, nearby entity detects and moves toward it

### Phase 5.3: Evolved Signal Meanings

**Goal**: Signals acquire meaning through evolution.

- [ ] Signal meanings are NOT defined -- they emerge from co-evolution of emitter and receiver BTs
- [ ] Scenario: entity that emits signal type 3 when finding food + entities that respond to type 3 by approaching → both survive better → signal 3 becomes "food here"
- [ ] Alarm calls emerge: entity emits signal when attacked, kin flee in response
- [ ] Mating calls emerge: entity emits signal when reproductive_urge is high
- [ ] Verify: signal usage patterns correlate with environmental contexts
- [ ] Test: run 100,000 ticks, analyze signal emission contexts, verify non-random correlation with events

### Phase 5.4: Group Formation

**Goal**: Persistent groups (tribes) form from social bonds.

- [ ] TribeId component: assigned when entities maintain sustained positive relationships
- [ ] Tribe formation: when N entities (configurable) have mutual positive relationships above threshold
- [ ] Tribe membership: entities join/leave tribes based on relationship scores
- [ ] Tribe-level data: member list, territory centroid, population, total resources
- [ ] Tribe identity: color, derived from founding members' genomes
- [ ] Viewer: tribe overlay, tribe statistics panel
- [ ] Test: place entities in food-rich area, verify tribe forms after sustained interaction

### Phase 5.5: Territory

**Goal**: Tribes claim and defend territory.

- [ ] Territory concept: cells where tribe members spend most time
- [ ] Territorial behavior: prefer to stay within tribe territory
- [ ] Border detection: where territories of different tribes meet
- [ ] Territorial defense: increased aggression toward non-tribe members near territory
- [ ] Viewer: territory boundaries, heatmap of tribe presence
- [ ] Test: two tribes near each other develop distinct territories with defended border

### Phase 5.6: Group Combat

**Goal**: Tribes fight tribes.

- [ ] Group coordination: entities in same tribe preferentially attack same target
- [ ] Strength in numbers: group attack is more effective than individual attacks
- [ ] Retreat behavior: entity flees when tribe is losing (many members dead/fleeing)
- [ ] War events: sustained combat between two tribes logged as higher-level event
- [ ] Viewer: war visualization, tribe population changes during conflict
- [ ] Test: two tribes encounter each other, combat emerges, losing tribe retreats or is destroyed

### Phase 5.7: Teaching and Cultural Transmission

**Goal**: Knowledge spreads beyond genetics.

- [ ] Teaching system: entity observes tribe member's successful action → adds to own memory
- [ ] Learned behaviors: entity that observes successful food-finding adds food location to memory
- [ ] Cultural traits: behaviors that spread through observation rather than genetics
- [ ] Memory tag: distinguish genetic memory (from genome BT) from cultural memory (from observation)
- [ ] Accumulated knowledge: tribes that teach have collective knowledge larger than any individual
- [ ] Test: entity A finds food, entity B observes, entity B navigates to food without having found it independently

---

## Era 6: Tool Use and Construction

*After this era: entities create and use objects. They build structures, farm, and craft tools. Material culture emerges.*

### Phase 6.1: Object System

**Goal**: Objects can exist in the world independently of entities.

- [ ] WorldObject component: position, object_type, material_properties, durability, creator
- [ ] MaterialProperty struct: hardness, sharpness, weight, flexibility, nutritional_value
- [ ] ObjectType: derived from material properties (not named categories)
- [ ] Objects persist in the world, occupy space, can be perceived
- [ ] Object decay: durability decreases over time, object eventually disappears
- [ ] Viewer: render objects as shapes based on material properties
- [ ] Test: spawn object, verify it persists, verify it decays

### Phase 6.2: Object Interaction

**Goal**: Entities can pick up, carry, and drop objects.

- [ ] Inventory component: list of carried objects, max carry capacity (from genome: size/strength)
- [ ] New BT actions: PickUp, Drop, UseObject
- [ ] New BT conditions: HasObject(property_filter), NearbyObject(property_filter)
- [ ] Carry cost: carrying objects reduces movement speed proportional to weight
- [ ] Object perception: objects appear in perception like resources
- [ ] Test: entity picks up object, carries it, drops it at new location

### Phase 6.3: Object Creation -- Simple Tools

**Goal**: Entities can create objects from raw materials.

- [ ] Raw materials: resources now have material properties (wood = flexible+light, stone = hard+heavy)
- [ ] Blueprint component: a recipe in the genome -- "combine material A at location → produce object with properties"
- [ ] Creation action: entity at resource → spend energy → create object
- [ ] Blueprint is part of genome: evolves via GP (like behavior trees)
- [ ] Blueprint representation: tree of operations (gather, combine, shape) with material parameters
- [ ] First emergent tools: sharp stone (high sharpness from hard material)
- [ ] Test: entity with blueprint creates object from resource, object has correct properties

### Phase 6.4: Tool Use -- Capability Modification

**Goal**: Using objects modifies entity capabilities.

- [ ] Tool effects: object properties modify entity stats while held/used
  - [ ] Sharp object: +attack_damage
  - [ ] Large object: +defense (shield-like)
  - [ ] Container object: +carry_capacity
  - [ ] Pointed object: +resource_gathering_speed
- [ ] Effect calculation: tool_bonus = f(relevant_material_properties)
- [ ] Tool selection: BT decides which carried object to use for which action
- [ ] Tools break: durability decreases with use
- [ ] Test: entity with sharp tool deals more damage; entity with container carries more food

### Phase 6.5: Structures

**Goal**: Entities build persistent constructions in the world.

- [ ] Structure component: position, size, material, structure_type, builder, durability
- [ ] Structure types emerge from material properties:
  - [ ] Wall: high hardness, blocks movement
  - [ ] Shelter: enclosure that reduces environmental damage to entities inside
  - [ ] Platform: flat surface, enables stacking
- [ ] Construction system: entity places materials at location over multiple ticks
- [ ] ConstructionSite component: tracks progress, required materials, blueprint
- [ ] Spatial index integration: structures block movement, provide cover
- [ ] Viewer: structure rendering, construction progress indicators
- [ ] Test: entity builds wall, verify movement blocked; entity builds shelter, verify damage reduction

### Phase 6.6: Crafting -- Combining Objects

**Goal**: Objects can be combined into more complex objects.

- [ ] Crafting blueprint: genome-encoded recipe (input objects → output object)
- [ ] Crafting action: entity with required objects in inventory → spend energy → produce new object
- [ ] Material combination rules: combined properties based on input properties
  - [ ] Sharp stone + flexible wood → sharp-on-stick (higher reach + sharpness)
  - [ ] Properties emerge from the combination formula, not from named recipes
- [ ] Blueprint evolution: crafting recipes evolve via GP alongside behavior trees
- [ ] Multi-step crafting: output of one recipe can be input to another
- [ ] Test: entity crafts combined object, verify properties are combination of inputs

### Phase 6.7: Agriculture

**Goal**: Entities modify the environment to grow food.

- [ ] New BT action: PlantResource(type) -- place a resource seed at location
- [ ] Planted resources: grow over time from seed → mature → harvestable
- [ ] Tending: entities that visit planted resources boost growth rate
- [ ] Crop types: different resources have different growth rates, nutritional values
- [ ] Field concept: cluster of planted resources becomes a "farm" (emergent)
- [ ] Farming knowledge: which resources to plant, where, when (in memory/blueprint)
- [ ] Test: entity plants resource, resource grows, entity harvests mature resource

### Phase 6.8: Storage and Stockpiling

**Goal**: Entities can store resources and objects for later use.

- [ ] Storage structure: a built structure that holds objects/resources
- [ ] Deposit/withdraw actions: entities can place items in storage, retrieve later
- [ ] Shared storage: tribe members can access tribal storage
- [ ] Hoarding behavior: emergent from BT evolution (store when abundant, retrieve when scarce)
- [ ] Viewer: storage contents panel, resource flow visualization
- [ ] Test: entity stores food during abundance, retrieves during scarcity

### Phase 6.9: Knowledge Transmission for Construction

**Goal**: Building knowledge spreads through culture, not just genetics.

- [ ] Observation-based learning: entity watches another build → learns blueprint
- [ ] Learned blueprints stored in memory (not genome) -- can be forgotten
- [ ] Teaching: entity demonstrates construction to tribe member
- [ ] Blueprint quality: more practice → higher quality constructions (refinement in memory)
- [ ] Innovation: occasionally, mutation in learned blueprint produces improvement
- [ ] Cultural evolution of technology: improvements spread faster than genetic evolution
- [ ] Test: entity A builds structure, entity B observes and learns, entity B builds similar structure

### Phase 6.10: Infrastructure

**Goal**: Connected structures enable civilization-scale construction.

- [ ] Roads: cleared/flattened paths that increase movement speed
- [ ] Bridges: structures that enable crossing water
- [ ] Walls and fortifications: defensive perimeters around settlements
- [ ] Housing: structures that reduce energy cost (shelter from environment)
- [ ] Irrigation: channels that increase resource growth in adjacent cells
- [ ] Settlement detection: cluster of structures + persistent population = settlement
- [ ] Viewer: infrastructure overlay, settlement boundaries
- [ ] Test: verify tribe builds structures that form a connected settlement

---

## Era 7: Narrative

*After this era: the system identifies, tracks, and narrates emergent stories.*

### Phase 7.1: Event Significance Scoring

**Goal**: Not all events are equally interesting. Score them.

- [ ] Significance function: importance = state_change_magnitude × rarity × protagonist_relevance × irreversibility × relationship_involvement
- [ ] Significant event buffer: events above threshold stored in narrative buffer
- [ ] Entity interest score: long-lived, many offspring, many kills, extensive travel, many relationships
- [ ] Auto-tracking: entities above interest threshold are automatically tracked
- [ ] Test: death of a long-lived entity scores higher than death of a newborn

### Phase 7.2: Arc Detection

**Goal**: Identify narrative patterns in event sequences.

- [ ] Arc types: Rivalry (repeated combat between same pair), Alliance (sustained cooperation), Migration (group movement across biomes), Extinction (species population → 0), Rise (entity or tribe gaining territory/population), Fall (losing territory/population)
- [ ] Pattern matcher: sliding window over significant events, match against arc templates
- [ ] StoryArc struct: type, protagonists, events, start_tick, tension_curve
- [ ] Multiple concurrent arcs tracked
- [ ] Test: simulate two entities repeatedly fighting, verify rivalry arc detected

### Phase 7.3: Entity Biography

**Goal**: Every tracked entity has a readable life story.

- [ ] Biography generator: compile significant events for entity into chronological narrative
- [ ] Chapter detection: group events into life phases (youth, exploration, combat, parenthood, etc.)
- [ ] Key relationships: identify most important relationships from memory and social data
- [ ] Legacy: offspring, territory influenced, tribe membership history
- [ ] Viewer: entity biography page with timeline, key events, relationships
- [ ] Test: select long-lived entity, verify biography contains all major life events

### Phase 7.4: History Browser

**Goal**: Navigate simulation history interactively.

- [ ] Timeline scrubber: drag through simulation history
- [ ] Seek to tick: load snapshot, replay forward
- [ ] Bookmark system: save interesting moments with notes
- [ ] "What-if" forking: branch from any snapshot with modified parameters
- [ ] Event search: find all events matching criteria (entity, type, tick range)
- [ ] Historical analytics: population graphs, species diversity, resource levels over time
- [ ] Viewer: timeline component, bookmark list, analytics charts
- [ ] Test: bookmark a moment, seek to it, verify correct state displayed

### Phase 7.5: LLM Narration

**Goal**: AI-generated prose narration of emergent stories.

- [ ] LLM integration: Claude API or similar
- [ ] Narrative prompt construction: feed arc events + entity details → receive prose
- [ ] Real-time narration mode: narrate ongoing arcs as they develop
- [ ] Historical narration: generate summary of entity life or civilization history
- [ ] Narration styles: documentary, epic, journal, clinical
- [ ] Narration cache: don't re-narrate events already narrated
- [ ] Viewer: narration panel alongside simulation, narration playback
- [ ] Test: trigger arc, request narration, verify coherent prose output

---

## Era 8: Scale

*After this era: simulation handles 100k+ entities efficiently. Save/load works. Multiple viewers.*

### Phase 8.1: Performance Profiling

**Goal**: Identify bottlenecks before optimizing.

- [ ] Tick timing: measure time per system per tick
- [ ] Hotspot analysis: profile with `cargo flamegraph` or `perf`
- [ ] Memory profiling: per-component memory usage, total entity memory
- [ ] Spatial index performance: queries per tick, average neighbors per query
- [ ] Network bandwidth: bytes per tick per client
- [ ] Establish baseline metrics at 10k, 50k, 100k entities
- [ ] Test: generate performance report at each entity count

### Phase 8.2: Multi-threaded Simulation

**Goal**: Use all CPU cores.

- [ ] World partitioning: divide world into regions (quadtree or fixed grid)
- [ ] Thread pool: `rayon` for parallel system execution
- [ ] Identify parallelizable systems: perception, drives, aging (read-only entity state)
- [ ] Identify sequential systems: movement, combat, reproduction (write entity state)
- [ ] Parallel perception: each region's entities processed independently
- [ ] Thread-safe spatial index: concurrent read, single-writer rebuild
- [ ] Cross-region interactions: queue for next tick
- [ ] Test: verify identical results between single-threaded and multi-threaded execution (determinism preserved)

### Phase 8.3: Level of Detail Simulation

**Goal**: Distant regions simulate at lower fidelity.

- [ ] LOD levels: Full (all systems), Reduced (skip perception, simplified BT), Minimal (just aging/metabolism)
- [ ] LOD assignment: based on distance from any connected viewer's viewport
- [ ] LOD transitions: smooth handoff when viewer pans
- [ ] Population-level simulation for Minimal LOD: statistical birth/death rates instead of individual ticks
- [ ] Test: verify populations in distant regions evolve similarly to full-fidelity simulation (statistically)

### Phase 8.4: Save/Load

**Goal**: Full simulation state to/from disk.

- [ ] Save: serialize all entities + components + environment + RNG state + event log metadata
- [ ] Load: deserialize and reconstruct full simulation
- [ ] File format: bincode with version header
- [ ] Compression: zstd for save files
- [ ] Auto-save: configurable interval
- [ ] CLI: `--save <path>` and `--load <path>`
- [ ] Test: save at tick 10,000, load, continue to 20,000, verify identical to continuous run

### Phase 8.5: Multiple Viewers

**Goal**: Many browsers connected simultaneously.

- [ ] Per-client sessions with independent viewports and detail levels
- [ ] Broadcast system: compute deltas once, filter per viewport
- [ ] Client limit: configurable max connections
- [ ] Graceful degradation: reduce detail level if server is overloaded
- [ ] Admin viewer: sees everything, can issue commands
- [ ] Spectator viewer: read-only, no commands
- [ ] Test: connect 10 viewers with different viewports, verify each receives correct data

---

## Era 9: Civilization

*After this era: settlements exist with internal structure. Trade routes connect settlements. Political hierarchy emerges. Wars are fought over resources and territory.*

### Phase 9.1: Settlement Formation

**Goal**: Persistent settlements emerge from entity behavior.

- [ ] Settlement detection: algorithm identifies clusters of structures + persistent population
- [ ] Settlement entity: virtual entity representing the settlement (population, structures, territory, resources)
- [ ] Settlement naming: generated from founding entities' genome hashes
- [ ] Settlement growth/decline: tracked over time
- [ ] Viewer: settlement markers, settlement detail panel, settlement history
- [ ] Test: tribe builds structures in an area, verify settlement detected

### Phase 9.2: Resource Specialization

**Goal**: Different settlements produce different things.

- [ ] Regional resources: biome determines available raw materials
- [ ] Settlement specialization: settlements near forests produce wood items, near mountains produce stone items
- [ ] Surplus: settlements produce more of their specialty than they consume
- [ ] Deficit: settlements lack materials not available in their biome
- [ ] Test: verify settlements in different biomes produce different object types

### Phase 9.3: Trade

**Goal**: Resources flow between settlements.

- [ ] Trade behavior: entities carry surplus resources to neighboring settlements
- [ ] Trade routes: paths frequently traveled by traders become "trade routes" (emergent)
- [ ] Value estimation: entities learn which resources are scarce elsewhere (from memory)
- [ ] Trade signals: settlements emit signals indicating surplus/deficit
- [ ] Mutual benefit: both settlements gain from trade (comparative advantage)
- [ ] Viewer: trade route visualization, resource flow arrows
- [ ] Test: settlement A has surplus food, settlement B has surplus tools, verify trade emerges

### Phase 9.4: Defense and Warfare

**Goal**: Settlements defend themselves. Inter-settlement conflict.

- [ ] Defensive structures: walls, watchtowers (detect approaching threats at range)
- [ ] Garrison behavior: some entities stay near settlement for defense
- [ ] Raiding: aggressive tribes/settlements attack others for resources
- [ ] Siege mechanics: sustained assault on fortified settlement
- [ ] Conquest: defeated settlement absorbed by victor
- [ ] War history: recorded as major narrative event
- [ ] Viewer: military events, defensive structures, battle visualization
- [ ] Test: resource-poor settlement raids resource-rich one, verify combat around structures

### Phase 9.5: Emergent Hierarchy

**Goal**: Political/social structure within settlements.

- [ ] Leadership: entity with highest social influence (most relationships, most respect) becomes de facto leader
- [ ] Leader behavior: leader's BT influences tribe/settlement behavior
- [ ] Specialization of labor: entities with different BTs fill different roles (farmer, builder, warrior, scout)
- [ ] Resource distribution: how resources are shared within settlement (leader decides? equal? need-based?)
- [ ] Succession: when leader dies, new leader emerges from social dynamics
- [ ] Test: verify settlement has identifiable leader, verify leadership changes on death

### Phase 9.6: Cultural Identity

**Goal**: Settlements develop distinct cultures.

- [ ] Cultural markers: shared signal usage patterns, construction styles, behavioral norms
- [ ] Cultural drift: isolated settlements diverge culturally over time
- [ ] Cultural exchange: trade and contact spread cultural elements
- [ ] Language divergence: signal meanings drift between separated groups
- [ ] Viewer: cultural comparison between settlements
- [ ] Test: separate two settlements for many generations, verify measurable cultural divergence

### Phase 9.7: NEAT Integration

**Goal**: Neural network modules within behavior trees for nuanced decisions.

- [ ] NEAT implementation: evolve small neural networks
- [ ] BT integration: neural networks as decision nodes within behavior trees
- [ ] Input: drive values, perception summary, memory features
- [ ] Output: action selection weights, parameter modulation
- [ ] Hybrid BT: interpretable high-level structure (GP-evolved) + nuanced low-level decisions (NEAT-evolved)
- [ ] Test: entities with NEAT-augmented BTs make more adaptive decisions than pure BT entities

---

## Era 10: The Third Dimension

*After this era: the world has height, depth, and volume. Entities can swim, climb, and fly.*

### Phase 10.1: 3D Coordinate System

**Goal**: Foundation for 3D world.

- [ ] Position3D, Velocity3D components (add Z axis)
- [ ] Backward compatibility: 2D systems still work (Z=0 for ground level)
- [ ] 3D distance calculations, 3D bounding volumes
- [ ] Config flag: `dimensions: 2 | 3`
- [ ] All existing systems updated to handle Z when present
- [ ] Test: entities with Z=0 behave identically to 2D mode

### Phase 10.2: 3D Terrain -- Height Map

**Goal**: The ground has elevation.

- [ ] Height map: 2D noise grid defining ground elevation at each (x,y)
- [ ] Terrain mesh generation from height map
- [ ] Elevation affects: movement speed (uphill slower), visibility (hilltops see farther), temperature (higher = colder)
- [ ] Slope limits: entities can't climb vertical cliffs (without climbing ability)
- [ ] Water level: height below threshold = water
- [ ] Test: verify entities navigate around steep terrain, prefer flat paths

### Phase 10.3: 3D Terrain -- Caves and Underground

**Goal**: Subterranean spaces exist.

- [ ] Cave generation: 3D noise with threshold → cave volumes
- [ ] Underground terrain types: stone, crystal, magma
- [ ] Underground resources: rare minerals only found in caves
- [ ] Darkness: underground has no light (affects perception)
- [ ] Cave entrances: surface-accessible openings to underground
- [ ] Underground ecology: separate from surface (new niches)
- [ ] Test: entities discover cave entrance, navigate underground, find unique resources

### Phase 10.4: Three.js Renderer

**Goal**: Replace PixiJS with Three.js for 3D visualization.

- [ ] Three.js setup: Scene, Camera, Renderer
- [ ] Terrain mesh rendering from height map
- [ ] Entity rendering: InstancedMesh for thousands of entities
- [ ] Per-instance attributes: position, color, size
- [ ] Camera controls: orbit, pan, zoom, follow entity
- [ ] Ground plane with terrain texture
- [ ] Performance target: 10k entities at 60fps
- [ ] Test: existing simulation renders in 3D, all entities visible

### Phase 10.5: 3D Entity Rendering

**Goal**: Entities look appropriate in 3D.

- [ ] Entity meshes: simple 3D shapes (spheres, capsules) based on species/size
- [ ] Composite rendering: multi-mesh clusters for composite organisms
- [ ] LOD rendering: distant entities as points, near entities as meshes
- [ ] Animation: simple bob/pulse for movement, flash for combat
- [ ] Structure rendering: 3D boxes, walls, roofs
- [ ] Resource rendering: 3D models for different resource types
- [ ] Test: zoom in on entity, verify appropriate 3D representation

### Phase 10.6: 3D Camera System

**Goal**: Navigate the 3D world intuitively.

- [ ] Orbit camera: rotate around a point
- [ ] Free camera: fly through the world
- [ ] Follow camera: track selected entity from adjustable distance
- [ ] Top-down camera: overhead view (like 2D mode but with depth)
- [ ] Cross-section view: slice the world horizontally, see underground at that depth
- [ ] Camera bookmarks: save/restore camera positions
- [ ] Smooth transitions between camera modes
- [ ] Test: all camera modes work, transitions are smooth

### Phase 10.7: 3D Spatial Index

**Goal**: Efficient 3D proximity queries.

- [ ] 3D grid hash: extend 2D spatial hash to 3 dimensions
- [ ] Octree alternative: for non-uniform entity distribution
- [ ] 3D range queries: find entities within sphere
- [ ] 3D ray queries: line-of-sight checks (for perception)
- [ ] Vertical perception: entities above/below can detect each other
- [ ] Performance benchmark: compare grid hash vs. octree at different densities
- [ ] Test: 3D proximity queries return correct results, performance acceptable at 10k entities

### Phase 10.8: Water and Fluid

**Goal**: Water bodies as navigable (or impassable) terrain.

- [ ] Water volumes: defined by terrain height vs. water level
- [ ] Floating: entities at water surface
- [ ] Swimming: movement in water (different speed, higher energy cost)
- [ ] Underwater: fully submerged volumes with reduced perception
- [ ] Aquatic specialization: entities evolve for water (faster swimming, underwater breathing analogue)
- [ ] Water resources: aquatic food sources
- [ ] Test: entities near water learn to swim for aquatic resources; aquatic species emerge

### Phase 10.9: Aerial Movement

**Goal**: Entities can move in 3D space above ground.

- [ ] Flight capability: genome trait (wing_strength, flight_efficiency)
- [ ] Flight mechanics: move in 3D with energy cost proportional to altitude + speed
- [ ] Gliding: using altitude for energy-efficient horizontal movement
- [ ] Aerial perception: flying entities see larger area from above
- [ ] Landing/takeoff: transition between ground and air movement
- [ ] Aerial predation: dive from above
- [ ] Test: entity evolves flight, verify it navigates in 3D, verify aerial advantages

### Phase 10.10: 3D Lighting and Atmosphere

**Goal**: Visual quality and environmental effects in 3D.

- [ ] Day/night cycle: directional light rotates, affects visibility
- [ ] Shadows: entities and structures cast shadows
- [ ] Fog/atmosphere: distant objects fade (also helps performance)
- [ ] Weather visualization: rain, snow particles in relevant biomes
- [ ] Underground darkness: no light below surface
- [ ] Bioluminescence: some evolved entities glow (emergent trait)
- [ ] Test: visual quality acceptable, day/night cycle affects entity behavior

### Phase 10.11: 3D Construction

**Goal**: Structures work in 3D.

- [ ] 3D structure placement: on ground, on slopes, underground
- [ ] Multi-story structures: stack vertically
- [ ] Ramps and stairs: enable vertical movement between levels
- [ ] Bridges over water: built between landmasses
- [ ] Cave dwellings: structures inside caves
- [ ] Structural integrity: tall structures require support
- [ ] Test: entity builds multi-story structure, verify it's navigable

### Phase 10.12: 3D Performance Optimization

**Goal**: 3D simulation at scale.

- [ ] Frustum culling: don't render entities outside camera view
- [ ] LOD meshes: simpler meshes at distance
- [ ] Instanced rendering optimization: batch similar entities
- [ ] Chunk-based terrain: only load/render nearby terrain
- [ ] GPU compute: offload particle effects, simple physics
- [ ] Target: 50k entities at 30fps in 3D
- [ ] Test: benchmark at 10k, 50k entities, verify frame rate targets

---

## Era 11: Open World

*After this era: the world is infinite and procedurally generated. Multiple civilizations discover each other.*

### Phase 11.1: Chunk System

**Goal**: World divided into loadable chunks.

- [ ] Chunk definition: fixed-size square (e.g., 256x256) of terrain + entities + structures
- [ ] Chunk states: Active (full simulation), Dormant (statistical simulation), Unloaded (on disk)
- [ ] Chunk activation: based on entity presence or viewer proximity
- [ ] Chunk border handling: entities crossing chunk boundaries
- [ ] Test: entity migrates from one chunk to another, verify seamless transition

### Phase 11.2: Procedural World Generation

**Goal**: New chunks generated on demand.

- [ ] World seed: single seed generates entire infinite world deterministically
- [ ] Biome generation: large-scale noise determines biome at any coordinate
- [ ] Terrain generation: per-chunk noise generates detailed terrain
- [ ] Resource distribution: biome-appropriate resources placed procedurally
- [ ] Feature generation: rivers, mountains, coastlines at world scale
- [ ] Test: generate chunk at coordinates (1000, 1000), verify terrain is consistent across regeneration

### Phase 11.3: Chunk Loading/Unloading

**Goal**: Memory-efficient world management.

- [ ] Active chunk limit: maximum chunks in full simulation
- [ ] Dormant simulation: statistical population/resource model for inactive chunks
- [ ] Disk serialization: unloaded chunks saved to disk
- [ ] Load on demand: chunk activates when entity migrates into it
- [ ] Viewer chunk streaming: send terrain data for chunks as viewer pans
- [ ] Test: entities spread across many chunks, verify only nearby chunks are active

### Phase 11.4: Long-Distance Migration

**Goal**: Entities and species spread across the world.

- [ ] Migration triggers: resource scarcity, overcrowding, curiosity drive
- [ ] Long-distance pathfinding: navigate between chunks toward perceived better conditions
- [ ] Species range maps: track where each species exists across chunks
- [ ] Colonization: entities arriving in uninhabited chunks establish new populations
- [ ] Viewer: world-scale species range map, migration path visualization
- [ ] Test: overpopulate a region, verify emigration to adjacent chunks

### Phase 11.5: Multi-Civilization Encounters

**Goal**: Isolated civilizations meet.

- [ ] First contact event: two civilizations encounter each other for the first time
- [ ] Communication barrier: different signal "languages" evolved independently
- [ ] Conflict or cooperation: outcome depends on evolved behaviors of both civilizations
- [ ] Cultural exchange: successful contact leads to signal/blueprint sharing
- [ ] Conquest: one civilization overwhelms another
- [ ] Narrative significance: first contact is always a major story event
- [ ] Viewer: civilization map, first-contact events highlighted
- [ ] Test: seed two separated civilizations, verify they eventually encounter each other

### Phase 11.6: World-Scale Analytics

**Goal**: Understand the entire world at a glance.

- [ ] Civilization map: color-coded territories across all chunks
- [ ] Population heatmap: entity density across world
- [ ] Resource flow: trade routes across world scale
- [ ] Species distribution: where each species lives
- [ ] Historical timeline: rise and fall of civilizations across the entire world
- [ ] Zoom levels: seamless zoom from world overview → continent → settlement → individual entity
- [ ] Test: viewer zoom from world scale to individual entity without interruption
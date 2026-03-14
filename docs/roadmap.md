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

- [x] MemoryEntry struct: tick, MemoryKind, importance, emotional_valence, location, associated_entity
- [x] MemoryKind enum: FoundFood, WasAttacked, AttackedOther, Reproduced, Encountered, EnvironmentChange, NearDeath, Migrated
- [x] Memory component: Vec<MemoryEntry> with capacity field, EvictionWeights
- [x] EvictionWeights in genome: recency_weight, importance_weight, emotional_weight, variety_weight
- [x] Memory capacity as genome trait (costs energy: higher capacity = higher metabolism)
- [x] Test: create memory, add entries, verify capacity enforced

### Phase 3.2: Memory -- Formation and Eviction

**Goal**: Entities form memories from experiences and manage capacity.

- [x] Memory formation system: after each tick, significant events become memories
  - [x] Eating → FoundFood memory (importance based on energy gained)
  - [x] Being attacked → WasAttacked memory (high importance, negative valence)
  - [x] Reproducing → Reproduced memory (positive valence)
  - [x] Near death (energy < 10%) → NearDeath memory (high importance)
- [x] Memory eviction: when over capacity, score each memory, evict lowest
- [x] Eviction scoring: weighted sum of recency, importance, |emotional_valence|
- [x] Test: fill memory to capacity, add new entry, verify lowest-scored entry evicted

### Phase 3.3: Memory -- Behavior Integration

**Goal**: Entities use memories to make better decisions.

- [x] New BT condition: RecallMemory(kind_filter, max_age_ticks) -- checks if relevant memory exists
- [x] New BT action: MoveTowardMemory(kind_filter) -- move toward location of matching memory
- [x] Example evolved BT: "if hungry AND recall food location → move to remembered food"
- [x] Fear drive updated: f(perceived_threats + count_of_WasAttacked_memories)
- [ ] Viewer: memory inspector panel for selected entity (list of memories with details)
- [x] Test: entity finds food, moves away, returns to remembered food location

### Phase 3.4: Social Relationships

**Goal**: Entities track interactions with specific other entities.

- [x] Social component: HashMap<EntityId, RelationshipScore>
- [x] Relationship score: running average of interaction valence (-1.0 to 1.0)
- [x] Positive interactions: reproduction, proximity without conflict, shared food area
- [x] Negative interactions: combat, resource competition
- [x] Kin recognition: detect shared genome similarity via species_id comparison
- [x] Social need drive: increases with time since positive social contact
- [x] Test: two entities that reproduce together develop positive relationship score

### Phase 3.5: Social Behavior

**Goal**: Social dynamics drive behavior.

- [x] New BT condition: NearbyEntity with filter options: Kin, NonKin, Positive (relationship > threshold), Negative
- [x] New BT action: MoveTowardEntity(filter, speed_factor)
- [x] New BT action: FleeFrom(filter, speed_factor)
- [x] Entities seek kin when social_need is high
- [x] Entities avoid entities with negative relationship scores
- [x] Viewer: relationship graph (nodes = entities, edges = relationships, color = valence)
- [x] Viewer: lineage tree for selected entity (ancestry visualization)
- [x] Test: entities with positive relationships cluster together; entities with negative relationships avoid each other

### Phase 3.6: Combat System

**Goal**: Entities can fight. Predator-prey dynamics.

- [x] New BT action: Attack(force_factor) -- deal damage to adjacent entity
- [x] Combat system: resolve attacks, apply damage to target health
- [x] Death by combat: health ≤ 0 → die, attacker gains energy from kill
- [x] Aggression drive: f(hunger, perceived_weakness_of_nearby, base_aggression)
- [x] Fear drive enhanced: high when perceiving larger/stronger entities
- [x] Entity size/strength trade-offs: larger = more damage + more health, but higher metabolism
- [x] Combat memory: WasAttacked and AttackedOther memory entries
- [x] Test: aggressive entity attacks weaker neighbor, gains energy; weak entity flees strong one

### Phase 3.7: Coevolutionary Dynamics

**Goal**: Arms races between predators and prey.

- [x] Verify predator species emerge (high aggression, large size, pursuit BTs)
- [x] Verify prey species emerge (high fear, fast speed, evasion BTs)
- [x] Verify arms race: as prey evolve speed, predators evolve pursuit strategies
- [x] Viewer: predator/prey species visualization, kill event indicators
- [x] Species interaction matrix: which species eat which
- [x] Test: run 50,000 ticks, verify predator and prey populations oscillate (Lotka-Volterra-like dynamics)

### Phase 3.8: Replay System

**Goal**: Save and replay simulation history.

- [x] Snapshot serialization: full world state → bincode → disk file
- [x] Snapshot schedule: every N ticks (configurable)
- [x] Snapshot loading: deserialize → reconstruct hecs World
- [x] Event log persistence: append events to disk file between snapshots
- [x] Replay: load snapshot, replay events forward to target tick
- [x] CLI: `--replay <snapshot-file> --to-tick <N>` mode
- [x] Test: run 10,000 ticks, save snapshot at 5000, load and replay to 10,000, verify identical state

---

## Era 4: Organism

*After this era: entities merge into multi-cellular organisms with specialized cells. Composite organisms are more capable than individuals.*

### Phase 4.1: Composition -- Basic Merging

**Goal**: Two entities can merge into a composite entity.

- [x] CompositeBody component: member list, leader entity ID
- [x] Composition compatibility: genome trait (composition_affinity, compatibility_vector)
- [x] New BT action: CompositionAttempt -- try to merge with adjacent compatible entity
- [x] Merge mechanics: new entity spawns with CompositeBody, members lose Position (move with parent)
- [x] Members retain: Genome, Memory, Identity (but not independent movement)
- [x] Composite entity inherits leader's BehaviorTree
- [x] Test: two compatible entities merge, verify composite entity created, verify members linked

### Phase 4.2: Composition -- Aggregate Capabilities

**Goal**: Composites are stronger than individuals.

- [x] CellRole enum: Locomotion, Sensing, Attack, Defense, Digestion, Reproduction, Undifferentiated
- [x] Role assignment: based on member genome traits (fastest → Locomotion, best sensors → Sensing, etc.)
- [x] Aggregate speed: sum of Locomotion members' speed contributions
- [x] Aggregate sensor range: max of Sensing members' ranges
- [x] Aggregate attack: sum of Attack members' force
- [x] Aggregate health: sum of all members' health
- [x] Energy distribution: Equal, Proportional, or Priority (genome-controlled)
- [x] Test: composite of 3 entities is faster, sees farther, and hits harder than any individual member

### Phase 4.3: Decomposition

**Goal**: Composites can break apart.

- [x] Decomposition triggers: composite dies, energy critically low, structural mutation
- [x] On decomposition: members regain Position (placed near composite's position), regain independent movement
- [x] Members retain memories from composite period
- [x] Partial decomposition: shed weakest member to save energy
- [x] Natural death of member: remove from composite, reduce capabilities
- [x] Test: starve a composite, verify it decomposes, verify members survive independently

### Phase 4.4: Composite Reproduction

**Goal**: Composite organisms reproduce as a unit.

- [x] Composite reproduction: leader genome + contributions from members
- [x] Offspring: new composite with copied member composition pattern
- [x] Member genomes in offspring: mutated copies of parent's members
- [x] Composition pattern as genome trait: "this organism type has N members in these roles"
- [ ] Viewer: composite rendering (cluster of circles bound together), member drill-down panel
- [x] Test: composite reproduces, offspring has similar structure

### Phase 4.5: Emergent Specialization

**Goal**: Evolution produces specialized cell types.

- [x] Verify over 50,000+ ticks: some lineages evolve toward sensing specialization (large sensor_range, small size)
- [x] Verify: some lineages evolve toward locomotion specialization (high speed, low energy cost)
- [x] Verify: some lineages evolve toward attack specialization (high aggression, large size)
- [x] Role optimization: members in composites evolve toward their assigned role
- [x] Performance: batch processing for composite systems, spatial index tuning
- [x] Persistent event log: events written to disk, not just in-memory
- [x] Test: run long evolution, verify measurable trait divergence between cell types

---

## Era 5: Society

*After this era: entities communicate, form tribes, control territory, and transmit knowledge to others.*

### Phase 5.1: Signal Emission

**Goal**: Entities can emit signals into the world.

- [x] Signal struct: emitter entity, signal_type (u8, no inherent meaning), position, radius, strength, decay_rate
- [x] New BT action: Signal(type) -- emit a signal
- [x] Signal storage: world-level Vec<Signal>, decaying each tick
- [ ] Signal visualization in viewer: expanding rings, color coded by type
- [x] Test: entity emits signal, signal exists in world, signal decays over time

### Phase 5.2: Signal Perception

**Goal**: Entities can detect signals.

- [x] Perception system extended: detect signals within sensor_range
- [x] PerceivedSignal struct: type, distance, direction, strength
- [x] New BT condition: DetectSignal(type) -- true if signal of type is perceived
- [x] New BT action: MoveTowardSignal(type) / FleeFromSignal(type)
- [x] Test: entity emits food signal, nearby entity detects and moves toward it

### Phase 5.3: Evolved Signal Meanings

**Goal**: Signals acquire meaning through evolution.

- [x] Signal meanings are NOT defined -- they emerge from co-evolution of emitter and receiver BTs
- [x] Scenario: entity that emits signal type 3 when finding food + entities that respond to type 3 by approaching → both survive better → signal 3 becomes "food here"
- [x] Alarm calls emerge: entity emits signal when attacked, kin flee in response
- [x] Mating calls emerge: entity emits signal when reproductive_urge is high
- [x] Verify: signal usage patterns correlate with environmental contexts
- [ ] Test: run 100,000 ticks, analyze signal emission contexts, verify non-random correlation with events

### Phase 5.4: Group Formation

**Goal**: Persistent groups (tribes) form from social bonds.

- [x] TribeId component: assigned when entities maintain sustained positive relationships
- [x] Tribe formation: when N entities (configurable) have mutual positive relationships above threshold
- [x] Tribe membership: entities join/leave tribes based on relationship scores
- [x] Tribe-level data: member list, territory centroid, population, total resources
- [x] Tribe identity: color, derived from founding members' genomes
- [ ] Viewer: tribe overlay, tribe statistics panel
- [x] Test: place entities in food-rich area, verify tribe forms after sustained interaction

### Phase 5.5: Territory

**Goal**: Tribes claim and defend territory.

- [x] Territory concept: cells where tribe members spend most time
- [x] Territorial behavior: prefer to stay within tribe territory
- [x] Border detection: where territories of different tribes meet
- [x] Territorial defense: increased aggression toward non-tribe members near territory
- [ ] Viewer: territory boundaries, heatmap of tribe presence
- [x] Test: two tribes near each other develop distinct territories with defended border

### Phase 5.6: Group Combat

**Goal**: Tribes fight tribes.

- [x] Group coordination: entities in same tribe preferentially attack same target
- [x] Strength in numbers: group attack is more effective than individual attacks
- [x] Retreat behavior: entity flees when tribe is losing (many members dead/fleeing)
- [x] War events: sustained combat between two tribes logged as higher-level event
- [ ] Viewer: war visualization, tribe population changes during conflict
- [x] Test: two tribes encounter each other, combat emerges, losing tribe retreats or is destroyed

### Phase 5.7: Teaching and Cultural Transmission

**Goal**: Knowledge spreads beyond genetics.

- [x] Teaching system: entity observes tribe member's successful action → adds to own memory
- [x] Learned behaviors: entity that observes successful food-finding adds food location to memory
- [x] Cultural traits: behaviors that spread through observation rather than genetics
- [x] Memory tag: distinguish genetic memory (from genome BT) from cultural memory (from observation)
- [x] Accumulated knowledge: tribes that teach have collective knowledge larger than any individual
- [x] Test: entity A finds food, entity B observes, entity B navigates to food without having found it independently

---

## Era 6: Tool Use and Construction

*After this era: entities create and use objects. They build structures, farm, and craft tools. Material culture emerges.*

### Phase 6.1: Object System

**Goal**: Objects can exist in the world independently of entities.

- [x] WorldObject component: position, object_type, material_properties, durability, creator
- [x] MaterialProperty struct: hardness, sharpness, weight, flexibility, nutritional_value
- [x] ObjectType: derived from material properties (not named categories)
- [x] Objects persist in the world, occupy space, can be perceived
- [x] Object decay: durability decreases over time, object eventually disappears
- [ ] Viewer: render objects as shapes based on material properties
- [x] Test: spawn object, verify it persists, verify it decays

### Phase 6.2: Object Interaction

**Goal**: Entities can pick up, carry, and drop objects.

- [x] Inventory component: list of carried objects, max carry capacity (from genome: size/strength)
- [x] New BT actions: PickUp, Drop, UseObject
- [x] New BT conditions: HasObject(property_filter), NearbyObject(property_filter)
- [x] Carry cost: carrying objects reduces movement speed proportional to weight
- [x] Object perception: objects appear in perception like resources
- [x] Test: entity picks up object, carries it, drops it at new location

### Phase 6.3: Object Creation -- Simple Tools

**Goal**: Entities can create objects from raw materials.

- [x] Raw materials: resources now have material properties (wood = flexible+light, stone = hard+heavy)
- [x] Blueprint component: a recipe in the genome -- "combine material A at location → produce object with properties"
- [x] Creation action: entity at resource → spend energy → create object
- [x] Blueprint is part of genome: evolves via GP (like behavior trees)
- [x] Blueprint representation: tree of operations (gather, combine, shape) with material parameters
- [x] First emergent tools: sharp stone (high sharpness from hard material)
- [x] Test: entity with blueprint creates object from resource, object has correct properties

### Phase 6.4: Tool Use -- Capability Modification

**Goal**: Using objects modifies entity capabilities.

- [x] Tool effects: object properties modify entity stats while held/used
  - [x] Sharp object: +attack_damage
  - [x] Large object: +defense (shield-like)
  - [x] Container object: +carry_capacity
  - [x] Pointed object: +resource_gathering_speed
- [x] Effect calculation: tool_bonus = f(relevant_material_properties)
- [x] Tool selection: BT decides which carried object to use for which action
- [x] Tools break: durability decreases with use
- [x] Test: entity with sharp tool deals more damage; entity with container carries more food

### Phase 6.5: Structures

**Goal**: Entities build persistent constructions in the world.

- [x] Structure component: position, size, material, structure_type, builder, durability
- [x] Structure types emerge from material properties:
  - [x] Wall: high hardness, blocks movement
  - [x] Shelter: enclosure that reduces environmental damage to entities inside
  - [x] Platform: flat surface, enables stacking
- [x] Construction system: entity places materials at location over multiple ticks
- [x] ConstructionSite component: tracks progress, required materials, blueprint
- [x] Spatial index integration: structures block movement, provide cover
- [ ] Viewer: structure rendering, construction progress indicators
- [x] Test: entity builds wall, verify movement blocked; entity builds shelter, verify damage reduction

### Phase 6.6: Crafting -- Combining Objects

**Goal**: Objects can be combined into more complex objects.

- [x] Crafting blueprint: genome-encoded recipe (input objects → output object)
- [x] Crafting action: entity with required objects in inventory → spend energy → produce new object
- [x] Material combination rules: combined properties based on input properties
  - [x] Sharp stone + flexible wood → sharp-on-stick (higher reach + sharpness)
  - [x] Properties emerge from the combination formula, not from named recipes
- [x] Blueprint evolution: crafting recipes evolve via GP alongside behavior trees
- [x] Multi-step crafting: output of one recipe can be input to another
- [x] Test: entity crafts combined object, verify properties are combination of inputs

### Phase 6.7: Agriculture

**Goal**: Entities modify the environment to grow food.

- [x] New BT action: PlantResource(type) -- place a resource seed at location
- [x] Planted resources: grow over time from seed → mature → harvestable
- [x] Tending: entities that visit planted resources boost growth rate
- [x] Crop types: different resources have different growth rates, nutritional values
- [x] Field concept: cluster of planted resources becomes a "farm" (emergent)
- [x] Farming knowledge: which resources to plant, where, when (in memory/blueprint)
- [x] Test: entity plants resource, resource grows, entity harvests mature resource

### Phase 6.8: Storage and Stockpiling

**Goal**: Entities can store resources and objects for later use.

- [x] Storage structure: a built structure that holds objects/resources
- [x] Deposit/withdraw actions: entities can place items in storage, retrieve later
- [x] Shared storage: tribe members can access tribal storage
- [x] Hoarding behavior: emergent from BT evolution (store when abundant, retrieve when scarce)
- [ ] Viewer: storage contents panel, resource flow visualization
- [x] Test: entity stores food during abundance, retrieves during scarcity

### Phase 6.9: Knowledge Transmission for Construction

**Goal**: Building knowledge spreads through culture, not just genetics.

- [x] Observation-based learning: entity watches another build → learns blueprint
- [x] Learned blueprints stored in memory (not genome) -- can be forgotten
- [x] Teaching: entity demonstrates construction to tribe member
- [x] Blueprint quality: more practice → higher quality constructions (refinement in memory)
- [x] Innovation: occasionally, mutation in learned blueprint produces improvement
- [x] Cultural evolution of technology: improvements spread faster than genetic evolution
- [x] Test: entity A builds structure, entity B observes and learns, entity B builds similar structure

### Phase 6.10: Infrastructure

**Goal**: Connected structures enable civilization-scale construction.

- [x] Roads: cleared/flattened paths that increase movement speed
- [x] Bridges: structures that enable crossing water
- [x] Walls and fortifications: defensive perimeters around settlements
- [x] Housing: structures that reduce energy cost (shelter from environment)
- [x] Irrigation: channels that increase resource growth in adjacent cells
- [x] Settlement detection: cluster of structures + persistent population = settlement
- [ ] Viewer: infrastructure overlay, settlement boundaries
- [x] Test: verify tribe builds structures that form a connected settlement

---

## Era 7: Narrative

*After this era: the system identifies, tracks, and narrates emergent stories.*

### Phase 7.1: Event Significance Scoring

**Goal**: Not all events are equally interesting. Score them.

- [x] Significance function: importance = state_change_magnitude × rarity × protagonist_relevance × irreversibility × relationship_involvement
- [x] Significant event buffer: events above threshold stored in narrative buffer
- [x] Entity interest score: long-lived, many offspring, many kills, extensive travel, many relationships
- [x] Auto-tracking: entities above interest threshold are automatically tracked
- [x] Test: death of a long-lived entity scores higher than death of a newborn

### Phase 7.2: Arc Detection

**Goal**: Identify narrative patterns in event sequences.

- [x] Arc types: Rivalry (repeated combat between same pair), Alliance (sustained cooperation), Migration (group movement across biomes), Extinction (species population → 0), Rise (entity or tribe gaining territory/population), Fall (losing territory/population)
- [x] Pattern matcher: sliding window over significant events, match against arc templates
- [x] StoryArc struct: type, protagonists, events, start_tick, tension_curve
- [x] Multiple concurrent arcs tracked
- [x] Test: simulate two entities repeatedly fighting, verify rivalry arc detected

### Phase 7.3: Entity Biography

**Goal**: Every tracked entity has a readable life story.

- [x] Biography generator: compile significant events for entity into chronological narrative
- [x] Chapter detection: group events into life phases (youth, exploration, combat, parenthood, etc.)
- [x] Key relationships: identify most important relationships from memory and social data
- [x] Legacy: offspring, territory influenced, tribe membership history
- [ ] Viewer: entity biography page with timeline, key events, relationships
- [x] Test: select long-lived entity, verify biography contains all major life events

### Phase 7.4: History Browser

**Goal**: Navigate simulation history interactively.

- [x] Timeline scrubber: drag through simulation history
- [x] Seek to tick: load snapshot, replay forward
- [x] Bookmark system: save interesting moments with notes
- [x] "What-if" forking: branch from any snapshot with modified parameters
- [x] Event search: find all events matching criteria (entity, type, tick range)
- [x] Historical analytics: population graphs, species diversity, resource levels over time
- [ ] Viewer: timeline component, bookmark list, analytics charts
- [x] Test: bookmark a moment, seek to it, verify correct state displayed

### Phase 7.5: LLM Narration

**Goal**: AI-generated prose narration of emergent stories.

- [x] LLM integration: Claude API or similar
- [x] Narrative prompt construction: feed arc events + entity details → receive prose
- [x] Real-time narration mode: narrate ongoing arcs as they develop
- [x] Historical narration: generate summary of entity life or civilization history
- [x] Narration styles: documentary, epic, journal, clinical
- [x] Narration cache: don't re-narrate events already narrated
- [ ] Viewer: narration panel alongside simulation, narration playback
- [x] Test: trigger arc, request narration, verify coherent prose output

---

## Era 8: Scale

*After this era: simulation handles 100k+ entities efficiently. Save/load works. Multiple viewers.*

### Phase 8.1: Performance Profiling

**Goal**: Identify bottlenecks before optimizing.

- [x] Tick timing: measure time per system per tick
- [x] Hotspot analysis: profile with `cargo flamegraph` or `perf`
- [x] Memory profiling: per-component memory usage, total entity memory
- [x] Spatial index performance: queries per tick, average neighbors per query
- [x] Network bandwidth: bytes per tick per client
- [x] Establish baseline metrics at 10k, 50k, 100k entities
- [x] Test: generate performance report at each entity count

### Phase 8.2: Multi-threaded Simulation

**Goal**: Use all CPU cores.

- [x] World partitioning: divide world into regions (quadtree or fixed grid)
- [x] Thread pool: `rayon` for parallel system execution
- [x] Identify parallelizable systems: perception, drives, aging (read-only entity state)
- [x] Identify sequential systems: movement, combat, reproduction (write entity state)
- [x] Parallel perception: each region's entities processed independently
- [x] Thread-safe spatial index: concurrent read, single-writer rebuild
- [x] Cross-region interactions: queue for next tick
- [x] Test: verify identical results between single-threaded and multi-threaded execution (determinism preserved)

### Phase 8.3: Level of Detail Simulation

**Goal**: Distant regions simulate at lower fidelity.

- [x] LOD levels: Full (all systems), Reduced (skip perception, simplified BT), Minimal (just aging/metabolism)
- [x] LOD assignment: based on distance from any connected viewer's viewport
- [x] LOD transitions: smooth handoff when viewer pans
- [x] Population-level simulation for Minimal LOD: statistical birth/death rates instead of individual ticks
- [x] Test: verify populations in distant regions evolve similarly to full-fidelity simulation (statistically)

### Phase 8.4: Save/Load

**Goal**: Full simulation state to/from disk.

- [x] Save: serialize all entities + components + environment + RNG state + event log metadata
- [x] Load: deserialize and reconstruct full simulation
- [x] File format: bincode with version header
- [x] Compression: zstd for save files
- [x] Auto-save: configurable interval
- [x] CLI: `--save <path>` and `--load <path>`
- [x] Test: save at tick 10,000, load, continue to 20,000, verify identical to continuous run

### Phase 8.5: Multiple Viewers

**Goal**: Many browsers connected simultaneously.

- [x] Per-client sessions with independent viewports and detail levels
- [x] Broadcast system: compute deltas once, filter per viewport
- [x] Client limit: configurable max connections
- [x] Graceful degradation: reduce detail level if server is overloaded
- [x] Admin viewer: sees everything, can issue commands
- [x] Spectator viewer: read-only, no commands
- [x] Test: connect 10 viewers with different viewports, verify each receives correct data

---

## Era 9: Civilization

*After this era: settlements exist with internal structure. Trade routes connect settlements. Political hierarchy emerges. Wars are fought over resources and territory.*

### Phase 9.1: Settlement Formation

**Goal**: Persistent settlements emerge from entity behavior.

- [x] Settlement detection: algorithm identifies clusters of structures + persistent population
- [x] Settlement entity: virtual entity representing the settlement (population, structures, territory, resources)
- [x] Settlement naming: generated from founding entities' genome hashes
- [x] Settlement growth/decline: tracked over time
- [ ] Viewer: settlement markers, settlement detail panel, settlement history
- [x] Test: tribe builds structures in an area, verify settlement detected

### Phase 9.2: Resource Specialization

**Goal**: Different settlements produce different things.

- [x] Regional resources: biome determines available raw materials
- [x] Settlement specialization: settlements near forests produce wood items, near mountains produce stone items
- [x] Surplus: settlements produce more of their specialty than they consume
- [x] Deficit: settlements lack materials not available in their biome
- [x] Test: verify settlements in different biomes produce different object types

### Phase 9.3: Trade

**Goal**: Resources flow between settlements.

- [x] Trade behavior: entities carry surplus resources to neighboring settlements
- [x] Trade routes: paths frequently traveled by traders become "trade routes" (emergent)
- [x] Value estimation: entities learn which resources are scarce elsewhere (from memory)
- [x] Trade signals: settlements emit signals indicating surplus/deficit
- [x] Mutual benefit: both settlements gain from trade (comparative advantage)
- [ ] Viewer: trade route visualization, resource flow arrows
- [x] Test: settlement A has surplus food, settlement B has surplus tools, verify trade emerges

### Phase 9.4: Defense and Warfare

**Goal**: Settlements defend themselves. Inter-settlement conflict.

- [x] Defensive structures: walls, watchtowers (detect approaching threats at range)
- [x] Garrison behavior: some entities stay near settlement for defense
- [x] Raiding: aggressive tribes/settlements attack others for resources
- [x] Siege mechanics: sustained assault on fortified settlement
- [x] Conquest: defeated settlement absorbed by victor
- [x] War history: recorded as major narrative event
- [ ] Viewer: military events, defensive structures, battle visualization
- [x] Test: resource-poor settlement raids resource-rich one, verify combat around structures

### Phase 9.5: Emergent Hierarchy

**Goal**: Political/social structure within settlements.

- [x] Leadership: entity with highest social influence (most relationships, most respect) becomes de facto leader
- [x] Leader behavior: leader's BT influences tribe/settlement behavior
- [x] Specialization of labor: entities with different BTs fill different roles (farmer, builder, warrior, scout)
- [x] Resource distribution: how resources are shared within settlement (leader decides? equal? need-based?)
- [x] Succession: when leader dies, new leader emerges from social dynamics
- [x] Test: verify settlement has identifiable leader, verify leadership changes on death

### Phase 9.6: Cultural Identity

**Goal**: Settlements develop distinct cultures.

- [x] Cultural markers: shared signal usage patterns, construction styles, behavioral norms
- [x] Cultural drift: isolated settlements diverge culturally over time
- [x] Cultural exchange: trade and contact spread cultural elements
- [x] Language divergence: signal meanings drift between separated groups
- [ ] Viewer: cultural comparison between settlements
- [x] Test: separate two settlements for many generations, verify measurable cultural divergence

### Phase 9.7: NEAT Integration

**Goal**: Neural network modules within behavior trees for nuanced decisions.

- [x] NEAT implementation: evolve small neural networks
- [x] BT integration: neural networks as decision nodes within behavior trees
- [x] Input: drive values, perception summary, memory features
- [x] Output: action selection weights, parameter modulation
- [x] Hybrid BT: interpretable high-level structure (GP-evolved) + nuanced low-level decisions (NEAT-evolved)
- [x] Test: entities with NEAT-augmented BTs make more adaptive decisions than pure BT entities

---

## Era 10: The Third Dimension

*After this era: the world has height, depth, and volume. Entities can swim, climb, and fly.*

### Phase 10.1: 3D Coordinate System

**Goal**: Foundation for 3D world.

- [x] Position3D, Velocity3D components (add Z axis)
- [x] Backward compatibility: 2D systems still work (Z=0 for ground level)
- [x] 3D distance calculations, 3D bounding volumes
- [x] Config flag: `dimensions: 2 | 3`
- [x] All existing systems updated to handle Z when present
- [x] Test: entities with Z=0 behave identically to 2D mode

### Phase 10.2: 3D Terrain -- Height Map

**Goal**: The ground has elevation.

- [x] Height map: 2D noise grid defining ground elevation at each (x,y)
- [x] Terrain mesh generation from height map
- [x] Elevation affects: movement speed (uphill slower), visibility (hilltops see farther), temperature (higher = colder)
- [x] Slope limits: entities can't climb vertical cliffs (without climbing ability)
- [x] Water level: height below threshold = water
- [x] Test: verify entities navigate around steep terrain, prefer flat paths

### Phase 10.3: 3D Terrain -- Caves and Underground

**Goal**: Subterranean spaces exist.

- [x] Cave generation: 3D noise with threshold → cave volumes
- [x] Underground terrain types: stone, crystal, magma
- [x] Underground resources: rare minerals only found in caves
- [x] Darkness: underground has no light (affects perception)
- [x] Cave entrances: surface-accessible openings to underground
- [x] Underground ecology: separate from surface (new niches)
- [x] Test: entities discover cave entrance, navigate underground, find unique resources

### Phase 10.4: Three.js Renderer

**Goal**: Replace PixiJS with Three.js for 3D visualization.

- [x] Three.js setup: Scene, Camera, Renderer
- [x] Terrain mesh rendering from height map
- [x] Entity rendering: InstancedMesh for thousands of entities
- [x] Per-instance attributes: position, color, size
- [x] Camera controls: orbit, pan, zoom, follow entity
- [x] Ground plane with terrain texture
- [x] Performance target: 10k entities at 60fps
- [x] Test: existing simulation renders in 3D, all entities visible

### Phase 10.5: 3D Entity Rendering

**Goal**: Entities look appropriate in 3D.

- [x] Entity meshes: simple 3D shapes (spheres, capsules) based on species/size
- [x] Composite rendering: multi-mesh clusters for composite organisms
- [x] LOD rendering: distant entities as points, near entities as meshes
- [x] Animation: simple bob/pulse for movement, flash for combat
- [x] Structure rendering: 3D boxes, walls, roofs
- [x] Resource rendering: 3D models for different resource types
- [x] Test: zoom in on entity, verify appropriate 3D representation

### Phase 10.6: 3D Camera System

**Goal**: Navigate the 3D world intuitively.

- [x] Orbit camera: rotate around a point
- [x] Free camera: fly through the world
- [x] Follow camera: track selected entity from adjustable distance
- [x] Top-down camera: overhead view (like 2D mode but with depth)
- [x] Cross-section view: slice the world horizontally, see underground at that depth
- [x] Camera bookmarks: save/restore camera positions
- [x] Smooth transitions between camera modes
- [x] Test: all camera modes work, transitions are smooth

### Phase 10.7: 3D Spatial Index

**Goal**: Efficient 3D proximity queries.

- [x] 3D grid hash: extend 2D spatial hash to 3 dimensions
- [x] Octree alternative: for non-uniform entity distribution
- [x] 3D range queries: find entities within sphere
- [x] 3D ray queries: line-of-sight checks (for perception)
- [x] Vertical perception: entities above/below can detect each other
- [x] Performance benchmark: compare grid hash vs. octree at different densities
- [x] Test: 3D proximity queries return correct results, performance acceptable at 10k entities

### Phase 10.8: Water and Fluid

**Goal**: Water bodies as navigable (or impassable) terrain.

- [x] Water volumes: defined by terrain height vs. water level
- [x] Floating: entities at water surface
- [x] Swimming: movement in water (different speed, higher energy cost)
- [x] Underwater: fully submerged volumes with reduced perception
- [x] Aquatic specialization: entities evolve for water (faster swimming, underwater breathing analogue)
- [x] Water resources: aquatic food sources
- [x] Test: entities near water learn to swim for aquatic resources; aquatic species emerge

### Phase 10.9: Aerial Movement

**Goal**: Entities can move in 3D space above ground.

- [x] Flight capability: genome trait (wing_strength, flight_efficiency)
- [x] Flight mechanics: move in 3D with energy cost proportional to altitude + speed
- [x] Gliding: using altitude for energy-efficient horizontal movement
- [x] Aerial perception: flying entities see larger area from above
- [x] Landing/takeoff: transition between ground and air movement
- [x] Aerial predation: dive from above
- [x] Test: entity evolves flight, verify it navigates in 3D, verify aerial advantages

### Phase 10.10: 3D Lighting and Atmosphere

**Goal**: Visual quality and environmental effects in 3D.

- [x] Day/night cycle: directional light rotates, affects visibility
- [x] Shadows: entities and structures cast shadows
- [x] Fog/atmosphere: distant objects fade (also helps performance)
- [x] Weather visualization: rain, snow particles in relevant biomes
- [x] Underground darkness: no light below surface
- [x] Bioluminescence: some evolved entities glow (emergent trait)
- [x] Test: visual quality acceptable, day/night cycle affects entity behavior

### Phase 10.11: 3D Construction

**Goal**: Structures work in 3D.

- [x] 3D structure placement: on ground, on slopes, underground
- [x] Multi-story structures: stack vertically
- [x] Ramps and stairs: enable vertical movement between levels
- [x] Bridges over water: built between landmasses
- [x] Cave dwellings: structures inside caves
- [x] Structural integrity: tall structures require support
- [x] Test: entity builds multi-story structure, verify it's navigable

### Phase 10.12: 3D Performance Optimization

**Goal**: 3D simulation at scale.

- [x] Frustum culling: don't render entities outside camera view
- [x] LOD meshes: simpler meshes at distance
- [x] Instanced rendering optimization: batch similar entities
- [x] Chunk-based terrain: only load/render nearby terrain
- [x] GPU compute: offload particle effects, simple physics
- [x] Target: 50k entities at 30fps in 3D
- [x] Test: benchmark at 10k, 50k entities, verify frame rate targets

---

## Era 11: Open World

*After this era: the world is infinite and procedurally generated. Multiple civilizations discover each other.*

### Phase 11.1: Chunk System

**Goal**: World divided into loadable chunks.

- [x] Chunk definition: fixed-size square (e.g., 256x256) of terrain + entities + structures
- [x] Chunk states: Active (full simulation), Dormant (statistical simulation), Unloaded (on disk)
- [x] Chunk activation: based on entity presence or viewer proximity
- [x] Chunk border handling: entities crossing chunk boundaries
- [x] Test: entity migrates from one chunk to another, verify seamless transition

### Phase 11.2: Procedural World Generation

**Goal**: New chunks generated on demand.

- [x] World seed: single seed generates entire infinite world deterministically
- [x] Biome generation: large-scale noise determines biome at any coordinate
- [x] Terrain generation: per-chunk noise generates detailed terrain
- [x] Resource distribution: biome-appropriate resources placed procedurally
- [x] Feature generation: rivers, mountains, coastlines at world scale
- [x] Test: generate chunk at coordinates (1000, 1000), verify terrain is consistent across regeneration

### Phase 11.3: Chunk Loading/Unloading

**Goal**: Memory-efficient world management.

- [x] Active chunk limit: maximum chunks in full simulation
- [x] Dormant simulation: statistical population/resource model for inactive chunks
- [x] Disk serialization: unloaded chunks saved to disk
- [x] Load on demand: chunk activates when entity migrates into it
- [x] Viewer chunk streaming: send terrain data for chunks as viewer pans
- [x] Test: entities spread across many chunks, verify only nearby chunks are active

### Phase 11.4: Long-Distance Migration

**Goal**: Entities and species spread across the world.

- [x] Migration triggers: resource scarcity, overcrowding, curiosity drive
- [x] Long-distance pathfinding: navigate between chunks toward perceived better conditions
- [x] Species range maps: track where each species exists across chunks
- [x] Colonization: entities arriving in uninhabited chunks establish new populations
- [ ] Viewer: world-scale species range map, migration path visualization
- [x] Test: overpopulate a region, verify emigration to adjacent chunks

### Phase 11.5: Multi-Civilization Encounters

**Goal**: Isolated civilizations meet.

- [x] First contact event: two civilizations encounter each other for the first time
- [x] Communication barrier: different signal "languages" evolved independently
- [x] Conflict or cooperation: outcome depends on evolved behaviors of both civilizations
- [x] Cultural exchange: successful contact leads to signal/blueprint sharing
- [x] Conquest: one civilization overwhelms another
- [x] Narrative significance: first contact is always a major story event
- [ ] Viewer: civilization map, first-contact events highlighted
- [x] Test: seed two separated civilizations, verify they eventually encounter each other

### Phase 11.6: World-Scale Analytics

**Goal**: Understand the entire world at a glance.

- [x] Civilization map: color-coded territories across all chunks
- [x] Population heatmap: entity density across world
- [x] Resource flow: trade routes across world scale
- [x] Species distribution: where each species lives
- [x] Historical timeline: rise and fall of civilizations across the entire world
- [x] Zoom levels: seamless zoom from world overview → continent → settlement → individual entity
- [x] Test: viewer zoom from world scale to individual entity without interruption
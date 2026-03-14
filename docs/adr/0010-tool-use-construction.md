# ADR-0010: Emergent Tool Use and Construction

**Status**: Accepted
**Date**: 2026-03-14
**Decision**: Implement tool use and construction through a material-properties-based object system where "tools" and "structures" are not named categories but emergent from physical properties

## Context

For the civilization vision, entities must be able to create and use objects, build structures, and develop agriculture. The core design principle of Autonomy is that behaviors emerge rather than being hard-coded. This applies to tool use: we don't define "hammer" or "house." We define material properties and let evolution discover what to build.

## Decision

### Object System

Objects are entities with material properties. There are no predefined object types:

```rust
struct WorldObject {
    position: Position,
    material: MaterialProperties,
    durability: f32,       // decreases over time and with use
    creator: Option<EntityId>,
    creation_tick: u64,
}

struct MaterialProperties {
    hardness: f32,        // 0.0 (soft) to 1.0 (diamond)
    sharpness: f32,       // 0.0 (blunt) to 1.0 (razor)
    weight: f32,          // affects carry cost and impact force
    flexibility: f32,     // 0.0 (rigid) to 1.0 (rope-like)
    size: f32,            // physical volume
    nutritional_value: f32, // can it be eaten?
    thermal_resistance: f32, // insulation value
}
```

### Blueprint System

Blueprints (recipes for creating objects) are encoded in the genome alongside behavior trees. They evolve via the same GP operators:

```rust
struct Blueprint {
    inputs: Vec<MaterialFilter>,    // what materials are needed
    operations: Vec<Operation>,      // what to do with them
    output_properties: MaterialPropertyModifiers, // how output differs from input
    energy_cost: f32,
}

enum Operation {
    Shape { target_sharpness: f32, target_shape: f32 },
    Combine { method: CombineMethod },
    Harden { method: HardenMethod },
    Place { relative_position: Position },
}
```

### Tool Effects

Objects modify entity capabilities through their material properties. No predefined tool categories:

- Object with high sharpness → bonus to attack damage and resource gathering
- Object with high hardness + large size → bonus to defense
- Object that is hollow/flexible → increases carry capacity (container)
- Object with high thermal_resistance → reduces environmental damage

### Structures

Structures are collections of placed objects that form spatial features:

- Objects with high hardness placed in a line → wall (blocks movement, detected by spatial index)
- Objects placed to form enclosure → shelter (reduces environmental damage inside)
- Cleared terrain + planted resources → farm (increased food production)

Structures are not a special entity type. They are emergent from the spatial arrangement of objects.

### Knowledge Transmission

Blueprints exist in two places:
1. **Genome**: inherited, evolves slowly via mutation/crossover
2. **Memory**: learned from observation, spreads culturally, can be forgotten

Cultural transmission is faster than genetic evolution. An entity that observes successful construction can learn the blueprint and teach it to others. This mirrors biological reality where tool use spreads culturally.

## Rationale

### Why material properties instead of named types?

| Approach | Pros | Cons |
|----------|------|------|
| Material properties (chosen) | Infinite combinatorial space. Novel tools can emerge. No design ceiling. | Harder to visualize. Harder to narrate. |
| Named tool types (Axe, Hammer, Shelter) | Easy to understand. Clear game design. | Finite design space. No emergence. Tools are designed, not discovered. |

The material-properties approach is essential for true emergence. We can't predict what tools entities will discover -- that's the point. A sharp stone on a stick is not a "spear" until evolution makes it one.

### Why blueprints in the genome?

Blueprints need to evolve because:
- Tool creation is a behavior, and behaviors evolve via GP
- Good tool designs provide survival advantage → selected for
- The blueprint is part of the "extended phenotype" (Dawkins) -- the organism's effect on its environment

Blueprints also exist in memory because:
- Cultural transmission is faster than genetic evolution
- One innovative entity can teach an entire tribe
- This creates a "technology acceleration" effect seen in real civilizations

## Consequences

- Objects add to the spatial index and affect pathfinding
- Viewer needs object rendering based on material properties (visual encoding of hardness, sharpness, etc.)
- Blueprint evolution adds complexity to the genome (larger search space)
- Construction events become narrative material ("Entity X built the first wall")
- Resource gathering becomes more nuanced (gather specific materials, not just "food")
- Memory must store blueprint knowledge (learned from observation)
- Performance: many persistent objects in the world increases spatial index size

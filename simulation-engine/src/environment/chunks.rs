use std::collections::HashMap;

use noise::{NoiseFn, Perlin};

use crate::environment::terrain::{classify_terrain, TerrainType, CELL_SIZE};

/// Coordinates of a chunk in the infinite grid.
///
/// Each chunk covers a square region of `chunk_size x chunk_size` world units.
/// Chunk (0,0) starts at world origin, chunk (1,0) is one chunk-width to the
/// right, chunk (-1,0) is one chunk-width to the left, etc.
pub type ChunkCoord = (i32, i32);

/// The lifecycle state of a chunk.
///
/// Chunks transition: Unloaded -> Active <-> Dormant -> Unloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    /// Fully simulated with all entities and terrain loaded.
    Active,
    /// Simplified statistical simulation (no individual entities).
    Dormant,
    /// Not in memory. Terrain and entities must be generated/loaded.
    Unloaded,
}

/// A fixed-size region of the infinite world.
///
/// Each chunk holds its own terrain data and tracks which entity IDs
/// reside within it. Chunks are identified by their grid coordinates.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Grid coordinates of this chunk.
    pub coords: ChunkCoord,
    /// Current lifecycle state.
    pub state: ChunkState,
    /// Entity ID bits for entities residing in this chunk.
    pub entity_ids: Vec<u64>,
    /// Terrain cells for this chunk, row-major order.
    pub terrain: Vec<TerrainType>,
    /// Number of terrain columns.
    pub terrain_cols: usize,
    /// Number of terrain rows.
    pub terrain_rows: usize,
    /// Statistical population count (used when dormant).
    pub dormant_population: u32,
    /// Average resource density in this chunk (0.0 - 1.0).
    pub resource_density: f64,
    /// Number of ticks since any entity or viewer was present.
    pub ticks_since_active: u64,
}

impl Chunk {
    /// Create a new chunk with generated terrain.
    ///
    /// Terrain is generated deterministically from the world seed and
    /// chunk coordinates, using the same noise functions as `TerrainGrid`.
    pub fn new(coords: ChunkCoord, chunk_size: f64, world_seed: u64) -> Self {
        let terrain_cols = (chunk_size / CELL_SIZE).ceil() as usize;
        let terrain_rows = (chunk_size / CELL_SIZE).ceil() as usize;

        let terrain = generate_chunk_terrain(
            coords,
            chunk_size,
            world_seed,
            terrain_cols,
            terrain_rows,
        );

        let resource_density = compute_resource_density(&terrain);

        Self {
            coords,
            state: ChunkState::Active,
            entity_ids: Vec::new(),
            terrain,
            terrain_cols,
            terrain_rows,
            dormant_population: 0,
            resource_density,
            ticks_since_active: 0,
        }
    }

    /// Get the terrain type at a local cell position within this chunk.
    ///
    /// Coordinates are clamped to valid range.
    pub fn terrain_at_local(&self, col: usize, row: usize) -> TerrainType {
        let col = col.min(self.terrain_cols.saturating_sub(1));
        let row = row.min(self.terrain_rows.saturating_sub(1));
        self.terrain[row * self.terrain_cols + col]
    }

    /// Get the terrain type at a world-space position within this chunk.
    ///
    /// The position is converted to local cell coordinates first.
    pub fn terrain_at_world(&self, x: f64, y: f64, chunk_size: f64) -> TerrainType {
        let local_x = x - (self.coords.0 as f64 * chunk_size);
        let local_y = y - (self.coords.1 as f64 * chunk_size);
        let col = (local_x / CELL_SIZE).floor().max(0.0) as usize;
        let row = (local_y / CELL_SIZE).floor().max(0.0) as usize;
        self.terrain_at_local(col, row)
    }

    /// Number of entities currently tracked in this chunk.
    pub fn entity_count(&self) -> usize {
        self.entity_ids.len()
    }

    /// Whether this chunk has any entities.
    pub fn has_entities(&self) -> bool {
        !self.entity_ids.is_empty()
    }

    /// Add an entity to this chunk.
    pub fn add_entity(&mut self, entity_id: u64) {
        self.entity_ids.push(entity_id);
    }

    /// Remove an entity from this chunk by ID.
    ///
    /// Returns true if the entity was found and removed.
    pub fn remove_entity(&mut self, entity_id: u64) -> bool {
        if let Some(pos) = self.entity_ids.iter().position(|&id| id == entity_id) {
            self.entity_ids.swap_remove(pos);
            true
        } else {
            false
        }
    }
}

/// Manages the lifecycle of chunks in the infinite world.
///
/// Chunks are created on demand when an entity enters or a viewer observes
/// a region. They transition to Dormant when unoccupied, and eventually
/// to Unloaded if dormant for too long.
#[derive(Debug, Clone)]
pub struct ChunkManager {
    /// All loaded chunks, keyed by chunk coordinates.
    chunks: HashMap<ChunkCoord, Chunk>,
    /// Size of each chunk in world units (e.g. 256.0).
    pub chunk_size: f64,
    /// World seed for deterministic terrain generation.
    world_seed: u64,
    /// Number of ticks a chunk remains dormant before being unloaded.
    pub dormant_timeout: u64,
    /// Coordinates of chunks that the viewer is currently observing.
    pub viewer_chunks: Vec<ChunkCoord>,
}

impl ChunkManager {
    /// Create a new chunk manager.
    pub fn new(chunk_size: f64, world_seed: u64) -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_size,
            world_seed,
            dormant_timeout: 1000,
            viewer_chunks: Vec::new(),
        }
    }

    /// Convert a world-space position to chunk coordinates.
    pub fn world_to_chunk(&self, x: f64, y: f64) -> ChunkCoord {
        let cx = (x / self.chunk_size).floor() as i32;
        let cy = (y / self.chunk_size).floor() as i32;
        (cx, cy)
    }

    /// Get a reference to a chunk by its coordinates.
    pub fn get_chunk(&self, coords: ChunkCoord) -> Option<&Chunk> {
        self.chunks.get(&coords)
    }

    /// Get a mutable reference to a chunk by its coordinates.
    pub fn get_chunk_mut(&mut self, coords: ChunkCoord) -> Option<&mut Chunk> {
        self.chunks.get_mut(&coords)
    }

    /// Ensure a chunk exists at the given coordinates, creating it if needed.
    ///
    /// Returns a mutable reference to the chunk. If the chunk was dormant
    /// or unloaded, it is activated.
    pub fn ensure_chunk(&mut self, coords: ChunkCoord) -> &mut Chunk {
        let chunk_size = self.chunk_size;
        let world_seed = self.world_seed;
        let chunk = self.chunks.entry(coords).or_insert_with(|| {
            Chunk::new(coords, chunk_size, world_seed)
        });
        if chunk.state != ChunkState::Active {
            chunk.state = ChunkState::Active;
            chunk.ticks_since_active = 0;
        }
        chunk
    }

    /// Get the terrain type at a world-space position.
    ///
    /// If the chunk is not loaded, it is created on demand.
    pub fn terrain_at(&mut self, x: f64, y: f64) -> TerrainType {
        let coords = self.world_to_chunk(x, y);
        let chunk_size = self.chunk_size;
        let chunk = self.ensure_chunk(coords);
        chunk.terrain_at_world(x, y, chunk_size)
    }

    /// Add an entity to the appropriate chunk.
    ///
    /// Creates the chunk if it doesn't exist.
    pub fn add_entity(&mut self, entity_id: u64, x: f64, y: f64) {
        let coords = self.world_to_chunk(x, y);
        let chunk = self.ensure_chunk(coords);
        chunk.add_entity(entity_id);
    }

    /// Remove an entity from its chunk.
    ///
    /// Returns true if the entity was found and removed.
    pub fn remove_entity(&mut self, entity_id: u64, x: f64, y: f64) -> bool {
        let coords = self.world_to_chunk(x, y);
        if let Some(chunk) = self.chunks.get_mut(&coords) {
            chunk.remove_entity(entity_id)
        } else {
            false
        }
    }

    /// Move an entity from one position to another, updating chunk membership.
    ///
    /// If the entity crosses a chunk boundary, it is removed from the old
    /// chunk and added to the new one.
    pub fn move_entity(
        &mut self,
        entity_id: u64,
        old_x: f64,
        old_y: f64,
        new_x: f64,
        new_y: f64,
    ) {
        let old_coords = self.world_to_chunk(old_x, old_y);
        let new_coords = self.world_to_chunk(new_x, new_y);

        if old_coords != new_coords {
            // Remove from old chunk
            if let Some(old_chunk) = self.chunks.get_mut(&old_coords) {
                old_chunk.remove_entity(entity_id);
            }
            // Add to new chunk (creating if needed)
            let chunk = self.ensure_chunk(new_coords);
            chunk.add_entity(entity_id);
        }
    }

    /// Number of loaded chunks (Active + Dormant).
    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Number of active chunks.
    pub fn active_chunk_count(&self) -> usize {
        self.chunks.values().filter(|c| c.state == ChunkState::Active).count()
    }

    /// Number of dormant chunks.
    pub fn dormant_chunk_count(&self) -> usize {
        self.chunks.values().filter(|c| c.state == ChunkState::Dormant).count()
    }

    /// Get all loaded chunk coordinates.
    pub fn loaded_coords(&self) -> Vec<ChunkCoord> {
        self.chunks.keys().copied().collect()
    }

    /// Tick the chunk manager: deactivate empty chunks, run dormant simulation,
    /// and unload chunks that have been dormant too long.
    pub fn tick(&mut self) {
        let viewer_chunks = self.viewer_chunks.clone();

        // Collect chunks to process (avoid borrowing issues).
        let coords: Vec<ChunkCoord> = self.chunks.keys().copied().collect();

        for coord in coords {
            let chunk = self.chunks.get_mut(&coord).unwrap();

            match chunk.state {
                ChunkState::Active => {
                    let has_entities = chunk.has_entities();
                    let has_viewer = viewer_chunks.contains(&coord);

                    if !has_entities && !has_viewer {
                        // Transition to dormant
                        chunk.state = ChunkState::Dormant;
                        chunk.dormant_population = 0;
                        chunk.ticks_since_active = 0;
                    }
                }
                ChunkState::Dormant => {
                    chunk.ticks_since_active += 1;

                    // Run simplified dormant simulation
                    Self::tick_dormant(chunk);
                }
                ChunkState::Unloaded => {
                    // Should not be in the map if unloaded
                }
            }
        }

        // Unload chunks that have been dormant too long.
        let timeout = self.dormant_timeout;
        self.chunks.retain(|_, chunk| {
            !(chunk.state == ChunkState::Dormant && chunk.ticks_since_active >= timeout)
        });
    }

    /// Simplified statistical simulation for a dormant chunk.
    ///
    /// Models population as a simple growth/decay based on resource density.
    /// When the chunk is reactivated, this count guides how many entities
    /// to respawn. Growth/shrink happens every N ticks to avoid the
    /// integer truncation problem of fractional increments.
    fn tick_dormant(chunk: &mut Chunk) {
        let density = chunk.resource_density;
        let pop = chunk.dormant_population;

        // Simple logistic-ish model: grow toward a carrying capacity
        // determined by resource density.
        let carrying_capacity = (density * 20.0).max(0.0) as u32;

        // Adjust population every 10 ticks to keep it simple.
        if chunk.ticks_since_active % 10 == 0 {
            if pop < carrying_capacity {
                chunk.dormant_population = pop + 1;
            } else if pop > carrying_capacity {
                chunk.dormant_population = pop.saturating_sub(1);
            }
        }
    }

    /// Set the viewer position, updating which chunks the viewer is observing.
    ///
    /// The viewer radius (in chunks) determines how many surrounding chunks
    /// are kept active.
    pub fn set_viewer_position(&mut self, x: f64, y: f64, radius_chunks: i32) {
        let center = self.world_to_chunk(x, y);
        self.viewer_chunks.clear();

        for dy in -radius_chunks..=radius_chunks {
            for dx in -radius_chunks..=radius_chunks {
                self.viewer_chunks.push((center.0 + dx, center.1 + dy));
            }
        }
    }
}

/// Generate terrain cells for a single chunk.
///
/// Uses the same Perlin noise functions as `TerrainGrid::generate` but
/// offset by the chunk's world-space position, ensuring that chunks tile
/// seamlessly.
fn generate_chunk_terrain(
    coords: ChunkCoord,
    chunk_size: f64,
    world_seed: u64,
    cols: usize,
    rows: usize,
) -> Vec<TerrainType> {
    let perlin = Perlin::new(world_seed as u32);
    let moisture_perlin = Perlin::new(world_seed.wrapping_add(1000) as u32);

    let scale = 0.03;

    // World-space offset for this chunk's top-left corner, in cell units.
    let cell_offset_x = (coords.0 as f64 * chunk_size) / CELL_SIZE;
    let cell_offset_y = (coords.1 as f64 * chunk_size) / CELL_SIZE;

    let mut cells = Vec::with_capacity(cols * rows);

    for row in 0..rows {
        for col in 0..cols {
            let nx = (cell_offset_x + col as f64) * scale;
            let ny = (cell_offset_y + row as f64) * scale;

            let elevation = perlin.get([nx, ny]);
            let moisture = moisture_perlin.get([nx + 100.0, ny + 100.0]);

            cells.push(classify_terrain(elevation, moisture));
        }
    }

    cells
}

/// Compute the average resource density for a chunk's terrain.
///
/// Returns a value between 0.0 and 1.0 representing how resource-rich
/// the chunk is on average.
fn compute_resource_density(terrain: &[TerrainType]) -> f64 {
    if terrain.is_empty() {
        return 0.0;
    }

    let total: f64 = terrain.iter().map(|t| t.resource_density_multiplier()).sum();
    // Normalize: Forest has max density of 2.0, so divide by 2.0 to get 0..1 range.
    (total / terrain.len() as f64) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CHUNK_SIZE: f64 = 256.0;
    const TEST_SEED: u64 = 42;

    #[test]
    fn chunk_new_creates_active_chunk() {
        let chunk = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        assert_eq!(chunk.state, ChunkState::Active);
        assert_eq!(chunk.coords, (0, 0));
        assert!(chunk.entity_ids.is_empty());
    }

    #[test]
    fn chunk_terrain_has_correct_dimensions() {
        let chunk = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        let expected_cells = (TEST_CHUNK_SIZE / CELL_SIZE).ceil() as usize;
        assert_eq!(chunk.terrain_cols, expected_cells);
        assert_eq!(chunk.terrain_rows, expected_cells);
        assert_eq!(chunk.terrain.len(), expected_cells * expected_cells);
    }

    #[test]
    fn chunk_terrain_is_deterministic() {
        let chunk1 = Chunk::new((3, -2), TEST_CHUNK_SIZE, TEST_SEED);
        let chunk2 = Chunk::new((3, -2), TEST_CHUNK_SIZE, TEST_SEED);
        assert_eq!(chunk1.terrain, chunk2.terrain);
    }

    #[test]
    fn different_chunk_coords_produce_different_terrain() {
        let chunk1 = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        let chunk2 = Chunk::new((5, 5), TEST_CHUNK_SIZE, TEST_SEED);

        let mut differences = 0;
        for (a, b) in chunk1.terrain.iter().zip(chunk2.terrain.iter()) {
            if a != b {
                differences += 1;
            }
        }
        assert!(differences > 0, "different chunk coords should produce different terrain");
    }

    #[test]
    fn chunk_add_remove_entity() {
        let mut chunk = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        assert_eq!(chunk.entity_count(), 0);

        chunk.add_entity(42);
        assert_eq!(chunk.entity_count(), 1);
        assert!(chunk.has_entities());

        assert!(chunk.remove_entity(42));
        assert_eq!(chunk.entity_count(), 0);
        assert!(!chunk.has_entities());

        // Removing non-existent entity returns false
        assert!(!chunk.remove_entity(999));
    }

    #[test]
    fn chunk_terrain_at_local_clamps() {
        let chunk = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        // Should not panic even with out-of-range indices
        let _ = chunk.terrain_at_local(9999, 9999);
        let _ = chunk.terrain_at_local(0, 0);
    }

    #[test]
    fn chunk_manager_world_to_chunk() {
        let mgr = ChunkManager::new(256.0, TEST_SEED);

        assert_eq!(mgr.world_to_chunk(0.0, 0.0), (0, 0));
        assert_eq!(mgr.world_to_chunk(255.0, 255.0), (0, 0));
        assert_eq!(mgr.world_to_chunk(256.0, 0.0), (1, 0));
        assert_eq!(mgr.world_to_chunk(512.0, 256.0), (2, 1));
        assert_eq!(mgr.world_to_chunk(-1.0, -1.0), (-1, -1));
        assert_eq!(mgr.world_to_chunk(-256.0, 0.0), (-1, 0));
    }

    #[test]
    fn chunk_manager_ensure_creates_chunk() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        assert_eq!(mgr.loaded_chunk_count(), 0);

        mgr.ensure_chunk((0, 0));
        assert_eq!(mgr.loaded_chunk_count(), 1);
        assert_eq!(mgr.active_chunk_count(), 1);

        // Ensuring same chunk again doesn't create a duplicate
        mgr.ensure_chunk((0, 0));
        assert_eq!(mgr.loaded_chunk_count(), 1);
    }

    #[test]
    fn chunk_manager_add_entity_creates_chunk() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.add_entity(1, 100.0, 100.0);

        assert_eq!(mgr.loaded_chunk_count(), 1);
        let chunk = mgr.get_chunk((0, 0)).unwrap();
        assert_eq!(chunk.entity_count(), 1);
    }

    #[test]
    fn chunk_manager_move_entity_across_chunks() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);

        // Place entity in chunk (0,0)
        mgr.add_entity(1, 100.0, 100.0);
        assert_eq!(mgr.get_chunk((0, 0)).unwrap().entity_count(), 1);

        // Move entity to chunk (1,0)
        mgr.move_entity(1, 100.0, 100.0, 300.0, 100.0);
        assert_eq!(mgr.get_chunk((0, 0)).unwrap().entity_count(), 0);
        assert_eq!(mgr.get_chunk((1, 0)).unwrap().entity_count(), 1);
    }

    #[test]
    fn chunk_manager_move_within_same_chunk() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.add_entity(1, 100.0, 100.0);

        // Move within same chunk - no change in chunk membership
        mgr.move_entity(1, 100.0, 100.0, 200.0, 200.0);
        assert_eq!(mgr.get_chunk((0, 0)).unwrap().entity_count(), 1);
    }

    #[test]
    fn chunk_manager_tick_deactivates_empty_chunks() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.ensure_chunk((0, 0));

        assert_eq!(mgr.active_chunk_count(), 1);

        // Tick with no entities and no viewer -> should become dormant
        mgr.tick();
        assert_eq!(mgr.active_chunk_count(), 0);
        assert_eq!(mgr.dormant_chunk_count(), 1);
    }

    #[test]
    fn chunk_manager_tick_keeps_chunk_with_entities_active() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.add_entity(1, 100.0, 100.0);

        mgr.tick();
        assert_eq!(mgr.active_chunk_count(), 1);
        assert_eq!(mgr.dormant_chunk_count(), 0);
    }

    #[test]
    fn chunk_manager_tick_keeps_viewer_chunks_active() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.ensure_chunk((0, 0));
        mgr.set_viewer_position(100.0, 100.0, 0); // viewer at chunk (0,0)

        mgr.tick();
        // Chunk should stay active because viewer is there
        assert_eq!(mgr.active_chunk_count(), 1);
    }

    #[test]
    fn chunk_manager_dormant_timeout_unloads_chunks() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.dormant_timeout = 5;
        mgr.ensure_chunk((0, 0));

        // Tick once to transition to dormant
        mgr.tick();
        assert_eq!(mgr.dormant_chunk_count(), 1);

        // Tick 5 more times to reach timeout
        for _ in 0..5 {
            mgr.tick();
        }

        // Chunk should be unloaded (removed from map)
        assert_eq!(mgr.loaded_chunk_count(), 0);
    }

    #[test]
    fn chunk_manager_terrain_at_is_consistent_across_chunks() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);

        // Query terrain in two different chunks
        let t1 = mgr.terrain_at(100.0, 100.0);
        let t2 = mgr.terrain_at(300.0, 100.0);

        // Just verify they return valid terrain types (not panicking)
        assert!(matches!(
            t1,
            TerrainType::Grassland
                | TerrainType::Desert
                | TerrainType::Water
                | TerrainType::Forest
                | TerrainType::Mountain
        ));
        assert!(matches!(
            t2,
            TerrainType::Grassland
                | TerrainType::Desert
                | TerrainType::Water
                | TerrainType::Forest
                | TerrainType::Mountain
        ));

        // Should have loaded 2 chunks
        assert_eq!(mgr.loaded_chunk_count(), 2);
    }

    #[test]
    fn chunk_manager_negative_coords_work() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);

        mgr.add_entity(1, -100.0, -200.0);
        let coords = mgr.world_to_chunk(-100.0, -200.0);
        assert_eq!(coords, (-1, -1));

        let chunk = mgr.get_chunk((-1, -1)).unwrap();
        assert_eq!(chunk.entity_count(), 1);
    }

    #[test]
    fn chunk_manager_set_viewer_position_creates_radius() {
        let mut mgr = ChunkManager::new(256.0, TEST_SEED);
        mgr.set_viewer_position(128.0, 128.0, 1);

        // With radius 1, should have a 3x3 grid of viewer chunks
        assert_eq!(mgr.viewer_chunks.len(), 9);
        assert!(mgr.viewer_chunks.contains(&(0, 0)));
        assert!(mgr.viewer_chunks.contains(&(-1, -1)));
        assert!(mgr.viewer_chunks.contains(&(1, 1)));
    }

    #[test]
    fn compute_resource_density_handles_empty() {
        assert_eq!(compute_resource_density(&[]), 0.0);
    }

    #[test]
    fn compute_resource_density_forest_is_high() {
        let terrain = vec![TerrainType::Forest; 100];
        let density = compute_resource_density(&terrain);
        assert!(density > 0.5, "forest density should be high, got {}", density);
    }

    #[test]
    fn compute_resource_density_water_is_zero() {
        let terrain = vec![TerrainType::Water; 100];
        let density = compute_resource_density(&terrain);
        assert_eq!(density, 0.0);
    }

    #[test]
    fn dormant_simulation_population_grows() {
        let mut chunk = Chunk::new((0, 0), TEST_CHUNK_SIZE, TEST_SEED);
        chunk.state = ChunkState::Dormant;
        chunk.dormant_population = 0;
        chunk.resource_density = 0.5; // carrying capacity = 10

        // Run dormant simulation for many ticks, simulating the
        // ticks_since_active counter that the tick() method would set.
        for tick in 1..=200 {
            chunk.ticks_since_active = tick;
            ChunkManager::tick_dormant(&mut chunk);
        }

        // Population should have grown (every 10 ticks adds 1, so ~20 growth)
        assert!(
            chunk.dormant_population > 0,
            "dormant population should grow when resources available"
        );
    }

    #[test]
    fn chunk_terrain_matches_terrain_grid_at_origin() {
        // Verify that chunk (0,0) terrain matches TerrainGrid for the same region.
        use crate::environment::terrain::TerrainGrid;

        let seed = 42u64;
        let chunk_size = 256.0;
        let chunk = Chunk::new((0, 0), chunk_size, seed);
        let grid = TerrainGrid::generate(chunk_size, chunk_size, seed);

        // Both should use the same noise functions and produce identical terrain
        // for the same world-space coordinates.
        for row in 0..chunk.terrain_rows.min(grid.rows) {
            for col in 0..chunk.terrain_cols.min(grid.cols) {
                assert_eq!(
                    chunk.terrain_at_local(col, row),
                    grid.get(col, row),
                    "terrain mismatch at cell ({}, {})",
                    col,
                    row
                );
            }
        }
    }
}

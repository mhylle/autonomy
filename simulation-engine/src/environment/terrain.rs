use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

/// The type of terrain in a grid cell.
///
/// Each terrain type affects movement speed and resource density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainType {
    Grassland,
    Desert,
    Water,
    Forest,
    Mountain,
}

impl TerrainType {
    /// Movement speed multiplier for entities on this terrain.
    ///
    /// Water returns 0.0 because it is impassable.
    pub fn movement_speed_multiplier(self) -> f64 {
        match self {
            TerrainType::Grassland => 1.0,
            TerrainType::Desert => 0.8,
            TerrainType::Water => 0.0,
            TerrainType::Forest => 0.7,
            TerrainType::Mountain => 0.4,
        }
    }

    /// Resource density multiplier for this terrain.
    ///
    /// Controls how likely resources are to appear on this terrain.
    /// Water returns 0.0 because no resources spawn there.
    pub fn resource_density_multiplier(self) -> f64 {
        match self {
            TerrainType::Grassland => 1.0,
            TerrainType::Desert => 0.1,
            TerrainType::Water => 0.0,
            TerrainType::Forest => 2.0,
            TerrainType::Mountain => 0.3,
        }
    }

    /// Whether entities can enter this terrain cell.
    pub fn is_passable(self) -> bool {
        self != TerrainType::Water
    }
}

/// Cell size in simulation units. Each cell is a square of this side length.
pub const CELL_SIZE: f64 = 10.0;

/// Maximum elevation value (mountains).
pub const MAX_ELEVATION: f64 = 100.0;

/// Slope penalty factor: each unit of elevation gain reduces movement speed
/// by this fraction (clamped so speed never goes below 10% of base).
const SLOPE_SPEED_PENALTY: f64 = 0.05;

/// Minimum movement speed multiplier from slope (prevents complete stop).
const MIN_SLOPE_MULTIPLIER: f64 = 0.1;

/// A grid of terrain cells covering the simulation world.
///
/// The grid is generated once from Perlin noise using the world seed and
/// remains static throughout the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainGrid {
    cells: Vec<TerrainType>,
    /// Per-cell elevation values (0.0 = sea level, MAX_ELEVATION = peak).
    /// Empty when 3D is disabled.
    #[serde(default)]
    elevation: Vec<f64>,
    /// Per-cell underground/cave flag. True = cave exists beneath this cell.
    /// Empty when 3D is disabled.
    #[serde(default)]
    caves: Vec<bool>,
    pub cols: usize,
    pub rows: usize,
    pub cell_size: f64,
}

impl TerrainGrid {
    /// Generate a terrain grid from Perlin noise.
    ///
    /// The `seed` determines the noise pattern, making generation
    /// deterministic for replay. The grid covers `world_width` x
    /// `world_height` simulation units.
    pub fn generate(world_width: f64, world_height: f64, seed: u64) -> Self {
        Self::generate_with_3d(world_width, world_height, seed, false)
    }

    /// Generate a terrain grid, optionally including 3D elevation and caves.
    pub fn generate_with_3d(
        world_width: f64,
        world_height: f64,
        seed: u64,
        enable_3d: bool,
    ) -> Self {
        let cols = (world_width / CELL_SIZE).ceil() as usize;
        let rows = (world_height / CELL_SIZE).ceil() as usize;
        let total = cols * rows;

        // Perlin::new takes a u32 seed.
        let perlin = Perlin::new(seed as u32);

        // Use a second noise layer for moisture (determines forest vs desert).
        let moisture_perlin = Perlin::new(seed.wrapping_add(1000) as u32);

        let mut cells = Vec::with_capacity(total);

        // Scale factor controls feature size. Larger = bigger biomes.
        let scale = 0.03;

        // Raw elevation values (Perlin output, roughly -1..1) used for both
        // terrain classification and height map.
        let mut raw_elevations = Vec::with_capacity(total);

        for row in 0..rows {
            for col in 0..cols {
                let nx = col as f64 * scale;
                let ny = row as f64 * scale;

                let elevation = perlin.get([nx, ny]);
                let moisture = moisture_perlin.get([nx + 100.0, ny + 100.0]);

                raw_elevations.push(elevation);
                let terrain = classify_terrain(elevation, moisture);
                cells.push(terrain);
            }
        }

        // Build elevation and cave layers when 3D is enabled.
        let (elevation, caves) = if enable_3d {
            let elevation: Vec<f64> = raw_elevations
                .iter()
                .map(|&e| raw_elevation_to_height(e))
                .collect();

            // Cave generation: use a separate 3D-seeded Perlin noise.
            // A cell is a cave when the noise value exceeds a threshold AND
            // the cell is above sea level (not water).
            let cave_perlin = Perlin::new(seed.wrapping_add(5000) as u32);
            let cave_scale = 0.06;
            let cave_threshold = 0.3;

            let caves: Vec<bool> = (0..total)
                .map(|i| {
                    let r = i / cols;
                    let c = i % cols;
                    let nx = c as f64 * cave_scale;
                    let ny = r as f64 * cave_scale;
                    let nz = elevation[i] * 0.01; // tie cave likelihood to depth
                    let cave_noise = cave_perlin.get([nx, ny, nz]);
                    cave_noise > cave_threshold && cells[i] != TerrainType::Water
                })
                .collect();

            (elevation, caves)
        } else {
            (Vec::new(), Vec::new())
        };

        Self {
            cells,
            elevation,
            caves,
            cols,
            rows,
            cell_size: CELL_SIZE,
        }
    }

    /// Look up the terrain type at a world-space position.
    ///
    /// Coordinates are clamped to the grid bounds.
    pub fn terrain_at(&self, x: f64, y: f64) -> TerrainType {
        let (col, row) = self.world_to_cell(x, y);
        self.get(col, row)
    }

    /// Get the terrain type at grid coordinates.
    ///
    /// Coordinates are clamped to valid range.
    pub fn get(&self, col: usize, row: usize) -> TerrainType {
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.rows.saturating_sub(1));
        self.cells[row * self.cols + col]
    }

    /// Convert world-space coordinates to grid cell indices.
    pub fn world_to_cell(&self, x: f64, y: f64) -> (usize, usize) {
        let col = (x / self.cell_size).floor().max(0.0) as usize;
        let row = (y / self.cell_size).floor().max(0.0) as usize;
        (
            col.min(self.cols.saturating_sub(1)),
            row.min(self.rows.saturating_sub(1)),
        )
    }

    /// Check whether a world-space position is on passable terrain.
    pub fn is_passable(&self, x: f64, y: f64) -> bool {
        self.terrain_at(x, y).is_passable()
    }

    /// Get the movement speed multiplier at a world-space position.
    pub fn movement_multiplier_at(&self, x: f64, y: f64) -> f64 {
        self.terrain_at(x, y).movement_speed_multiplier()
    }

    /// Get the resource density multiplier at a world-space position.
    pub fn resource_density_at(&self, x: f64, y: f64) -> f64 {
        self.terrain_at(x, y).resource_density_multiplier()
    }

    /// Total number of cells in the grid.
    pub fn cell_count(&self) -> usize {
        self.cols * self.rows
    }

    /// Count cells of a given terrain type.
    pub fn count_terrain(&self, terrain: TerrainType) -> usize {
        self.cells.iter().filter(|&&t| t == terrain).count()
    }

    // --- 3D elevation API ---

    /// Whether this grid has elevation data.
    pub fn has_elevation(&self) -> bool {
        !self.elevation.is_empty()
    }

    /// Get the elevation at grid coordinates. Returns 0.0 if elevation is
    /// not generated (2D mode).
    pub fn elevation_at_cell(&self, col: usize, row: usize) -> f64 {
        if self.elevation.is_empty() {
            return 0.0;
        }
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.rows.saturating_sub(1));
        self.elevation[row * self.cols + col]
    }

    /// Get the elevation at a world-space position. Returns 0.0 in 2D mode.
    pub fn elevation_at(&self, x: f64, y: f64) -> f64 {
        let (col, row) = self.world_to_cell(x, y);
        self.elevation_at_cell(col, row)
    }

    /// Compute the slope-based movement multiplier between two positions.
    ///
    /// Going uphill reduces speed; downhill is unpenalised. Returns 1.0
    /// when elevation data is absent (2D mode).
    pub fn slope_multiplier(&self, from_x: f64, from_y: f64, to_x: f64, to_y: f64) -> f64 {
        if self.elevation.is_empty() {
            return 1.0;
        }
        let from_elev = self.elevation_at(from_x, from_y);
        let to_elev = self.elevation_at(to_x, to_y);
        let gain = (to_elev - from_elev).max(0.0);
        (1.0 - gain * SLOPE_SPEED_PENALTY).max(MIN_SLOPE_MULTIPLIER)
    }

    // --- Cave API ---

    /// Whether this grid has cave data.
    pub fn has_caves(&self) -> bool {
        !self.caves.is_empty()
    }

    /// Whether the cell at grid coordinates is a cave. Returns false in 2D mode.
    pub fn is_cave_at_cell(&self, col: usize, row: usize) -> bool {
        if self.caves.is_empty() {
            return false;
        }
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.rows.saturating_sub(1));
        self.caves[row * self.cols + col]
    }

    /// Whether the position is above a cave. Returns false in 2D mode.
    pub fn is_cave_at(&self, x: f64, y: f64) -> bool {
        let (col, row) = self.world_to_cell(x, y);
        self.is_cave_at_cell(col, row)
    }

    /// Count cells that are caves.
    pub fn count_caves(&self) -> usize {
        self.caves.iter().filter(|&&c| c).count()
    }
}

/// Map raw Perlin noise elevation (roughly -1..1) to a height value in
/// [0, MAX_ELEVATION]. Water cells will have low elevation; mountains high.
fn raw_elevation_to_height(raw: f64) -> f64 {
    // Remap -1..1 to 0..MAX_ELEVATION
    ((raw + 1.0) / 2.0).clamp(0.0, 1.0) * MAX_ELEVATION
}

/// Classify a cell into a terrain type based on elevation and moisture.
///
/// Elevation roughly in [-1, 1] from Perlin noise:
/// - Very low elevation -> Water
/// - High elevation -> Mountain
/// - Mid elevation + high moisture -> Forest
/// - Mid elevation + low moisture -> Desert
/// - Otherwise -> Grassland
pub fn classify_terrain(elevation: f64, moisture: f64) -> TerrainType {
    if elevation < -0.3 {
        TerrainType::Water
    } else if elevation > 0.5 {
        TerrainType::Mountain
    } else if moisture > 0.2 {
        TerrainType::Forest
    } else if moisture < -0.2 {
        TerrainType::Desert
    } else {
        TerrainType::Grassland
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_type_movement_multipliers() {
        assert_eq!(TerrainType::Grassland.movement_speed_multiplier(), 1.0);
        assert_eq!(TerrainType::Desert.movement_speed_multiplier(), 0.8);
        assert_eq!(TerrainType::Water.movement_speed_multiplier(), 0.0);
        assert_eq!(TerrainType::Forest.movement_speed_multiplier(), 0.7);
        assert_eq!(TerrainType::Mountain.movement_speed_multiplier(), 0.4);
    }

    #[test]
    fn terrain_type_resource_multipliers() {
        assert_eq!(TerrainType::Grassland.resource_density_multiplier(), 1.0);
        assert_eq!(TerrainType::Desert.resource_density_multiplier(), 0.1);
        assert_eq!(TerrainType::Water.resource_density_multiplier(), 0.0);
        assert_eq!(TerrainType::Forest.resource_density_multiplier(), 2.0);
        assert_eq!(TerrainType::Mountain.resource_density_multiplier(), 0.3);
    }

    #[test]
    fn water_is_impassable() {
        assert!(!TerrainType::Water.is_passable());
        assert!(TerrainType::Grassland.is_passable());
        assert!(TerrainType::Desert.is_passable());
        assert!(TerrainType::Forest.is_passable());
        assert!(TerrainType::Mountain.is_passable());
    }

    #[test]
    fn generate_creates_correct_grid_dimensions() {
        let grid = TerrainGrid::generate(500.0, 500.0, 42);
        assert_eq!(grid.cols, 50);
        assert_eq!(grid.rows, 50);
        assert_eq!(grid.cell_count(), 2500);
    }

    #[test]
    fn generate_is_deterministic() {
        let grid1 = TerrainGrid::generate(200.0, 200.0, 42);
        let grid2 = TerrainGrid::generate(200.0, 200.0, 42);

        assert_eq!(grid1.cols, grid2.cols);
        assert_eq!(grid1.rows, grid2.rows);
        for row in 0..grid1.rows {
            for col in 0..grid1.cols {
                assert_eq!(
                    grid1.get(col, row),
                    grid2.get(col, row),
                    "mismatch at ({}, {})",
                    col,
                    row
                );
            }
        }
    }

    #[test]
    fn different_seeds_produce_different_terrain() {
        let grid1 = TerrainGrid::generate(200.0, 200.0, 42);
        let grid2 = TerrainGrid::generate(200.0, 200.0, 99);

        let mut differences = 0;
        for row in 0..grid1.rows {
            for col in 0..grid1.cols {
                if grid1.get(col, row) != grid2.get(col, row) {
                    differences += 1;
                }
            }
        }
        // With different seeds, at least some cells should differ.
        assert!(differences > 0, "different seeds should produce different terrain");
    }

    #[test]
    fn world_to_cell_basic() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        assert_eq!(grid.world_to_cell(0.0, 0.0), (0, 0));
        assert_eq!(grid.world_to_cell(15.0, 25.0), (1, 2));
        assert_eq!(grid.world_to_cell(99.0, 99.0), (9, 9));
    }

    #[test]
    fn world_to_cell_clamps_negative() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        assert_eq!(grid.world_to_cell(-10.0, -10.0), (0, 0));
    }

    #[test]
    fn world_to_cell_clamps_overflow() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        let (col, row) = grid.world_to_cell(999.0, 999.0);
        assert!(col < grid.cols);
        assert!(row < grid.rows);
    }

    #[test]
    fn terrain_at_returns_valid_type() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        // Just verify it doesn't panic for various positions.
        for x in (0..100).step_by(10) {
            for y in (0..100).step_by(10) {
                let _terrain = grid.terrain_at(x as f64, y as f64);
            }
        }
    }

    #[test]
    fn grid_contains_multiple_terrain_types() {
        // With a large enough grid, we expect some diversity.
        let grid = TerrainGrid::generate(500.0, 500.0, 42);
        let mut types_seen = std::collections::HashSet::new();
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                types_seen.insert(grid.get(col, row));
            }
        }
        // We should see at least 3 different terrain types.
        assert!(
            types_seen.len() >= 3,
            "expected at least 3 terrain types, got {}",
            types_seen.len()
        );
    }

    #[test]
    fn classify_terrain_water_at_low_elevation() {
        assert_eq!(classify_terrain(-0.5, 0.0), TerrainType::Water);
    }

    #[test]
    fn classify_terrain_mountain_at_high_elevation() {
        assert_eq!(classify_terrain(0.7, 0.0), TerrainType::Mountain);
    }

    #[test]
    fn classify_terrain_forest_at_high_moisture() {
        assert_eq!(classify_terrain(0.0, 0.5), TerrainType::Forest);
    }

    #[test]
    fn classify_terrain_desert_at_low_moisture() {
        assert_eq!(classify_terrain(0.0, -0.5), TerrainType::Desert);
    }

    #[test]
    fn classify_terrain_grassland_at_mid_values() {
        assert_eq!(classify_terrain(0.0, 0.0), TerrainType::Grassland);
    }

    #[test]
    fn passability_check_consistent_with_terrain_type() {
        let grid = TerrainGrid::generate(200.0, 200.0, 42);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let terrain = grid.get(col, row);
                let x = col as f64 * grid.cell_size + 1.0;
                let y = row as f64 * grid.cell_size + 1.0;
                assert_eq!(
                    grid.is_passable(x, y),
                    terrain.is_passable(),
                    "passability mismatch at ({}, {})",
                    col,
                    row
                );
            }
        }
    }

    #[test]
    fn movement_multiplier_at_consistent() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        let terrain = grid.terrain_at(50.0, 50.0);
        assert_eq!(
            grid.movement_multiplier_at(50.0, 50.0),
            terrain.movement_speed_multiplier()
        );
    }

    #[test]
    fn resource_density_at_consistent() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        let terrain = grid.terrain_at(50.0, 50.0);
        assert_eq!(
            grid.resource_density_at(50.0, 50.0),
            terrain.resource_density_multiplier()
        );
    }

    // --- 3D elevation tests ---

    #[test]
    fn generate_2d_has_no_elevation() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        assert!(!grid.has_elevation());
        assert_eq!(grid.elevation_at(50.0, 50.0), 0.0);
    }

    #[test]
    fn generate_3d_has_elevation() {
        let grid = TerrainGrid::generate_with_3d(100.0, 100.0, 42, true);
        assert!(grid.has_elevation());
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let elev = grid.elevation_at_cell(col, row);
                assert!(
                    elev >= 0.0 && elev <= MAX_ELEVATION,
                    "elevation {} out of bounds at ({}, {})",
                    elev,
                    col,
                    row
                );
            }
        }
    }

    #[test]
    fn elevation_is_deterministic() {
        let grid1 = TerrainGrid::generate_with_3d(200.0, 200.0, 42, true);
        let grid2 = TerrainGrid::generate_with_3d(200.0, 200.0, 42, true);
        for row in 0..grid1.rows {
            for col in 0..grid1.cols {
                assert_eq!(
                    grid1.elevation_at_cell(col, row),
                    grid2.elevation_at_cell(col, row),
                    "elevation mismatch at ({}, {})",
                    col,
                    row
                );
            }
        }
    }

    #[test]
    fn slope_multiplier_flat_is_one() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        assert_eq!(grid.slope_multiplier(10.0, 10.0, 20.0, 20.0), 1.0);
    }

    #[test]
    fn slope_multiplier_uphill_reduces_speed() {
        let grid = TerrainGrid::generate_with_3d(500.0, 500.0, 42, true);
        for row in 0..grid.rows.saturating_sub(1) {
            let elev_here = grid.elevation_at_cell(0, row);
            let elev_next = grid.elevation_at_cell(0, row + 1);
            if elev_next > elev_here + 1.0 {
                let from_x = 1.0;
                let from_y = row as f64 * grid.cell_size + 1.0;
                let to_x = 1.0;
                let to_y = (row + 1) as f64 * grid.cell_size + 1.0;
                let mult = grid.slope_multiplier(from_x, from_y, to_x, to_y);
                assert!(
                    mult < 1.0,
                    "uphill movement should reduce speed, got multiplier {}",
                    mult
                );
                return;
            }
        }
    }

    #[test]
    fn slope_multiplier_downhill_is_one() {
        let grid = TerrainGrid::generate_with_3d(500.0, 500.0, 42, true);
        for row in 0..grid.rows.saturating_sub(1) {
            let elev_here = grid.elevation_at_cell(0, row);
            let elev_next = grid.elevation_at_cell(0, row + 1);
            if elev_next < elev_here - 1.0 {
                let from_x = 1.0;
                let from_y = row as f64 * grid.cell_size + 1.0;
                let to_x = 1.0;
                let to_y = (row + 1) as f64 * grid.cell_size + 1.0;
                let mult = grid.slope_multiplier(from_x, from_y, to_x, to_y);
                assert_eq!(mult, 1.0, "downhill movement should not be penalised");
                return;
            }
        }
    }

    // --- Cave tests ---

    #[test]
    fn generate_2d_has_no_caves() {
        let grid = TerrainGrid::generate(100.0, 100.0, 42);
        assert!(!grid.has_caves());
        assert!(!grid.is_cave_at(50.0, 50.0));
    }

    #[test]
    fn generate_3d_has_caves() {
        let grid = TerrainGrid::generate_with_3d(500.0, 500.0, 42, true);
        assert!(grid.has_caves());
        let cave_count = grid.count_caves();
        assert!(
            cave_count > 0,
            "expected at least some caves in 500x500 world"
        );
    }

    #[test]
    fn caves_not_in_water() {
        let grid = TerrainGrid::generate_with_3d(500.0, 500.0, 42, true);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                if grid.get(col, row) == TerrainType::Water {
                    assert!(
                        !grid.is_cave_at_cell(col, row),
                        "cave should not exist in water at ({}, {})",
                        col,
                        row
                    );
                }
            }
        }
    }

    #[test]
    fn terrain_grid_deserialize_without_3d_fields() {
        let grid_2d = TerrainGrid::generate(100.0, 100.0, 42);
        let json = serde_json::to_string(&grid_2d).unwrap();
        let restored: TerrainGrid = serde_json::from_str(&json).unwrap();
        assert!(!restored.has_elevation());
        assert!(!restored.has_caves());
        assert_eq!(restored.cols, grid_2d.cols);
        assert_eq!(restored.rows, grid_2d.rows);
    }

    #[test]
    fn generate_with_3d_false_same_as_generate() {
        let grid_a = TerrainGrid::generate(200.0, 200.0, 42);
        let grid_b = TerrainGrid::generate_with_3d(200.0, 200.0, 42, false);
        assert_eq!(grid_a.cols, grid_b.cols);
        assert_eq!(grid_a.rows, grid_b.rows);
        assert!(!grid_a.has_elevation());
        assert!(!grid_b.has_elevation());
        for row in 0..grid_a.rows {
            for col in 0..grid_a.cols {
                assert_eq!(grid_a.get(col, row), grid_b.get(col, row));
            }
        }
    }
}

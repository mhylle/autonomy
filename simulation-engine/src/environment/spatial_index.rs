/// Grid-based spatial hash for fast proximity queries.
///
/// Divides the world into cells of `cell_size` width/height. Each cell
/// tracks which entity IDs and resource indices are present, enabling
/// O(n*k) "find neighbors within radius" queries instead of O(n^2).
///
/// Entities store an optional z coordinate for 3D queries. The 2D grid
/// partitions on (x, y) only; z is checked during the distance filter.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    cell_size: f64,
    cols: usize,
    rows: usize,
    width: f64,
    height: f64,
    /// cell -> [(entity_id_bits, x, y, z)]
    entity_cells: Vec<Vec<(u64, f64, f64, f64)>>,
    /// cell -> [(resource_index, x, y, z)]
    resource_cells: Vec<Vec<(usize, f64, f64, f64)>>,
}

impl SpatialIndex {
    /// Create a new spatial index covering a world of the given dimensions.
    ///
    /// `cell_size` controls the granularity of the grid. A good default is 50.0.
    pub fn new(width: f64, height: f64, cell_size: f64) -> Self {
        let cols = (width / cell_size).ceil() as usize;
        let rows = (height / cell_size).ceil() as usize;
        let total = cols * rows;

        Self {
            cell_size,
            cols,
            rows,
            width,
            height,
            entity_cells: vec![Vec::new(); total],
            resource_cells: vec![Vec::new(); total],
        }
    }

    /// The world width this index covers.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// The world height this index covers.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Remove all entities and resources from the index.
    pub fn clear(&mut self) {
        for cell in &mut self.entity_cells {
            cell.clear();
        }
        for cell in &mut self.resource_cells {
            cell.clear();
        }
    }

    /// Insert an entity into the spatial index (2D, z=0).
    ///
    /// We store `entity_id_bits` (from `hecs::Entity::to_bits()`) rather
    /// than `hecs::Entity` directly so the spatial index stays decoupled
    /// from the ECS crate.
    pub fn insert_entity(&mut self, entity_id_bits: u64, x: f64, y: f64) {
        self.insert_entity_3d(entity_id_bits, x, y, 0.0);
    }

    /// Insert an entity with a z coordinate.
    pub fn insert_entity_3d(&mut self, entity_id_bits: u64, x: f64, y: f64, z: f64) {
        let idx = self.cell_index(x, y);
        self.entity_cells[idx].push((entity_id_bits, x, y, z));
    }

    /// Insert a resource into the spatial index by its index in the
    /// resource list (2D, z=0).
    pub fn insert_resource(&mut self, resource_index: usize, x: f64, y: f64) {
        self.insert_resource_3d(resource_index, x, y, 0.0);
    }

    /// Insert a resource with a z coordinate.
    pub fn insert_resource_3d(&mut self, resource_index: usize, x: f64, y: f64, z: f64) {
        let idx = self.cell_index(x, y);
        self.resource_cells[idx].push((resource_index, x, y, z));
    }

    /// Find all entities within `radius` of the point `(x, y)`.
    /// Uses 2D distance (ignores z).
    ///
    /// Returns a vec of `(entity_id_bits, entity_x, entity_y)`.
    pub fn query_entities_in_radius(&self, x: f64, y: f64, radius: f64) -> Vec<(u64, f64, f64)> {
        let r_sq = radius * radius;
        let mut results = Vec::new();

        for cell_idx in self.cells_in_radius(x, y, radius) {
            for &(id, ex, ey, _ez) in &self.entity_cells[cell_idx] {
                let dx = ex - x;
                let dy = ey - y;
                if dx * dx + dy * dy <= r_sq {
                    results.push((id, ex, ey));
                }
            }
        }

        results
    }

    /// Find all entities within a 3D sphere of `radius` around `(x, y, z)`.
    ///
    /// Returns a vec of `(entity_id_bits, entity_x, entity_y, entity_z)`.
    pub fn query_entities_in_radius_3d(
        &self,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<(u64, f64, f64, f64)> {
        let r_sq = radius * radius;
        let mut results = Vec::new();

        // The 2D grid cells still partition by (x, y); we check z in the
        // inner loop.
        for cell_idx in self.cells_in_radius(x, y, radius) {
            for &(id, ex, ey, ez) in &self.entity_cells[cell_idx] {
                let dx = ex - x;
                let dy = ey - y;
                let dz = ez - z;
                if dx * dx + dy * dy + dz * dz <= r_sq {
                    results.push((id, ex, ey, ez));
                }
            }
        }

        results
    }

    /// Find all resources within `radius` of the point `(x, y)`.
    /// Uses 2D distance (ignores z).
    ///
    /// Returns a vec of `(resource_index, resource_x, resource_y)`.
    pub fn query_resources_in_radius(
        &self,
        x: f64,
        y: f64,
        radius: f64,
    ) -> Vec<(usize, f64, f64)> {
        let r_sq = radius * radius;
        let mut results = Vec::new();

        for cell_idx in self.cells_in_radius(x, y, radius) {
            for &(idx, rx, ry, _rz) in &self.resource_cells[cell_idx] {
                let dx = rx - x;
                let dy = ry - y;
                if dx * dx + dy * dy <= r_sq {
                    results.push((idx, rx, ry));
                }
            }
        }

        results
    }

    /// Find all resources within a 3D sphere of `radius` around `(x, y, z)`.
    ///
    /// Returns a vec of `(resource_index, resource_x, resource_y, resource_z)`.
    pub fn query_resources_in_radius_3d(
        &self,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<(usize, f64, f64, f64)> {
        let r_sq = radius * radius;
        let mut results = Vec::new();

        for cell_idx in self.cells_in_radius(x, y, radius) {
            for &(idx, rx, ry, rz) in &self.resource_cells[cell_idx] {
                let dx = rx - x;
                let dy = ry - y;
                let dz = rz - z;
                if dx * dx + dy * dy + dz * dz <= r_sq {
                    results.push((idx, rx, ry, rz));
                }
            }
        }

        results
    }

    /// Map a world-space position to a cell index, clamping to valid range.
    fn cell_index(&self, x: f64, y: f64) -> usize {
        let col = (x / self.cell_size).floor() as isize;
        let row = (y / self.cell_size).floor() as isize;

        let col = col.clamp(0, (self.cols as isize) - 1) as usize;
        let row = row.clamp(0, (self.rows as isize) - 1) as usize;

        row * self.cols + col
    }

    /// Return the indices of all cells that could overlap with a circle
    /// centered at `(x, y)` with the given `radius`.
    fn cells_in_radius(&self, x: f64, y: f64, radius: f64) -> Vec<usize> {
        let min_col = ((x - radius) / self.cell_size).floor() as isize;
        let max_col = ((x + radius) / self.cell_size).floor() as isize;
        let min_row = ((y - radius) / self.cell_size).floor() as isize;
        let max_row = ((y + radius) / self.cell_size).floor() as isize;

        let min_col = min_col.clamp(0, (self.cols as isize) - 1) as usize;
        let max_col = max_col.clamp(0, (self.cols as isize) - 1) as usize;
        let min_row = min_row.clamp(0, (self.rows as isize) - 1) as usize;
        let max_row = max_row.clamp(0, (self.rows as isize) - 1) as usize;

        let mut indices = Vec::new();
        for row in min_row..=max_row {
            for col in min_col..=max_col {
                indices.push(row * self.cols + col);
            }
        }
        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 200.0;
    const HEIGHT: f64 = 200.0;
    const CELL_SIZE: f64 = 50.0;

    fn make_index() -> SpatialIndex {
        SpatialIndex::new(WIDTH, HEIGHT, CELL_SIZE)
    }

    #[test]
    fn empty_index_returns_no_entities() {
        let index = make_index();
        let results = index.query_entities_in_radius(100.0, 100.0, 50.0);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_index_returns_no_resources() {
        let index = make_index();
        let results = index.query_resources_in_radius(100.0, 100.0, 50.0);
        assert!(results.is_empty());
    }

    #[test]
    fn insert_and_query_single_entity() {
        let mut index = make_index();
        index.insert_entity(42, 100.0, 100.0);

        let results = index.query_entities_in_radius(100.0, 100.0, 10.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 42);
        assert_eq!(results[0].1, 100.0);
        assert_eq!(results[0].2, 100.0);
    }

    #[test]
    fn query_only_returns_entities_within_radius() {
        let mut index = make_index();
        // Entity at (10, 10) - should be found
        index.insert_entity(1, 10.0, 10.0);
        // Entity at (15, 10) - should be found (distance = 5)
        index.insert_entity(2, 15.0, 10.0);
        // Entity at (100, 100) - too far away
        index.insert_entity(3, 100.0, 100.0);

        let results = index.query_entities_in_radius(10.0, 10.0, 20.0);
        assert_eq!(results.len(), 2);

        let ids: Vec<u64> = results.iter().map(|r| r.0).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(!ids.contains(&3));
    }

    #[test]
    fn large_radius_queries_multiple_cells() {
        let mut index = make_index();
        // Place entities in different cells (cell_size = 50)
        index.insert_entity(1, 10.0, 10.0); // cell (0,0)
        index.insert_entity(2, 60.0, 10.0); // cell (1,0)
        index.insert_entity(3, 10.0, 60.0); // cell (0,1)
        index.insert_entity(4, 190.0, 190.0); // cell (3,3) - far away

        // Radius large enough to reach cells (0,0), (1,0), (0,1)
        let results = index.query_entities_in_radius(30.0, 30.0, 60.0);
        let ids: Vec<u64> = results.iter().map(|r| r.0).collect();

        assert!(ids.contains(&1)); // distance ~28.3
        assert!(ids.contains(&2)); // distance ~36.1
        assert!(ids.contains(&3)); // distance ~36.1
        assert!(!ids.contains(&4)); // distance ~226
    }

    #[test]
    fn entity_at_origin() {
        let mut index = make_index();
        index.insert_entity(1, 0.0, 0.0);

        let results = index.query_entities_in_radius(0.0, 0.0, 5.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn entity_at_boundary() {
        let mut index = make_index();
        // Place entity near the right/bottom edge
        index.insert_entity(1, 199.0, 199.0);

        let results = index.query_entities_in_radius(199.0, 199.0, 5.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn negative_coordinates_clamped() {
        let mut index = make_index();
        // Insert at negative coords - should be clamped to cell (0,0)
        index.insert_entity(1, -5.0, -5.0);

        let results = index.query_entities_in_radius(0.0, 0.0, 20.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn resource_insert_and_query() {
        let mut index = make_index();
        index.insert_resource(0, 50.0, 50.0);
        index.insert_resource(1, 55.0, 50.0);
        index.insert_resource(2, 150.0, 150.0);

        let results = index.query_resources_in_radius(50.0, 50.0, 10.0);
        assert_eq!(results.len(), 2);

        let indices: Vec<usize> = results.iter().map(|r| r.0).collect();
        assert!(indices.contains(&0));
        assert!(indices.contains(&1));
        assert!(!indices.contains(&2));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut index = make_index();
        index.insert_entity(1, 50.0, 50.0);
        index.insert_resource(0, 50.0, 50.0);

        index.clear();

        let entities = index.query_entities_in_radius(50.0, 50.0, 100.0);
        let resources = index.query_resources_in_radius(50.0, 50.0, 100.0);
        assert!(entities.is_empty());
        assert!(resources.is_empty());
    }

    #[test]
    fn distance_check_uses_euclidean() {
        let mut index = make_index();
        // Entity at distance exactly 10.0 from (0,0)
        index.insert_entity(1, 6.0, 8.0); // sqrt(36 + 64) = 10.0

        // Radius of exactly 10.0 should include it (<= check)
        let results = index.query_entities_in_radius(0.0, 0.0, 10.0);
        assert_eq!(results.len(), 1);

        // Radius of 9.99 should exclude it
        let results = index.query_entities_in_radius(0.0, 0.0, 9.99);
        assert!(results.is_empty());
    }

    #[test]
    fn many_entities_in_same_cell() {
        let mut index = make_index();
        for i in 0..100 {
            index.insert_entity(i, 25.0, 25.0); // all in cell (0,0)
        }

        let results = index.query_entities_in_radius(25.0, 25.0, 1.0);
        assert_eq!(results.len(), 100);
    }

    #[test]
    fn query_at_cell_boundary_finds_neighbors() {
        let mut index = make_index();
        // Entity just inside cell (0,0) at x=49
        index.insert_entity(1, 49.0, 25.0);
        // Entity just inside cell (1,0) at x=51
        index.insert_entity(2, 51.0, 25.0);

        // Query from the boundary - radius should span both cells
        let results = index.query_entities_in_radius(50.0, 25.0, 5.0);
        assert_eq!(results.len(), 2);
    }

    // --- 3D spatial index tests ---

    #[test]
    fn insert_entity_3d_and_query_2d_ignores_z() {
        let mut index = make_index();
        // Two entities at same (x,y) but different z
        index.insert_entity_3d(1, 50.0, 50.0, 0.0);
        index.insert_entity_3d(2, 50.0, 50.0, 100.0);

        // 2D query should find both (z is ignored)
        let results = index.query_entities_in_radius(50.0, 50.0, 5.0);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_entities_3d_filters_by_z() {
        let mut index = make_index();
        // Entity at ground level
        index.insert_entity_3d(1, 50.0, 50.0, 0.0);
        // Entity high in the air
        index.insert_entity_3d(2, 50.0, 50.0, 100.0);

        // 3D query with small radius from ground - should only find entity 1
        let results = index.query_entities_in_radius_3d(50.0, 50.0, 0.0, 10.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[0].3, 0.0); // z coordinate returned

        // 3D query with large radius should find both
        let results = index.query_entities_in_radius_3d(50.0, 50.0, 50.0, 60.0);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_resources_3d_filters_by_z() {
        let mut index = make_index();
        index.insert_resource_3d(0, 50.0, 50.0, 0.0);
        index.insert_resource_3d(1, 50.0, 50.0, 80.0);

        // 3D query from ground
        let results = index.query_resources_in_radius_3d(50.0, 50.0, 0.0, 10.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn query_3d_sphere_distance() {
        let mut index = make_index();
        // Entity at (50, 50, 30) -- 3D distance from (50, 50, 0) is 30
        index.insert_entity_3d(1, 50.0, 50.0, 30.0);

        // Radius 29 should miss
        let results = index.query_entities_in_radius_3d(50.0, 50.0, 0.0, 29.0);
        assert!(results.is_empty());

        // Radius 30 should hit (distance exactly 30, uses <=)
        let results = index.query_entities_in_radius_3d(50.0, 50.0, 0.0, 30.0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn backward_compat_insert_entity_has_z_zero() {
        let mut index = make_index();
        // Old-style 2D insert
        index.insert_entity(1, 50.0, 50.0);

        // 3D query from z=0 with small radius should find it
        let results = index.query_entities_in_radius_3d(50.0, 50.0, 0.0, 5.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].3, 0.0);
    }
}

use crate::net::server::ViewportBounds;

/// Level-of-detail tiers for entity simulation fidelity.
///
/// Entities far from any viewer's viewport can be simulated with
/// reduced fidelity to save CPU time. The three levels are:
///
/// - `Full`: All systems run (perception, drives, decision, etc.)
/// - `Reduced`: Skip perception (most expensive read-only system)
/// - `Minimal`: Only aging/metabolism; no AI at all
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodLevel {
    /// All systems execute for this entity.
    Full,
    /// Skip perception (the most expensive read-only system).
    Reduced,
    /// Only aging and metabolism. No perception, drives, or decisions.
    Minimal,
}

/// Distance thresholds for LOD assignment, expressed as multiples of
/// the viewport's diagonal size.
///
/// - Within 1.0x viewport diagonal: Full
/// - Within 2.0x viewport diagonal: Reduced
/// - Beyond 2.0x: Minimal
const LOD_FULL_FACTOR: f64 = 1.0;
const LOD_REDUCED_FACTOR: f64 = 2.0;

/// Compute the LOD level for an entity at (ex, ey) given the current viewport.
///
/// If the viewport is very large (wider than the world), everything gets Full.
pub fn compute_lod(ex: f64, ey: f64, viewport: &ViewportBounds) -> LodLevel {
    let vp_cx = viewport.x + viewport.width / 2.0;
    let vp_cy = viewport.y + viewport.height / 2.0;

    let vp_diag = (viewport.width * viewport.width + viewport.height * viewport.height).sqrt();

    let dx = ex - vp_cx;
    let dy = ey - vp_cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist <= vp_diag * LOD_FULL_FACTOR {
        LodLevel::Full
    } else if dist <= vp_diag * LOD_REDUCED_FACTOR {
        LodLevel::Reduced
    } else {
        LodLevel::Minimal
    }
}

/// Compute LOD assignments for a batch of entity positions.
///
/// Returns a Vec of (entity_index, LodLevel) in the same order as input.
pub fn compute_lod_batch(
    positions: &[(f64, f64)],
    viewport: &ViewportBounds,
) -> Vec<LodLevel> {
    positions
        .iter()
        .map(|&(x, y)| compute_lod(x, y, viewport))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_viewport() -> ViewportBounds {
        ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
            zoom: 1.0,
        }
    }

    #[test]
    fn entity_at_viewport_center_is_full() {
        let vp = default_viewport();
        assert_eq!(compute_lod(100.0, 100.0, &vp), LodLevel::Full);
    }

    #[test]
    fn entity_near_viewport_edge_is_full() {
        let vp = default_viewport();
        // Viewport center is (100, 100), diagonal ~ 282.8
        // Entity at (200, 100) is ~100 away, well within 1x diagonal
        assert_eq!(compute_lod(200.0, 100.0, &vp), LodLevel::Full);
    }

    #[test]
    fn entity_moderately_far_is_reduced() {
        let vp = default_viewport();
        // Viewport center is (100, 100), diagonal ~ 282.8
        // Entity at (450, 100) is 350 away: > 282.8 (1x) but < 565.6 (2x) -> Reduced
        assert_eq!(compute_lod(450.0, 100.0, &vp), LodLevel::Reduced);
    }

    #[test]
    fn entity_very_far_is_minimal() {
        let vp = default_viewport();
        // Viewport center is (100, 100), diagonal ~ 282.8
        // Entity at (900, 100) is 800 away: > 565.6 (2x diagonal) -> Minimal
        assert_eq!(compute_lod(900.0, 100.0, &vp), LodLevel::Minimal);
    }

    #[test]
    fn large_viewport_everything_full() {
        let vp = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 10_000.0,
            height: 10_000.0,
            zoom: 1.0,
        };
        // Even distant entities are Full because viewport diagonal is huge.
        assert_eq!(compute_lod(400.0, 400.0, &vp), LodLevel::Full);
        assert_eq!(compute_lod(0.0, 0.0, &vp), LodLevel::Full);
    }

    #[test]
    fn batch_computation_matches_individual() {
        let vp = default_viewport();
        let positions = vec![
            (100.0, 100.0), // Full
            (450.0, 100.0), // Reduced
            (900.0, 100.0), // Minimal
        ];
        let results = compute_lod_batch(&positions, &vp);
        assert_eq!(results[0], LodLevel::Full);
        assert_eq!(results[1], LodLevel::Reduced);
        assert_eq!(results[2], LodLevel::Minimal);
    }

    #[test]
    fn small_viewport_more_entities_reduced() {
        let vp = ViewportBounds {
            x: 100.0,
            y: 100.0,
            width: 50.0,
            height: 50.0,
            zoom: 4.0,
        };
        // Center is (125, 125), diagonal ~ 70.7
        // Entity at (125, 125) -> Full
        assert_eq!(compute_lod(125.0, 125.0, &vp), LodLevel::Full);
        // Entity at (250, 125) is 125 away: > 70.7 (1x) but < 141.4 (2x) -> Reduced
        assert_eq!(compute_lod(250.0, 125.0, &vp), LodLevel::Reduced);
        // Entity at (400, 125) is 275 away: > 141.4 (2x) -> Minimal
        assert_eq!(compute_lod(400.0, 125.0, &vp), LodLevel::Minimal);
    }
}

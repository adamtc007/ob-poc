//! Signed Distance Field library for viewport rendering
//!
//! Provides distance field primitives and operations for:
//! - Confidence zone halos (fuzzy membership visualization)
//! - Cluster blob shapes (organic grouping)
//! - Hit testing with smooth boundaries
//! - Edge proximity detection

/// 2D point type
pub type Point = [f32; 2];

// =============================================================================
// PRIMITIVES
// =============================================================================

/// Distance to circle edge (negative = inside)
#[inline]
pub fn circle(point: Point, center: Point, radius: f32) -> f32 {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    (dx * dx + dy * dy).sqrt() - radius
}

/// Distance to axis-aligned rectangle edge
#[inline]
pub fn rect(point: Point, center: Point, half_size: Point) -> f32 {
    let dx = (point[0] - center[0]).abs() - half_size[0];
    let dy = (point[1] - center[1]).abs() - half_size[1];

    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    let inside = dx.max(dy).min(0.0);

    outside + inside
}

/// Distance to rounded rectangle
#[inline]
pub fn rounded_rect(point: Point, center: Point, half_size: Point, radius: f32) -> f32 {
    let inner_half = [
        (half_size[0] - radius).max(0.0),
        (half_size[1] - radius).max(0.0),
    ];
    rect(point, center, inner_half) - radius
}

/// Distance to line segment with thickness
pub fn line_segment(point: Point, a: Point, b: Point, thickness: f32) -> f32 {
    let pa = [point[0] - a[0], point[1] - a[1]];
    let ba = [b[0] - a[0], b[1] - a[1]];

    let ba_len_sq = ba[0] * ba[0] + ba[1] * ba[1];
    if ba_len_sq < 0.0001 {
        // Degenerate line (a == b), treat as circle
        return circle(point, a, thickness);
    }

    let h = ((pa[0] * ba[0] + pa[1] * ba[1]) / ba_len_sq).clamp(0.0, 1.0);

    let dx = pa[0] - ba[0] * h;
    let dy = pa[1] - ba[1] * h;

    (dx * dx + dy * dy).sqrt() - thickness
}

// =============================================================================
// OPERATIONS
// =============================================================================

/// Union of two shapes (minimum distance)
#[inline]
pub fn union(d1: f32, d2: f32) -> f32 {
    d1.min(d2)
}

/// Intersection of two shapes (maximum distance)
#[inline]
pub fn intersection(d1: f32, d2: f32) -> f32 {
    d1.max(d2)
}

/// Subtraction (d1 minus d2)
#[inline]
pub fn subtraction(d1: f32, d2: f32) -> f32 {
    d1.max(-d2)
}

/// Smooth union - blends shapes together organically
/// k controls smoothness (0.1 = very smooth, 1.0 = sharp)
#[inline]
pub fn smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    if k < 0.0001 {
        return union(d1, d2);
    }
    let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    lerp(d2, d1, h) - k * h * (1.0 - h)
}

/// Smooth intersection
#[inline]
pub fn smooth_intersection(d1: f32, d2: f32, k: f32) -> f32 {
    if k < 0.0001 {
        return intersection(d1, d2);
    }
    let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    lerp(d2, d1, h) + k * h * (1.0 - h)
}

/// Offset/dilate a shape (positive = grow, negative = shrink)
#[inline]
pub fn offset(d: f32, amount: f32) -> f32 {
    d - amount
}

/// Round the edges of a shape
#[inline]
pub fn round(d: f32, radius: f32) -> f32 {
    d - radius
}

// =============================================================================
// CLUSTER BLOB
// =============================================================================

/// Generate a blob shape that encompasses multiple circles
/// Uses smooth union to create organic, cloud-like boundary
pub fn cluster_blob(point: Point, circles: &[(Point, f32)], smoothness: f32) -> f32 {
    if circles.is_empty() {
        return f32::MAX;
    }

    let mut d = circle(point, circles[0].0, circles[0].1);

    for &(center, radius) in &circles[1..] {
        let d2 = circle(point, center, radius);
        d = smooth_union(d, d2, smoothness);
    }

    d
}

// =============================================================================
// CONFIDENCE HALO
// =============================================================================

/// Confidence zone rendering data for uncertainty visualization
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceHalo {
    pub center: Point,
    pub core_radius: f32,
    /// Confidence level 0.0 - 1.0 (1.0 = certain, 0.0 = uncertain)
    pub confidence: f32,
}

impl ConfidenceHalo {
    /// Create a new confidence halo
    pub fn new(center: Point, core_radius: f32, confidence: f32) -> Self {
        Self {
            center,
            core_radius,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Get the alpha/opacity at a given point based on distance
    /// Core (high confidence) = solid
    /// Shell (medium) = semi-transparent
    /// Penumbra (low) = fading edge
    pub fn alpha_at(&self, point: Point) -> f32 {
        let d = circle(point, self.center, self.core_radius);

        if d < 0.0 {
            // Inside core
            self.confidence
        } else {
            // Outside - fade based on confidence
            let falloff = 1.0 - self.confidence; // Lower confidence = wider halo
            let halo_width = self.core_radius * falloff * 0.5;

            if halo_width < 0.001 {
                return 0.0;
            }

            if d < halo_width {
                // In the halo
                let t = d / halo_width;
                self.confidence * (1.0 - t * t) // Quadratic falloff
            } else {
                0.0
            }
        }
    }

    /// Get the outer radius including halo
    pub fn outer_radius(&self) -> f32 {
        let falloff = 1.0 - self.confidence;
        self.core_radius * (1.0 + falloff * 0.5)
    }

    /// Check if a point is within the halo (including penumbra)
    pub fn contains(&self, point: Point) -> bool {
        let d = circle(point, self.center, self.outer_radius());
        d < 0.0
    }
}

// =============================================================================
// HIT TESTING
// =============================================================================

/// SDF-based hit testing result with gradient (tells you direction to nearest edge)
#[derive(Debug, Clone, Copy)]
pub struct HitResult {
    /// Signed distance to edge (negative = inside)
    pub distance: f32,
    /// Whether point is inside the shape
    pub inside: bool,
    /// Normalized direction to nearest edge (for hover effects)
    pub gradient: Point,
}

/// Compute hit result with gradient for a circle
pub fn hit_test_circle(point: Point, center: Point, radius: f32) -> HitResult {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let dist_to_center = (dx * dx + dy * dy).sqrt();

    let distance = dist_to_center - radius;
    let inside = distance < 0.0;

    // Gradient points away from center (toward edge if inside, away if outside)
    let gradient = if dist_to_center > 0.001 {
        [dx / dist_to_center, dy / dist_to_center]
    } else {
        [1.0, 0.0] // Arbitrary direction at center
    };

    HitResult {
        distance,
        inside,
        gradient,
    }
}

/// Compute hit result for a rectangle
pub fn hit_test_rect(point: Point, center: Point, half_size: Point) -> HitResult {
    let distance = rect(point, center, half_size);
    let inside = distance < 0.0;

    // Approximate gradient by checking which edge is closest
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];

    let edge_x = half_size[0] - dx.abs();
    let edge_y = half_size[1] - dy.abs();

    let gradient = if edge_x < edge_y {
        [dx.signum(), 0.0]
    } else {
        [0.0, dy.signum()]
    };

    HitResult {
        distance,
        inside,
        gradient,
    }
}

// =============================================================================
// HELPERS
// =============================================================================

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_circle_distance() {
        // Center of circle - distance is -radius
        assert!(approx_eq(circle([0.0, 0.0], [0.0, 0.0], 10.0), -10.0));
        // On edge - distance is 0
        assert!(approx_eq(circle([10.0, 0.0], [0.0, 0.0], 10.0), 0.0));
        // Outside - distance is positive
        assert!(approx_eq(circle([15.0, 0.0], [0.0, 0.0], 10.0), 5.0));
        // Inside - distance is negative
        assert!(approx_eq(circle([5.0, 0.0], [0.0, 0.0], 10.0), -5.0));
    }

    #[test]
    fn test_rect_distance() {
        let center = [0.0, 0.0];
        let half = [10.0, 5.0];

        // Center
        assert!(rect([0.0, 0.0], center, half) < 0.0);
        // Inside
        assert!(rect([5.0, 2.0], center, half) < 0.0);
        // On edge (x)
        assert!(approx_eq(rect([10.0, 0.0], center, half), 0.0));
        // Outside
        assert!(rect([15.0, 0.0], center, half) > 0.0);
    }

    #[test]
    fn test_rounded_rect() {
        let center = [0.0, 0.0];
        let half = [10.0, 5.0];
        let radius = 2.0;

        // Should be smaller than regular rect
        let d_regular = rect([9.0, 4.0], center, half);
        let d_rounded = rounded_rect([9.0, 4.0], center, half, radius);
        assert!(d_rounded > d_regular);
    }

    #[test]
    fn test_line_segment() {
        let a = [0.0, 0.0];
        let b = [10.0, 0.0];
        let thickness = 2.0;

        // On the line
        assert!(approx_eq(line_segment([5.0, 0.0], a, b, thickness), -2.0));
        // At endpoint
        assert!(approx_eq(line_segment([0.0, 0.0], a, b, thickness), -2.0));
        // Above the line
        assert!(approx_eq(line_segment([5.0, 4.0], a, b, thickness), 2.0));
    }

    #[test]
    fn test_union() {
        assert!(approx_eq(union(5.0, 3.0), 3.0));
        assert!(approx_eq(union(-2.0, 3.0), -2.0));
    }

    #[test]
    fn test_intersection() {
        assert!(approx_eq(intersection(5.0, 3.0), 5.0));
        assert!(approx_eq(intersection(-2.0, 3.0), 3.0));
    }

    #[test]
    fn test_smooth_union() {
        let c1 = circle([0.0, 0.0], [-5.0, 0.0], 8.0);
        let c2 = circle([0.0, 0.0], [5.0, 0.0], 8.0);

        // Smooth union should be less than or equal to regular union at the junction
        let regular = union(c1, c2);
        let smooth = smooth_union(c1, c2, 4.0);

        assert!(smooth <= regular + EPSILON);
    }

    #[test]
    fn test_cluster_blob() {
        let circles = vec![([-10.0, 0.0], 8.0), ([10.0, 0.0], 8.0), ([0.0, 10.0], 6.0)];

        // Point at center should be inside (negative distance)
        let d = cluster_blob([0.0, 3.0], &circles, 5.0);
        assert!(d < 0.0);

        // Point far away should be outside (positive distance)
        let d = cluster_blob([100.0, 100.0], &circles, 5.0);
        assert!(d > 0.0);
    }

    #[test]
    fn test_cluster_blob_empty() {
        let d = cluster_blob([0.0, 0.0], &[], 5.0);
        assert_eq!(d, f32::MAX);
    }

    #[test]
    fn test_confidence_halo_inside_core() {
        let halo = ConfidenceHalo::new([0.0, 0.0], 20.0, 0.7);

        // Inside core - full confidence alpha
        assert!(approx_eq(halo.alpha_at([0.0, 0.0]), 0.7));
        assert!(approx_eq(halo.alpha_at([10.0, 0.0]), 0.7));
    }

    #[test]
    fn test_confidence_halo_outside() {
        let halo = ConfidenceHalo::new([0.0, 0.0], 20.0, 0.7);

        // Far outside halo - zero alpha
        assert!(approx_eq(halo.alpha_at([100.0, 0.0]), 0.0));
    }

    #[test]
    fn test_confidence_halo_penumbra() {
        let halo = ConfidenceHalo::new([0.0, 0.0], 20.0, 0.5);

        // In penumbra - should have partial alpha
        let outer = halo.outer_radius();
        let mid_point = [(20.0 + outer) / 2.0, 0.0];
        let alpha = halo.alpha_at(mid_point);

        assert!(alpha > 0.0);
        assert!(alpha < 0.5);
    }

    #[test]
    fn test_confidence_halo_outer_radius() {
        // High confidence = small halo
        let halo_high = ConfidenceHalo::new([0.0, 0.0], 20.0, 0.95);
        // Low confidence = large halo
        let halo_low = ConfidenceHalo::new([0.0, 0.0], 20.0, 0.3);

        assert!(halo_high.outer_radius() < halo_low.outer_radius());
    }

    #[test]
    fn test_hit_test_circle() {
        let center = [0.0, 0.0];
        let radius = 10.0;

        // Inside
        let result = hit_test_circle([5.0, 0.0], center, radius);
        assert!(result.inside);
        assert!(result.distance < 0.0);

        // Outside
        let result = hit_test_circle([15.0, 0.0], center, radius);
        assert!(!result.inside);
        assert!(result.distance > 0.0);

        // Gradient should point outward
        assert!(result.gradient[0] > 0.0);
    }

    #[test]
    fn test_hit_test_rect() {
        let center = [0.0, 0.0];
        let half = [10.0, 5.0];

        // Inside
        let result = hit_test_rect([0.0, 0.0], center, half);
        assert!(result.inside);

        // Outside
        let result = hit_test_rect([15.0, 0.0], center, half);
        assert!(!result.inside);
    }

    #[test]
    fn test_offset() {
        let d = circle([15.0, 0.0], [0.0, 0.0], 10.0); // d = 5
        let grown = offset(d, 3.0); // d = 2 (shape grew by 3)
        assert!(approx_eq(grown, 2.0));

        let shrunk = offset(d, -3.0); // d = 8 (shape shrunk by 3)
        assert!(approx_eq(shrunk, 8.0));
    }
}

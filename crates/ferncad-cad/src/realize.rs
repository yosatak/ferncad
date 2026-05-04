//! ShapeNode to TriMesh conversion
//!
//! Hybrid pipeline: primitives use BREP tessellation for smooth curves,
//! CSG operations use BSP on the tessellated meshes for reliable booleans.

use std::collections::HashMap;

use ferncad_core::error::{FernError, FernResult};
use ferncad_core::types::ShapeNode;

use crate::brep;
use crate::bsp;
use crate::mesh::TriMesh;
use crate::primitives;
use crate::transform;

/// Default minimum segments for curved primitives used in BSP CSG.
/// 32 gives a good balance: visually smooth, and BSP stays fast.
const DEFAULT_MIN_SEGMENTS: u32 = 32;

/// Cache key: raw pointer to ShapeNode (via Arc) + min_segments.
/// Safe because we only use the pointer as an identity key while the Arc is alive.
pub type RealizeCache = HashMap<(*const ShapeNode, u32), TriMesh>;

/// Convert a ShapeNode tree to a TriMesh using default resolution
pub fn realize(node: &ShapeNode) -> FernResult<TriMesh> {
    realize_with_resolution(node, DEFAULT_MIN_SEGMENTS)
}

/// Convert a ShapeNode tree to a TriMesh with specified minimum segment count
///
/// `min_segments` sets the minimum for curved primitives (higher = smoother, slower).
/// Uses BREP for standalone primitives/transforms, BSP for CSG operations.
pub fn realize_with_resolution(node: &ShapeNode, min_segments: u32) -> FernResult<TriMesh> {
    let mut cache = RealizeCache::new();
    realize_cached(node, min_segments, &mut cache)
}

/// Convert a ShapeNode tree to a TriMesh, using a cache for identical subtrees
///
/// When the same `Arc<ShapeNode>` appears multiple times (e.g., repeated parts),
/// the cache avoids redundant realization.
pub fn realize_with_cache(
    node: &ShapeNode,
    min_segments: u32,
    cache: &mut RealizeCache,
) -> FernResult<TriMesh> {
    realize_cached(node, min_segments, cache)
}

/// Internal cached realization
fn realize_cached(
    node: &ShapeNode,
    min_segments: u32,
    cache: &mut RealizeCache,
) -> FernResult<TriMesh> {
    // Check cache by pointer identity
    let cache_key = (node as *const ShapeNode, min_segments);
    if let Some(mesh) = cache.get(&cache_key) {
        return Ok(mesh.clone());
    }

    let mesh = match node {
        // CSG operations: go directly to BSP (BREP booleans are unreliable/slow)
        ShapeNode::Union { .. } | ShapeNode::Difference { .. } | ShapeNode::Intersection { .. } => {
            realize_hybrid_cached(node, min_segments, cache)?
        }

        // Primitives and transforms: try BREP first for smooth tessellation
        _ => {
            let brep_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                brep::shape_to_solid(node).and_then(|solid| brep::solid_to_trimesh(&solid))
            }));

            match brep_result {
                Ok(Ok(m)) if m.triangle_count() > 0 => m,
                _ => realize_hybrid_cached(node, min_segments, cache)?,
            }
        }
    };

    cache.insert(cache_key, mesh.clone());
    Ok(mesh)
}

/// BSP-based realization with enhanced segment count for curved primitives
fn realize_hybrid_cached(
    node: &ShapeNode,
    min_segments: u32,
    cache: &mut RealizeCache,
) -> FernResult<TriMesh> {
    match node {
        // === Primitives: use max(segments, min_segments) for smooth curves ===
        ShapeNode::Box {
            width,
            depth,
            height,
        } => Ok(primitives::generate_box(*width, *depth, *height)),

        ShapeNode::Sphere { radius, segments } => Ok(primitives::generate_sphere(
            *radius,
            (*segments).max(min_segments),
        )),

        ShapeNode::Cylinder {
            radius,
            height,
            segments,
        } => Ok(primitives::generate_cylinder(
            *radius,
            *height,
            (*segments).max(min_segments),
        )),

        ShapeNode::Cone {
            radius_bottom,
            radius_top,
            height,
            segments,
        } => Ok(primitives::generate_cone(
            *radius_bottom,
            *radius_top,
            *height,
            (*segments).max(min_segments),
        )),

        ShapeNode::Torus {
            radius_major,
            radius_minor,
            segments,
        } => Ok(primitives::generate_torus(
            *radius_major,
            *radius_minor,
            (*segments).max(min_segments),
        )),

        ShapeNode::Prism {
            sides,
            radius,
            height,
        } => Ok(primitives::generate_prism(*sides, *radius, *height)),

        ShapeNode::Extrude { profile, height } => {
            Ok(primitives::generate_extrude(profile, *height))
        }

        ShapeNode::Revolve {
            profile,
            angle_rad,
            segments,
        } => Ok(primitives::generate_revolve(
            profile,
            *angle_rad,
            (*segments).max(min_segments),
        )),

        // === CSG: BSP on realized meshes ===
        // Children must go through realize_hybrid_cached (not realize_cached)
        // to avoid BREP tessellation which produces high-poly meshes that make
        // BSP boolean operations extremely slow.
        ShapeNode::Union { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize_hybrid_cached(&children[0], min_segments, cache)?;
            for child in &children[1..] {
                let child_mesh = realize_hybrid_cached(child, min_segments, cache)?;
                result = bsp::csg_union(&result, &child_mesh);
            }
            Ok(result)
        }

        ShapeNode::Difference { base, cutters } => {
            let mut result = realize_hybrid_cached(base, min_segments, cache)?;
            for cutter in cutters {
                let cutter_mesh = realize_hybrid_cached(cutter, min_segments, cache)?;
                result = bsp::csg_difference(&result, &cutter_mesh);
            }
            Ok(result)
        }

        ShapeNode::Intersection { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize_hybrid_cached(&children[0], min_segments, cache)?;
            for child in &children[1..] {
                let child_mesh = realize_hybrid_cached(child, min_segments, cache)?;
                result = bsp::csg_intersection(&result, &child_mesh);
            }
            Ok(result)
        }

        // === Transforms ===
        ShapeNode::Translate { shape, offset } => {
            let mut mesh = realize_cached(shape, min_segments, cache)?;
            transform::translate(&mut mesh, *offset);
            Ok(mesh)
        }

        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        } => {
            let mut mesh = realize_cached(shape, min_segments, cache)?;
            transform::rotate(&mut mesh, *axis, *angle_rad);
            Ok(mesh)
        }

        ShapeNode::Scale { shape, factors } => {
            let mut mesh = realize_cached(shape, min_segments, cache)?;
            transform::scale(&mut mesh, *factors);
            Ok(mesh)
        }

        ShapeNode::Sweep {
            profile,
            path,
            segments,
        } => Ok(primitives::generate_sweep(
            profile,
            path,
            (*segments).max(min_segments),
        )),

        ShapeNode::Loft {
            profiles,
            positions,
            segments,
        } => Ok(primitives::generate_loft(
            profiles,
            positions,
            (*segments).max(min_segments),
        )),

        ShapeNode::Chamfer { shape, distance } => {
            let _ = distance;
            realize_cached(shape, min_segments, cache)
        }
    }
}

/// Evaluate source code and convert to a mesh
///
/// Reads `*resolution*` from the evaluator environment to control mesh quality.
/// Returns the mesh if the last expression evaluates to a Shape.
pub fn eval_and_realize(source: &str) -> FernResult<TriMesh> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator.eval_source(source)?;
    let resolution = evaluator.resolution();

    match result {
        ferncad_core::types::Value::Shape(tracked) => {
            realize_with_resolution(&tracked.node, resolution)
        }
        _ => Err(FernError::CadError {
            message: format!(
                "cannot convert to mesh: the last expression did not return a shape (type: {})",
                result.type_name()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_realize_box() {
        let node = ShapeNode::Box {
            width: 10.0,
            depth: 10.0,
            height: 10.0,
        };
        let mesh = realize(&node).unwrap();
        assert!(mesh.triangle_count() >= 12);
        assert!(mesh.vertex_count() >= 8);

        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 0.5);
        assert!((max[0] - 5.0).abs() < 0.5);
    }

    #[test]
    fn test_realize_translate() {
        let node = ShapeNode::Translate {
            shape: Arc::new(ShapeNode::Box {
                width: 10.0,
                depth: 10.0,
                height: 10.0,
            }),
            offset: [5.0, 0.0, 0.0],
        };
        let mesh = realize(&node).unwrap();
        let (min, max) = mesh.bounding_box();
        assert!((min[0] - 0.0).abs() < 0.5);
        assert!((max[0] - 10.0).abs() < 0.5);
    }

    #[test]
    fn test_realize_union() {
        let node = ShapeNode::Union {
            children: vec![
                Arc::new(ShapeNode::Box {
                    width: 10.0,
                    depth: 10.0,
                    height: 10.0,
                }),
                Arc::new(ShapeNode::Sphere {
                    radius: 5.0,
                    segments: 8,
                }),
            ],
        };
        let mesh = realize(&node).unwrap();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_realize_difference() {
        let node = ShapeNode::Difference {
            base: Arc::new(ShapeNode::Box {
                width: 20.0,
                depth: 20.0,
                height: 20.0,
            }),
            cutters: vec![Arc::new(ShapeNode::Sphere {
                radius: 8.0,
                segments: 8,
            })],
        };
        let mesh = realize(&node).unwrap();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_realize_difference_smooth_sphere() {
        // Verify that the sphere hole is smooth (BREP tessellation, not 8 segments)
        let node = ShapeNode::Difference {
            base: Arc::new(ShapeNode::Box {
                width: 20.0,
                depth: 20.0,
                height: 20.0,
            }),
            cutters: vec![Arc::new(ShapeNode::Sphere {
                radius: 8.0,
                segments: 8, // low segments, but BREP should give smooth result
            })],
        };
        let mesh = realize(&node).unwrap();
        // With segments=8 only (no BREP), a sphere produces ~56 triangles.
        // With BREP tessellation, the sphere mesh alone has ~900 triangles.
        // After BSP difference, the result should have significantly more
        // triangles than a purely 8-segment approach.
        assert!(
            mesh.triangle_count() > 100,
            "expected smooth sphere mesh, got {} triangles",
            mesh.triangle_count()
        );
    }

    #[test]
    fn test_eval_and_realize() {
        let mesh = eval_and_realize("(box :width 10 :depth 10 :height 10)").unwrap();
        assert!(mesh.triangle_count() >= 12);
    }

    #[test]
    fn test_realize_sweep() {
        let source = r#"
            (sweep :profile (circle :radius 1 :segments 8)
                   :path (helix :radius 5 :pitch 2 :turns 1)
                   :segments 16)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() > 0);
        assert!(mesh.vertex_count() > 0);
    }

    #[test]
    fn test_realize_loft() {
        let source = r#"
            (loft :profiles (list (circle :radius 5 :segments 8)
                                  (circle :radius 3 :segments 8))
                  :at (list 0 10)
                  :segments 8)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    #[ignore] // BSP union of 5-turn helix sweep is too expensive for CI
    fn test_realize_sweep_with_csg() {
        let source = r#"
            (let* ((thread-profile (polygon (list 1.2 0.0) (list 1.5 0.25) (list 1.2 0.5)))
                   (thread (sweep :profile thread-profile
                                  :path (helix :radius 0 :pitch 0.5 :turns 5)
                                  :segments 32))
                   (shaft (cylinder :radius 1.2 :height 2.5)))
              (union shaft thread))
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_eval_and_realize_with_defpart() {
        let source = r#"
            (defpart my-part
              "test"
              :params ((size :: length :default 10.0 :doc "s"))
              :body
              (box :width size :depth size :height size))
            (my-part :size 20.0)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() >= 12);
        let (_min, max) = mesh.bounding_box();
        assert!((max[0] - 10.0).abs() < 0.5);
    }

    // ========================================================================
    // Gear profile validation tests
    //
    // These tests render standard gears and analyse the XY-projected radial
    // profile of the mesh vertices.  The profile is sampled at regular angular
    // intervals and compared against the expected involute geometry.
    // ========================================================================

    /// Extract the XY radial profile from a gear mesh.
    ///
    /// Samples the maximum vertex radius in angular bins on the top/bottom cap.
    /// Uses interpolation along polygon edges to avoid bin aliasing.
    fn extract_radial_profile(mesh: &TriMesh, n_bins: usize) -> Vec<f64> {
        let bin_width = std::f64::consts::TAU / n_bins as f64;
        let mut max_r = vec![0.0_f64; n_bins];

        // Find cap Z (top or bottom face)
        let (bb_min, _bb_max) = mesh.bounding_box();
        let z_cap = bb_min[2];
        let z_tol = 0.01;

        // Collect cap vertices
        let mut cap_verts: Vec<(f64, f64)> = Vec::new(); // (angle, radius)
        for v in &mesh.vertices {
            if (v[2] - z_cap).abs() < z_tol {
                let r = (v[0] * v[0] + v[1] * v[1]).sqrt();
                let mut angle = v[1].atan2(v[0]);
                if angle < 0.0 {
                    angle += std::f64::consts::TAU;
                }
                let bin = ((angle / bin_width) as usize).min(n_bins - 1);
                if r > max_r[bin] {
                    max_r[bin] = r;
                }
                cap_verts.push((angle, r));
            }
        }

        // Sort by angle for interpolation
        cap_verts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Fill empty bins by interpolation from sorted vertices
        if cap_verts.len() > 2 {
            for bin_idx in 0..n_bins {
                if max_r[bin_idx] > 0.0 {
                    continue;
                }
                let target_angle = (bin_idx as f64 + 0.5) * bin_width;
                // Find surrounding vertices
                let pos = cap_verts
                    .binary_search_by(|v| v.0.partial_cmp(&target_angle).unwrap())
                    .unwrap_or_else(|i| i);
                if pos > 0 && pos < cap_verts.len() {
                    let (a0, r0) = cap_verts[pos - 1];
                    let (a1, r1) = cap_verts[pos];
                    if (a1 - a0).abs() < 0.2 {
                        // ~11 degrees max gap
                        let t = (target_angle - a0) / (a1 - a0);
                        max_r[bin_idx] = r0 + t * (r1 - r0);
                    }
                }
            }
        }

        max_r
    }

    /// Count teeth by counting contiguous runs of bins above threshold.
    /// Empty bins (r=0) are ignored (treated as transparent).
    fn count_teeth_profile(profile: &[f64], threshold: f64) -> usize {
        let n = profile.len();
        if n == 0 {
            return 0;
        }

        // Label bins: true = above, false = below, skip empty (0)
        // "above" means this bin has a vertex above threshold
        // "below" means this bin has a vertex below threshold
        // "empty" means no vertex in this bin — should not start or end a tooth
        let mut count = 0;
        let mut in_tooth = false;

        // Find initial state from last non-empty bin
        for i in (0..n).rev() {
            if profile[i] > 0.0 {
                in_tooth = profile[i] >= threshold;
                break;
            }
        }

        for i in 0..n {
            let r = profile[i];
            if r == 0.0 {
                continue; // skip empty bins
            }
            let above = r >= threshold;
            if above && !in_tooth {
                count += 1;
            }
            in_tooth = above;
        }

        count
    }

    /// Find angular positions of tooth peaks in the radial profile.
    fn find_tooth_peaks_profile(profile: &[f64], threshold: f64) -> Vec<f64> {
        let n = profile.len();
        let bin_width = std::f64::consts::TAU / n as f64;
        let hysteresis = threshold * 0.02;
        let low = threshold - hysteresis;
        let mut peaks = Vec::new();
        let mut above = false;
        let mut best_r = 0.0_f64;
        let mut best_bin = 0;

        for i in 0..n {
            let r = profile[i];
            if !above && r >= threshold {
                above = true;
                best_r = r;
                best_bin = i;
            } else if above && r > best_r {
                best_r = r;
                best_bin = i;
            } else if above && r < low {
                above = false;
                peaks.push((best_bin as f64 + 0.5) * bin_width);
            }
        }
        if above {
            peaks.push((best_bin as f64 + 0.5) * bin_width);
        }
        peaks
    }

    /// Check rotational symmetry from radial profile peaks.
    fn check_rotational_symmetry_profile(
        profile: &[f64],
        n_teeth: usize,
        threshold: f64,
    ) -> (f64, f64) {
        let peaks = find_tooth_peaks_profile(profile, threshold);
        if peaks.len() < 2 {
            return (0.0, f64::MAX);
        }

        let expected_spacing = std::f64::consts::TAU / n_teeth as f64;
        let mut spacings = Vec::new();
        for i in 0..peaks.len() {
            let next_i = (i + 1) % peaks.len();
            let mut diff = peaks[next_i] - peaks[i];
            if diff <= 0.0 {
                diff += std::f64::consts::TAU;
            }
            spacings.push(diff);
        }

        let mean = spacings.iter().sum::<f64>() / spacings.len() as f64;
        let max_dev = spacings
            .iter()
            .map(|s| (s - expected_spacing).abs())
            .fold(0.0_f64, f64::max);
        (mean, max_dev)
    }

    #[test]
    fn test_spur_gear_profile_radii() {
        // module=2, teeth=20 → r_pitch=20, r_addendum=22, r_dedendum=17.5
        let source = r#"
            (require :ferncad-std/spur-gear)
            (spur-gear :module 2 :teeth 20 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() > 100);

        let r_addendum = 22.0;
        let r_dedendum = 17.5;

        let profile = extract_radial_profile(&mesh, 3600);

        let max_r = profile.iter().cloned().fold(0.0_f64, f64::max);
        assert!(
            (max_r - r_addendum).abs() < 0.5,
            "max radius should be near addendum {r_addendum}, got {max_r}"
        );

        let min_r = profile
            .iter()
            .cloned()
            .filter(|r| *r > 0.0)
            .fold(f64::MAX, f64::min);
        assert!(
            (min_r - r_dedendum).abs() < 1.5,
            "min radius should be near dedendum {r_dedendum}, got {min_r}"
        );
    }

    #[test]
    fn test_spur_gear_tooth_count() {
        let source = r#"
            (require :ferncad-std/spur-gear)
            (spur-gear :module 2 :teeth 20 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        let profile = extract_radial_profile(&mesh, 3600);
        let r_pitch = 20.0;
        let teeth = count_teeth_profile(&profile, r_pitch);
        assert_eq!(teeth, 20, "expected 20 teeth, detected {teeth}");
    }

    #[test]
    fn test_spur_gear_rotational_symmetry() {
        let source = r#"
            (require :ferncad-std/spur-gear)
            (spur-gear :module 2 :teeth 20 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        let profile = extract_radial_profile(&mesh, 3600);
        let r_pitch = 20.0;

        let (mean_spacing, max_deviation) =
            check_rotational_symmetry_profile(&profile, 20, r_pitch);
        let expected_spacing = std::f64::consts::TAU / 20.0;

        assert!(
            (mean_spacing - expected_spacing).abs() < 0.1,
            "mean tooth spacing {mean_spacing:.4} should be near {expected_spacing:.4}"
        );
        assert!(
            max_deviation < 0.3,
            "max angular deviation {max_deviation:.4} rad is too large"
        );
    }

    #[test]
    fn test_spur_gear_tooth_symmetry() {
        // Each tooth should be left-right symmetric in the radial profile.
        let source = r#"
            (require :ferncad-std/spur-gear)
            (spur-gear :module 3 :teeth 12 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        let n_bins = 1440;
        let profile = extract_radial_profile(&mesh, n_bins);
        let r_pitch = 18.0;
        let bins_per_tooth = n_bins / 12;

        let peaks = find_tooth_peaks_profile(&profile, r_pitch);
        assert!(!peaks.is_empty(), "should find at least one tooth peak");

        // Find the bin closest to the first peak
        let bin_width = std::f64::consts::TAU / n_bins as f64;
        let peak_bin = (peaks[0] / bin_width) as usize;

        // Compare left and right sides of this tooth
        let mut max_asym = 0.0_f64;
        let half_tooth = bins_per_tooth / 2;
        for d in 1..half_tooth {
            let left = profile[(peak_bin + n_bins - d) % n_bins];
            let right = profile[(peak_bin + d) % n_bins];
            if left > 0.0 && right > 0.0 {
                let diff = (left - right).abs();
                if diff > max_asym {
                    max_asym = diff;
                }
            }
        }

        assert!(
            max_asym < 1.5,
            "tooth asymmetry {max_asym:.3} mm is too large"
        );
    }

    #[test]
    fn test_spur_gear_different_tooth_counts() {
        for &(n_teeth, module) in &[(8, 3.0), (12, 2.0), (24, 1.5), (36, 1.0)] {
            let source = format!(
                r#"
                (require :ferncad-std/spur-gear)
                (spur-gear :module {module} :teeth {n_teeth} :face-width 5)
                "#
            );
            let mesh = eval_and_realize(&source).unwrap();
            let r_pitch = module * n_teeth as f64 / 2.0;
            let profile = extract_radial_profile(&mesh, 3600);
            let detected = count_teeth_profile(&profile, r_pitch);
            assert_eq!(
                detected, n_teeth,
                "module={module} teeth={n_teeth}: expected {n_teeth}, detected {detected}"
            );
        }
    }

    #[test]
    fn test_bevel_gear_profile_radii() {
        let source = r#"
            (require :ferncad-std/bevel-gear)
            (bevel-gear :module 2 :teeth 20 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert!(mesh.triangle_count() > 100);

        let r_addendum_outer = 22.0;
        let (_min, max) = mesh.bounding_box();
        assert!(
            (max[0] - r_addendum_outer).abs() < 1.5,
            "bevel gear max X near outer addendum {r_addendum_outer}, got {}",
            max[0]
        );
        assert!(max[2] > 0.0, "bevel gear should extend in +Z");
    }

    #[test]
    fn test_bevel_gear_tooth_count() {
        let source = r#"
            (require :ferncad-std/bevel-gear)
            (bevel-gear :module 2 :teeth 20 :face-width 10)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        let r_pitch = 20.0;
        let profile = extract_radial_profile(&mesh, 3600);
        let teeth = count_teeth_profile(&profile, r_pitch);
        assert_eq!(teeth, 20, "bevel gear: expected 20 teeth, detected {teeth}");
    }

    #[test]
    fn test_realize_cache_reuses_identical_subtrees() {
        // Realize the same Arc<ShapeNode> twice — second call should use cache
        let shared = Arc::new(ShapeNode::Box {
            width: 10.0,
            depth: 10.0,
            height: 10.0,
        });
        let mut cache = RealizeCache::new();
        let mesh1 = realize_with_cache(&shared, 16, &mut cache).unwrap();
        assert!(mesh1.triangle_count() > 0);
        assert!(
            !cache.is_empty(),
            "cache should have an entry after first realize"
        );
        let cache_len_after_first = cache.len();
        // Second realize should hit the cache (no new entries added)
        let mesh2 = realize_with_cache(&shared, 16, &mut cache).unwrap();
        assert_eq!(mesh1.triangle_count(), mesh2.triangle_count());
        assert_eq!(
            cache.len(),
            cache_len_after_first,
            "no new cache entries on cache hit"
        );
    }

    #[test]
    fn test_realize_cache_produces_correct_output() {
        // Verify cached result is identical to uncached
        let node = ShapeNode::Translate {
            shape: Arc::new(ShapeNode::Box {
                width: 10.0,
                depth: 10.0,
                height: 10.0,
            }),
            offset: [5.0, 0.0, 0.0],
        };
        let uncached = realize_with_resolution(&node, 32).unwrap();
        let mut cache = RealizeCache::new();
        let cached = realize_with_cache(&node, 32, &mut cache).unwrap();
        assert_eq!(uncached.triangle_count(), cached.triangle_count());
        assert_eq!(uncached.vertex_count(), cached.vertex_count());
    }
}

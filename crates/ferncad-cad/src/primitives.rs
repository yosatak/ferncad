//! Primitive shape mesh generation
//!
//! Directly generates meshes for box, sphere, cylinder, cone, and prism.

use std::f64::consts::PI;

use crate::mesh::TriMesh;
use ferncad_core::types::PathNode;

/// Default number of segments
pub const DEFAULT_SEGMENTS: u32 = 32;

/// Generate a box mesh (centered at origin)
///
/// 8 vertices, 12 triangles
pub fn generate_box(width: f64, depth: f64, height: f64) -> TriMesh {
    let hw = width / 2.0;
    let hd = depth / 2.0;
    let hh = height / 2.0;

    let vertices = vec![
        // front face (z+)
        [-hw, -hd, hh], // 0
        [hw, -hd, hh],  // 1
        [hw, hd, hh],   // 2
        [-hw, hd, hh],  // 3
        // back face (z-)
        [-hw, -hd, -hh], // 4
        [hw, -hd, -hh],  // 5
        [hw, hd, -hh],   // 6
        [-hw, hd, -hh],  // 7
    ];

    let triangles = vec![
        // front face (z+)
        [0, 1, 2],
        [0, 2, 3],
        // back face (z-)
        [5, 4, 7],
        [5, 7, 6],
        // top face (y+)
        [3, 2, 6],
        [3, 6, 7],
        // bottom face (y-)
        [4, 5, 1],
        [4, 1, 0],
        // right face (x+)
        [1, 5, 6],
        [1, 6, 2],
        // left face (x-)
        [4, 0, 3],
        [4, 3, 7],
    ];

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a sphere mesh (UV sphere, centered at origin)
pub fn generate_sphere(radius: f64, segments: u32) -> TriMesh {
    let rings = segments / 2;
    let sectors = segments;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Generate vertices
    for ring in 0..=rings {
        let phi = PI * ring as f64 / rings as f64;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for sector in 0..=sectors {
            let theta = 2.0 * PI * sector as f64 / sectors as f64;
            let x = sin_phi * theta.cos() * radius;
            let y = sin_phi * theta.sin() * radius;
            let z = cos_phi * radius;
            vertices.push([x, y, z]);
        }
    }

    // Generate triangles
    let cols = sectors + 1;
    for ring in 0..rings {
        for sector in 0..sectors {
            let a = ring * cols + sector;
            let b = a + 1;
            let c = a + cols;
            let d = c + 1;

            let a = a as usize;
            let b = b as usize;
            let c = c as usize;
            let d = d as usize;

            if ring > 0 {
                triangles.push([a, c, b]);
            }
            if ring < rings - 1 {
                triangles.push([b, c, d]);
            }
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a cylinder mesh (centered at origin, along Z axis)
pub fn generate_cylinder(radius: f64, height: f64, segments: u32) -> TriMesh {
    let hh = height / 2.0;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Top cap center
    let top_center = vertices.len();
    vertices.push([0.0, 0.0, hh]);

    // Top cap rim
    let top_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius * theta.cos(), radius * theta.sin(), hh]);
    }

    // Bottom cap center
    let bot_center = vertices.len();
    vertices.push([0.0, 0.0, -hh]);

    // Bottom cap rim
    let bot_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius * theta.cos(), radius * theta.sin(), -hh]);
    }

    let seg = segments as usize;

    // Top cap triangles
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([top_center, top_ring_start + i, top_ring_start + next]);
    }

    // Bottom cap triangles (reversed winding)
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([bot_center, bot_ring_start + next, bot_ring_start + i]);
    }

    // Side faces
    for i in 0..seg {
        let next = (i + 1) % seg;
        let t0 = top_ring_start + i;
        let t1 = top_ring_start + next;
        let b0 = bot_ring_start + i;
        let b1 = bot_ring_start + next;
        triangles.push([t0, b0, b1]);
        triangles.push([t0, b1, t1]);
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a cone mesh (centered at origin, along Z axis)
pub fn generate_cone(radius_bottom: f64, radius_top: f64, height: f64, segments: u32) -> TriMesh {
    // If radius_top is near zero, the cone has a sharp apex;
    // otherwise it is a truncated cone.
    let hh = height / 2.0;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Top face
    let top_center = vertices.len();
    vertices.push([0.0, 0.0, hh]);

    let top_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius_top * theta.cos(), radius_top * theta.sin(), hh]);
    }

    // Bottom face
    let bot_center = vertices.len();
    vertices.push([0.0, 0.0, -hh]);

    let bot_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([
            radius_bottom * theta.cos(),
            radius_bottom * theta.sin(),
            -hh,
        ]);
    }

    let seg = segments as usize;

    // Top cap (only if radius_top > 0)
    if radius_top > 1e-10 {
        for i in 0..seg {
            let next = (i + 1) % seg;
            triangles.push([top_center, top_ring_start + i, top_ring_start + next]);
        }
    }

    // Bottom cap
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([bot_center, bot_ring_start + next, bot_ring_start + i]);
    }

    // Side faces
    for i in 0..seg {
        let next = (i + 1) % seg;
        if radius_top > 1e-10 {
            let t0 = top_ring_start + i;
            let t1 = top_ring_start + next;
            let b0 = bot_ring_start + i;
            let b1 = bot_ring_start + next;
            triangles.push([t0, b0, b1]);
            triangles.push([t0, b1, t1]);
        } else {
            // Sharp cone: triangles from apex to bottom rim
            let b0 = bot_ring_start + i;
            let b1 = bot_ring_start + next;
            triangles.push([top_center, b0, b1]);
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a prism mesh (centered at origin, along Z axis)
pub fn generate_prism(sides: u32, radius: f64, height: f64) -> TriMesh {
    generate_cylinder(radius, height, sides)
}

/// Generate a torus mesh (centered at origin, around Z axis)
pub fn generate_torus(radius_major: f64, radius_minor: f64, segments: u32) -> TriMesh {
    let ring_segments = segments;
    let tube_segments = segments / 2;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    for i in 0..=ring_segments {
        let theta = 2.0 * PI * i as f64 / ring_segments as f64;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        for j in 0..=tube_segments {
            let phi = 2.0 * PI * j as f64 / tube_segments as f64;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            let x = (radius_major + radius_minor * cos_phi) * cos_theta;
            let y = (radius_major + radius_minor * cos_phi) * sin_theta;
            let z = radius_minor * sin_phi;
            vertices.push([x, y, z]);
        }
    }

    let cols = tube_segments + 1;
    for i in 0..ring_segments {
        for j in 0..tube_segments {
            let a = (i * cols + j) as usize;
            let b = (i * cols + j + 1) as usize;
            let c = ((i + 1) * cols + j) as usize;
            let d = ((i + 1) * cols + j + 1) as usize;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a mesh by extruding a 2D polygon (in XY plane) along Z axis
///
/// Uses ear-clipping triangulation for top/bottom caps and quad-strip for sides.
pub fn generate_extrude(profile: &[[f64; 2]], height: f64) -> TriMesh {
    let n = profile.len();
    let hh = height / 2.0;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Bottom cap vertices (z = -hh)
    for p in profile {
        vertices.push([p[0], p[1], -hh]);
    }
    // Top cap vertices (z = +hh)
    for p in profile {
        vertices.push([p[0], p[1], hh]);
    }

    // Bottom cap triangulation (fan from vertex 0, reversed winding)
    for i in 1..n - 1 {
        triangles.push([0, i + 1, i]);
    }
    // Top cap triangulation (fan from vertex n)
    for i in 1..n - 1 {
        triangles.push([n, n + i, n + i + 1]);
    }

    // Side faces (quad strip)
    for i in 0..n {
        let j = (i + 1) % n;
        let b0 = i;
        let b1 = j;
        let t0 = n + i;
        let t1 = n + j;
        triangles.push([b0, b1, t1]);
        triangles.push([b0, t1, t0]);
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Generate a mesh by revolving a 2D profile (in XZ plane) around Z axis
///
/// Each profile point (x, z) is revolved to create a surface of revolution.
pub fn generate_revolve(profile: &[[f64; 2]], angle_rad: f64, segments: u32) -> TriMesh {
    let n = profile.len();
    let seg = segments as usize;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Generate vertices: for each angular step, rotate each profile point
    for s in 0..=seg {
        let theta = angle_rad * s as f64 / seg as f64;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        for p in profile {
            let x = p[0] * cos_t;
            let y = p[0] * sin_t;
            let z = p[1];
            vertices.push([x, y, z]);
        }
    }

    // Generate triangles: connect adjacent rings
    for s in 0..seg {
        for i in 0..n - 1 {
            let a = s * n + i;
            let b = s * n + i + 1;
            let c = (s + 1) * n + i;
            let d = (s + 1) * n + i + 1;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }

    // Cap the ends if it's a full revolution (close the shape)
    let full_rev = (angle_rad - 2.0 * PI).abs() < 0.01;
    if !full_rev {
        // Add end caps for partial revolution (fan triangulation)
        // Start cap (s=0)
        for i in 1..n - 1 {
            triangles.push([0, i + 1, i]);
        }
        // End cap (s=seg)
        let base = seg * n;
        for i in 1..n - 1 {
            triangles.push([base, base + i, base + i + 1]);
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

// === Vector math helpers ===

fn vec3_normalize(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn vec3_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec3_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Compute initial normal vector perpendicular to the path tangent at t=0
fn initial_normal(path: &PathNode) -> [f64; 3] {
    let tangent = vec3_normalize(path.tangent(0.0));
    // Choose a reference vector not parallel to tangent
    let up = if tangent[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    vec3_normalize(vec3_cross(tangent, up))
}

/// Rotation-minimizing frame: project previous normal to be perpendicular to new tangent
fn rotation_minimizing_normal(tangent: [f64; 3], prev_normal: [f64; 3]) -> [f64; 3] {
    let d = vec3_dot(prev_normal, tangent);
    let projected = [
        prev_normal[0] - d * tangent[0],
        prev_normal[1] - d * tangent[1],
        prev_normal[2] - d * tangent[2],
    ];
    vec3_normalize(projected)
}

/// Compute the radial frame for a helix at parameter t.
///
/// For helical paths, the profile must always point radially outward (not use RMF).
/// Returns (normal, binormal) where normal points radially outward from the helix axis.
fn helix_radial_frame(path: &PathNode, t: f64) -> ([f64; 3], [f64; 3]) {
    let pos = path.position(t);
    let tangent = vec3_normalize(path.tangent(t));
    // Radial outward direction: project position onto XY plane and normalize
    let radial = vec3_normalize([pos[0], pos[1], 0.0]);
    // Binormal = tangent × radial (completes right-handed frame)
    let binormal = vec3_cross(tangent, radial);
    (radial, binormal)
}

/// Generate a mesh by sweeping a 2D profile along a 3D path
///
/// For helix paths, uses an analytical radial frame so the profile always points
/// outward from the helix axis (correct for screw threads).
/// For other paths, uses a rotation-minimizing frame.
pub fn generate_sweep(profile: &[[f64; 2]], path: &PathNode, segments: u32) -> TriMesh {
    let n = profile.len();
    let seg = segments as usize;
    let mut vertices = Vec::with_capacity((seg + 1) * n);
    let mut triangles = Vec::new();

    let use_radial = matches!(path, PathNode::Helix { .. });
    let mut prev_normal = initial_normal(path);

    for s in 0..=seg {
        let t = s as f64 / seg as f64;
        let pos = path.position(t);

        let (normal, binormal) = if use_radial {
            helix_radial_frame(path, t)
        } else {
            let tangent = vec3_normalize(path.tangent(t));
            let normal = rotation_minimizing_normal(tangent, prev_normal);
            let binormal = vec3_cross(tangent, normal);
            prev_normal = normal;
            (normal, binormal)
        };

        for p in profile {
            let x = pos[0] + p[0] * normal[0] + p[1] * binormal[0];
            let y = pos[1] + p[0] * normal[1] + p[1] * binormal[1];
            let z = pos[2] + p[0] * normal[2] + p[1] * binormal[2];
            vertices.push([x, y, z]);
        }
    }

    // Connect adjacent rings (closed profile loop)
    for s in 0..seg {
        for i in 0..n {
            let next_i = (i + 1) % n;
            let a = s * n + i;
            let b = s * n + next_i;
            let c = (s + 1) * n + i;
            let d = (s + 1) * n + next_i;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }

    // End caps (fan triangulation)
    // Start cap
    for i in 1..n - 1 {
        triangles.push([0, i + 1, i]);
    }
    // End cap
    let base = seg * n;
    for i in 1..n - 1 {
        triangles.push([base, base + i, base + i + 1]);
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// Resample a 2D profile to exactly `target` points via linear interpolation along perimeter
fn resample_profile(profile: &[[f64; 2]], target: usize) -> Vec<[f64; 2]> {
    if profile.len() == target {
        return profile.to_vec();
    }

    let n = profile.len();
    // Compute cumulative arc lengths
    let mut cum_lengths = Vec::with_capacity(n + 1);
    cum_lengths.push(0.0);
    for i in 0..n {
        let next = (i + 1) % n;
        let dx = profile[next][0] - profile[i][0];
        let dy = profile[next][1] - profile[i][1];
        cum_lengths.push(cum_lengths[i] + (dx * dx + dy * dy).sqrt());
    }
    let total_length = cum_lengths[n];

    let mut result = Vec::with_capacity(target);
    for j in 0..target {
        let target_len = total_length * j as f64 / target as f64;
        // Find segment containing target_len
        let mut seg_idx = 0;
        for k in 0..n {
            if cum_lengths[k + 1] >= target_len {
                seg_idx = k;
                break;
            }
        }
        let seg_start = cum_lengths[seg_idx];
        let seg_end = cum_lengths[seg_idx + 1];
        let seg_len = seg_end - seg_start;
        let frac = if seg_len > 1e-12 {
            (target_len - seg_start) / seg_len
        } else {
            0.0
        };
        let next_idx = (seg_idx + 1) % n;
        result.push([
            profile[seg_idx][0] * (1.0 - frac) + profile[next_idx][0] * frac,
            profile[seg_idx][1] * (1.0 - frac) + profile[next_idx][1] * frac,
        ]);
    }
    result
}

/// Generate a mesh by lofting between multiple 2D profiles at specified Z positions
///
/// Profiles are resampled to a common point count, then interpolated between sections.
pub fn generate_loft(profiles: &[Vec<[f64; 2]>], positions: &[f64], segments: u32) -> TriMesh {
    let target_count = profiles
        .iter()
        .map(|p| p.len())
        .max()
        .unwrap_or(3)
        .max(segments as usize);

    let resampled: Vec<Vec<[f64; 2]>> = profiles
        .iter()
        .map(|p| resample_profile(p, target_count))
        .collect();

    let n = target_count;
    let num_sections = resampled.len();
    let seg = segments as usize;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // Generate interpolated rings between each pair of sections
    for section in 0..num_sections - 1 {
        let z0 = positions[section];
        let z1 = positions[section + 1];
        let p0 = &resampled[section];
        let p1 = &resampled[section + 1];

        let num_steps = if section == num_sections - 2 {
            seg + 1
        } else {
            seg
        };

        for s in 0..num_steps {
            let frac = s as f64 / seg as f64;
            let z = z0 + (z1 - z0) * frac;
            for i in 0..n {
                let x = p0[i][0] * (1.0 - frac) + p1[i][0] * frac;
                let y = p0[i][1] * (1.0 - frac) + p1[i][1] * frac;
                vertices.push([x, y, z]);
            }
        }
    }

    // Total rings
    let total_rings = (num_sections - 1) * seg + 1;

    // Connect adjacent rings
    for r in 0..total_rings - 1 {
        for i in 0..n {
            let next_i = (i + 1) % n;
            let a = r * n + i;
            let b = r * n + next_i;
            let c = (r + 1) * n + i;
            let d = (r + 1) * n + next_i;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }

    // Bottom cap
    for i in 1..n - 1 {
        triangles.push([0, i + 1, i]);
    }
    // Top cap
    let base = (total_rings - 1) * n;
    for i in 1..n - 1 {
        triangles.push([base, base + i, base + i + 1]);
    }

    TriMesh {
        vertices,
        triangles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_mesh() {
        let mesh = generate_box(10.0, 20.0, 30.0);
        assert_eq!(mesh.vertex_count(), 8);
        assert_eq!(mesh.triangle_count(), 12);

        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 1e-10);
        assert!((max[0] - 5.0).abs() < 1e-10);
        assert!((min[1] - (-10.0)).abs() < 1e-10);
        assert!((max[1] - 10.0).abs() < 1e-10);
        assert!((min[2] - (-15.0)).abs() < 1e-10);
        assert!((max[2] - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_sphere_mesh() {
        let mesh = generate_sphere(5.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        for i in 0..3 {
            assert!((min[i] - (-5.0)).abs() < 0.5);
            assert!((max[i] - 5.0).abs() < 0.5);
        }
    }

    #[test]
    fn test_cylinder_mesh() {
        let mesh = generate_cylinder(5.0, 20.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        assert!((min[2] - (-10.0)).abs() < 1e-10);
        assert!((max[2] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_cone_mesh() {
        let mesh = generate_cone(5.0, 0.0, 10.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_prism_mesh() {
        let mesh = generate_prism(6, 5.0, 10.0);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_torus_mesh() {
        let mesh = generate_torus(20.0, 3.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_to_flat_arrays() {
        let mesh = generate_box(10.0, 10.0, 10.0);
        let (positions, normals) = mesh.to_flat_arrays();
        // 12 triangles x 3 vertices x 3 coordinates
        assert_eq!(positions.len(), 12 * 3 * 3);
        assert_eq!(normals.len(), 12 * 3 * 3);
    }

    #[test]
    fn test_sweep_helix() {
        // Square profile swept along a helix
        let profile = vec![[0.5, 0.0], [0.0, 0.5], [-0.5, 0.0], [0.0, -0.5]];
        let path = PathNode::Helix {
            radius: 5.0,
            pitch: 2.0,
            turns: 1.0,
        };
        let mesh = generate_sweep(&profile, &path, 32);
        // (32+1) rings * 4 profile points = 132 vertices
        assert_eq!(mesh.vertex_count(), 33 * 4);
        assert!(mesh.triangle_count() > 0);
        // No NaN vertices
        for v in &mesh.vertices {
            assert!(!v[0].is_nan() && !v[1].is_nan() && !v[2].is_nan());
        }
    }

    #[test]
    fn test_sweep_arc() {
        let profile = vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]];
        let path = PathNode::Arc {
            radius: 10.0,
            angle_rad: std::f64::consts::PI,
        };
        let mesh = generate_sweep(&profile, &path, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
        for v in &mesh.vertices {
            assert!(!v[0].is_nan() && !v[1].is_nan() && !v[2].is_nan());
        }
    }

    #[test]
    fn test_loft_two_circles() {
        // Approximate circles as octagons
        let n = 8;
        let circle = |r: f64| -> Vec<[f64; 2]> {
            (0..n)
                .map(|i| {
                    let theta = 2.0 * PI * i as f64 / n as f64;
                    [r * theta.cos(), r * theta.sin()]
                })
                .collect()
        };
        let profiles = vec![circle(5.0), circle(3.0)];
        let positions = vec![0.0, 10.0];
        let mesh = generate_loft(&profiles, &positions, 8);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
        // Check Z range
        let (min, max) = mesh.bounding_box();
        assert!((min[2] - 0.0).abs() < 0.01);
        assert!((max[2] - 10.0).abs() < 0.01);
    }
}

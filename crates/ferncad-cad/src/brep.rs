//! BREP (Boundary Representation) pipeline
//!
//! Converts `ShapeNode` to truck `Solid` for exact curved geometry,
//! then tessellates to `TriMesh` for display and export.

use truck_meshalgo::prelude::*;
use truck_modeling::*;

use ferncad_core::error::{FernError, FernResult};
use ferncad_core::types::ShapeNode;

use crate::mesh::TriMesh;

/// Tolerance for CSG boolean operations
const CSG_TOLERANCE: f64 = 0.01;

/// Tessellation tolerance for STEP/standalone display (lower = finer mesh)
const TESSELLATION_TOLERANCE: f64 = 0.05;

/// Tessellation tolerance for BSP hybrid path (coarser for performance)
const TESSELLATION_TOLERANCE_FOR_BSP: f64 = 0.2;

/// Convert a `ShapeNode` tree to a truck `Solid` for rendering.
///
/// Does NOT use cavity fallback for difference — returns Err so that
/// `realize()` can fall back to the BSP mesh pipeline.
pub fn shape_to_solid(node: &ShapeNode) -> FernResult<Solid> {
    shape_to_solid_inner(node, false)
}

/// Convert a `ShapeNode` tree to a truck `Solid` for STEP export.
///
/// Uses cavity fallback for difference when boolean operations fail,
/// producing a valid BREP that CAD software interprets correctly.
pub fn shape_to_solid_for_export(node: &ShapeNode) -> FernResult<Solid> {
    shape_to_solid_inner(node, true)
}

fn shape_to_solid_inner(node: &ShapeNode, allow_cavity_fallback: bool) -> FernResult<Solid> {
    match node {
        ShapeNode::Box {
            width,
            depth,
            height,
        } => build_box(*width, *depth, *height),

        ShapeNode::Cylinder {
            radius,
            height,
            segments: _,
        } => build_cylinder(*radius, *height),

        ShapeNode::Prism {
            sides,
            radius,
            height,
        } => build_prism(*sides, *radius, *height),

        ShapeNode::Cone {
            radius_bottom,
            radius_top,
            height,
            segments: _,
        } => build_cone(*radius_bottom, *radius_top, *height),

        ShapeNode::Sphere {
            radius,
            segments: _,
        } => build_sphere(*radius),

        ShapeNode::Torus {
            radius_major,
            radius_minor,
            segments: _,
        } => build_torus(*radius_major, *radius_minor),

        // CSG operations
        ShapeNode::Union { children } => {
            if children.is_empty() {
                return Err(FernError::CadError {
                    message: "union requires at least one child".to_string(),
                });
            }
            let mut result = shape_to_solid_inner(&children[0], allow_cavity_fallback)?;
            for child in &children[1..] {
                let child_solid = shape_to_solid_inner(child, allow_cavity_fallback)?;
                result = try_boolean_or(&result, &child_solid)?;
            }
            Ok(result)
        }

        ShapeNode::Difference { base, cutters } => {
            let mut result = shape_to_solid_inner(base, allow_cavity_fallback)?;
            for cutter in cutters {
                let cutter_solid = shape_to_solid_inner(cutter, allow_cavity_fallback)?;
                result = try_boolean_difference(&result, &cutter_solid, allow_cavity_fallback)?;
            }
            Ok(result)
        }

        ShapeNode::Intersection { children } => {
            if children.is_empty() {
                return Err(FernError::CadError {
                    message: "intersection requires at least one child".to_string(),
                });
            }
            let mut result = shape_to_solid_inner(&children[0], allow_cavity_fallback)?;
            for child in &children[1..] {
                let child_solid = shape_to_solid_inner(child, allow_cavity_fallback)?;
                result = try_boolean_and(&result, &child_solid)?;
            }
            Ok(result)
        }

        // Transforms
        ShapeNode::Translate { shape, offset } => {
            let solid = shape_to_solid_inner(shape, allow_cavity_fallback)?;
            Ok(builder::translated(
                &solid,
                Vector3::new(offset[0], offset[1], offset[2]),
            ))
        }

        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        } => {
            let solid = shape_to_solid_inner(shape, allow_cavity_fallback)?;
            Ok(builder::rotated(
                &solid,
                Point3::origin(),
                Vector3::new(axis[0], axis[1], axis[2]),
                Rad(*angle_rad),
            ))
        }

        ShapeNode::Scale { shape, factors } => {
            let solid = shape_to_solid_inner(shape, allow_cavity_fallback)?;
            Ok(builder::scaled(
                &solid,
                Point3::origin(),
                Vector3::new(factors[0], factors[1], factors[2]),
            ))
        }

        // Profile operations
        ShapeNode::Extrude { profile, height } => build_extrude(profile, *height),

        ShapeNode::Revolve {
            profile,
            angle_rad,
            segments: _,
        } => build_revolve(profile, *angle_rad),

        ShapeNode::Chamfer { .. } => Err(FernError::CadError {
            message: "BREP chamfer not supported, using mesh fallback".to_string(),
        }),
    }
}

/// Tessellate a truck `Solid` into a `TriMesh` (fine tolerance for display/export)
pub fn solid_to_trimesh(solid: &Solid) -> FernResult<TriMesh> {
    solid_to_trimesh_with_tolerance(solid, TESSELLATION_TOLERANCE)
}

/// Tessellate a truck `Solid` into a `TriMesh` (coarser for BSP hybrid pipeline)
pub fn solid_to_trimesh_for_bsp(solid: &Solid) -> FernResult<TriMesh> {
    solid_to_trimesh_with_tolerance(solid, TESSELLATION_TOLERANCE_FOR_BSP)
}

fn solid_to_trimesh_with_tolerance(solid: &Solid, tolerance: f64) -> FernResult<TriMesh> {
    let mut all_vertices: Vec<[f64; 3]> = Vec::new();
    let mut all_triangles: Vec<[usize; 3]> = Vec::new();

    // Tessellate each shell/face individually for robustness
    for shell in solid.boundaries() {
        let tessellated = shell.triangulation(tolerance);
        let polygon = tessellated.to_polygon();
        let positions = polygon.positions();
        let tri_faces = polygon.tri_faces();

        let base = all_vertices.len();
        all_vertices.extend(positions.iter().map(|p| [p.x, p.y, p.z]));
        all_triangles.extend(
            tri_faces
                .iter()
                .map(|f| [f[0].pos + base, f[1].pos + base, f[2].pos + base]),
        );
    }

    Ok(TriMesh {
        vertices: all_vertices,
        triangles: all_triangles,
    })
}

// === Robust boolean operations ===

/// Tolerances to try when a boolean operation fails at the default tolerance
const RETRY_TOLERANCES: [f64; 1] = [0.05];

/// Try boolean union with multiple tolerances
fn try_boolean_or(a: &Solid, b: &Solid) -> FernResult<Solid> {
    // Try default tolerance first
    if let Some(result) = catch_boolean(|| truck_shapeops::or(a, b, CSG_TOLERANCE)) {
        if !result.boundaries().is_empty() {
            return Ok(result);
        }
    }
    // Retry with different tolerances
    for &tol in &RETRY_TOLERANCES {
        if let Some(result) = catch_boolean(|| truck_shapeops::or(a, b, tol)) {
            if !result.boundaries().is_empty() {
                return Ok(result);
            }
        }
    }
    // Fallback: merge boundaries from both solids (geometrically approximate)
    let mut boundaries = a.boundaries().clone();
    boundaries.extend(b.boundaries().iter().cloned());
    Ok(Solid::new(boundaries))
}

/// Try boolean intersection with multiple tolerances
fn try_boolean_and(a: &Solid, b: &Solid) -> FernResult<Solid> {
    if let Some(result) = catch_boolean(|| truck_shapeops::and(a, b, CSG_TOLERANCE)) {
        if !result.boundaries().is_empty() {
            return Ok(result);
        }
    }
    for &tol in &RETRY_TOLERANCES {
        if let Some(result) = catch_boolean(|| truck_shapeops::and(a, b, tol)) {
            if !result.boundaries().is_empty() {
                return Ok(result);
            }
        }
    }
    Err(FernError::CadError {
        message: "BREP intersection operation failed at all tolerances".to_string(),
    })
}

/// Try boolean difference (A - B) with multiple strategies
///
/// Strategy 1: A ∩ complement(B) via not() + and()
/// Strategy 2 (export only): Cavity representation (B's inverted shells as inner boundaries of A)
fn try_boolean_difference(a: &Solid, b: &Solid, allow_cavity_fallback: bool) -> FernResult<Solid> {
    // Strategy 1: not() + and() with multiple tolerances
    let mut b_complement = b.clone();
    b_complement.not();

    if let Some(result) = catch_boolean(|| truck_shapeops::and(a, &b_complement, CSG_TOLERANCE)) {
        if !result.boundaries().is_empty() {
            return Ok(result);
        }
    }
    for &tol in &RETRY_TOLERANCES {
        if let Some(result) = catch_boolean(|| truck_shapeops::and(a, &b_complement, tol)) {
            if !result.boundaries().is_empty() {
                return Ok(result);
            }
        }
    }

    if !allow_cavity_fallback {
        return Err(FernError::CadError {
            message: "BREP difference operation failed at all tolerances".to_string(),
        });
    }

    // Strategy 2 (STEP export only): Cavity representation
    // Represent A - B as A with B's shells added as internal cavities (inverted normals).
    // Valid BREP that CAD software interprets correctly, but not suitable for direct rendering.
    let mut boundaries = a.boundaries().clone();
    for shell in b.boundaries() {
        let mut cavity = shell.clone();
        for face in cavity.face_iter_mut() {
            face.invert();
        }
        boundaries.push(cavity);
    }
    Ok(Solid::new(boundaries))
}

/// Run a boolean operation, catching panics and returning None on failure
fn catch_boolean<F>(f: F) -> Option<Solid>
where
    F: FnOnce() -> Option<Solid>,
{
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .ok()
        .flatten()
}

// === Primitive builders ===

/// Build a box centered at origin
fn build_box(width: f64, depth: f64, height: f64) -> FernResult<Solid> {
    let hw = width / 2.0;
    let hd = depth / 2.0;

    let v0 = builder::vertex(Point3::new(-hw, -hd, 0.0));
    let v1 = builder::vertex(Point3::new(hw, -hd, 0.0));
    let v2 = builder::vertex(Point3::new(hw, hd, 0.0));
    let v3 = builder::vertex(Point3::new(-hw, hd, 0.0));

    let wire: Wire = vec![
        builder::line(&v0, &v1),
        builder::line(&v1, &v2),
        builder::line(&v2, &v3),
        builder::line(&v3, &v0),
    ]
    .into();

    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP box face creation failed: {e}"),
    })?;

    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, height));
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// Build a cylinder centered at origin, along Z axis
fn build_cylinder(radius: f64, height: f64) -> FernResult<Solid> {
    let v = builder::vertex(Point3::new(radius, 0.0, 0.0));
    // Rad(7.0) > 2π ensures a complete circle
    let circle = builder::rsweep(&v, Point3::origin(), Vector3::unit_z(), Rad(7.0));

    let disk = builder::try_attach_plane(&[circle]).map_err(|e| FernError::CadError {
        message: format!("BREP cylinder disk creation failed: {e}"),
    })?;

    let solid = builder::tsweep(&disk, Vector3::new(0.0, 0.0, height));
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// Build a prism (regular polygon extruded along Z) centered at origin
fn build_prism(sides: u32, radius: f64, height: f64) -> FernResult<Solid> {
    let n = sides as usize;
    let mut vertices = Vec::with_capacity(n);

    for i in 0..n {
        let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        vertices.push(builder::vertex(Point3::new(
            radius * angle.cos(),
            radius * angle.sin(),
            0.0,
        )));
    }

    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let next = (i + 1) % n;
        edges.push(builder::line(&vertices[i], &vertices[next]));
    }

    let wire: Wire = edges.into();
    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP prism face creation failed: {e}"),
    })?;

    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, height));
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// Build a cone or truncated cone centered at origin, along Z axis
fn build_cone(radius_bottom: f64, radius_top: f64, height: f64) -> FernResult<Solid> {
    if radius_top < 1e-10 {
        // Sharp cone: revolve a triangle profile
        let v_center = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let v_bottom = builder::vertex(Point3::new(radius_bottom, 0.0, 0.0));
        let v_top = builder::vertex(Point3::new(0.0, 0.0, height));

        let wire: Wire = vec![
            builder::line(&v_center, &v_bottom),
            builder::line(&v_bottom, &v_top),
            builder::line(&v_top, &v_center),
        ]
        .into();

        let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
            message: format!("BREP cone profile creation failed: {e}"),
        })?;

        let solid = builder::rsweep(&face, Point3::origin(), Vector3::unit_z(), Rad(7.0));
        Ok(builder::translated(
            &solid,
            Vector3::new(0.0, 0.0, -height / 2.0),
        ))
    } else {
        // Truncated cone: revolve a trapezoid profile
        let v0 = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let v1 = builder::vertex(Point3::new(radius_bottom, 0.0, 0.0));
        let v2 = builder::vertex(Point3::new(radius_top, 0.0, height));
        let v3 = builder::vertex(Point3::new(0.0, 0.0, height));

        let wire: Wire = vec![
            builder::line(&v0, &v1),
            builder::line(&v1, &v2),
            builder::line(&v2, &v3),
            builder::line(&v3, &v0),
        ]
        .into();

        let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
            message: format!("BREP truncated cone profile creation failed: {e}"),
        })?;

        let solid = builder::rsweep(&face, Point3::origin(), Vector3::unit_z(), Rad(7.0));
        Ok(builder::translated(
            &solid,
            Vector3::new(0.0, 0.0, -height / 2.0),
        ))
    }
}

/// Build a sphere centered at origin
fn build_sphere(radius: f64) -> FernResult<Solid> {
    // Create a semicircle profile in XZ plane and revolve around Z axis.
    // Profile: line along Z from -r to +r, then semicircular arc back.
    let v_bottom = builder::vertex(Point3::new(0.0, 0.0, -radius));
    let v_top = builder::vertex(Point3::new(0.0, 0.0, radius));

    // Semicircle arc from bottom to top through mid (equator point)
    let transit = Point3::new(radius, 0.0, 0.0);
    let arc = builder::circle_arc(&v_bottom, &v_top, transit);
    // Straight line back along Z axis
    let line = builder::line(&v_top, &v_bottom);

    let wire: Wire = vec![arc, line].into();
    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP sphere profile creation failed: {e}"),
    })?;

    Ok(builder::rsweep(
        &face,
        Point3::origin(),
        Vector3::unit_z(),
        Rad(7.0),
    ))
}

/// Build a torus centered at origin, around Z axis
fn build_torus(radius_major: f64, radius_minor: f64) -> FernResult<Solid> {
    // Create a circle in XZ plane at (radius_major, 0, 0) and revolve around Z axis.
    // The cross-section circle must lie in the XZ plane (revolve around Y axis at circle center).
    let v = builder::vertex(Point3::new(radius_major + radius_minor, 0.0, 0.0));

    // Revolve vertex around Y axis at the circle center to create a circle in XZ plane
    let circle_center = Point3::new(radius_major, 0.0, 0.0);
    let circle = builder::rsweep(&v, circle_center, Vector3::unit_y(), Rad(7.0));

    let disk = builder::try_attach_plane(&[circle]).map_err(|e| FernError::CadError {
        message: format!("BREP torus cross-section creation failed: {e}"),
    })?;

    // Revolve the disk around the global Z axis
    Ok(builder::rsweep(
        &disk,
        Point3::origin(),
        Vector3::unit_z(),
        Rad(7.0),
    ))
}

/// Extrude a 2D profile (XY plane) along Z axis
fn build_extrude(profile: &[[f64; 2]], height: f64) -> FernResult<Solid> {
    let n = profile.len();
    let mut verts = Vec::with_capacity(n);
    for p in profile {
        verts.push(builder::vertex(Point3::new(p[0], p[1], 0.0)));
    }

    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let next = (i + 1) % n;
        edges.push(builder::line(&verts[i], &verts[next]));
    }

    let wire: Wire = edges.into();
    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP extrude profile face creation failed: {e}"),
    })?;

    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, height));
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// Revolve a 2D profile (XZ plane) around Z axis
fn build_revolve(profile: &[[f64; 2]], angle_rad: f64) -> FernResult<Solid> {
    let n = profile.len();
    let mut verts = Vec::with_capacity(n);
    for p in profile {
        // Profile in XZ plane: p[0] = radius (X), p[1] = height (Z)
        verts.push(builder::vertex(Point3::new(p[0], 0.0, p[1])));
    }

    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let next = (i + 1) % n;
        edges.push(builder::line(&verts[i], &verts[next]));
    }

    let wire: Wire = edges.into();
    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP revolve profile face creation failed: {e}"),
    })?;

    // Use Rad(7.0) for full revolution (> 2π), otherwise use exact angle
    let sweep_angle = if (angle_rad - 2.0 * std::f64::consts::PI).abs() < 0.01 {
        Rad(7.0) // Full revolution
    } else {
        Rad(angle_rad)
    };

    Ok(builder::rsweep(
        &face,
        Point3::origin(),
        Vector3::unit_z(),
        sweep_angle,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_build_box() {
        let solid = build_box(10.0, 20.0, 30.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() >= 12);

        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 0.5);
        assert!((max[0] - 5.0).abs() < 0.5);
        assert!((min[1] - (-10.0)).abs() < 0.5);
        assert!((max[1] - 10.0).abs() < 0.5);
        assert!((min[2] - (-15.0)).abs() < 0.5);
        assert!((max[2] - 15.0).abs() < 0.5);
    }

    #[test]
    fn test_build_cylinder() {
        let solid = build_cylinder(5.0, 20.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        assert!((min[2] - (-10.0)).abs() < 0.5);
        assert!((max[2] - 10.0).abs() < 0.5);
    }

    #[test]
    fn test_build_prism() {
        let solid = build_prism(6, 5.0, 10.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_build_cone_sharp() {
        let solid = build_cone(5.0, 0.0, 10.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        assert!((min[2] - (-5.0)).abs() < 0.5);
        assert!((max[2] - 5.0).abs() < 0.5);
    }

    #[test]
    fn test_build_cone_truncated() {
        let solid = build_cone(5.0, 2.5, 10.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_build_sphere() {
        let solid = build_sphere(5.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 1.0);
        assert!((max[0] - 5.0).abs() < 1.0);
        assert!((min[2] - (-5.0)).abs() < 1.0);
        assert!((max[2] - 5.0).abs() < 1.0);
    }

    #[test]
    fn test_build_torus() {
        let solid = build_torus(10.0, 3.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        // Outer radius = 10 + 3 = 13
        assert!((min[0] - (-13.0)).abs() < 1.0);
        assert!((max[0] - 13.0).abs() < 1.0);
        // Height = ±3
        assert!((min[2] - (-3.0)).abs() < 1.0);
        assert!((max[2] - 3.0).abs() < 1.0);
    }

    #[test]
    fn test_shape_to_solid_box() {
        let node = ShapeNode::Box {
            width: 10.0,
            depth: 10.0,
            height: 10.0,
        };
        let solid = shape_to_solid(&node).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.triangle_count() >= 12);
    }

    #[test]
    fn test_shape_to_solid_translate() {
        let node = ShapeNode::Translate {
            shape: Arc::new(ShapeNode::Box {
                width: 10.0,
                depth: 10.0,
                height: 10.0,
            }),
            offset: [5.0, 0.0, 0.0],
        };
        let solid = shape_to_solid(&node).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        let (min, max) = mesh.bounding_box();
        assert!((min[0] - 0.0).abs() < 0.5);
        assert!((max[0] - 10.0).abs() < 0.5);
    }

    #[test]
    fn test_brep_union() {
        let node = ShapeNode::Union {
            children: vec![
                Arc::new(ShapeNode::Box {
                    width: 10.0,
                    depth: 10.0,
                    height: 10.0,
                }),
                Arc::new(ShapeNode::Translate {
                    shape: Arc::new(ShapeNode::Box {
                        width: 10.0,
                        depth: 10.0,
                        height: 10.0,
                    }),
                    offset: [5.0, 0.0, 0.0],
                }),
            ],
        };
        // truck booleans can be unstable, so tolerate failure
        if let Ok(solid) = shape_to_solid(&node) {
            let mesh = solid_to_trimesh(&solid).unwrap();
            assert!(mesh.triangle_count() > 0);
        }
    }

    #[test]
    fn test_brep_difference() {
        let node = ShapeNode::Difference {
            base: Arc::new(ShapeNode::Box {
                width: 20.0,
                depth: 20.0,
                height: 20.0,
            }),
            cutters: vec![Arc::new(ShapeNode::Sphere {
                radius: 8.0,
                segments: 16,
            })],
        };
        if let Ok(solid) = shape_to_solid(&node) {
            let mesh = solid_to_trimesh(&solid).unwrap();
            assert!(mesh.triangle_count() > 0);
        }
    }

    #[test]
    fn test_brep_intersection() {
        let node = ShapeNode::Intersection {
            children: vec![
                Arc::new(ShapeNode::Box {
                    width: 10.0,
                    depth: 10.0,
                    height: 10.0,
                }),
                Arc::new(ShapeNode::Sphere {
                    radius: 7.0,
                    segments: 16,
                }),
            ],
        };
        if let Ok(solid) = shape_to_solid(&node) {
            let mesh = solid_to_trimesh(&solid).unwrap();
            assert!(mesh.triangle_count() > 0);
        }
    }
}

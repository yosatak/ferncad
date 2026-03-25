//! ShapeNode to TriMesh conversion
//!
//! Hybrid pipeline: primitives use BREP tessellation for smooth curves,
//! CSG operations use BSP on the tessellated meshes for reliable booleans.

use ferncad_core::error::{FernError, FernResult};
use ferncad_core::types::ShapeNode;

use crate::brep;
use crate::bsp;
use crate::mesh::TriMesh;
use crate::primitives;
use crate::transform;

/// Minimum segments for curved primitives used in BSP CSG.
/// 32 gives a good balance: visually smooth, and BSP stays fast.
/// (16 = noticeably angular, 48+ = slow BSP on complex CSG)
const BSP_SEGMENTS: u32 = 32;

/// Convert a ShapeNode tree to a TriMesh
///
/// Uses BREP for standalone primitives/transforms, BSP for CSG operations.
pub fn realize(node: &ShapeNode) -> FernResult<TriMesh> {
    match node {
        // CSG operations: go directly to BSP (BREP booleans are unreliable/slow)
        ShapeNode::Union { .. } | ShapeNode::Difference { .. } | ShapeNode::Intersection { .. } => {
            realize_hybrid(node)
        }

        // Primitives and transforms: try BREP first for smooth tessellation
        _ => {
            let brep_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                brep::shape_to_solid(node).and_then(|solid| brep::solid_to_trimesh(&solid))
            }));

            match brep_result {
                Ok(Ok(mesh)) if mesh.triangle_count() > 0 => Ok(mesh),
                _ => realize_hybrid(node),
            }
        }
    }
}

/// BSP-based realization with enhanced segment count for curved primitives
fn realize_hybrid(node: &ShapeNode) -> FernResult<TriMesh> {
    match node {
        // === Primitives: use max(segments, BSP_SEGMENTS) for smooth curves ===
        ShapeNode::Box {
            width,
            depth,
            height,
        } => Ok(primitives::generate_box(*width, *depth, *height)),

        ShapeNode::Sphere { radius, segments } => Ok(primitives::generate_sphere(
            *radius,
            (*segments).max(BSP_SEGMENTS),
        )),

        ShapeNode::Cylinder {
            radius,
            height,
            segments,
        } => Ok(primitives::generate_cylinder(
            *radius,
            *height,
            (*segments).max(BSP_SEGMENTS),
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
            (*segments).max(BSP_SEGMENTS),
        )),

        ShapeNode::Torus {
            radius_major,
            radius_minor,
            segments,
        } => Ok(primitives::generate_torus(
            *radius_major,
            *radius_minor,
            (*segments).max(BSP_SEGMENTS),
        )),

        ShapeNode::Prism {
            sides,
            radius,
            height,
        } => Ok(primitives::generate_prism(*sides, *radius, *height)),

        // === CSG: BSP on realized meshes ===
        ShapeNode::Union { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize_hybrid(&children[0])?;
            for child in &children[1..] {
                let child_mesh = realize_hybrid(child)?;
                result = bsp::csg_union(&result, &child_mesh);
            }
            Ok(result)
        }

        ShapeNode::Difference { base, cutters } => {
            let mut result = realize_hybrid(base)?;
            for cutter in cutters {
                let cutter_mesh = realize_hybrid(cutter)?;
                result = bsp::csg_difference(&result, &cutter_mesh);
            }
            Ok(result)
        }

        ShapeNode::Intersection { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize_hybrid(&children[0])?;
            for child in &children[1..] {
                let child_mesh = realize_hybrid(child)?;
                result = bsp::csg_intersection(&result, &child_mesh);
            }
            Ok(result)
        }

        // === Transforms ===
        ShapeNode::Translate { shape, offset } => {
            let mut mesh = realize_hybrid(shape)?;
            transform::translate(&mut mesh, *offset);
            Ok(mesh)
        }

        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        } => {
            let mut mesh = realize_hybrid(shape)?;
            transform::rotate(&mut mesh, *axis, *angle_rad);
            Ok(mesh)
        }

        ShapeNode::Scale { shape, factors } => {
            let mut mesh = realize_hybrid(shape)?;
            transform::scale(&mut mesh, *factors);
            Ok(mesh)
        }
    }
}

/// Evaluate source code and convert to a mesh
///
/// Returns the mesh if the last expression evaluates to a Shape.
pub fn eval_and_realize(source: &str) -> FernResult<TriMesh> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator.eval_source(source)?;

    match result {
        ferncad_core::types::Value::Shape(node) => realize(&node),
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
}

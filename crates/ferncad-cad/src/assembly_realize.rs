//! Assembly mesh generation
//!
//! Converts an AssemblyDef into per-part TriMesh instances.

use ferncad_core::assembly::AssemblyDef;
use ferncad_core::error::{FernError, FernResult};

use crate::mesh::TriMesh;
use crate::realize;
use crate::transform;

/// Per-part mesh information
#[derive(Debug, Clone)]
pub struct PartMesh {
    /// Part instance name
    pub name: String,
    /// Mesh
    pub mesh: TriMesh,
    /// Color (RGB 0-1)
    pub color: [f64; 3],
}

/// Convert an assembly into per-part TriMesh instances
///
/// Realizes each part instance's ShapeNode,
/// applies the transform matrix, and attaches the part name and color.
pub fn realize_assembly(assembly: &AssemblyDef) -> FernResult<Vec<PartMesh>> {
    let mut result = Vec::new();

    for part_instance in &assembly.parts {
        // Realize ShapeNode if available
        let mesh = if let Some(shape) = &part_instance.shape {
            realize::realize(shape)?
        } else {
            // No shape -> empty mesh
            TriMesh::new()
        };

        // Apply transform matrix
        let mut mesh = mesh;
        let t = &part_instance.transform;

        // Translation from 4x4 matrix (column-major)
        let offset = [t[12], t[13], t[14]];
        if offset[0].abs() > 1e-10 || offset[1].abs() > 1e-10 || offset[2].abs() > 1e-10 {
            transform::translate(&mut mesh, offset);
        }

        result.push(PartMesh {
            name: part_instance.name.clone(),
            mesh,
            color: part_instance.color,
        });
    }

    Ok(result)
}

/// Evaluate source code and return meshes for an assembly or single shape
pub fn eval_and_realize_parts(source: &str) -> FernResult<Vec<PartMesh>> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator.eval_source(source)?;

    match result {
        ferncad_core::types::Value::Assembly(assembly) => realize_assembly(&assembly),
        ferncad_core::types::Value::Shape(node) => {
            let mesh = realize::realize(&node)?;
            Ok(vec![PartMesh {
                name: "shape".to_string(),
                mesh,
                color: [0.53, 0.53, 0.80],
            }])
        }
        _ => Err(FernError::CadError {
            message: format!(
                "cannot convert to mesh: the last expression did not return a shape or assembly (type: {})",
                result.type_name()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_shape_as_parts() {
        let parts = eval_and_realize_parts("(box :width 10 :depth 10 :height 10)").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].name, "shape");
        assert_eq!(parts[0].mesh.triangle_count(), 12);
    }

    #[test]
    fn test_assembly_basic() {
        let source = r#"
            (assembly "test-assembly"
              (place :part (box :width 10 :depth 10 :height 5) :as :plate)
              (place :part (cylinder :radius 2 :height 15) :as :pin
                     :at #p(0 0 5)))
        "#;
        let parts = eval_and_realize_parts(source).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name, "plate");
        assert_eq!(parts[1].name, "pin");
        // plate is box -> 12 triangles
        assert_eq!(parts[0].mesh.triangle_count(), 12);
        // pin is cylinder -> triangles > 0
        assert!(parts[1].mesh.triangle_count() > 0);
        // pin should be offset at Z=5
        let (min, _) = parts[1].mesh.bounding_box();
        assert!(
            min[2] > -3.0,
            "pin min z = {}, should be offset to Z=5",
            min[2]
        );
    }

    #[test]
    fn test_assembly_with_constraints() {
        let source = r#"
            (assembly "constrained"
              (place :part (box :width 10 :depth 10 :height 5) :as :base)
              (place :part (cylinder :radius 2 :height 10) :as :pin)
              (mate (face :pin :bottom) (face :base :top))
              (align-axis (axis :pin :center) (axis :base :hole)))
        "#;
        let parts = eval_and_realize_parts(source).unwrap();
        assert_eq!(parts.len(), 2);
    }
}

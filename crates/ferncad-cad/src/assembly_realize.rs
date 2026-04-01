//! Assembly mesh generation
//!
//! Converts an AssemblyDef into per-part TriMesh instances.

use ferncad_core::assembly::AssemblyDef;
use ferncad_core::error::{FernError, FernResult, SourceSpan};

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
    /// Source span (byte range) for editor↔viewer highlighting
    pub span: SourceSpan,
}

/// Convert an assembly into per-part TriMesh instances
///
/// Realizes each part instance's ShapeNode,
/// applies the transform matrix, and attaches the part name and color.
pub fn realize_assembly(assembly: &AssemblyDef) -> FernResult<Vec<PartMesh>> {
    realize_assembly_with_resolution(assembly, 32)
}

/// Convert an assembly into per-part TriMesh instances with specified resolution
pub fn realize_assembly_with_resolution(
    assembly: &AssemblyDef,
    min_segments: u32,
) -> FernResult<Vec<PartMesh>> {
    let mut result = Vec::new();
    let mut cache = realize::RealizeCache::new();

    for part_instance in &assembly.parts {
        let (mesh, span) = if let Some(tracked) = &part_instance.shape {
            let m = realize::realize_with_cache(&tracked.node, min_segments, &mut cache)?;
            (m, tracked.span.clone())
        } else {
            // No shape -> empty mesh
            (TriMesh::new(), SourceSpan::dummy())
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
            span,
        });
    }

    Ok(result)
}

/// Evaluate source code and return meshes for an assembly or single shape
///
/// Reads `*resolution*` from the evaluator environment to control mesh quality.
pub fn eval_and_realize_parts(source: &str) -> FernResult<Vec<PartMesh>> {
    eval_and_realize_parts_with_files(source, &[])
}

/// Evaluate source code with project file context and return meshes.
///
/// `files` is a list of `(filename, source)` pairs that are registered as
/// user modules for `require` resolution.
pub fn eval_and_realize_parts_with_files(
    source: &str,
    files: &[(String, String)],
) -> FernResult<Vec<PartMesh>> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    for (name, src) in files {
        evaluator
            .module_loader_mut()
            .add_user_module(name.clone(), src.clone());
    }
    let result = evaluator.eval_source(source)?;
    let resolution = evaluator.resolution();

    match result {
        ferncad_core::types::Value::Assembly(assembly) => {
            realize_assembly_with_resolution(&assembly, resolution)
        }
        ferncad_core::types::Value::Shape(tracked) => {
            let mesh = realize::realize_with_resolution(&tracked.node, resolution)?;
            Ok(vec![PartMesh {
                name: "shape".to_string(),
                mesh,
                color: [0.53, 0.53, 0.80],
                span: tracked.span.clone(),
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
        assert!(parts[0].mesh.triangle_count() >= 12);
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
        // plate is box
        assert!(parts[0].mesh.triangle_count() >= 12);
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

    #[test]
    fn test_single_shape_has_span() {
        let source = "(box :width 10 :depth 10 :height 10)";
        let parts = eval_and_realize_parts(source).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].span.start, 0);
        assert_eq!(parts[0].span.end, source.len());
    }

    #[test]
    fn test_assembly_parts_have_spans() {
        let source = r#"(assembly "bracket"
  (place :part (box :width 40 :depth 20 :height 5) :as :base)
  (place :part (cylinder :radius 3 :height 15) :as :pin
         :at #p(10 0 5)))"#;
        let parts = eval_and_realize_parts(source).unwrap();
        assert_eq!(parts.len(), 2);
        // Each part should have a non-dummy span
        for part in &parts {
            assert!(
                part.span.end > 0,
                "part '{}' should have non-dummy span",
                part.name
            );
        }
    }
}

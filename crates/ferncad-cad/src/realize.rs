//! ShapeNode → TriMesh 変換
//!
//! 評価器が生成した CSG ツリーを三角形メッシュに変換する。

use ferncad_core::error::{FernError, FernResult};
use ferncad_core::types::ShapeNode;

use crate::bsp;
use crate::mesh::TriMesh;
use crate::primitives;
use crate::transform;

/// ShapeNode ツリーを TriMesh に変換する
///
/// # Errors
///
/// CSG 演算に必要な子ノードがない場合にエラーを返す。
pub fn realize(node: &ShapeNode) -> FernResult<TriMesh> {
    match node {
        // === プリミティブ ===
        ShapeNode::Box {
            width,
            depth,
            height,
        } => Ok(primitives::generate_box(*width, *depth, *height)),

        ShapeNode::Sphere { radius, segments } => {
            Ok(primitives::generate_sphere(*radius, *segments))
        }

        ShapeNode::Cylinder {
            radius,
            height,
            segments,
        } => Ok(primitives::generate_cylinder(*radius, *height, *segments)),

        ShapeNode::Cone {
            radius_bottom,
            radius_top,
            height,
            segments,
        } => Ok(primitives::generate_cone(
            *radius_bottom,
            *radius_top,
            *height,
            *segments,
        )),

        ShapeNode::Torus {
            radius_major,
            radius_minor,
            segments,
        } => Ok(primitives::generate_torus(
            *radius_major,
            *radius_minor,
            *segments,
        )),

        ShapeNode::Prism {
            sides,
            radius,
            height,
        } => Ok(primitives::generate_prism(*sides, *radius, *height)),

        // === CSG 演算 ===
        ShapeNode::Union { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize(&children[0])?;
            for child in &children[1..] {
                let child_mesh = realize(child)?;
                result = bsp::csg_union(&result, &child_mesh);
            }
            Ok(result)
        }

        ShapeNode::Difference { base, cutters } => {
            let mut result = realize(base)?;
            for cutter in cutters {
                let cutter_mesh = realize(cutter)?;
                result = bsp::csg_difference(&result, &cutter_mesh);
            }
            Ok(result)
        }

        ShapeNode::Intersection { children } => {
            if children.is_empty() {
                return Ok(TriMesh::new());
            }
            let mut result = realize(&children[0])?;
            for child in &children[1..] {
                let child_mesh = realize(child)?;
                result = bsp::csg_intersection(&result, &child_mesh);
            }
            Ok(result)
        }

        // === 変換 ===
        ShapeNode::Translate { shape, offset } => {
            let mut mesh = realize(shape)?;
            transform::translate(&mut mesh, *offset);
            Ok(mesh)
        }

        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        } => {
            let mut mesh = realize(shape)?;
            transform::rotate(&mut mesh, *axis, *angle_rad);
            Ok(mesh)
        }

        ShapeNode::Scale { shape, factors } => {
            let mut mesh = realize(shape)?;
            transform::scale(&mut mesh, *factors);
            Ok(mesh)
        }
    }
}

/// ソースコードを評価し、メッシュに変換する
///
/// 最後の式が Shape を返した場合、そのメッシュを返す。
pub fn eval_and_realize(source: &str) -> FernResult<TriMesh> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator.eval_source(source)?;

    match result {
        ferncad_core::types::Value::Shape(node) => realize(&node),
        _ => Err(FernError::CadError {
            message: format!(
                "メッシュ化できません: 最後の式が形状を返しませんでした（型: {}）",
                result.type_name_ja()
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
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.vertex_count(), 8);
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
        assert!((min[0] - 0.0).abs() < 1e-10);
        assert!((max[0] - 10.0).abs() < 1e-10);
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
    fn test_eval_and_realize() {
        let mesh = eval_and_realize("(box :width 10 :depth 10 :height 10)").unwrap();
        assert_eq!(mesh.triangle_count(), 12);
    }

    #[test]
    fn test_eval_and_realize_with_defpart() {
        let source = r#"
            (defpart my-part
              "テスト"
              :params ((size :: length :default 10.0 :doc "s"))
              :body
              (box :width size :depth size :height size))
            (my-part :size 20.0)
        "#;
        let mesh = eval_and_realize(source).unwrap();
        assert_eq!(mesh.triangle_count(), 12);
        let (_min, max) = mesh.bounding_box();
        assert!((max[0] - 10.0).abs() < 1e-10);
    }
}

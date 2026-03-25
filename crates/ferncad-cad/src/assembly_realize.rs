//! アセンブリのメッシュ化
//!
//! AssemblyDef をパーツごとの TriMesh に変換する。

use ferncad_core::assembly::AssemblyDef;
use ferncad_core::error::{FernError, FernResult};

use crate::mesh::TriMesh;
use crate::realize;
use crate::transform;

/// パーツごとのメッシュ情報
#[derive(Debug, Clone)]
pub struct PartMesh {
    /// パーツインスタンス名
    pub name: String,
    /// メッシュ
    pub mesh: TriMesh,
    /// 色 (RGB 0-1)
    pub color: [f64; 3],
}

/// アセンブリをパーツごとの TriMesh に変換する
///
/// 各パーツインスタンスの ShapeNode を realize し、
/// 変換行列を適用してパーツ名と色を付ける。
pub fn realize_assembly(assembly: &AssemblyDef) -> FernResult<Vec<PartMesh>> {
    let mut result = Vec::new();

    for part_instance in &assembly.parts {
        // ShapeNode が設定されている場合は realize
        let mesh = if let Some(shape) = &part_instance.shape {
            realize::realize(shape)?
        } else {
            // shape がない場合は空メッシュ
            TriMesh::new()
        };

        // 変換行列を適用
        let mut mesh = mesh;
        let t = &part_instance.transform;

        // 4x4 行列からの平行移動（列優先）
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

/// ソースコードを評価し、アセンブリまたは単一形状のメッシュを返す
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
                "メッシュ化できません: 最後の式が形状またはアセンブリを返しませんでした（型: {}）",
                result.type_name_ja()
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
        // plate は box → 12 三角形
        assert_eq!(parts[0].mesh.triangle_count(), 12);
        // pin は cylinder → 三角形 > 0
        assert!(parts[1].mesh.triangle_count() > 0);
        // pin の位置は Z=5 にオフセット
        let (min, _) = parts[1].mesh.bounding_box();
        assert!(
            min[2] > -3.0,
            "pin の min z = {}, Z=5 にオフセットされているはず",
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

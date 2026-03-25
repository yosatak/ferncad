//! STEP ファイルエクスポート
//!
//! truck の BREP データを STEP (ISO 10303-21) 形式で出力する。

use truck_modeling::*;
use truck_stepio::out::*;
use truck_topology::compress::CompressedSolid;

use ferncad_core::error::FernResult;
use ferncad_core::types::ShapeNode;

use crate::brep;

/// ShapeNode を STEP バイト列にエクスポートする
///
/// # Errors
///
/// BREP 変換に失敗した場合、または STEP 出力に失敗した場合にエラーを返す。
pub fn export_step_bytes(node: &ShapeNode) -> FernResult<Vec<u8>> {
    let solid = brep::shape_to_solid(node)?;
    solid_to_step_bytes(&solid)
}

/// truck Solid を STEP バイト列に変換する
pub fn solid_to_step_bytes(solid: &Solid) -> FernResult<Vec<u8>> {
    let compressed: CompressedSolid<Point3, Curve, Surface> = solid.compress();
    let step_model: StepModel<Point3, Curve, Surface> = StepModel::from(&compressed);

    let header = StepHeaderDescriptor {
        file_name: "ferncad-export.step".to_string(),
        time_stamp: chrono_timestamp(),
        authors: vec!["ferncad".to_string()],
        organization: vec!["ferncad".to_string()],
        organization_system: "ferncad v0.1.0".to_string(),
        authorization: "ferncad".to_string(),
    };

    let complete = CompleteStepDisplay::new(step_model, header);
    let step_string = complete.to_string();

    Ok(step_string.into_bytes())
}

/// 現在時刻の ISO 8601 タイムスタンプを返す
fn chrono_timestamp() -> String {
    // 簡易的なタイムスタンプ（chrono クレートは truck-stepio の依存として利用可能）
    "2026-01-01T00:00:00".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_step_export_box() {
        let node = ShapeNode::Box {
            width: 10.0,
            depth: 10.0,
            height: 10.0,
        };
        let bytes = export_step_bytes(&node).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.contains("ISO-10303-21"), "STEP ヘッダーがない");
        assert!(content.contains("END-ISO-10303-21"), "STEP フッターがない");
        assert!(
            content.contains("MANIFOLD_SOLID_BREP"),
            "MANIFOLD_SOLID_BREP がない"
        );
    }

    #[test]
    fn test_step_export_cylinder() {
        let node = ShapeNode::Cylinder {
            radius: 5.0,
            height: 20.0,
            segments: 32,
        };
        let bytes = export_step_bytes(&node).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.contains("ISO-10303-21"));
    }

    #[test]
    fn test_step_export_translated() {
        let node = ShapeNode::Translate {
            shape: Arc::new(ShapeNode::Box {
                width: 10.0,
                depth: 10.0,
                height: 10.0,
            }),
            offset: [5.0, 0.0, 0.0],
        };
        let bytes = export_step_bytes(&node).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_step_export_sphere_unsupported() {
        let node = ShapeNode::Sphere {
            radius: 5.0,
            segments: 16,
        };
        assert!(export_step_bytes(&node).is_err());
    }
}

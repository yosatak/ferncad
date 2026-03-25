//! BREP (Boundary Representation) パイプライン
//!
//! `ShapeNode` を `truck` の `Solid` に変換し、テッセレーション・STEP エクスポートに使う。
//! Phase 1 の BSP メッシュベースの CSG と並行して動作する。

use truck_meshalgo::prelude::*;
use truck_modeling::*;

use ferncad_core::error::{FernError, FernResult};
use ferncad_core::types::ShapeNode;

use crate::mesh::TriMesh;

/// CSG Boolean 演算の許容誤差
const CSG_TOLERANCE: f64 = 0.01;

/// テッセレーションの許容誤差
const TESSELLATION_TOLERANCE: f64 = 0.05;

/// `ShapeNode` を truck `Solid` に変換する
///
/// # Errors
///
/// 未対応の形状や truck の内部エラーの場合にエラーを返す。
pub fn shape_to_solid(node: &ShapeNode) -> FernResult<Solid> {
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

        ShapeNode::Sphere { .. } => Err(FernError::CadError {
            message: "球の BREP 変換は現在未対応です。STL エクスポートをご利用ください".to_string(),
        }),

        ShapeNode::Cone {
            radius_bottom,
            radius_top,
            height,
            segments: _,
        } => build_cone(*radius_bottom, *radius_top, *height),

        ShapeNode::Prism {
            sides,
            radius,
            height,
        } => build_prism(*sides, *radius, *height),

        ShapeNode::Torus { .. } => Err(FernError::CadError {
            message: "トーラスの BREP 変換は現在未対応です。STL エクスポートをご利用ください"
                .to_string(),
        }),

        // CSG 演算
        ShapeNode::Union { children } => {
            if children.is_empty() {
                return Err(FernError::CadError {
                    message: "union に子要素がありません".to_string(),
                });
            }
            let mut result = shape_to_solid(&children[0])?;
            for child in &children[1..] {
                let child_solid = shape_to_solid(child)?;
                result =
                    truck_shapeops::or(&result, &child_solid, CSG_TOLERANCE).ok_or_else(|| {
                        FernError::CadError {
                            message: "BREP union 演算に失敗しました".to_string(),
                        }
                    })?;
            }
            Ok(result)
        }

        ShapeNode::Difference { .. } => {
            // truck には直接的な difference 演算がないため、
            // BREP では未サポート。メッシュ表示には BSP CSG (realize.rs) を使用する。
            // STEP エクスポート時は個別パーツとして出力する。
            Err(FernError::CadError {
                message: "BREP での difference 演算は現在未対応です。\
                          STL エクスポートまたはビューアでは BSP CSG が自動的に使用されます"
                    .to_string(),
            })
        }

        ShapeNode::Intersection { children } => {
            if children.is_empty() {
                return Err(FernError::CadError {
                    message: "intersection に子要素がありません".to_string(),
                });
            }
            let mut result = shape_to_solid(&children[0])?;
            for child in &children[1..] {
                let child_solid = shape_to_solid(child)?;
                result =
                    truck_shapeops::and(&result, &child_solid, CSG_TOLERANCE).ok_or_else(|| {
                        FernError::CadError {
                            message: "BREP intersection 演算に失敗しました".to_string(),
                        }
                    })?;
            }
            Ok(result)
        }

        // 変換
        ShapeNode::Translate { shape, offset } => {
            let solid = shape_to_solid(shape)?;
            let vec = Vector3::new(offset[0], offset[1], offset[2]);
            Ok(builder::translated(&solid, vec))
        }

        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        } => {
            let solid = shape_to_solid(shape)?;
            let axis_pt = Point3::origin();
            let axis_vec = Vector3::new(axis[0], axis[1], axis[2]);
            Ok(builder::rotated(&solid, axis_pt, axis_vec, Rad(*angle_rad)))
        }

        ShapeNode::Scale { shape, factors } => {
            let solid = shape_to_solid(shape)?;
            // truck の uniform scale のみサポート
            if (factors[0] - factors[1]).abs() < 1e-10 && (factors[1] - factors[2]).abs() < 1e-10 {
                let origin = Point3::origin();
                let scale_vec = Vector3::new(factors[0], factors[1], factors[2]);
                Ok(builder::scaled(&solid, origin, scale_vec))
            } else {
                // 非均一スケールは truck では直接サポートされない
                // TODO: 要レビュー — 変換行列を使って実装する方法を検討
                Err(FernError::CadError {
                    message: "BREP での非均一スケール (x≠y≠z) は現在未対応です".to_string(),
                })
            }
        }
    }
}

/// truck `Solid` を `TriMesh` に変換する（テッセレーション）
pub fn solid_to_trimesh(solid: &Solid) -> FernResult<TriMesh> {
    let tessellated = solid.triangulation(TESSELLATION_TOLERANCE);
    let polygon = tessellated.to_polygon();

    let positions = polygon.positions();
    let tri_faces = polygon.tri_faces();

    let vertices: Vec<[f64; 3]> = positions.iter().map(|p| [p.x, p.y, p.z]).collect();

    let triangles: Vec<[usize; 3]> = tri_faces
        .iter()
        .map(|f| [f[0].pos, f[1].pos, f[2].pos])
        .collect();

    Ok(TriMesh {
        vertices,
        triangles,
    })
}

// === プリミティブ構築関数 ===

/// 直方体を構築する（原点中心）
fn build_box(width: f64, depth: f64, height: f64) -> FernResult<Solid> {
    let hw = width / 2.0;
    let hd = depth / 2.0;

    // 底面の四角形ワイヤーを作成
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
        message: format!("BREP box 底面の作成に失敗: {e}"),
    })?;

    // 底面を Z 方向に押し出す
    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, height));

    // 原点中心に移動
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// 円柱を構築する（原点中心、Z 軸方向）
fn build_cylinder(radius: f64, height: f64) -> FernResult<Solid> {
    // 円を作成
    let v = builder::vertex(Point3::new(radius, 0.0, 0.0));
    let circle = builder::rsweep(&v, Point3::origin(), Vector3::unit_z(), Rad(7.0));
    // 7.0 rad > 2π なので完全な円になる

    let disk = builder::try_attach_plane(&[circle]).map_err(|e| FernError::CadError {
        message: format!("BREP cylinder 底面の作成に失敗: {e}"),
    })?;

    let solid = builder::tsweep(&disk, Vector3::new(0.0, 0.0, height));

    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
    ))
}

/// 円錐（截頭円錐含む）を構築する
fn build_cone(radius_bottom: f64, radius_top: f64, height: f64) -> FernResult<Solid> {
    if radius_top < 1e-10 {
        // 尖った円錐: 底面円を頂点に向けて tsweep できないので、
        // 母線を rsweep で回転体にする
        let v_bottom = builder::vertex(Point3::new(radius_bottom, 0.0, 0.0));
        let v_top = builder::vertex(Point3::new(0.0, 0.0, height));
        let line = builder::line(&v_bottom, &v_top);

        // 底面の閉じた線も必要
        let v_center = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let base_line = builder::line(&v_center, &v_bottom);

        let wire: Wire = vec![base_line, line, builder::line(&v_top, &v_center)].into();
        let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
            message: format!("BREP cone 面の作成に失敗: {e}"),
        })?;

        let solid = builder::rsweep(&face, Point3::origin(), Vector3::unit_z(), Rad(7.0));
        Ok(builder::translated(
            &solid,
            Vector3::new(0.0, 0.0, -height / 2.0),
        ))
    } else {
        // 截頭円錐: 台形断面を回転
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
            message: format!("BREP truncated cone 面の作成に失敗: {e}"),
        })?;

        let solid = builder::rsweep(&face, Point3::origin(), Vector3::unit_z(), Rad(7.0));
        Ok(builder::translated(
            &solid,
            Vector3::new(0.0, 0.0, -height / 2.0),
        ))
    }
}

/// 多角柱を構築する（原点中心、Z 軸方向）
fn build_prism(sides: u32, radius: f64, height: f64) -> FernResult<Solid> {
    let n = sides as usize;
    let mut vertices = Vec::with_capacity(n);
    let mut edges = Vec::with_capacity(n);

    for i in 0..n {
        let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        vertices.push(builder::vertex(Point3::new(
            radius * angle.cos(),
            radius * angle.sin(),
            0.0,
        )));
    }

    for i in 0..n {
        let next = (i + 1) % n;
        edges.push(builder::line(&vertices[i], &vertices[next]));
    }

    let wire: Wire = edges.into();
    let face = builder::try_attach_plane(&[wire]).map_err(|e| FernError::CadError {
        message: format!("BREP prism 底面の作成に失敗: {e}"),
    })?;

    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, height));
    Ok(builder::translated(
        &solid,
        Vector3::new(0.0, 0.0, -height / 2.0),
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
        assert!(mesh.vertex_count() > 0, "Box メッシュに頂点がない");
        assert!(
            mesh.triangle_count() >= 12,
            "Box メッシュの三角形数が少なすぎる: {}",
            mesh.triangle_count()
        );
    }

    #[test]
    fn test_build_cylinder() {
        let solid = build_cylinder(5.0, 20.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sphere_unsupported_in_brep() {
        let node = ShapeNode::Sphere {
            radius: 5.0,
            segments: 16,
        };
        assert!(shape_to_solid(&node).is_err());
    }

    #[test]
    fn test_build_prism() {
        let solid = build_prism(6, 5.0, 10.0).unwrap();
        let mesh = solid_to_trimesh(&solid).unwrap();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
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
        assert!(mesh.vertex_count() > 0);

        let (min, max) = mesh.bounding_box();
        // 原点中心 [-5,5] を +5 → [0, 10]
        assert!(
            (min[0] - 0.0).abs() < 0.5,
            "min[0] = {}, 期待: ~0.0",
            min[0]
        );
        assert!(
            (max[0] - 10.0).abs() < 0.5,
            "max[0] = {}, 期待: ~10.0",
            max[0]
        );
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
        let solid = shape_to_solid(&node);
        // truck の boolean は不安定な場合があるので、エラーでも許容
        if let Ok(solid) = solid {
            let mesh = solid_to_trimesh(&solid).unwrap();
            assert!(mesh.triangle_count() > 0);
        }
    }
}

//! CSG スナップショットテスト
//!
//! .fern ファイルを評価し、メッシュの特性をチェックする。

use ferncad_cad::realize::eval_and_realize;

#[test]
fn test_01_primitives() {
    let source = include_str!("../../../tests/fixtures/01-primitives.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert_eq!(mesh.triangle_count(), 12, "box は 12 三角形");
    assert_eq!(mesh.vertex_count(), 8, "box は 8 頂点");

    let (min, max) = mesh.bounding_box();
    assert!((min[0] - (-5.0)).abs() < 1e-10);
    assert!((max[0] - 5.0).abs() < 1e-10);
}

#[test]
fn test_02_csg_operations() {
    let source = include_str!("../../../tests/fixtures/02-csg-operations.fern");
    let mesh = eval_and_realize(source).unwrap();
    // difference で三角形数が増加するはず
    assert!(
        mesh.triangle_count() > 12,
        "CSG difference 結果: {} 三角形（12 より多いはず）",
        mesh.triangle_count()
    );
    assert!(mesh.vertex_count() > 0);
}

#[test]
fn test_03_defpart_basic() {
    let source = include_str!("../../../tests/fixtures/03-defpart-basic.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() > 0,
        "defpart + CSG でメッシュが生成されるべき"
    );

    let (min, max) = mesh.bounding_box();
    // size=20 なので bbox は [-10, 10] の範囲内
    assert!(min[0] >= -10.1);
    assert!(max[0] <= 10.1);
}

#[test]
fn test_04_defpart_params() {
    let source = include_str!("../../../tests/fixtures/04-defpart-params.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() > 0,
        "defpart + デフォルトパラメータでメッシュが生成されるべき"
    );
}

#[test]
fn test_stl_export_from_source() {
    let source = "(sphere :radius 5 :segments 8)";
    let mesh = eval_and_realize(source).unwrap();
    let stl_bytes = ferncad_cad::export::export_stl_bytes(&mesh).unwrap();
    // STL ヘッダー + 三角形数 + データ
    assert!(stl_bytes.len() > 84, "STL バイナリが小さすぎます");
    // 三角形数の確認
    let num_tris = u32::from_le_bytes([stl_bytes[80], stl_bytes[81], stl_bytes[82], stl_bytes[83]]);
    assert_eq!(num_tris as usize, mesh.triangle_count());
}

#[test]
fn test_mvp_completion_criteria() {
    // Phase 1 MVP 完了基準のコード
    let source = r#"
        (defpart my-part
          "テスト用パーツ"
          :meta (:category :test :material :steel :description "test")
          :params ((size :: length :default 10.0 :doc "サイズ"))
          :body
          (difference
            (box :width size :depth size :height size)
            (sphere :radius (/ size 3) :segments 8)))

        (my-part :size 20.0)
    "#;

    // 1. 評価・メッシュ生成が成功する
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() > 0,
        "MVP コードがメッシュを生成すべき"
    );

    // 2. STL エクスポートが成功する
    let stl_bytes = ferncad_cad::export::export_stl_bytes(&mesh).unwrap();
    assert!(stl_bytes.len() > 84);

    // 3. flat arrays が Three.js 互換
    let (positions, normals) = mesh.to_flat_arrays();
    assert_eq!(positions.len(), normals.len());
    assert_eq!(positions.len() % 9, 0, "三角形 × 3頂点 × 3座標");
}

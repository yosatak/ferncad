//! CSG snapshot tests
//!
//! Evaluates .fern files and checks mesh properties.

use ferncad_cad::realize::eval_and_realize;

#[test]
fn test_01_primitives() {
    let source = include_str!("../../../tests/fixtures/01-primitives.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() >= 12,
        "box should have at least 12 triangles"
    );
    assert!(
        mesh.vertex_count() >= 8,
        "box should have at least 8 vertices"
    );

    let (min, max) = mesh.bounding_box();
    assert!((min[0] - (-5.0)).abs() < 0.5);
    assert!((max[0] - 5.0).abs() < 0.5);
}

#[test]
fn test_02_csg_operations() {
    let source = include_str!("../../../tests/fixtures/02-csg-operations.fern");
    let mesh = eval_and_realize(source).unwrap();
    // CSG difference should increase triangle count
    assert!(
        mesh.triangle_count() > 12,
        "CSG difference result: {} triangles (should be more than 12)",
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
        "defpart + CSG should generate a mesh"
    );

    let (min, max) = mesh.bounding_box();
    // size=20, so bbox should be within [-10, 10]
    assert!(min[0] >= -10.1);
    assert!(max[0] <= 10.1);
}

#[test]
fn test_04_defpart_params() {
    let source = include_str!("../../../tests/fixtures/04-defpart-params.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() > 0,
        "defpart + default parameters should generate a mesh"
    );
}

#[test]
fn test_stl_export_from_source() {
    let source = "(sphere :radius 5 :segments 8)";
    let mesh = eval_and_realize(source).unwrap();
    let stl_bytes = ferncad_cad::export::export_stl_bytes(&mesh).unwrap();
    // STL header + triangle count + data
    assert!(stl_bytes.len() > 84, "STL binary is too small");
    // Verify triangle count
    let num_tris = u32::from_le_bytes([stl_bytes[80], stl_bytes[81], stl_bytes[82], stl_bytes[83]]);
    assert_eq!(num_tris as usize, mesh.triangle_count());
}

#[test]
fn test_mvp_completion_criteria() {
    // Phase 1 MVP completion criteria code
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

    // 1. Evaluation and mesh generation should succeed
    let mesh = eval_and_realize(source).unwrap();
    assert!(mesh.triangle_count() > 0, "MVP code should generate a mesh");

    // 2. STL export should succeed
    let stl_bytes = ferncad_cad::export::export_stl_bytes(&mesh).unwrap();
    assert!(stl_bytes.len() > 84);

    // 3. Flat arrays should be Three.js compatible
    let (positions, normals) = mesh.to_flat_arrays();
    assert_eq!(positions.len(), normals.len());
    assert_eq!(
        positions.len() % 9,
        0,
        "triangles x 3 vertices x 3 coordinates"
    );
}

#[test]
fn test_05_assembly() {
    let source = include_str!("../../../tests/fixtures/05-assembly.fern");
    let parts = ferncad_cad::assembly_realize::eval_and_realize_parts(source).unwrap();
    assert_eq!(parts.len(), 2, "assembly should have 2 parts");
    assert_eq!(parts[0].name, "plate");
    assert_eq!(parts[1].name, "pin");
    assert!(parts[0].mesh.triangle_count() > 0);
    assert!(parts[1].mesh.triangle_count() > 0);
}

#[test]
fn test_06_standard_lib() {
    let source = include_str!("../../../tests/fixtures/06-standard-lib.fern");
    let mesh = eval_and_realize(source).unwrap();
    assert!(
        mesh.triangle_count() > 0,
        "standard library m3-bolt should generate a mesh"
    );
}

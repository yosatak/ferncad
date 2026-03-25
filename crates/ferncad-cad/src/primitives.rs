//! プリミティブ形状のメッシュ生成
//!
//! box, sphere, cylinder, cone, prism のメッシュを直接生成する。

use std::f64::consts::PI;

use crate::mesh::TriMesh;

/// デフォルトのセグメント数
pub const DEFAULT_SEGMENTS: u32 = 32;

/// 直方体メッシュを生成する（原点中心）
///
/// 8 頂点、12 三角形
pub fn generate_box(width: f64, depth: f64, height: f64) -> TriMesh {
    let hw = width / 2.0;
    let hd = depth / 2.0;
    let hh = height / 2.0;

    let vertices = vec![
        // 前面 (z+)
        [-hw, -hd, hh], // 0
        [hw, -hd, hh],  // 1
        [hw, hd, hh],   // 2
        [-hw, hd, hh],  // 3
        // 後面 (z-)
        [-hw, -hd, -hh], // 4
        [hw, -hd, -hh],  // 5
        [hw, hd, -hh],   // 6
        [-hw, hd, -hh],  // 7
    ];

    let triangles = vec![
        // 前面 (z+)
        [0, 1, 2],
        [0, 2, 3],
        // 後面 (z-)
        [5, 4, 7],
        [5, 7, 6],
        // 上面 (y+)
        [3, 2, 6],
        [3, 6, 7],
        // 下面 (y-)
        [4, 5, 1],
        [4, 1, 0],
        // 右面 (x+)
        [1, 5, 6],
        [1, 6, 2],
        // 左面 (x-)
        [4, 0, 3],
        [4, 3, 7],
    ];

    TriMesh {
        vertices,
        triangles,
    }
}

/// 球メッシュを生成する（UV sphere、原点中心）
pub fn generate_sphere(radius: f64, segments: u32) -> TriMesh {
    let rings = segments / 2;
    let sectors = segments;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // 頂点生成
    for ring in 0..=rings {
        let phi = PI * ring as f64 / rings as f64;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for sector in 0..=sectors {
            let theta = 2.0 * PI * sector as f64 / sectors as f64;
            let x = sin_phi * theta.cos() * radius;
            let y = sin_phi * theta.sin() * radius;
            let z = cos_phi * radius;
            vertices.push([x, y, z]);
        }
    }

    // 三角形生成
    let cols = sectors + 1;
    for ring in 0..rings {
        for sector in 0..sectors {
            let a = ring * cols + sector;
            let b = a + 1;
            let c = a + cols;
            let d = c + 1;

            let a = a as usize;
            let b = b as usize;
            let c = c as usize;
            let d = d as usize;

            if ring > 0 {
                triangles.push([a, c, b]);
            }
            if ring < rings - 1 {
                triangles.push([b, c, d]);
            }
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// 円柱メッシュを生成する（原点中心、Z 軸方向）
pub fn generate_cylinder(radius: f64, height: f64, segments: u32) -> TriMesh {
    let hh = height / 2.0;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // 上キャップ中心
    let top_center = vertices.len();
    vertices.push([0.0, 0.0, hh]);

    // 上キャップ外周
    let top_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius * theta.cos(), radius * theta.sin(), hh]);
    }

    // 下キャップ中心
    let bot_center = vertices.len();
    vertices.push([0.0, 0.0, -hh]);

    // 下キャップ外周
    let bot_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius * theta.cos(), radius * theta.sin(), -hh]);
    }

    let seg = segments as usize;

    // 上キャップ三角形
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([top_center, top_ring_start + i, top_ring_start + next]);
    }

    // 下キャップ三角形（裏向き）
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([bot_center, bot_ring_start + next, bot_ring_start + i]);
    }

    // 側面
    for i in 0..seg {
        let next = (i + 1) % seg;
        let t0 = top_ring_start + i;
        let t1 = top_ring_start + next;
        let b0 = bot_ring_start + i;
        let b1 = bot_ring_start + next;
        triangles.push([t0, b0, b1]);
        triangles.push([t0, b1, t1]);
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// 円錐メッシュを生成する（原点中心、Z 軸方向）
pub fn generate_cone(radius_bottom: f64, radius_top: f64, height: f64, segments: u32) -> TriMesh {
    // radius_top が 0 に近い場合は先端が尖った円錐
    // そうでなければ truncated cone
    let hh = height / 2.0;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    // 上面
    let top_center = vertices.len();
    vertices.push([0.0, 0.0, hh]);

    let top_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([radius_top * theta.cos(), radius_top * theta.sin(), hh]);
    }

    // 下面
    let bot_center = vertices.len();
    vertices.push([0.0, 0.0, -hh]);

    let bot_ring_start = vertices.len();
    for i in 0..segments {
        let theta = 2.0 * PI * i as f64 / segments as f64;
        vertices.push([
            radius_bottom * theta.cos(),
            radius_bottom * theta.sin(),
            -hh,
        ]);
    }

    let seg = segments as usize;

    // 上キャップ（radius_top > 0 の場合のみ）
    if radius_top > 1e-10 {
        for i in 0..seg {
            let next = (i + 1) % seg;
            triangles.push([top_center, top_ring_start + i, top_ring_start + next]);
        }
    }

    // 下キャップ
    for i in 0..seg {
        let next = (i + 1) % seg;
        triangles.push([bot_center, bot_ring_start + next, bot_ring_start + i]);
    }

    // 側面
    for i in 0..seg {
        let next = (i + 1) % seg;
        if radius_top > 1e-10 {
            let t0 = top_ring_start + i;
            let t1 = top_ring_start + next;
            let b0 = bot_ring_start + i;
            let b1 = bot_ring_start + next;
            triangles.push([t0, b0, b1]);
            triangles.push([t0, b1, t1]);
        } else {
            // 尖った円錐: 頂点から底面三角形
            let b0 = bot_ring_start + i;
            let b1 = bot_ring_start + next;
            triangles.push([top_center, b0, b1]);
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

/// 多角柱メッシュを生成する（原点中心、Z 軸方向）
pub fn generate_prism(sides: u32, radius: f64, height: f64) -> TriMesh {
    generate_cylinder(radius, height, sides)
}

/// トーラスメッシュを生成する（原点中心、Z 軸周り）
pub fn generate_torus(radius_major: f64, radius_minor: f64, segments: u32) -> TriMesh {
    let ring_segments = segments;
    let tube_segments = segments / 2;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    for i in 0..=ring_segments {
        let theta = 2.0 * PI * i as f64 / ring_segments as f64;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        for j in 0..=tube_segments {
            let phi = 2.0 * PI * j as f64 / tube_segments as f64;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            let x = (radius_major + radius_minor * cos_phi) * cos_theta;
            let y = (radius_major + radius_minor * cos_phi) * sin_theta;
            let z = radius_minor * sin_phi;
            vertices.push([x, y, z]);
        }
    }

    let cols = tube_segments + 1;
    for i in 0..ring_segments {
        for j in 0..tube_segments {
            let a = (i * cols + j) as usize;
            let b = (i * cols + j + 1) as usize;
            let c = ((i + 1) * cols + j) as usize;
            let d = ((i + 1) * cols + j + 1) as usize;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }

    TriMesh {
        vertices,
        triangles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_mesh() {
        let mesh = generate_box(10.0, 20.0, 30.0);
        assert_eq!(mesh.vertex_count(), 8);
        assert_eq!(mesh.triangle_count(), 12);

        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 1e-10);
        assert!((max[0] - 5.0).abs() < 1e-10);
        assert!((min[1] - (-10.0)).abs() < 1e-10);
        assert!((max[1] - 10.0).abs() < 1e-10);
        assert!((min[2] - (-15.0)).abs() < 1e-10);
        assert!((max[2] - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_sphere_mesh() {
        let mesh = generate_sphere(5.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        for i in 0..3 {
            assert!((min[i] - (-5.0)).abs() < 0.5);
            assert!((max[i] - 5.0).abs() < 0.5);
        }
    }

    #[test]
    fn test_cylinder_mesh() {
        let mesh = generate_cylinder(5.0, 20.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);

        let (min, max) = mesh.bounding_box();
        assert!((min[2] - (-10.0)).abs() < 1e-10);
        assert!((max[2] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_cone_mesh() {
        let mesh = generate_cone(5.0, 0.0, 10.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_prism_mesh() {
        let mesh = generate_prism(6, 5.0, 10.0);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_torus_mesh() {
        let mesh = generate_torus(20.0, 3.0, 16);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_to_flat_arrays() {
        let mesh = generate_box(10.0, 10.0, 10.0);
        let (positions, normals) = mesh.to_flat_arrays();
        // 12 三角形 × 3 頂点 × 3 座標
        assert_eq!(positions.len(), 12 * 3 * 3);
        assert_eq!(normals.len(), 12 * 3 * 3);
    }
}

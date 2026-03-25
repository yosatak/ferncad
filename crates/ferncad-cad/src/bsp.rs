//! BSP ツリーベースの CSG 演算
//!
//! csg.js (Evan Wallace) のアルゴリズムを Rust で実装。
//! 三角形メッシュに対して union, difference, intersection を行う。

use crate::mesh::{cross, dot, lerp, normalize, scale_vec, sub, TriMesh};

/// 分類の許容誤差
const EPSILON: f64 = 1e-5;

/// 平面
#[derive(Debug, Clone)]
struct Plane {
    normal: [f64; 3],
    w: f64,
}

/// ポリゴン（三角形）
#[derive(Debug, Clone)]
struct Polygon {
    vertices: Vec<[f64; 3]>,
}

/// BSP ツリーノード
#[derive(Debug, Clone)]
struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<Polygon>,
}

/// 頂点の平面に対する分類
#[derive(Debug, Clone, Copy, PartialEq)]
enum Classification {
    Coplanar = 0,
    Front = 1,
    Back = 2,
    Spanning = 3,
}

impl Plane {
    /// 3頂点から平面を作成する
    fn from_points(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<Self> {
        let n = normalize(cross(sub(b, a), sub(c, a)));
        let len_sq = dot(n, n);
        if len_sq < 0.5 {
            return None; // 退化三角形
        }
        Some(Self {
            normal: n,
            w: dot(n, a),
        })
    }

    /// 平面を反転する
    fn flip(&mut self) {
        self.normal = scale_vec(self.normal, -1.0);
        self.w = -self.w;
    }
}

impl Polygon {
    /// ポリゴンを反転する（頂点順を逆にする）
    fn flip(&mut self) {
        self.vertices.reverse();
    }

    /// ポリゴンの平面を取得する
    fn plane(&self) -> Option<Plane> {
        if self.vertices.len() < 3 {
            return None;
        }
        Plane::from_points(self.vertices[0], self.vertices[1], self.vertices[2])
    }
}

impl BspNode {
    /// 空のノードを作成する
    fn new() -> Self {
        Self {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        }
    }

    /// ポリゴンリストから BSP ツリーを構築する
    fn build(polygons: Vec<Polygon>) -> Option<Self> {
        if polygons.is_empty() {
            return None;
        }
        let mut node = BspNode::new();
        node.add_polygons(polygons);
        Some(node)
    }

    /// ポリゴンを追加する
    fn add_polygons(&mut self, polygons: Vec<Polygon>) {
        if polygons.is_empty() {
            return;
        }

        if self.plane.is_none() {
            // 最初のポリゴンの平面を分割平面とする
            self.plane = polygons[0].plane();
            if self.plane.is_none() {
                // 退化ポリゴンをスキップして次を試す
                for p in &polygons {
                    if let Some(plane) = p.plane() {
                        self.plane = Some(plane);
                        break;
                    }
                }
                if self.plane.is_none() {
                    return;
                }
            }
        }

        let plane = self.plane.as_ref().unwrap().clone();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();
        let mut front_polys = Vec::new();
        let mut back_polys = Vec::new();

        for polygon in polygons {
            split_polygon(
                &plane,
                polygon,
                &mut coplanar_front,
                &mut coplanar_back,
                &mut front_polys,
                &mut back_polys,
            );
        }

        // coplanar ポリゴンはこのノードに格納
        self.polygons.extend(coplanar_front);
        self.polygons.extend(coplanar_back);

        if !front_polys.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(BspNode::new()));
            }
            self.front.as_mut().unwrap().add_polygons(front_polys);
        }

        if !back_polys.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(BspNode::new()));
            }
            self.back.as_mut().unwrap().add_polygons(back_polys);
        }
    }

    /// すべてのポリゴンを収集する
    fn all_polygons(&self) -> Vec<Polygon> {
        let mut result = self.polygons.clone();
        if let Some(front) = &self.front {
            result.extend(front.all_polygons());
        }
        if let Some(back) = &self.back {
            result.extend(back.all_polygons());
        }
        result
    }

    /// ツリーを反転する
    fn invert(&mut self) {
        for poly in &mut self.polygons {
            poly.flip();
        }
        if let Some(plane) = &mut self.plane {
            plane.flip();
        }
        if let Some(front) = &mut self.front {
            front.invert();
        }
        if let Some(back) = &mut self.back {
            back.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    /// このツリーの内側にあるポリゴンを除去する
    fn clip_polygons(&self, polygons: &[Polygon]) -> Vec<Polygon> {
        if self.plane.is_none() {
            return polygons.to_vec();
        }

        let plane = self.plane.as_ref().unwrap();
        let mut front = Vec::new();
        let mut back = Vec::new();

        for polygon in polygons {
            let mut coplanar_front = Vec::new();
            let mut coplanar_back = Vec::new();
            split_polygon(
                plane,
                polygon.clone(),
                &mut coplanar_front,
                &mut coplanar_back,
                &mut front,
                &mut back,
            );
            front.extend(coplanar_front);
            back.extend(coplanar_back);
        }

        let front = if let Some(f) = &self.front {
            f.clip_polygons(&front)
        } else {
            front
        };

        let back = if let Some(b) = &self.back {
            b.clip_polygons(&back)
        } else {
            Vec::new() // 後方にツリーがない → 後方のポリゴンは削除
        };

        let mut result = front;
        result.extend(back);
        result
    }

    /// 他のツリーでクリップする
    fn clip_to(&mut self, other: &BspNode) {
        self.polygons = other.clip_polygons(&self.polygons);
        if let Some(front) = &mut self.front {
            front.clip_to(other);
        }
        if let Some(back) = &mut self.back {
            back.clip_to(other);
        }
    }
}

/// ポリゴンを平面で分割する
fn split_polygon(
    plane: &Plane,
    polygon: Polygon,
    coplanar_front: &mut Vec<Polygon>,
    coplanar_back: &mut Vec<Polygon>,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
) {
    let mut polygon_type = Classification::Coplanar as u8;
    let mut types = Vec::with_capacity(polygon.vertices.len());

    for vertex in &polygon.vertices {
        let t = dot(plane.normal, *vertex) - plane.w;
        let vtype = if t < -EPSILON {
            Classification::Back
        } else if t > EPSILON {
            Classification::Front
        } else {
            Classification::Coplanar
        };
        polygon_type |= vtype as u8;
        types.push(vtype);
    }

    match polygon_type.try_into() {
        Ok(Classification::Coplanar) => {
            if dot(plane.normal, polygon.plane().map_or([0.0; 3], |p| p.normal)) > 0.0 {
                coplanar_front.push(polygon);
            } else {
                coplanar_back.push(polygon);
            }
        }
        Ok(Classification::Front) => front.push(polygon),
        Ok(Classification::Back) => back.push(polygon),
        _ => {
            // Spanning: ポリゴンを分割
            let mut f_verts = Vec::new();
            let mut b_verts = Vec::new();
            let n = polygon.vertices.len();

            for i in 0..n {
                let j = (i + 1) % n;
                let ti = types[i];
                let tj = types[j];
                let vi = polygon.vertices[i];
                let vj = polygon.vertices[j];

                if ti != Classification::Back {
                    f_verts.push(vi);
                }
                if ti != Classification::Front {
                    b_verts.push(vi);
                }

                if (ti as u8 | tj as u8) == Classification::Spanning as u8 {
                    // 交点を計算
                    let t = (plane.w - dot(plane.normal, vi)) / dot(plane.normal, sub(vj, vi));
                    let v = lerp(vi, vj, t);
                    f_verts.push(v);
                    b_verts.push(v);
                }
            }

            if f_verts.len() >= 3 {
                // ファンで三角形分割
                for polys in triangulate(&f_verts) {
                    front.push(polys);
                }
            }
            if b_verts.len() >= 3 {
                for polys in triangulate(&b_verts) {
                    back.push(polys);
                }
            }
        }
    }
}

impl TryFrom<u8> for Classification {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Classification::Coplanar),
            1 => Ok(Classification::Front),
            2 => Ok(Classification::Back),
            3 => Ok(Classification::Spanning),
            _ => Err(()),
        }
    }
}

/// 多角形を三角形ファンで分割する
fn triangulate(vertices: &[[f64; 3]]) -> Vec<Polygon> {
    let mut result = Vec::new();
    for i in 1..vertices.len() - 1 {
        result.push(Polygon {
            vertices: vec![vertices[0], vertices[i], vertices[i + 1]],
        });
    }
    result
}

// === TriMesh ↔ Polygon 変換 ===

/// TriMesh をポリゴンリストに変換する
fn mesh_to_polygons(mesh: &TriMesh) -> Vec<Polygon> {
    mesh.triangles
        .iter()
        .map(|tri| Polygon {
            vertices: vec![
                mesh.vertices[tri[0]],
                mesh.vertices[tri[1]],
                mesh.vertices[tri[2]],
            ],
        })
        .collect()
}

/// ポリゴンリストを TriMesh に変換する
fn polygons_to_mesh(polygons: &[Polygon]) -> TriMesh {
    let mut mesh = TriMesh::new();
    for poly in polygons {
        if poly.vertices.len() < 3 {
            continue;
        }
        let base = mesh.vertices.len();
        mesh.vertices.extend_from_slice(&poly.vertices);
        // ファンで三角形化
        for i in 1..poly.vertices.len() - 1 {
            mesh.triangles.push([base, base + i, base + i + 1]);
        }
    }
    mesh
}

// === 公開 CSG 演算 ===

/// CSG union (A ∪ B)
pub fn csg_union(a: &TriMesh, b: &TriMesh) -> TriMesh {
    let a_polys = mesh_to_polygons(a);
    let b_polys = mesh_to_polygons(b);

    let mut a_bsp = match BspNode::build(a_polys) {
        Some(n) => n,
        None => return b.clone(),
    };
    let mut b_bsp = match BspNode::build(b_polys) {
        Some(n) => n,
        None => return a.clone(),
    };

    a_bsp.clip_to(&b_bsp);
    b_bsp.clip_to(&a_bsp);
    b_bsp.invert();
    b_bsp.clip_to(&a_bsp);
    b_bsp.invert();

    let mut all_polys = a_bsp.all_polygons();
    all_polys.extend(b_bsp.all_polygons());
    polygons_to_mesh(&all_polys)
}

/// CSG difference (A - B)
pub fn csg_difference(a: &TriMesh, b: &TriMesh) -> TriMesh {
    let a_polys = mesh_to_polygons(a);
    let b_polys = mesh_to_polygons(b);

    let mut a_bsp = match BspNode::build(a_polys) {
        Some(n) => n,
        None => return TriMesh::new(),
    };
    let mut b_bsp = match BspNode::build(b_polys) {
        Some(n) => n,
        None => return a.clone(),
    };

    a_bsp.invert();
    a_bsp.clip_to(&b_bsp);
    b_bsp.clip_to(&a_bsp);
    b_bsp.invert();
    b_bsp.clip_to(&a_bsp);
    b_bsp.invert();

    let mut all_polys = a_bsp.all_polygons();
    all_polys.extend(b_bsp.all_polygons());

    let mut result = BspNode::build(all_polys).unwrap_or_else(BspNode::new);
    result.invert();
    polygons_to_mesh(&result.all_polygons())
}

/// CSG intersection (A ∩ B)
pub fn csg_intersection(a: &TriMesh, b: &TriMesh) -> TriMesh {
    let a_polys = mesh_to_polygons(a);
    let b_polys = mesh_to_polygons(b);

    let mut a_bsp = match BspNode::build(a_polys) {
        Some(n) => n,
        None => return TriMesh::new(),
    };
    let mut b_bsp = match BspNode::build(b_polys) {
        Some(n) => n,
        None => return TriMesh::new(),
    };

    a_bsp.invert();
    b_bsp.clip_to(&a_bsp);
    b_bsp.invert();
    a_bsp.clip_to(&b_bsp);
    b_bsp.clip_to(&a_bsp);

    let mut all_polys = a_bsp.all_polygons();
    all_polys.extend(b_bsp.all_polygons());

    let mut result = BspNode::build(all_polys).unwrap_or_else(BspNode::new);
    result.invert();
    polygons_to_mesh(&result.all_polygons())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::generate_box;

    #[test]
    fn test_union_non_overlapping() {
        let a = generate_box(10.0, 10.0, 10.0);
        let mut b = generate_box(10.0, 10.0, 10.0);
        // b を右に移動
        for v in &mut b.vertices {
            v[0] += 20.0;
        }
        let result = csg_union(&a, &b);
        assert!(result.triangle_count() > 0);
        // 非重複なので三角形数は a + b
        assert_eq!(
            result.triangle_count(),
            a.triangle_count() + b.triangle_count()
        );
    }

    #[test]
    fn test_union_overlapping() {
        let a = generate_box(10.0, 10.0, 10.0);
        let mut b = generate_box(10.0, 10.0, 10.0);
        // b を少し右に移動（重なる）
        for v in &mut b.vertices {
            v[0] += 5.0;
        }
        let result = csg_union(&a, &b);
        assert!(result.triangle_count() > 0);
    }

    #[test]
    fn test_difference() {
        let a = generate_box(20.0, 20.0, 20.0);
        let b = generate_box(10.0, 10.0, 10.0);
        let result = csg_difference(&a, &b);
        assert!(result.triangle_count() > 0);
        // difference は元より多い三角形が生まれるはず
        assert!(result.triangle_count() > a.triangle_count());
    }

    #[test]
    fn test_intersection() {
        let a = generate_box(20.0, 20.0, 20.0);
        let b = generate_box(10.0, 10.0, 10.0);
        let result = csg_intersection(&a, &b);
        assert!(result.triangle_count() > 0);
    }
}

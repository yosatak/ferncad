//! BSP tree-based CSG operations
//!
//! A Rust implementation of the csg.js (Evan Wallace) algorithm.
//! Performs union, difference, and intersection on triangle meshes.
//! Uses iterative traversal to avoid stack overflow on large meshes.

use crate::mesh::{cross, dot, lerp, normalize, scale_vec, sub, TriMesh};

/// Classification tolerance
const EPSILON: f64 = 1e-5;

/// Plane
#[derive(Debug, Clone)]
struct Plane {
    normal: [f64; 3],
    w: f64,
}

/// Polygon (triangle)
#[derive(Debug, Clone)]
struct Polygon {
    vertices: Vec<[f64; 3]>,
}

/// BSP tree node
#[derive(Debug, Clone)]
struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<Polygon>,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum Classification {
    Coplanar = 0,
    Front = 1,
    Back = 2,
    Spanning = 3,
}

impl Plane {
    fn from_points(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<Self> {
        let normal = normalize(cross(sub(b, a), sub(c, a)));
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        if len < EPSILON {
            return None;
        }
        Some(Plane {
            w: dot(normal, a),
            normal,
        })
    }

    fn flip(&mut self) {
        self.normal = scale_vec(self.normal, -1.0);
        self.w = -self.w;
    }
}

impl Polygon {
    fn plane(&self) -> Option<Plane> {
        if self.vertices.len() < 3 {
            return None;
        }
        Plane::from_points(self.vertices[0], self.vertices[1], self.vertices[2])
    }

    fn flip(&mut self) {
        self.vertices.reverse();
    }
}

impl BspNode {
    fn new() -> Self {
        BspNode {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        }
    }

    fn build(polygons: Vec<Polygon>) -> Option<Self> {
        if polygons.is_empty() {
            return None;
        }
        let mut node = BspNode::new();
        node.add_polygons(polygons);
        Some(node)
    }

    /// Add polygons to the tree (iterative to avoid stack overflow)
    fn add_polygons(&mut self, polygons: Vec<Polygon>) {
        // Work queue: (node pointer, polygons to add)
        let mut queue: Vec<(*mut BspNode, Vec<Polygon>)> = vec![(self as *mut _, polygons)];

        while let Some((node_ptr, polys)) = queue.pop() {
            if polys.is_empty() {
                continue;
            }

            // SAFETY: we hold exclusive access to the tree during build
            let node = unsafe { &mut *node_ptr };

            if node.plane.is_none() {
                // Find first non-degenerate polygon for the splitting plane
                for p in &polys {
                    if let Some(plane) = p.plane() {
                        node.plane = Some(plane);
                        break;
                    }
                }
                if node.plane.is_none() {
                    continue;
                }
            }

            let plane = node.plane.as_ref().unwrap().clone();
            let mut front_polys = Vec::new();
            let mut back_polys = Vec::new();

            for polygon in polys {
                let mut coplanar_front = Vec::new();
                let mut coplanar_back = Vec::new();
                split_polygon(
                    &plane,
                    polygon,
                    &mut coplanar_front,
                    &mut coplanar_back,
                    &mut front_polys,
                    &mut back_polys,
                );
                node.polygons.extend(coplanar_front);
                node.polygons.extend(coplanar_back);
            }

            if !front_polys.is_empty() {
                if node.front.is_none() {
                    node.front = Some(Box::new(BspNode::new()));
                }
                queue.push((node.front.as_mut().unwrap().as_mut() as *mut _, front_polys));
            }

            if !back_polys.is_empty() {
                if node.back.is_none() {
                    node.back = Some(Box::new(BspNode::new()));
                }
                queue.push((node.back.as_mut().unwrap().as_mut() as *mut _, back_polys));
            }
        }
    }

    /// Collect all polygons (iterative)
    fn all_polygons(&self) -> Vec<Polygon> {
        let mut result = Vec::new();
        let mut stack: Vec<&BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            result.extend(node.polygons.iter().cloned());
            if let Some(front) = &node.front {
                stack.push(front);
            }
            if let Some(back) = &node.back {
                stack.push(back);
            }
        }
        result
    }

    /// Invert the tree (iterative)
    fn invert(&mut self) {
        let mut stack: Vec<*mut BspNode> = vec![self as *mut _];
        while let Some(node_ptr) = stack.pop() {
            // SAFETY: we hold exclusive access to the tree
            let node = unsafe { &mut *node_ptr };
            for poly in &mut node.polygons {
                poly.flip();
            }
            if let Some(plane) = &mut node.plane {
                plane.flip();
            }
            std::mem::swap(&mut node.front, &mut node.back);
            if let Some(front) = &mut node.front {
                stack.push(front.as_mut() as *mut _);
            }
            if let Some(back) = &mut node.back {
                stack.push(back.as_mut() as *mut _);
            }
        }
    }

    /// Remove polygons that are inside this tree (iterative)
    fn clip_polygons(&self, polygons: &[Polygon]) -> Vec<Polygon> {
        // Process pairs of (tree node, polygons to clip) iteratively
        let mut result = Vec::new();
        let mut work: Vec<(&BspNode, Vec<Polygon>)> = vec![(self, polygons.to_vec())];

        while let Some((node, polys)) = work.pop() {
            if node.plane.is_none() {
                result.extend(polys);
                continue;
            }

            let plane = node.plane.as_ref().unwrap();
            let mut front = Vec::new();
            let mut back = Vec::new();

            for polygon in polys {
                let mut coplanar_front = Vec::new();
                let mut coplanar_back = Vec::new();
                split_polygon(
                    plane,
                    polygon,
                    &mut coplanar_front,
                    &mut coplanar_back,
                    &mut front,
                    &mut back,
                );
                front.extend(coplanar_front);
                back.extend(coplanar_back);
            }

            if !front.is_empty() {
                if let Some(f) = &node.front {
                    work.push((f, front));
                } else {
                    result.extend(front);
                }
            }

            if !back.is_empty() {
                if let Some(b) = &node.back {
                    work.push((b, back));
                }
                // no back tree -> discard back polygons
            }
        }

        result
    }

    /// Clip this tree against another tree (iterative)
    fn clip_to(&mut self, other: &BspNode) {
        let mut stack: Vec<*mut BspNode> = vec![self as *mut _];
        while let Some(node_ptr) = stack.pop() {
            // SAFETY: we hold exclusive access to the tree
            let node = unsafe { &mut *node_ptr };
            node.polygons = other.clip_polygons(&node.polygons);
            if let Some(front) = &mut node.front {
                stack.push(front.as_mut() as *mut _);
            }
            if let Some(back) = &mut node.back {
                stack.push(back.as_mut() as *mut _);
            }
        }
    }
}

/// Split a polygon by a plane
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
            Classification::Back as u8
        } else if t > EPSILON {
            Classification::Front as u8
        } else {
            Classification::Coplanar as u8
        };
        polygon_type |= vtype;
        types.push(vtype);
    }

    match polygon_type {
        0 => {
            // Coplanar
            if dot(plane.normal, polygon.plane().map_or([0.0; 3], |p| p.normal)) > 0.0 {
                coplanar_front.push(polygon);
            } else {
                coplanar_back.push(polygon);
            }
        }
        1 => front.push(polygon), // Front
        2 => back.push(polygon),  // Back
        _ => {
            // Spanning
            let mut f_verts = Vec::new();
            let mut b_verts = Vec::new();
            let n = polygon.vertices.len();

            for i in 0..n {
                let j = (i + 1) % n;
                let ti = types[i];
                let tj = types[j];
                let vi = polygon.vertices[i];
                let vj = polygon.vertices[j];

                if ti != Classification::Back as u8 {
                    f_verts.push(vi);
                }
                if ti != Classification::Front as u8 {
                    b_verts.push(vi);
                }

                if (ti | tj) == Classification::Spanning as u8 {
                    let t = (plane.w - dot(plane.normal, vi)) / dot(plane.normal, sub(vj, vi));
                    let v = lerp(vi, vj, t);
                    f_verts.push(v);
                    b_verts.push(v);
                }
            }

            if f_verts.len() >= 3 {
                front.push(Polygon { vertices: f_verts });
            }
            if b_verts.len() >= 3 {
                back.push(Polygon { vertices: b_verts });
            }
        }
    }
}

/// Convert a TriMesh to a list of polygons
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

/// Convert a list of polygons to a TriMesh
fn polygons_to_mesh(polygons: &[Polygon]) -> TriMesh {
    let mut mesh = TriMesh::new();
    for poly in polygons {
        if poly.vertices.len() < 3 {
            continue;
        }
        let base = mesh.vertices.len();
        mesh.vertices.extend_from_slice(&poly.vertices);
        // Triangulate using fan
        for i in 1..poly.vertices.len() - 1 {
            mesh.triangles.push([base, base + i, base + i + 1]);
        }
    }
    mesh
}

// === Public CSG operations ===

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
        // Move b to the right
        for v in &mut b.vertices {
            v[0] += 20.0;
        }
        let result = csg_union(&a, &b);
        assert_eq!(result.triangle_count(), 24);
    }

    #[test]
    fn test_union_overlapping() {
        let a = generate_box(10.0, 10.0, 10.0);
        let mut b = generate_box(10.0, 10.0, 10.0);
        for v in &mut b.vertices {
            v[0] += 5.0;
        }
        let result = csg_union(&a, &b);
        assert!(result.triangle_count() > 0);
    }

    #[test]
    fn test_difference() {
        let a = generate_box(10.0, 10.0, 10.0);
        let b = generate_box(5.0, 5.0, 5.0);
        let result = csg_difference(&a, &b);
        assert!(result.triangle_count() > 12);
    }

    #[test]
    fn test_intersection() {
        let a = generate_box(10.0, 10.0, 10.0);
        let mut b = generate_box(10.0, 10.0, 10.0);
        for v in &mut b.vertices {
            v[0] += 5.0;
        }
        let result = csg_intersection(&a, &b);
        assert!(result.triangle_count() > 0);
    }
}

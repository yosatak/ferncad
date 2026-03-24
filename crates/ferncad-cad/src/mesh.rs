//! Triangle mesh type definition
//!
//! Core data structure for the CAD kernel.

/// Triangle mesh
#[derive(Debug, Clone)]
pub struct TriMesh {
    /// Vertex coordinates `[x, y, z]`
    pub vertices: Vec<[f64; 3]>,
    /// Triangles (triplets of vertex indices)
    pub triangles: Vec<[usize; 3]>,
}

impl TriMesh {
    /// Create an empty mesh
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
        }
    }

    /// Number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of triangles
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Compute face normals for each triangle
    pub fn compute_face_normals(&self) -> Vec<[f64; 3]> {
        self.triangles
            .iter()
            .map(|tri| {
                let v0 = self.vertices[tri[0]];
                let v1 = self.vertices[tri[1]];
                let v2 = self.vertices[tri[2]];
                let e1 = sub(v1, v0);
                let e2 = sub(v2, v0);
                normalize(cross(e1, e2))
            })
            .collect()
    }

    /// "Flatten" the mesh so each triangle has its own vertices.
    /// Returns `(positions, normals)` directly usable by Three.js `BufferGeometry`.
    pub fn to_flat_arrays(&self) -> (Vec<f32>, Vec<f32>) {
        let normals = self.compute_face_normals();
        let num_tris = self.triangles.len();
        let mut positions = Vec::with_capacity(num_tris * 9);
        let mut flat_normals = Vec::with_capacity(num_tris * 9);

        for (i, tri) in self.triangles.iter().enumerate() {
            let n = normals[i];
            for &vi in tri {
                let v = self.vertices[vi];
                positions.push(v[0] as f32);
                positions.push(v[1] as f32);
                positions.push(v[2] as f32);
                flat_normals.push(n[0] as f32);
                flat_normals.push(n[1] as f32);
                flat_normals.push(n[2] as f32);
            }
        }

        (positions, flat_normals)
    }

    /// Compute the bounding box `([min_x, min_y, min_z], [max_x, max_y, max_z])`
    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        if self.vertices.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        for v in &self.vertices {
            for i in 0..3 {
                if v[i] < min[i] {
                    min[i] = v[i];
                }
                if v[i] > max[i] {
                    max[i] = v[i];
                }
            }
        }
        (min, max)
    }

    /// Merge another mesh into this one (adjusting indices)
    pub fn merge(&mut self, other: &TriMesh) {
        let offset = self.vertices.len();
        self.vertices.extend_from_slice(&other.vertices);
        for tri in &other.triangles {
            self.triangles
                .push([tri[0] + offset, tri[1] + offset, tri[2] + offset]);
        }
    }
}

impl Default for TriMesh {
    fn default() -> Self {
        Self::new()
    }
}

// === Vector math helpers ===

/// Vector subtraction
pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Vector addition
pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Scalar multiplication
pub fn scale_vec(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// Dot product
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Vector length
pub fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

/// Normalize a vector
pub fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len = length(v);
    if len < 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    scale_vec(v, 1.0 / len)
}

/// Linear interpolation
pub fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

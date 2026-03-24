//! Affine transforms for triangle meshes
//!
//! Applies translate, rotate, and scale directly to vertex coordinates.

use crate::mesh::TriMesh;

/// Translate a mesh
pub fn translate(mesh: &mut TriMesh, offset: [f64; 3]) {
    for v in &mut mesh.vertices {
        v[0] += offset[0];
        v[1] += offset[1];
        v[2] += offset[2];
    }
}

/// Rotate a mesh around an axis (angle in radians)
///
/// Uses the Rodrigues rotation formula.
pub fn rotate(mesh: &mut TriMesh, axis: [f64; 3], angle_rad: f64) {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if len < 1e-12 {
        return;
    }
    let k = [axis[0] / len, axis[1] / len, axis[2] / len];
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    for v in &mut mesh.vertices {
        let vx = v[0];
        let vy = v[1];
        let vz = v[2];

        // k x v
        let kxv = [
            k[1] * vz - k[2] * vy,
            k[2] * vx - k[0] * vz,
            k[0] * vy - k[1] * vx,
        ];

        // k . v
        let kdv = k[0] * vx + k[1] * vy + k[2] * vz;

        // v' = v*cos(a) + (k x v)*sin(a) + k*(k . v)*(1-cos(a))
        v[0] = vx * cos_a + kxv[0] * sin_a + k[0] * kdv * (1.0 - cos_a);
        v[1] = vy * cos_a + kxv[1] * sin_a + k[1] * kdv * (1.0 - cos_a);
        v[2] = vz * cos_a + kxv[2] * sin_a + k[2] * kdv * (1.0 - cos_a);
    }
}

/// Scale a mesh
pub fn scale(mesh: &mut TriMesh, factors: [f64; 3]) {
    for v in &mut mesh.vertices {
        v[0] *= factors[0];
        v[1] *= factors[1];
        v[2] *= factors[2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::generate_box;

    #[test]
    fn test_translate() {
        let mut mesh = generate_box(10.0, 10.0, 10.0);
        translate(&mut mesh, [10.0, 20.0, 30.0]);
        let (min, max) = mesh.bounding_box();
        assert!((min[0] - 5.0).abs() < 1e-10); // -5 + 10 = 5
        assert!((max[0] - 15.0).abs() < 1e-10); // 5 + 10 = 15
        assert!((min[1] - 15.0).abs() < 1e-10); // -5 + 20 = 15
        assert!((max[1] - 25.0).abs() < 1e-10); // 5 + 20 = 25
    }

    #[test]
    fn test_rotate_90_z() {
        let mut mesh = generate_box(10.0, 10.0, 10.0);
        rotate(&mut mesh, [0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2);
        let (min, max) = mesh.bounding_box();
        // A 10x10 box rotated 90 degrees around Z -> bounding box should be unchanged
        assert!((min[0] - (-5.0)).abs() < 1e-8);
        assert!((max[0] - 5.0).abs() < 1e-8);
    }

    #[test]
    fn test_scale() {
        let mut mesh = generate_box(10.0, 10.0, 10.0);
        scale(&mut mesh, [2.0, 3.0, 1.0]);
        let (min, max) = mesh.bounding_box();
        assert!((min[0] - (-10.0)).abs() < 1e-10);
        assert!((max[0] - 10.0).abs() < 1e-10);
        assert!((min[1] - (-15.0)).abs() < 1e-10);
        assert!((max[1] - 15.0).abs() < 1e-10);
    }
}

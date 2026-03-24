//! STL export
//!
//! Outputs in binary STL format.

use std::io::Write;

use crate::mesh::TriMesh;

/// Binary STL header size
const STL_HEADER_SIZE: usize = 80;

/// Export a mesh in binary STL format
///
/// # Errors
///
/// Returns an error if a write error occurs.
pub fn export_stl(mesh: &TriMesh, writer: &mut impl Write) -> Result<(), std::io::Error> {
    let normals = mesh.compute_face_normals();

    // Header (80 bytes)
    let mut header = [0u8; STL_HEADER_SIZE];
    let label = b"ferncad STL export";
    header[..label.len()].copy_from_slice(label);
    writer.write_all(&header)?;

    // Triangle count (4 bytes, little-endian)
    let num_triangles = mesh.triangles.len() as u32;
    writer.write_all(&num_triangles.to_le_bytes())?;

    // Each triangle
    for (i, tri) in mesh.triangles.iter().enumerate() {
        let n = normals[i];
        // Normal vector (12 bytes)
        writer.write_all(&(n[0] as f32).to_le_bytes())?;
        writer.write_all(&(n[1] as f32).to_le_bytes())?;
        writer.write_all(&(n[2] as f32).to_le_bytes())?;

        // 3 vertices (12 bytes each)
        for &vi in tri {
            let v = mesh.vertices[vi];
            writer.write_all(&(v[0] as f32).to_le_bytes())?;
            writer.write_all(&(v[1] as f32).to_le_bytes())?;
            writer.write_all(&(v[2] as f32).to_le_bytes())?;
        }

        // Attribute byte count (2 bytes)
        writer.write_all(&0u16.to_le_bytes())?;
    }

    Ok(())
}

/// Export a mesh to binary STL format as `Vec<u8>`
pub fn export_stl_bytes(mesh: &TriMesh) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::new();
    export_stl(mesh, &mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::generate_box;

    #[test]
    fn test_stl_export() {
        let mesh = generate_box(10.0, 10.0, 10.0);
        let bytes = export_stl_bytes(&mesh).unwrap();

        // Header 80 + triangle count 4 + (normal 12 + vertices 36 + attr 2) x 12
        let expected_size = STL_HEADER_SIZE + 4 + 50 * 12;
        assert_eq!(bytes.len(), expected_size);

        // Verify header
        assert_eq!(&bytes[..18], b"ferncad STL export");

        // Verify triangle count
        let num_tris = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(num_tris, 12);
    }
}

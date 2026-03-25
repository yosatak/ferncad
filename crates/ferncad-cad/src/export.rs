//! STL エクスポート
//!
//! バイナリ STL フォーマットで出力する。

use std::io::Write;

use crate::mesh::TriMesh;

/// バイナリ STL のヘッダーサイズ
const STL_HEADER_SIZE: usize = 80;

/// メッシュをバイナリ STL 形式でエクスポートする
///
/// # Errors
///
/// 書き込みエラーが発生した場合にエラーを返す。
pub fn export_stl(mesh: &TriMesh, writer: &mut impl Write) -> Result<(), std::io::Error> {
    let normals = mesh.compute_face_normals();

    // ヘッダー（80 バイト）
    let mut header = [0u8; STL_HEADER_SIZE];
    let label = b"ferncad STL export";
    header[..label.len()].copy_from_slice(label);
    writer.write_all(&header)?;

    // 三角形数（4 バイト、リトルエンディアン）
    let num_triangles = mesh.triangles.len() as u32;
    writer.write_all(&num_triangles.to_le_bytes())?;

    // 各三角形
    for (i, tri) in mesh.triangles.iter().enumerate() {
        let n = normals[i];
        // 法線ベクトル（12 バイト）
        writer.write_all(&(n[0] as f32).to_le_bytes())?;
        writer.write_all(&(n[1] as f32).to_le_bytes())?;
        writer.write_all(&(n[2] as f32).to_le_bytes())?;

        // 3 頂点（各 12 バイト）
        for &vi in tri {
            let v = mesh.vertices[vi];
            writer.write_all(&(v[0] as f32).to_le_bytes())?;
            writer.write_all(&(v[1] as f32).to_le_bytes())?;
            writer.write_all(&(v[2] as f32).to_le_bytes())?;
        }

        // アトリビュートバイトカウント（2 バイト）
        writer.write_all(&0u16.to_le_bytes())?;
    }

    Ok(())
}

/// メッシュをバイナリ STL 形式の `Vec<u8>` にエクスポートする
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

        // ヘッダー 80 + 三角形数 4 + (法線12 + 頂点36 + attr2) × 12
        let expected_size = STL_HEADER_SIZE + 4 + 50 * 12;
        assert_eq!(bytes.len(), expected_size);

        // ヘッダー確認
        assert_eq!(&bytes[..18], b"ferncad STL export");

        // 三角形数確認
        let num_tris = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(num_tris, 12);
    }
}

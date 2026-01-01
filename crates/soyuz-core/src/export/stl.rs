//! STL file export (Binary format)
//!
//! STL (stereolithography) is a simple mesh format commonly used for 3D printing.
//! This implementation exports in binary STL format which is more compact and
//! widely supported than ASCII STL.
//!
//! Note: STL does not support materials, textures, or vertex colors.

use crate::mesh::Mesh;
use crate::Result;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export a mesh to binary STL format
///
/// Binary STL format:
/// - 80 bytes: Header (arbitrary text, we use "Soyuz STL Export")
/// - 4 bytes: Number of triangles (u32 little-endian)
/// - For each triangle (50 bytes):
///   - 12 bytes: Normal vector (3 x f32 little-endian)
///   - 36 bytes: 3 vertices (9 x f32 little-endian)
///   - 2 bytes: Attribute byte count (usually 0)
pub fn export_stl(mesh: &Mesh, path: &Path) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Header (80 bytes, padded with spaces)
    let header = format!(
        "Soyuz STL Export - {} vertices, {} triangles",
        mesh.vertices.len(),
        mesh.indices.len() / 3
    );
    let mut header_bytes = [b' '; 80];
    let header_len = header.len().min(80);
    header_bytes[..header_len].copy_from_slice(&header.as_bytes()[..header_len]);
    writer.write_all(&header_bytes)?;

    // Number of triangles (u32 little-endian)
    let num_triangles = (mesh.indices.len() / 3) as u32;
    writer.write_all(&num_triangles.to_le_bytes())?;

    // Write each triangle
    for tri in mesh.indices.chunks(3) {
        let v0 = &mesh.vertices[tri[0] as usize];
        let v1 = &mesh.vertices[tri[1] as usize];
        let v2 = &mesh.vertices[tri[2] as usize];

        // Compute face normal from vertices (cross product)
        // Using the vertex normals average would be smoother but STL expects face normals
        let edge1 = [
            v1.position[0] - v0.position[0],
            v1.position[1] - v0.position[1],
            v1.position[2] - v0.position[2],
        ];
        let edge2 = [
            v2.position[0] - v0.position[0],
            v2.position[1] - v0.position[1],
            v2.position[2] - v0.position[2],
        ];
        let normal = [
            edge1[1] * edge2[2] - edge1[2] * edge2[1],
            edge1[2] * edge2[0] - edge1[0] * edge2[2],
            edge1[0] * edge2[1] - edge1[1] * edge2[0],
        ];
        // Normalize
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        let normal = if len > 0.0 {
            [normal[0] / len, normal[1] / len, normal[2] / len]
        } else {
            [0.0, 0.0, 1.0] // Default up if degenerate
        };

        // Write normal (3 x f32)
        writer.write_all(&normal[0].to_le_bytes())?;
        writer.write_all(&normal[1].to_le_bytes())?;
        writer.write_all(&normal[2].to_le_bytes())?;

        // Write vertex 0 (3 x f32)
        writer.write_all(&v0.position[0].to_le_bytes())?;
        writer.write_all(&v0.position[1].to_le_bytes())?;
        writer.write_all(&v0.position[2].to_le_bytes())?;

        // Write vertex 1 (3 x f32)
        writer.write_all(&v1.position[0].to_le_bytes())?;
        writer.write_all(&v1.position[1].to_le_bytes())?;
        writer.write_all(&v1.position[2].to_le_bytes())?;

        // Write vertex 2 (3 x f32)
        writer.write_all(&v2.position[0].to_le_bytes())?;
        writer.write_all(&v2.position[1].to_le_bytes())?;
        writer.write_all(&v2.position[2].to_le_bytes())?;

        // Attribute byte count (2 bytes, usually 0)
        writer.write_all(&0u16.to_le_bytes())?;
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Vertex;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("soyuz_test_{}", name))
    }

    #[test]
    fn test_export_stl_simple() {
        // Create a simple triangle mesh
        let mesh = Mesh {
            vertices: vec![
                Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                },
                Vertex {
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        };

        let path = temp_path("triangle.stl");
        export_stl(&mesh, &path).unwrap();

        // Verify file exists and has correct size
        // 80 (header) + 4 (count) + 50 (one triangle) = 134 bytes
        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), 134);

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_stl_cube() {
        // A cube has 12 triangles (2 per face * 6 faces)
        // Size = 80 + 4 + (50 * 12) = 684 bytes
        let mesh = Mesh {
            vertices: vec![
                // Front face
                Vertex { position: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
                Vertex { position: [0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
                Vertex { position: [0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
                Vertex { position: [-0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
                // Back face
                Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
                Vertex { position: [0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
                Vertex { position: [0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
                Vertex { position: [-0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
            ],
            indices: vec![
                // Front
                0, 1, 2, 0, 2, 3,
                // Back
                5, 4, 7, 5, 7, 6,
                // Left
                4, 0, 3, 4, 3, 7,
                // Right
                1, 5, 6, 1, 6, 2,
                // Top
                3, 2, 6, 3, 6, 7,
                // Bottom
                4, 5, 1, 4, 1, 0,
            ],
        };

        let path = temp_path("cube.stl");
        export_stl(&mesh, &path).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), 684); // 80 + 4 + (50 * 12)

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}

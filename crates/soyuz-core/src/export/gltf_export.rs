//! GLTF/GLB file export

// String writing is infallible, so .expect() is safe here
#![allow(clippy::expect_used)]

use crate::Result;
use crate::mesh::Mesh;
use std::path::Path;

/// Helper macro for writeln to a String buffer.
/// String writing is infallible, so we use `expect()` with a clear message.
macro_rules! writeln_str {
    ($dst:expr) => {
        writeln!($dst).expect("String write is infallible")
    };
    ($dst:expr, $($arg:tt)*) => {
        writeln!($dst, $($arg)*).expect("String write is infallible")
    };
}

/// Export a mesh to GLTF (JSON + external .bin) or GLB (single binary) format,
/// chosen by the target file extension.
pub fn export_gltf(mesh: &Mesh, path: &Path) -> Result<()> {
    // An empty mesh would produce a POSITION accessor with min = [f32::MAX] and
    // max = [f32::MIN] (min > max) — invalid glTF. Fail with a clear message.
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(crate::Error::Export(
            "cannot export an empty mesh: the SDF produced no geometry (check the \
             sampling bounds and resolution)"
                .to_string(),
        ));
    }

    let is_glb = path.extension().is_some_and(|ext| ext == "glb");
    let mesh_buffer = build_mesh_buffer(mesh);

    if is_glb {
        let json = build_gltf_json(mesh, mesh_buffer.len(), None);
        write_glb(path, &json, &mesh_buffer)
    } else {
        // The JSON references the binary payload by file name, so derive both
        // from the target path (model.gltf -> model.bin).
        let bin_path = path.with_extension("bin");
        let bin_uri = bin_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                crate::Error::Export(format!("invalid export path: {}", path.display()))
            })?
            .to_string();
        let json = build_gltf_json(mesh, mesh_buffer.len(), Some(&bin_uri));
        std::fs::write(path, &json)?;
        std::fs::write(&bin_path, &mesh_buffer)?;
        Ok(())
    }
}

/// Interleave the mesh into the single glTF binary buffer layout:
/// positions, then normals, then UVs, then indices.
fn build_mesh_buffer(mesh: &Mesh) -> Vec<u8> {
    let positions_size = mesh.vertices.len() * 12;
    let normals_size = mesh.vertices.len() * 12;
    let uvs_size = mesh.vertices.len() * 8;
    let indices_size = mesh.indices.len() * 4;

    let mut buffer = Vec::with_capacity(positions_size + normals_size + uvs_size + indices_size);
    for v in &mesh.vertices {
        buffer.extend_from_slice(bytemuck::cast_slice(&v.position));
    }
    for v in &mesh.vertices {
        buffer.extend_from_slice(bytemuck::cast_slice(&v.normal));
    }
    for v in &mesh.vertices {
        buffer.extend_from_slice(bytemuck::cast_slice(&v.uv));
    }
    buffer.extend_from_slice(bytemuck::cast_slice(&mesh.indices));
    buffer
}

fn write_glb(path: &Path, json: &str, mesh_buffer: &[u8]) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let json_bytes = json.as_bytes();
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let bin_padding = (4 - (mesh_buffer.len() % 4)) % 4;

    let total_size = 12  // GLB header
        + 8 + json_bytes.len() + json_padding  // JSON chunk
        + 8 + mesh_buffer.len() + bin_padding; // BIN chunk

    let mut file = File::create(path)?;

    // GLB header
    file.write_all(b"glTF")?;
    file.write_all(&2u32.to_le_bytes())?;
    file.write_all(&(total_size as u32).to_le_bytes())?;

    // JSON chunk
    file.write_all(&((json_bytes.len() + json_padding) as u32).to_le_bytes())?;
    file.write_all(&0x4E4F_534A_u32.to_le_bytes())?; // "JSON"
    file.write_all(json_bytes)?;
    file.write_all(&vec![0x20u8; json_padding])?;

    // BIN chunk
    file.write_all(&((mesh_buffer.len() + bin_padding) as u32).to_le_bytes())?;
    file.write_all(&0x004E_4942_u32.to_le_bytes())?; // "BIN\0"
    file.write_all(mesh_buffer)?;
    file.write_all(&vec![0u8; bin_padding])?;

    Ok(())
}

/// Build the glTF JSON document. `bin_uri` is `None` for GLB (the buffer is
/// the embedded binary chunk) and the external `.bin` file name otherwise.
#[allow(clippy::too_many_lines)] // The JSON template reads best as one linear builder
fn build_gltf_json(mesh: &Mesh, buffer_size: usize, bin_uri: Option<&str>) -> String {
    use std::fmt::Write;

    let vertex_count = mesh.vertices.len();
    let index_count = mesh.indices.len();

    let positions_size = vertex_count * 12;
    let normals_size = vertex_count * 12;
    let uvs_size = vertex_count * 8;
    let indices_size = index_count * 4;

    let positions_offset = 0;
    let normals_offset = positions_size;
    let uvs_offset = normals_offset + normals_size;
    let indices_offset = uvs_offset + uvs_size;

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in &mesh.vertices {
        for i in 0..3 {
            min[i] = min[i].min(v.position[i]);
            max[i] = max[i].max(v.position[i]);
        }
    }

    let mut json = String::new();

    writeln_str!(json, "{{");
    writeln_str!(
        json,
        r#"  "asset": {{ "version": "2.0", "generator": "Soyuz" }},"#
    );
    writeln_str!(json, r#"  "scene": 0,"#);
    writeln_str!(json, r#"  "scenes": [{{ "nodes": [0] }}],"#);
    writeln_str!(json, r#"  "nodes": [{{ "mesh": 0 }}],"#);

    // Meshes
    writeln_str!(json, r#"  "meshes": [{{"#);
    writeln_str!(json, r#"    "primitives": [{{"#);
    writeln_str!(
        json,
        r#"      "attributes": {{ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 }},"#
    );
    writeln_str!(json, r#"      "indices": 3"#);
    writeln_str!(json, r#"    }}]"#);
    writeln_str!(json, r#"  }}],"#);

    // Accessors
    writeln_str!(json, r#"  "accessors": ["#);
    writeln_str!(
        json,
        r#"    {{ "bufferView": 0, "componentType": 5126, "count": {}, "type": "VEC3", "min": [{}, {}, {}], "max": [{}, {}, {}] }},"#,
        vertex_count,
        min[0],
        min[1],
        min[2],
        max[0],
        max[1],
        max[2]
    );
    writeln_str!(
        json,
        r#"    {{ "bufferView": 1, "componentType": 5126, "count": {}, "type": "VEC3" }},"#,
        vertex_count
    );
    writeln_str!(
        json,
        r#"    {{ "bufferView": 2, "componentType": 5126, "count": {}, "type": "VEC2" }},"#,
        vertex_count
    );
    writeln_str!(
        json,
        r#"    {{ "bufferView": 3, "componentType": 5125, "count": {}, "type": "SCALAR" }}"#,
        index_count
    );
    writeln_str!(json, r#"  ],"#);

    // Buffer views
    writeln_str!(json, r#"  "bufferViews": ["#);
    writeln_str!(
        json,
        r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }},"#,
        positions_offset,
        positions_size
    );
    writeln_str!(
        json,
        r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }},"#,
        normals_offset,
        normals_size
    );
    writeln_str!(
        json,
        r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }},"#,
        uvs_offset,
        uvs_size
    );
    writeln_str!(
        json,
        r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }}"#,
        indices_offset,
        indices_size
    );
    writeln_str!(json, r#"  ],"#);

    // Buffer
    if let Some(uri) = bin_uri {
        writeln_str!(
            json,
            r#"  "buffers": [{{ "uri": "{}", "byteLength": {} }}]"#,
            uri,
            buffer_size
        );
    } else {
        writeln_str!(
            json,
            r#"  "buffers": [{{ "byteLength": {} }}]"#,
            buffer_size
        );
    }

    writeln_str!(json, "}}");

    json
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Vertex;
    use glam::{Vec2, Vec3};

    fn create_test_mesh() -> Mesh {
        Mesh {
            vertices: vec![
                Vertex::new(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec2::new(0.0, 0.0)),
                Vertex::new(Vec3::new(1.0, 0.0, 0.0), Vec3::Y, Vec2::new(1.0, 0.0)),
                Vertex::new(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::new(0.0, 1.0)),
            ],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn test_export_basic() {
        let mesh = create_test_mesh();
        let temp_path = std::env::temp_dir().join("test_basic.glb");
        let result = export_gltf(&mesh, &temp_path);
        assert!(result.is_ok());
        assert!(temp_path.exists());
        std::fs::remove_file(&temp_path).ok();
    }

    #[test]
    fn test_export_gltf_bin_uri_matches_written_file() {
        let mesh = create_test_mesh();
        let temp_dir = std::env::temp_dir().join(format!("soyuz_gltf_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let temp_path = temp_dir.join("model.gltf");

        export_gltf(&mesh, &temp_path).expect("export should succeed");

        let json = std::fs::read_to_string(&temp_path).expect("gltf JSON should be readable");
        let root: serde_json::Value = serde_json::from_str(&json).expect("gltf JSON should parse");
        assert_eq!(
            root["buffers"][0]["uri"].as_str(),
            Some("model.bin"),
            "buffer uri must reference the .bin file actually written"
        );
        assert!(temp_dir.join("model.bin").exists());

        gltf::Gltf::open(&temp_path).expect("gltf crate should parse exported JSON");
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}

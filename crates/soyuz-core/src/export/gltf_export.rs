//! GLTF/GLB file export with full PBR material support

// String writing is infallible, so .expect() is safe here
// Large JSON builder function is intentionally a single unit
// Result wrapper kept for future error handling paths
#![allow(clippy::expect_used)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::uninlined_format_args)]

use crate::Result;

/// Helper macro for writing to a String buffer.
/// String writing is infallible, so we use `expect()` with a clear message.
macro_rules! write_str {
    ($dst:expr, $($arg:tt)*) => {
        write!($dst, $($arg)*).expect("String write is infallible")
    };
}

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
use crate::material::{Material, MeshWithMaterial, RasterizedMaterial};
use crate::mesh::Mesh;
use std::path::Path;

/// Export options for GLTF
#[derive(Debug, Clone)]
pub struct GltfExportOptions {
    /// Texture resolution for rasterized materials
    pub texture_size: u32,
    /// Whether to embed textures in the file
    pub embed_textures: bool,
    /// Whether to include material data
    pub include_material: bool,
}

impl Default for GltfExportOptions {
    fn default() -> Self {
        Self {
            texture_size: 1024,
            embed_textures: true,
            include_material: true,
        }
    }
}

/// Export a mesh to GLTF format (without material)
pub fn export_gltf(mesh: &Mesh, path: &Path) -> Result<()> {
    export_gltf_with_options(mesh, None, path, &GltfExportOptions::default())
}

/// Export a mesh with material to GLTF format
pub fn export_gltf_with_material(mesh_mat: &MeshWithMaterial, path: &Path) -> Result<()> {
    export_gltf_with_options(
        &mesh_mat.mesh,
        Some(&mesh_mat.material),
        path,
        &GltfExportOptions::default(),
    )
}

/// Export a mesh with optional material and custom options
pub fn export_gltf_with_options(
    mesh: &Mesh,
    material: Option<&Material>,
    path: &Path,
    options: &GltfExportOptions,
) -> Result<()> {
    let is_glb = path.extension().is_some_and(|ext| ext == "glb");

    // Rasterize material if present
    let rasterized = if options.include_material {
        material.map(|m| m.rasterize(options.texture_size))
    } else {
        None
    };

    // Build the GLTF structure
    let gltf_data = build_gltf_data(mesh, material, rasterized.as_ref(), is_glb, options)?;

    if is_glb {
        write_glb(path, &gltf_data)?;
    } else {
        write_gltf_separate(path, &gltf_data)?;
    }

    Ok(())
}

/// All data needed for GLTF export
struct GltfData {
    json: String,
    mesh_buffer: Vec<u8>,
    texture_buffers: Vec<Vec<u8>>,
    #[allow(dead_code)]
    external_bin_uri: Option<String>,
}

fn build_gltf_data(
    mesh: &Mesh,
    material: Option<&Material>,
    rasterized: Option<&RasterizedMaterial>,
    is_glb: bool,
    _options: &GltfExportOptions,
) -> Result<GltfData> {
    // Calculate buffer sizes
    let positions_size = mesh.vertices.len() * 12;
    let normals_size = mesh.vertices.len() * 12;
    let uvs_size = mesh.vertices.len() * 8;
    let indices_size = mesh.indices.len() * 4;

    // Calculate bounds
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in &mesh.vertices {
        for i in 0..3 {
            min[i] = min[i].min(v.position[i]);
            max[i] = max[i].max(v.position[i]);
        }
    }

    // Build mesh buffer
    let mut mesh_buffer =
        Vec::with_capacity(positions_size + normals_size + uvs_size + indices_size);

    for v in &mesh.vertices {
        mesh_buffer.extend_from_slice(bytemuck::cast_slice(&v.position));
    }
    for v in &mesh.vertices {
        mesh_buffer.extend_from_slice(bytemuck::cast_slice(&v.normal));
    }
    for v in &mesh.vertices {
        mesh_buffer.extend_from_slice(bytemuck::cast_slice(&v.uv));
    }
    mesh_buffer.extend_from_slice(bytemuck::cast_slice(&mesh.indices));

    // Build texture buffers if we have a rasterized material
    let mut texture_buffers = Vec::new();
    let mut texture_info = Vec::new();

    if let Some(rast) = rasterized {
        let tex_bytes = rast.as_png_bytes();

        // Albedo texture (always present)
        texture_buffers.push(tex_bytes.albedo);
        texture_info.push(("baseColorTexture", texture_buffers.len() - 1));

        // Metallic-roughness texture
        texture_buffers.push(tex_bytes.metallic_roughness);
        texture_info.push(("metallicRoughnessTexture", texture_buffers.len() - 1));

        // Normal map
        if let Some(normal) = tex_bytes.normal {
            texture_buffers.push(normal);
            texture_info.push(("normalTexture", texture_buffers.len() - 1));
        }

        // Emissive
        if let Some(emissive) = tex_bytes.emissive {
            texture_buffers.push(emissive);
            texture_info.push(("emissiveTexture", texture_buffers.len() - 1));
        }
    }

    // Build JSON
    let json = build_gltf_json_with_material(
        mesh.vertices.len(),
        mesh.indices.len(),
        mesh_buffer.len(),
        &min,
        &max,
        positions_size,
        normals_size,
        uvs_size,
        material,
        &texture_info,
        &texture_buffers,
        is_glb,
    );

    Ok(GltfData {
        json,
        mesh_buffer,
        texture_buffers,
        external_bin_uri: if is_glb {
            None
        } else {
            Some("mesh.bin".to_string())
        },
    })
}

fn write_glb(path: &Path, data: &GltfData) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let json_bytes = data.json.as_bytes();
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;

    // Calculate total binary size (mesh buffer + all textures)
    let mut total_bin_size = data.mesh_buffer.len();
    for tex in &data.texture_buffers {
        // Add padding between buffers
        let padding = (4 - (total_bin_size % 4)) % 4;
        total_bin_size += padding + tex.len();
    }
    let bin_padding = (4 - (total_bin_size % 4)) % 4;

    let total_size = 12  // GLB header
        + 8 + json_bytes.len() + json_padding  // JSON chunk
        + 8 + total_bin_size + bin_padding; // BIN chunk

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
    file.write_all(&((total_bin_size + bin_padding) as u32).to_le_bytes())?;
    file.write_all(&0x004E_4942_u32.to_le_bytes())?; // "BIN\0"
    file.write_all(&data.mesh_buffer)?;

    // Write texture data with padding
    let mut current_offset = data.mesh_buffer.len();
    for tex in &data.texture_buffers {
        let padding = (4 - (current_offset % 4)) % 4;
        file.write_all(&vec![0u8; padding])?;
        file.write_all(tex)?;
        current_offset += padding + tex.len();
    }

    file.write_all(&vec![0u8; bin_padding])?;

    Ok(())
}

fn write_gltf_separate(path: &Path, data: &GltfData) -> Result<()> {
    // Write JSON file
    std::fs::write(path, &data.json)?;

    // Write binary file
    let bin_path = path.with_extension("bin");
    std::fs::write(&bin_path, &data.mesh_buffer)?;

    // Write texture files
    let parent = path.parent().unwrap_or(Path::new("."));
    for (i, tex) in data.texture_buffers.iter().enumerate() {
        let tex_path = parent.join(format!("texture_{}.png", i));
        std::fs::write(&tex_path, tex)?;
    }

    Ok(())
}

#[allow(clippy::needless_raw_string_hashes)] // Raw strings are more readable for JSON templates
fn build_gltf_json_with_material(
    vertex_count: usize,
    index_count: usize,
    mesh_buffer_size: usize,
    min: &[f32; 3],
    max: &[f32; 3],
    positions_size: usize,
    normals_size: usize,
    uvs_size: usize,
    material: Option<&Material>,
    texture_info: &[(&str, usize)],
    texture_buffers: &[Vec<u8>],
    is_glb: bool,
) -> String {
    use std::fmt::Write;

    let mut json = String::new();

    let positions_offset = 0;
    let normals_offset = positions_size;
    let uvs_offset = normals_offset + normals_size;
    let indices_offset = uvs_offset + uvs_size;
    let indices_size = index_count * 4;

    // Calculate texture buffer offsets
    let mut texture_offsets = Vec::new();
    let mut current_offset = mesh_buffer_size;
    for tex in texture_buffers {
        let padding = (4 - (current_offset % 4)) % 4;
        current_offset += padding;
        texture_offsets.push(current_offset);
        current_offset += tex.len();
    }

    let total_buffer_size = current_offset;

    // Start JSON
    writeln_str!(json, "{{");
    writeln_str!(
        json,
        r#"  "asset": {{ "version": "2.0", "generator": "Soyuz" }},"#
    );
    writeln_str!(json, r#"  "scene": 0,"#);
    writeln_str!(json, r#"  "scenes": [{{ "nodes": [0] }}],"#);
    writeln_str!(json, r#"  "nodes": [{{ "mesh": 0 }}],"#);

    // Meshes
    let material_idx = if material.is_some() {
        r#", "material": 0"#
    } else {
        ""
    };
    writeln_str!(json, r#"  "meshes": [{{"#);
    writeln_str!(json, r#"    "primitives": [{{"#);
    writeln_str!(
        json,
        r#"      "attributes": {{ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 }},"#
    );
    writeln_str!(json, r#"      "indices": 3{}"#, material_idx);
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
    write_str!(
        json,
        r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }}"#,
        indices_offset,
        indices_size
    );

    // Add buffer views for textures
    for (offset, tex) in texture_offsets.iter().zip(texture_buffers.iter()) {
        writeln_str!(json, ",");
        write_str!(
            json,
            r#"    {{ "buffer": 0, "byteOffset": {}, "byteLength": {} }}"#,
            offset,
            tex.len()
        );
    }
    writeln_str!(json);
    writeln_str!(json, r#"  ],"#);

    // Materials
    if let Some(mat) = material {
        writeln_str!(json, r#"  "materials": [{{"#);
        writeln_str!(json, r#"    "pbrMetallicRoughness": {{"#);

        // Base color
        let base_color = mat.base_color_factor();
        write_str!(
            json,
            r#"      "baseColorFactor": [{}, {}, {}, {}]"#,
            base_color[0],
            base_color[1],
            base_color[2],
            base_color[3]
        );

        // Base color texture
        if texture_info
            .iter()
            .any(|(name, _)| *name == "baseColorTexture")
        {
            writeln_str!(json, ",");
            write_str!(json, r#"      "baseColorTexture": {{ "index": 0 }}"#);
        }

        // Metallic-roughness texture
        if texture_info
            .iter()
            .any(|(name, _)| *name == "metallicRoughnessTexture")
        {
            writeln_str!(json, ",");
            write_str!(
                json,
                r#"      "metallicRoughnessTexture": {{ "index": 1 }}"#
            );
        }

        writeln_str!(json, ",");
        writeln_str!(
            json,
            r#"      "metallicFactor": {},"#,
            mat.metallic_factor()
        );
        writeln_str!(
            json,
            r#"      "roughnessFactor": {}"#,
            mat.roughness_factor()
        );
        writeln_str!(json, r#"    }}"#);

        // Normal texture
        if texture_info
            .iter()
            .any(|(name, _)| *name == "normalTexture")
        {
            let idx = texture_info
                .iter()
                .position(|(name, _)| *name == "normalTexture")
                .expect("normalTexture should exist after any() check");
            writeln_str!(json, r#"    ,"normalTexture": {{ "index": {} }}"#, idx);
        }

        // Emissive
        if texture_info
            .iter()
            .any(|(name, _)| *name == "emissiveTexture")
        {
            let idx = texture_info
                .iter()
                .position(|(name, _)| *name == "emissiveTexture")
                .expect("emissiveTexture should exist after any() check");
            writeln_str!(json, r#"    ,"emissiveTexture": {{ "index": {} }}"#, idx);
            writeln_str!(json, r#"    ,"emissiveFactor": [1.0, 1.0, 1.0]"#);
        }

        writeln_str!(json, r#"  }}],"#);
    }

    // Textures and images
    if !texture_buffers.is_empty() {
        writeln_str!(json, r#"  "textures": ["#);
        for i in 0..texture_buffers.len() {
            if i > 0 {
                writeln_str!(json, ",");
            }
            write_str!(json, r#"    {{ "source": {}, "sampler": 0 }}"#, i);
        }
        writeln_str!(json);
        writeln_str!(json, r#"  ],"#);

        writeln_str!(json, r#"  "samplers": [{{"#);
        writeln_str!(json, r#"    "magFilter": 9729,"#); // LINEAR
        writeln_str!(json, r#"    "minFilter": 9987,"#); // LINEAR_MIPMAP_LINEAR
        writeln_str!(json, r#"    "wrapS": 10497,"#); // REPEAT
        writeln_str!(json, r#"    "wrapT": 10497"#); // REPEAT
        writeln_str!(json, r#"  }}],"#);

        writeln_str!(json, r#"  "images": ["#);
        for i in 0..texture_buffers.len() {
            if i > 0 {
                writeln_str!(json, ",");
            }
            if is_glb {
                write_str!(
                    json,
                    r#"    {{ "bufferView": {}, "mimeType": "image/png" }}"#,
                    4 + i
                );
            } else {
                write_str!(json, r#"    {{ "uri": "texture_{}.png" }}"#, i);
            }
        }
        writeln_str!(json);
        writeln_str!(json, r#"  ],"#);
    }

    // Buffer
    if is_glb {
        writeln_str!(
            json,
            r#"  "buffers": [{{ "byteLength": {} }}]"#,
            total_buffer_size
        );
    } else {
        writeln_str!(
            json,
            r#"  "buffers": [{{ "uri": "mesh.bin", "byteLength": {} }}]"#,
            mesh_buffer_size
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
    fn test_export_with_material() {
        let mesh = create_test_mesh();
        let material = Material::pbr()
            .albedo_color(0.8, 0.2, 0.2)
            .roughness(0.5)
            .metallic(0.0);

        let mesh_mat = MeshWithMaterial::new(mesh, material);
        let temp_path = std::env::temp_dir().join("test_material.glb");
        let result = export_gltf_with_material(&mesh_mat, &temp_path);
        assert!(result.is_ok());
        assert!(temp_path.exists());
        std::fs::remove_file(&temp_path).ok();
    }
}

//! OBJ file export

#![allow(clippy::uninlined_format_args)]

use crate::Result;
use crate::mesh::Mesh;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export a mesh to OBJ format
pub fn export_obj(mesh: &Mesh, path: &Path) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Header
    writeln!(writer, "# Soyuz OBJ Export")?;
    writeln!(writer, "# Vertices: {}", mesh.vertices.len())?;
    writeln!(writer, "# Triangles: {}", mesh.indices.len() / 3)?;
    writeln!(writer)?;

    // Vertices
    for v in &mesh.vertices {
        writeln!(
            writer,
            "v {} {} {}",
            v.position[0], v.position[1], v.position[2]
        )?;
    }
    writeln!(writer)?;

    // Texture coordinates
    for v in &mesh.vertices {
        writeln!(writer, "vt {} {}", v.uv[0], v.uv[1])?;
    }
    writeln!(writer)?;

    // Normals
    for v in &mesh.vertices {
        writeln!(writer, "vn {} {} {}", v.normal[0], v.normal[1], v.normal[2])?;
    }
    writeln!(writer)?;

    // Faces (OBJ uses 1-based indexing)
    for tri in mesh.indices.chunks(3) {
        let i0 = tri[0] + 1;
        let i1 = tri[1] + 1;
        let i2 = tri[2] + 1;
        writeln!(
            writer,
            "f {}/{}/{} {}/{}/{} {}/{}/{}",
            i0, i0, i0, i1, i1, i1, i2, i2, i2
        )?;
    }

    writer.flush()?;
    Ok(())
}

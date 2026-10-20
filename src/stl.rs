//! Binary STL uses millimetre coordinates; the format itself does not encode a unit.
use crate::geometry::Mesh;
use std::io::{self, Write};
pub fn write_binary(mut writer: impl Write, meshes: &[&Mesh]) -> io::Result<()> {
    let count: usize = meshes.iter().map(|m| m.triangles().count()).sum();
    let count = u32::try_from(count)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Trop de triangles"))?;
    let mut header = [0u8; 80];
    let text = b"SolidCraft Rust | coordinates in millimeters";
    header[..text.len()].copy_from_slice(text);
    writer.write_all(&header)?;
    writer.write_all(&count.to_le_bytes())?;
    for mesh in meshes {
        for triangle in mesh.triangles() {
            let normal = (triangle[1] - triangle[0])
                .cross(triangle[2] - triangle[0])
                .normalized();
            for vector in std::iter::once(normal).chain(triangle) {
                for v in vector.array() {
                    let v = v as f32;
                    if !v.is_finite() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Coordonnee non finie",
                        ));
                    }
                    writer.write_all(&v.to_le_bytes())?;
                }
            }
            writer.write_all(&0u16.to_le_bytes())?;
        }
    }
    writer.flush()
}

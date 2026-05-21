use core::{alloc::Layout, ffi::c_void};

use aligned_vec::{AVec, ConstAlign, avec};
use alloc::{
    alloc::dealloc,
    format, slice,
    string::{String, ToString},
    sync::{Arc, Weak},
};
use bevy_ecs::component::Component;
use bytemuck::{Pod, Zeroable};
use psp::sys::{GuPrimitive, IoOpenFlags, TexturePixelFormat, sceIoClose, sceIoRead};

use crate::asset_handling::{Asset, IoError, fio::File, primitives, server::TextureHandle};

// #[derive(Clone)]
// pub struct Primitive {
//     name: String,
//     data: MeshData,
// }
//
// impl Primitive {
//     pub fn new(name: &str, data: MeshData) -> Self {
//         Primitive {
//             name: name.to_string(),
//             data,
//         }
//     }
// }
//
// impl Asset for Primitive {
//     type Output = Arc<MeshData>;
//
//     fn name(&self) -> String {
//         self.name.clone()
//     }
//     fn load(&self) -> Result<Arc<MeshData>, IoError> {
//         Ok(Arc::new(self.data.clone()))
//     }
// }

// #[repr(C, align(4))]
#[derive(Clone)]
pub struct MeshData {
    pub vertices: AVec<super::primitives::Vertex, ConstAlign<16>>,
    pub indices: Option<AVec<u16, ConstAlign<16>>>,
    pub primitive_type: GuPrimitive,
}

impl Default for MeshData {
    fn default() -> Self {
        MeshData {
            vertices: AVec::new(16),
            indices: None,
            primitive_type: GuPrimitive::Triangles,
        }
    }
}

/// Mesh object supports loading custom object definition files
#[derive(Clone, Eq, PartialEq)]
pub struct MeshAsset {
    path: String,
}

impl MeshAsset {
    pub fn new(path: &'_ str) -> Self {
        MeshAsset {
            path: String::from(path),
        }
    }
}

impl Asset for MeshAsset {
    type Output = Arc<MeshData>;

    fn name(&self) -> String {
        self.path.split(['/', '\\']).last().unwrap().to_string()
    }

    fn path(&self) -> String {
        self.path.clone()
    }

    fn load(&self) -> Result<Self::Output, IoError> {
        unsafe {
            let file = File::new(self.path.clone(), IoOpenFlags::RD_ONLY)?;

            let layout = Layout::from_size_align(file.size() as usize, 16)
                .map_err(|e| IoError(format!("Error in creating final layout: {}", e)))?;
            let buf = alloc::alloc::alloc(layout) as *mut c_void;
            if sceIoRead(file.fd(), buf, file.size() as u32) < 0 {
                dealloc(buf as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not read file \"{}\" of size: {}",
                    self.path,
                    file.size()
                )));
            }

            if sceIoClose(file.fd()) < 0 {
                dealloc(buf as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not close file \"{}\" of size: {}",
                    self.path,
                    file.size()
                )));
            }

            let bytes = slice::from_raw_parts(buf as *const u8, file.size() as usize);
            let mut cursor = 0;

            /// Helper for extracting a u32 size byte
            let extract_u32 = |bytes: &[u8], c: usize| -> usize {
                u32::from_le_bytes(bytes[c..c + 4].try_into().unwrap()) as usize
            };

            // magic check
            assert_eq!(&bytes[0..4], b"MESH");
            cursor += 4;
            let num_meshes = extract_u32(&bytes, cursor);
            // mesh_count
            cursor += 4;

            // first mesh only
            let name_len = extract_u32(&bytes, cursor);
            cursor += 4 + name_len;

            // Extract vertex count for mesh
            let vertex_count = extract_u32(&bytes, cursor);
            cursor += 4;

            // Extract index count
            let index_count = extract_u32(&bytes, cursor);
            cursor += 4;

            let _primitive_type = bytes[cursor];
            cursor += 1; // First byte prim type
            cursor += 3; // 3 bytes padding (4 byte alignment)

            assert_eq!(20, size_of::<primitives::Vertex>());
            let vertex_bytes = &bytes[cursor..cursor + vertex_count * 20];
            cursor += vertex_count * 20; // Number of vertices * 20 bytes each
            let index_bytes = &bytes[cursor..cursor + index_count * 2];

            let verts: AVec<super::primitives::Vertex, ConstAlign<16>> =
                AVec::from_slice(16, bytemuck::cast_slice(vertex_bytes));
            let indices: AVec<u16, ConstAlign<16>> =
                AVec::from_slice(16, bytemuck::cast_slice(index_bytes));
            dealloc(buf as *mut u8, layout);

            Ok(Arc::new(MeshData {
                vertices: verts,
                indices: Some(indices),
                primitive_type: GuPrimitive::Triangles,
            }))
        }
    }
}

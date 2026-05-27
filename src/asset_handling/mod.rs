use aligned_vec::{AVec, ConstAlign};
use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use bevy_ecs::component::Component;
use core::fmt;
use psp::sys::{GuPrimitive, TexturePixelFormat};

mod fio;
mod image;
pub mod mesh;
mod primitives;
pub mod server;
pub mod texture;

use server::AssetServer;

use crate::asset_handling::{
    mesh::MeshData,
    server::{MeshHandle, TextureHandle},
};

pub use primitives::{ColorVertex, Vertex};

#[derive(Debug, Clone)]
pub struct IoError(String);

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Io Error; could not load file: {}", self.0)
    }
}

/// Different from the ['Texture'] component, the Mesh component holds a strong reference to the
/// data itself, so that asset_server access is not needed when using primitive shapes.
#[repr(C, align(4))]
#[derive(Clone, Component)]
pub struct Mesh {
    data: Arc<MeshData>,
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh {
            data: Arc::new(MeshData::default()),
        }
    }
}

impl Mesh {
    pub fn from_handle(handle: &MeshHandle) -> Option<Self> {
        handle.get().map(|arc| Mesh { data: arc })
    }

    // Don't know if I want this to be public yet
    fn from_data(data: MeshData) -> Self {
        Mesh {
            data: Arc::new(data),
        }
    }

    pub fn plane(x: f32, y: f32) -> Self {
        Self::from_data(primitives::plane(x, y))
    }
    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        Self::from_data(primitives::cuboid(x, y, z))
    }
    pub fn subdivided_plane(x: f32, y: f32, sx: usize, sy: usize) -> Self {
        Self::from_data(primitives::subdivided_plane(x, y, sx, sy))
    }
    pub fn cube(length: f32) -> Self {
        Self::from_data(primitives::cube(length))
    }
    pub fn cube_indexed(length: f32) -> Self {
        Self::from_data(primitives::cube_indexed(length))
    }
    pub fn indices(&self) -> &Option<AVec<u16, ConstAlign<16>>> {
        &self.data.indices
    }
    pub fn vertices(&self) -> &AVec<Vertex, ConstAlign<16>> {
        &self.data.vertices
    }
    pub fn primitive_type(&self) -> GuPrimitive {
        self.data.primitive_type
    }
}

#[repr(C, align(4))]
#[derive(Clone, Component)]
pub struct Material {
    handle: Option<TextureHandle>,
    texture_format: TexturePixelFormat,
    blend: bool,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            handle: None,
            texture_format: TexturePixelFormat::PsmT4,
            blend: false,
        }
    }
}

impl Material {
    pub fn new(handle: TextureHandle, texture_format: TexturePixelFormat, blend: bool) -> Self {
        Material {
            handle: Some(handle),
            texture_format,
            blend: blend,
        }
    }

    pub fn handle(&self) -> Option<&TextureHandle> {
        self.handle.as_ref()
    }

    pub fn texture_format(&self) -> TexturePixelFormat {
        self.texture_format
    }
    pub fn blend(&self) -> bool {
        self.blend
    }
}

/// This trait will be used for abstracting separate classes of assets. These functions are required
/// to be used with the assetserver
pub trait Asset {
    type Output: Clone;

    /// ID String of asset
    fn name(&self) -> String;

    /// Returns path URL string
    fn path(&self) -> String {
        "Placeholder".to_string()
    }

    /// Loads asset from asset_path to memory
    fn load(&self) -> Result<Self::Output, IoError>;
}

/// This trait extension ensures a store and retrieve implementation for assets from the asset server hashmap(s)
pub trait Storable: Asset {
    /// What type of Handle is associated with this asset type
    type Handle: Clone;

    /// Returns a handle to given data
    fn to_handle(arc: &Self::Output) -> Self::Handle;

    /// Stores handle in AssetServer
    fn store(server: &mut AssetServer, name: String, data: Self::Output);

    /// Retrieves a handle from the AssetServer
    fn retrieve(server: &AssetServer, name: &str) -> Option<Self::Output>;

    /// Returns (strong, weak) reference counts
    fn ref_counts(handle: &Self::Output) -> (usize, usize);

    /// Drops elements with insufficient number of references
    fn drop_unused(server: &mut AssetServer);
}

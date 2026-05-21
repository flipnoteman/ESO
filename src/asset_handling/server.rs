use alloc::{
    string::String,
    sync::{Arc, Weak},
};
use bevy_ecs::resource::Resource;
use hashbrown::HashMap;

use super::{Asset, IoError, Storable, mesh::*, texture::*};

impl Storable for Texture {
    type Handle = TextureHandle;

    fn to_handle(arc: &Self::Output) -> Self::Handle {
        TextureHandle(Arc::downgrade(arc))
    }

    fn store(server: &mut AssetServer, name: String, data: Self::Output) {
        server.texture_map.insert(name, data);
    }

    fn retrieve(server: &AssetServer, name: &str) -> Option<Self::Output> {
        server.texture_map.get(name).cloned()
    }

    fn ref_counts(data: &Self::Output) -> (usize, usize) {
        (Arc::strong_count(&data), Arc::weak_count(&data))
    }

    fn drop_unused(server: &mut AssetServer) {
        server
            .texture_map
            .retain(|_, data| Arc::weak_count(data) > 0);
    }
}

impl Storable for MeshAsset {
    type Handle = MeshHandle;

    fn to_handle(arc: &Self::Output) -> Self::Handle {
        MeshHandle(arc.clone())
    }

    fn store(server: &mut AssetServer, name: String, data: Self::Output) {
        server.mesh_map.insert(name, data);
    }

    fn retrieve(server: &AssetServer, name: &str) -> Option<Self::Output> {
        server.mesh_map.get(name).cloned()
    }

    fn ref_counts(data: &Self::Output) -> (usize, usize) {
        (Arc::strong_count(&data), Arc::weak_count(&data))
    }

    fn drop_unused(server: &mut AssetServer) {
        server
            .mesh_map
            .retain(|_, data| Arc::strong_count(data) > 1);
    }
}

#[derive(Clone)]
pub struct TextureHandle(Weak<TextureData>);
#[derive(Clone)]
pub struct MeshHandle(Arc<MeshData>);

impl TextureHandle {
    pub fn get(&self) -> Option<Arc<TextureData>> {
        self.0.upgrade()
    }

    pub fn is_alive(&self) -> bool {
        self.0.strong_count() > 0
    }
}

impl MeshHandle {
    pub fn get(&self) -> Option<Arc<MeshData>> {
        Some(self.0.clone())
    }
}

#[derive(Resource)]
pub struct AssetServer {
    texture_map: HashMap<String, Arc<TextureData>>,
    mesh_map: HashMap<String, Arc<MeshData>>,
}

impl Default for AssetServer {
    fn default() -> Self {
        AssetServer {
            texture_map: HashMap::new(),
            mesh_map: HashMap::new(),
        }
    }
}

impl AssetServer {
    pub fn add<A>(&mut self, asset: A) -> Result<A::Handle, IoError>
    where
        A: Storable,
    {
        // Check for existing asset with same path
        if let Some(e) = A::retrieve(self, &asset.name()) {
            // If exists, return weak handle to it
            return Ok(A::to_handle(&e));
        }

        // Load asset; get raw bytes; returns handle
        let data = asset.load()?;
        A::store(self, asset.name(), data.clone());

        // Return a weak handle for new data
        Ok(A::to_handle(&data))
    }

    /// Returns the size of the inner texture map
    pub fn size(&self) -> usize {
        self.texture_map.len()
    }

    /// Get a strong handle to the texture
    pub fn get<A: Storable>(&self, key: &'_ str) -> Option<A::Output> {
        A::retrieve(self, key)
    }

    /// Check for the amount of references to a given asset
    pub fn check_references<A: Storable>(&self, key: &str) -> Option<(usize, usize)> {
        A::retrieve(self, key).map(|data| A::ref_counts(&data))
    }

    /// Drop textures that no longer have external references
    pub fn drop_unused(&mut self) {
        Texture::drop_unused(self);
        MeshAsset::drop_unused(self);
    }
}

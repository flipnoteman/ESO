use aligned_vec::{AVec, ConstAlign};
use alloc::{boxed::Box, string::{String, ToString}, sync::Arc};
use hashbrown::HashMap;
use bevy_ecs::resource::Resource;

// A texture handle object that the user will actually interact with.
#[derive(Clone)]
pub struct TextureHandle {
    index: Option<usize>,
}

impl TextureHandle {
    pub fn new(index: Option<usize>) -> Self {
        TextureHandle {
            index
        }
    }
}

pub trait Asset {
    fn bytes(&mut self) -> *const u8;

    fn dims(&mut self) -> (usize, usize);

    fn name(&self) -> String {
        "Placeholder".to_string()
    }
}

// TODO: If this image has no references we need to unload it, but also we need to add textures to
// gpu ram when possible, and unload when they are not needed anymore
#[derive(Clone, Eq, PartialEq)]
pub struct Image {
    bytes: AVec<u8, ConstAlign<16>>,
    width: usize,
    height: usize,
    path: String,
}

impl Asset for Image {
    fn bytes(&mut self) -> *const u8 {
        self.bytes.as_ptr()
    }

    fn dims(&mut self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn name(&self) -> String {
       self.path.split(['/', '\\']).last().unwrap().to_string()
    }
}

impl Image {
    pub fn new(path: &'static str) -> Self {
        Image {
            bytes: AVec::new(16),
            width: 0,
            height: 0,
            path: String::from(path)
        }
    }
}

#[derive(Resource)]
pub struct AssetServer {
    texture_buffer: AVec<u8, ConstAlign<16>>,
    texture_map: HashMap<String, Arc<TextureHandle>> 
}

impl Default for AssetServer {
    fn default() -> Self {
        AssetServer {
            texture_buffer: AVec::new(16),
            texture_map: HashMap::new(),
        }
    }

}


// TODO: Figure out loading of textures, and the disconnect between the Asset type and the
// TextureHandle
impl AssetServer {
    pub fn add(&mut self, asset: impl Asset) -> Arc<TextureHandle> {
        let th = Arc::new(TextureHandle::new(None));
        let ret = th.clone();
        
        self.texture_map.insert(asset.name(), th);

        ret 
    }

    pub fn get(&self, key: &'static str) -> Option<Arc<TextureHandle>> {
        let n = String::from(key); 
        
        match self.texture_map.get(&n) {
            Some(t) => Some(t.clone()),
            None => None
        }       
    }

    pub fn check_references(&self, key: &'static str) -> usize {
        Arc::strong_count(&self.get(&key).unwrap())
    }
}

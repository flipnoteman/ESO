use core::{alloc::Layout, ffi::c_void};

use aligned_vec::{AVec, ConstAlign};
use alloc::{alloc::dealloc, boxed::Box, ffi::CString, fmt, format, slice, string::{String, ToString}, sync::Arc, vec::Vec};
use hashbrown::HashMap;
use bevy_ecs::resource::Resource;
use psp::sys::{sceIoClose, sceIoGetstat, sceIoOpen, sceIoOpenAsync, sceIoRead, sceIoReadAsync, IoOpenFlags, SceIoStat, SceUid};

use crate::psp_image::load_png_swizzled;
use crate::psp_image::load_png;

// A texture handle object that the user will actually interact with.
#[derive(Clone, Debug)]
pub struct TextureHandle {
    width: usize,
    height: usize,
    pitch: usize,
    pixels: AVec<u8, ConstAlign<16>>,
}

pub struct File {
    fd: SceUid,
    size: i64,
}

// TODO: Look into using the async functions provided in the Io api
pub fn open_file(filepath: String, io_flags: IoOpenFlags) -> Result<File, IoError> {
    unsafe {
        
        let path = CString::new(filepath).map_err(|_| IoError(format!("{}", "Error in converting filepath to CString".to_string())))?;

        let stat_layout = Layout::new::<SceIoStat>();
        let stats = alloc::alloc::alloc_zeroed(stat_layout) as *mut SceIoStat;
        if sceIoGetstat(path.as_ptr() as *const u8, stats) < 0 {
            dealloc(stats as *mut u8, stat_layout);
            return Err(IoError(format!("Could not find file: {:?}", path)));
        }

        let fd = sceIoOpen(path.as_ptr() as *const u8, io_flags, 0777);
        if fd.0 < 0 { return Err(IoError(format!("Failed to open file: {:?}.", path))) }

        
        let size = (*stats).st_size;

        dealloc(stats as *mut u8, stat_layout);
        
        Ok(File {
            fd,
            size 
        }) 
    }
}

impl TextureHandle {
    pub fn new(width: usize, height: usize, pitch: usize, pixels: AVec<u8, ConstAlign<16>>) -> Self {
        TextureHandle {
            width,
            height,
            pitch,
            pixels,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pitch(&self) -> usize {
        self.pitch
    }

    pub fn raw_bytes(&self) -> *const c_void {
        self.pixels.as_ptr() as *const c_void
    }
}

pub trait Asset {
//     fn bytes(&mut self) -> *const u8;
    fn name(&self) -> String;

    fn path(&self) -> String {
        "Placeholder".to_string()
    }

    fn load(&self) -> Result<(usize, usize, usize, AVec<u8, ConstAlign<16>>), IoError>;
}

// TODO: If this image has no references we need to unload it, but also we need to add textures to
// gpu ram when possible, and unload when they are not needed anymore 
#[derive(Clone, Eq, PartialEq)]
pub struct Image {
    path: String,
}

/// Representation of a bitmap font on disk.
#[derive(Clone, Eq, PartialEq)]
pub struct Font {
    path: String,
}

impl Asset for Image {
    fn path(&self) -> String {
        self.path.clone()
    }

    fn name(&self) -> String {
       self.path.split(['/', '\\']).last().unwrap().to_string()
    }

    fn load(&self) -> Result<(usize, usize, usize, AVec<u8, ConstAlign<16>>), IoError> {
        unsafe {
            let fd = open_file(self.path.clone(), IoOpenFlags::RD_ONLY)?; 
             
            let layout = Layout::from_size_align(fd.size as usize, 16).map_err(|e| IoError(format!("Error in creating final layout: {}", e)))?;
            let handle = alloc::alloc::alloc(layout) as *mut c_void;
            if sceIoRead(fd.fd, handle, fd.size as u32) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!("Could not read file \"{}\" of size: {}", self.path, fd.size))); 
            }

            if sceIoClose(fd.fd) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!("Could not close file \"{}\" of size: {}", self.path, fd.size))); 
            }

            let (w, h, p, data) = load_png_swizzled(slice::from_raw_parts(handle as *const u8, fd.size as usize)).map_err(|e| IoError(format!("Could not load and swizzle the png: {}", e)))?;
            let t_d = AVec::from_slice(16, data.as_ref());

            // Free the temporary buffer holding the raw file data
            dealloc(handle as *mut u8, layout);

            return Ok((w as usize, h as usize, p as usize, t_d));
        }
    }
}

impl Image {
    pub fn new(path: &'_ str) -> Self {
        Image {
            path: String::from(path),
        }
    }
}

impl Asset for Font {
    fn path(&self) -> String {
        self.path.clone()
    }

    fn name(&self) -> String {
        self.path.split(['/', '\\']).last().unwrap().to_string()
    }

    fn load(&self) -> Result<(usize, usize, usize, AVec<u8, ConstAlign<16>>), IoError> {
        unsafe {
            let fd = open_file(self.path.clone(), IoOpenFlags::RD_ONLY)?;

            let layout = Layout::from_size_align(fd.size as usize, 16)
                .map_err(|e| IoError(format!("Error in creating final layout: {}", e)))?;
            let handle = alloc::alloc::alloc(layout) as *mut c_void;
            if sceIoRead(fd.fd, handle, fd.size as u32) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not read file \"{}\" of size: {}",
                    self.path, fd.size
                )));
            }

            if sceIoClose(fd.fd) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not close file \"{}\" of size: {}",
                    self.path, fd.size
                )));
            }

            let (w, h, p, data) =
                load_png(slice::from_raw_parts(handle as *const u8, fd.size as usize))
                    .map_err(|e| IoError(format!("Could not load the png: {}", e)))?;
            let t_d = AVec::from_slice(16, data.as_ref());

            dealloc(handle as *mut u8, layout);

            Ok((w as usize, h as usize, p as usize, t_d))
        }
    }
}

impl Font {
    pub fn new(path: &'_ str) -> Self {
        Font {
            path: String::from(path),
        }
    }
}

pub struct AssetEntry {
    handle: Arc<TextureHandle>
}

impl AssetEntry {
    #[allow(dead_code)]
    pub fn handle(&self) -> Arc<TextureHandle> {
        self.handle.clone()
    }
}


#[derive(Resource)]
pub struct AssetServer {
    texture_map: HashMap<String, AssetEntry> 
}

impl Default for AssetServer {
    fn default() -> Self {
        AssetServer {
            texture_map: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IoError(String);

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Io Error; could not load file: {}", self.0)
    }
}

// TextureHandle
impl AssetServer {
    pub fn add(&mut self, asset: impl Asset) -> Result<Arc<TextureHandle>, IoError> {

        let name = asset.name();

        // Check for existing asset with same path
        if let Some(e) = self.texture_map.get(&name) {
            return Ok(e.handle.clone())
        }

        // Load asset; get raw bytes
        let (w, h, pitch, decoded_bytes) = asset.load()?;
        
        // Generate a texture handle and a reference-counted pointer to that data and store it in
        // the AssetServer
        let th = TextureHandle::new(w, h, pitch, decoded_bytes);
        let handle = Arc::new(th);
        self.texture_map.insert(asset.name(), AssetEntry { handle: handle.clone() });
        
        // Return the handle
        Ok(handle) 
    }

    /// Returns the size of the inner texture map
    pub fn size(&self) -> usize {
        self.texture_map.len()
    }

    /// Get a strong handle to the texture
    pub fn get(&self, key: &'_ str) -> Option<Arc<TextureHandle>> {
        self.texture_map.get(key).map(|entry| entry.handle.clone())
    }

    /// Check for the amount of references to a given asset
    pub fn check_references(&self, key: &'static str) -> Option<(usize, usize)> {
        self.texture_map.get(key).map(|arc| (Arc::strong_count(&arc.handle), Arc::weak_count(&arc.handle)))
    }

    /// Drop textures that no longer have external references
    pub fn drop_unused(&mut self) {
        self.texture_map.retain(|_, entry| Arc::strong_count(&entry.handle) > 1 || Arc::weak_count(&entry.handle) > 0);
    }
}

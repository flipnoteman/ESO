use core::{alloc::Layout, ffi::c_void};

use super::image::{load_png, load_png_swizzled};
use super::{Asset, IoError, fio::File};

use aligned_vec::{AVec, ConstAlign};
use alloc::{
    alloc::dealloc,
    format, slice,
    string::{String, ToString},
    sync::Arc,
};
use psp::sys::{IoOpenFlags, sceIoClose, sceIoRead};

// A texture handle object that the user will actually interact with.
#[derive(Clone, Debug)]
pub struct TextureData {
    width: usize,
    height: usize,
    pitch: usize,
    swizzled: bool,
    pixels: AVec<u8, ConstAlign<16>>,
}

impl TextureData {
    pub fn new(
        width: usize,
        height: usize,
        pitch: usize,
        swizzled: bool,
        pixels: AVec<u8, ConstAlign<16>>,
    ) -> Self {
        TextureData {
            width,
            height,
            pitch,
            swizzled,
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

    pub fn is_swizzled(&self) -> bool {
        self.swizzled
    }
}

/// Image asset, supports png images, and swizzling textures.
#[derive(Clone, Eq, PartialEq)]
pub struct Texture {
    path: String,
    swizzle: bool,
}

impl Asset for Texture {
    type Output = Arc<TextureData>;

    fn path(&self) -> String {
        self.path.clone()
    }

    fn name(&self) -> String {
        self.path.split(['/', '\\']).last().unwrap().to_string()
    }

    fn load(&self) -> Result<Arc<TextureData>, IoError> {
        unsafe {
            let file = File::new(self.path.clone(), IoOpenFlags::RD_ONLY)?;

            let layout = Layout::from_size_align(file.size() as usize, 16)
                .map_err(|e| IoError(format!("Error in creating final layout: {}", e)))?;
            let handle = alloc::alloc::alloc(layout) as *mut c_void;
            if sceIoRead(file.fd(), handle, file.size() as u32) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not read file \"{}\" of size: {}",
                    self.path,
                    file.size()
                )));
            }

            if sceIoClose(file.fd()) < 0 {
                dealloc(handle as *mut u8, layout);
                return Err(IoError(format!(
                    "Could not close file \"{}\" of size: {}",
                    self.path,
                    file.size()
                )));
            }

            let (w, h, p, data) = match self.swizzle {
                true => load_png_swizzled(slice::from_raw_parts(
                    handle as *const u8,
                    file.size() as usize,
                ))
                .map_err(|e| IoError(format!("Could not load and swizzle the png: {}", e)))?,
                false => load_png(slice::from_raw_parts(
                    handle as *const u8,
                    file.size() as usize,
                ))
                .map_err(|e| IoError(format!("Could not load png: {}", e)))?,
            };

            let t_d = AVec::from_slice(16, data.as_ref());

            // Free the temporary buffer holding the raw file data
            dealloc(handle as *mut u8, layout);

            return Ok(Arc::new(TextureData::new(
                w as usize,
                h as usize,
                p,
                self.swizzle,
                t_d,
            )));
        }
    }
}

impl Texture {
    pub fn new(path: &'_ str, swizzle: bool) -> Self {
        Texture {
            path: String::from(path),
            swizzle,
        }
    }
}

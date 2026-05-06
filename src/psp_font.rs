use core::{alloc::Layout, ffi::c_void, mem::zeroed, ptr::slice_from_raw_parts};

use alloc::{alloc::dealloc, slice, string::{String, ToString}, vec::Vec};
use bevy_ecs::{ptr, system::Res};
use psp::{sys::{self, sceIoClose, sceIoRead, GuPrimitive, IoOpenFlags, MatrixMode, ScePspFVector3, TexturePixelFormat, VertexType}, vram_alloc::{get_vram_allocator, SimpleVramAllocator}};

use crate::{println, psp_assets::{open_file, Font}, psp_geometry::{self, Sprite}, VramAllocator};

pub fn load_font(name: String) -> fontdue::Font {
    unsafe {
        let file = open_file(name.clone(), IoOpenFlags::RD_ONLY).expect("Could not open font");

        let buf_layout = Layout::from_size_align(file.size as usize, 16).expect("Could not create font layout");
        let handle = alloc::alloc::alloc(buf_layout) as *mut c_void;
        if sceIoRead(file.fd, handle, file.size as u32) < 0 {
            dealloc(handle as *mut u8, buf_layout);
            panic!("Could not read file, {:?}", name);
        }

        if sceIoClose(file.fd) < 0 {
            dealloc(handle as *mut u8, buf_layout);
            panic!("Could not close file, {:?}", name);
        }
        let s = slice_from_raw_parts(handle as *const u8, file.size as usize).as_ref().unwrap();
        fontdue::Font::from_bytes(s, fontdue::FontSettings::default()).unwrap()
    }
}

pub fn test_font(allocator: Res<VramAllocator>) {
    
    let font = load_font("ms0:/psp/game/cat_dev/eso/assets/Roboto-Black.ttf".to_string());
    render_font(&allocator.0, "Hello World", &font); 
}

pub fn render_font(allocator: &SimpleVramAllocator, text: &str, font: &fontdue::Font) {

    let texture_buffer = allocator.alloc_texture_pixels(
        (text.len() * 64) as u32,
        64,
        TexturePixelFormat::Psm8888,
    );

    let texture_buffer = unsafe {
        slice::from_raw_parts_mut(
        texture_buffer.as_mut_ptr_direct_to_vram() as *mut u32,
        text.len() as usize * 64 * 64,
        )
    };
    
    let vertex_buffer = allocator.alloc_sized::<psp_geometry::Vertex>(text.len() as u32 * 2);
    let vertex_buffer = unsafe {
        slice::from_raw_parts_mut(
            vertex_buffer.as_mut_ptr_direct_to_vram() as *mut psp_geometry::Vertex,
            text.len() * 2,
        )
    };

    let mut layout = fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYUp);

    layout.reset(&fontdue::layout::LayoutSettings::default());
    layout.append(
        &[font],
        &fontdue::layout::TextStyle::new(text, 60.0, 0),
    );

    let x_positions = layout.glyphs()
        .iter()
        .map(|glyph| glyph.x as i32)
        .collect::<Vec<i32>>();

    let sprites: Vec<Option<(u32, Sprite)>> = text
        .chars()
        .enumerate()
        .map(|(i, letter)|{
            if !letter.is_whitespace() {
                let (metrics, bitmap) = font.rasterize(letter, 60.0);
                let padded_width = (metrics.width + 3) & !3;
                let diff = padded_width - metrics.width;
                
                let mut j = 0;

                for (k, alpha) in bitmap.iter().enumerate() {
                    if k % metrics.width == 0 {
                        j += diff;
                    }
                    texture_buffer[j + i * 64 * 64] = 0x00ff_ffff | (*alpha as u32) << 24;
                    j += 1;
                }

                let sprite = Sprite::new(
                    0xFFFFFFFF,
                    0,
                    0,
                    metrics.width as u32,
                    metrics.height as u32,
                );
                 

                Some((padded_width as u32, sprite))
            } else {
                None
            }
        }).collect();
    
    for i in 0..text.len() {
        if let Some((w, mut sprite)) = sprites[i] {
            sprite.set_pos(x_positions[i], 100);
            vertex_buffer[i * 2..i*2 + 2].copy_from_slice(&sprite.as_vertices());

            unsafe {
                sys::sceGumMatrixMode(MatrixMode::Model);
                sys::sceGumLoadIdentity();
                sys::sceGumScale(&ScePspFVector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                });

                sys::sceGuTexImage(
                    sys::MipmapLevel::None,
                    64,
                    64,
                    w as i32,
                    texture_buffer[i * 64 * 64..(i+1) * 64 * 64].as_ptr() as _,
                );

                sys::sceGuTexScale(1.0 / 64 as f32, 1.0 / 64 as f32);
                
                sys::sceKernelDcacheWritebackInvalidateAll();

                sys::sceGumDrawArray(
                    GuPrimitive::Sprites,
                    VertexType::TEXTURE_32BITF
                        | VertexType::VERTEX_32BITF
                        | VertexType::TRANSFORM_3D,
                    vertex_buffer[i * 2..i * 2 + 2].len() as i32,
                    core::ptr::null_mut(),
                    vertex_buffer[i * 2..i * 2 + 2].as_ptr() as _,
                );

                
            }
        }
    }
}
// pub fn load_font() {
//     unsafe {
//         let params = SceFontNewLibParams {
//             user_data_addr: 0,
//             num_fonts: 2,
//             cache_data: 0,
//             alloc_func: None,
//             free_func: None,
//             open_func: None,
//             close_func: None,
//             read_func: None,
//             seek_func: None,
//             error_func: None,
//             io_finish_func: None,
//         };
// 
//         let mut error = SceFontErrorCode::Success;
// 
//         // Setup a new font library
//         let fl_handle = sceFontNewLib(&params, &mut error);
// 
//         sceFontGetNumFontList(hhh, error_code)
// 
//         // Load font from file into a font lib
//         let font_handle = sceFontOpenUserFile(
//             fl_handle,
//             "../assets/fonts/roboto-black.pgf".as_ptr(),
//             3,
//             &mut error,
//         );
// 
//         let mut font_info = zeroed::<SceFontInfo>();
// 
//         let i = sceFontGetFontInfo(fl_handle, &mut font_info);
// 
//         println!("{:?}, {:?}, {:x}", font_info.num_glyphs, error, i);
// 
//         sceFontClose(font_handle);
//         sceFontDoneLib(fl_handle);
//     }
// }

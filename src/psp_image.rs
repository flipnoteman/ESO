use minipng::{decode_png, decode_png_header};
use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use alloc::boxed::Box;
use alloc::vec;
use core::ptr;


/// Load a PNG from `bytes`, transcode to ABGR8888,
/// and return (w, h, pitch_in_pixels, 16-byte-aligned Box<[u8]>).
pub unsafe fn load_png(bytes: &[u8])
    -> Result<(u32, u32, usize, Box<[u8]>), &'static str>
{
    // 1) Decode with minipng
    let header = decode_png_header(bytes).map_err(|_| "bad header")?;
    let mut buf = vec![0; header.required_bytes_rgba8bpc()];          // :contentReference[oaicite:1]{index=1}
    let mut img  = decode_png(bytes, &mut buf).map_err(|_| "decode")?;
    img.convert_to_rgba8bpc();

    let w  = img.width()  as usize;
    let h  = img.height() as usize;

    // 2) Compute GE-friendly pitch (multiple of 8 pixels ≡ 16 bytes)
    let pitch_px = (w + 7) & !7;          // round up to 8-pixel blocks
    let bytes_per_row = pitch_px * 4;
    let size = bytes_per_row * h;

    // 3) Allocate 16-byte-aligned heap block
    let layout = Layout::from_size_align(size, 16).map_err(|_| "layout")?;
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() { return Err("OOM"); }

    let src = img.pixels();
    for y in 0..h {
        let src_row = &src[y * w * 4 .. (y + 1) * w * 4];
        let dst = unsafe { ptr.add(y * bytes_per_row) };

        // copy row as-is – NO channel swap
        unsafe { ptr::copy_nonoverlapping(src_row.as_ptr(), dst, w * 4); }
    }

    // 5) Hand the memory to Rust
    let slice = unsafe { core::slice::from_raw_parts_mut(ptr, size) };
    Ok((w as u32, h as u32, pitch_px, Box::from_raw(slice)))
}

/// Load a PNG from `bytes`, transcode to **ABGR8888**, write it in
/// **Swizzled** order, and return
/// `(width_px /*= pitch*/, height_px, pitch_px, 16-byte-aligned data)`.
///
/// Upload with the *swizzle* bit set:
/// ```rust
/// sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 1); // swizzled = 1
/// sys::sceGuTexImage(0, width as i32, height as i32, pitch_px as i32, tex.as_ptr() as _);
/// ```
pub unsafe fn load_png_swizzled(
    bytes: &[u8],
) -> Result<(u32, u32, usize, Box<[u8]>), &'static str> {
    // 1) Decode with minipng
    let header = decode_png_header(bytes).map_err(|_| "bad header")?;
    let mut buf = vec![0; header.required_bytes_rgba8bpc()];          // :contentReference[oaicite:1]{index=1}
    let mut img  = decode_png(bytes, &mut buf).map_err(|_| "decode")?;
    img.convert_to_rgba8bpc();

    let w  = img.width()  as usize;
    let h  = img.height() as usize;

    // 2) Compute GE-friendly pitch (multiple of 8 pixels ≡ 16 bytes)
    let pitch_px = (w + 7) & !7;          // round up to 8-pixel blocks
    let bytes_per_row = pitch_px * 4;
    let size = bytes_per_row * h;

    // 3) Allocate 16-byte-aligned heap block
    let layout = Layout::from_size_align(size, 16).map_err(|_| "layout")?;
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() { return Err("OOM"); }

    let src = img.pixels();
    for y in 0..h {
        let src_row = &src[y * w * 4 .. (y + 1) * w * 4];
        let dst = unsafe { ptr.add(y * bytes_per_row) };

        // copy row as-is – NO channel swap
        unsafe { ptr::copy_nonoverlapping(src_row.as_ptr(), dst, w * 4); }
    }
    
    // 4) Allocate the destination buffer (swizzled layout) -------------------
    let swizzled_ptr = alloc_zeroed(layout);
    if swizzled_ptr.is_null() {
        dealloc(ptr, layout);
        return Err("OOM2");
    }
    // Swizzle the loaded image now that its in RAW format. The PSP swizzle format is pretty
    // simple, just broken up into 16x8 blocks
    let width_blocks = bytes_per_row / 16;
    let height_blocks = h / 8;

    let src_pitch = (bytes_per_row - 16) / 4;
    let src_row = bytes_per_row * 8;


    let mut ysrc = ptr as *const u8;
    let mut dst  = swizzled_ptr as *mut u32;

    for _ in 0..height_blocks {
        let mut xsrc = ysrc;

        for _ in 0..width_blocks {
            let mut src = xsrc as *const u32;

            // 8 rows × 4 dwords  →  16 × 8-pixel block
            for _ in 0..8 {
                *dst = *src; dst = dst.add(1); src = src.add(1);
                *dst = *src; dst = dst.add(1); src = src.add(1);
                *dst = *src; dst = dst.add(1); src = src.add(1);
                *dst = *src; dst = dst.add(1); src = src.add(1);

                src = src.add(src_pitch);
            }

            xsrc = xsrc.add(16); // next 16-pixel block in this scan-line
        }

        ysrc = ysrc.add(src_row); // next 8-pixel block row
    }
    // ------------------------------------------------------------------------

    // 5) Free the temporary linear buffer and return the swizzled one --------
    dealloc(ptr, layout);    // 5) Hand the memory to Rust
    
    let slice = unsafe { core::slice::from_raw_parts_mut(swizzled_ptr, size) };
    Ok((w as u32, h as u32, pitch_px, Box::from_raw(slice)))
}

// fn swizzle_fast(ptr: *const u8, dst: *mut u32, w: )




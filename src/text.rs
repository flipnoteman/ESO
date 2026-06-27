//! Bitmap text rendering for the 2D HUD layer.
//!
//! Text is drawn from a 16x16 ASCII font atlas (`default_font.png`, 128x128 px,
//! 8x8 px glyphs). The glyph for ASCII code `c` lives at column `c % 16`, row
//! `c / 16`. Strings are turned into one textured quad per glyph and submitted
//! in the orthographic HUD pass, so screen coordinates are in pixels with the
//! origin at the top-left (matching `sceGumOrtho(0, W, H, 0, ...)`).

use core::ptr::null;

use alloc::string::{String, ToString};
use bevy_ecs::{component::Component, system::Query};
use psp::sys::{
    self, GuPrimitive, GuState, MipmapLevel, TextureColorComponent, TextureEffect, TextureFilter,
    TexturePixelFormat, VertexType, sceGuBlendFunc, sceGuColor, sceGuDisable, sceGuEnable,
};

use crate::asset_handling::{Vertex, server::TextureHandle};

/// Font atlas layout constants.
const ATLAS_COLS: u8 = 16;
const ATLAS_ROWS: u8 = 16;
/// Size of one glyph cell in atlas UV space (1/16).
const GLYPH_UV: f32 = 1.0 / 16.0;
/// Native glyph size in pixels (8x8 cells in a 128x128 atlas).
const GLYPH_PX: f32 = 8.0;

/// Default opaque white (PSP color is 0xAABBGGRR). Modulated against the white
/// font atlas this renders the glyphs unchanged; any other value tints them.
const COLOR_WHITE: u32 = 0xFFFF_FFFF;

/// A string drawn to a screen-space (pixel) location in the HUD pass.
///
/// Spawn it as an entity on its own; `render_text` picks up every `Text` each
/// frame. Mutate `content`, `x`/`y`, `scale`, or `color` to change it live.
#[derive(Component, Clone)]
pub struct Text {
    /// The string to draw. `'\n'` starts a new line; ASCII only.
    pub content: String,
    /// Top-left x in screen pixels.
    pub x: f32,
    /// Top-left y in screen pixels.
    pub y: f32,
    /// Glyph scale multiplier; rendered glyph size is `8.0 * scale` px.
    pub scale: f32,
    /// RGBA tint in PSP 0xAABBGGRR format, modulated against the atlas.
    pub color: u32,
    /// Weak handle to the font atlas texture (keeps it alive in the AssetServer).
    pub font: TextureHandle,
}

impl Text {
    /// Create white text at the origin with 1x scale. Chain `.at`/`.scale`/`.color`.
    pub fn new(content: impl Into<String>, font: TextureHandle) -> Self {
        Text {
            content: content.into(),
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            color: COLOR_WHITE,
            font,
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }

    /// Pixel width of the widest line at the current scale.
    pub fn pixel_width(&self) -> f32 {
        let cell = GLYPH_PX * self.scale;
        let mut max = 0usize;
        let mut cur = 0usize;
        for ch in self.content.bytes() {
            if ch == b'\n' {
                max = max.max(cur);
                cur = 0;
            } else {
                cur += 1;
            }
        }
        max.max(cur) as f32 * cell
    }
}

/// UV rect (u0, v0) of a glyph's top-left corner in the atlas. Non-printable /
/// out-of-range codes fall back to the (blank) NUL cell.
#[inline]
fn glyph_uv(code: u8) -> (f32, f32) {
    let col = (code % ATLAS_COLS) as f32;
    let row = (code / ATLAS_COLS) as f32;
    let _ = ATLAS_ROWS; // documents the 16-row assumption
    (col * GLYPH_UV, row * GLYPH_UV)
}

/// Renders every `Text` entity. Runs in the orthographic HUD pass: it sets up
/// its own ortho projection and 2D state so it is order-independent from the
/// other HUD draws, then restores depth/cull state for the next frame's 3D pass.
pub(crate) fn render_text(query: Query<&Text>) {
    unsafe {
        // 2D state: no depth, no culling, alpha blending on.
        sys::sceGuDisable(GuState::DepthTest);
        sys::sceGuDisable(GuState::CullFace);
        sceGuEnable(GuState::Blend);
        sceGuBlendFunc(
            sys::BlendOp::Add,
            sys::BlendFactor::SrcAlpha,
            sys::BlendFactor::OneMinusSrcAlpha,
            0,
            0,
        );

        // Orthographic projection in screen pixels, origin top-left.
        sys::sceGumMatrixMode(sys::MatrixMode::Projection);
        sys::sceGumLoadIdentity();
        sys::sceGumOrtho(
            0.0,
            psp::SCREEN_WIDTH as f32,
            psp::SCREEN_HEIGHT as f32,
            0.0,
            -1.0,
            1.0,
        );
        sys::sceGumMatrixMode(sys::MatrixMode::View);
        sys::sceGumLoadIdentity();
        sys::sceGumMatrixMode(sys::MatrixMode::Model);
        sys::sceGumLoadIdentity();

        let vertex_type =
            VertexType::VERTEX_32BITF | VertexType::TEXTURE_32BITF | VertexType::TRANSFORM_3D;

        for text in query.iter() {
            // Resolve the font atlas; skip if it has been freed.
            let Some(atlas) = text.font.get() else {
                continue;
            };

            // Count printable glyphs so we can size the vertex buffer exactly.
            let glyph_count = text
                .content
                .bytes()
                .filter(|&c| c != b' ' && c != b'\n')
                .count();
            if glyph_count == 0 {
                continue;
            }

            // Bind the font atlas (HUD textures are NOT swizzled). Modulate so the
            // per-text color tints the white glyphs.
            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
            sys::sceGuTexImage(
                MipmapLevel::None,
                atlas.width() as i32,
                atlas.height() as i32,
                atlas.pitch() as i32,
                atlas.raw_bytes(),
            );
            sys::sceGuTexFunc(TextureEffect::Modulate, TextureColorComponent::Rgba);
            sys::sceGuTexFilter(TextureFilter::Nearest, TextureFilter::Nearest);
            sys::sceGuTexScale(1.0, 1.0);
            sys::sceGuTexOffset(0.0, 0.0);
            sceGuColor(text.color);

            // Allocate a DL-local vertex buffer: 6 verts (2 tris) per glyph.
            let vert_count = glyph_count * 6;
            let buf = sys::sceGuGetMemory(
                (vert_count * core::mem::size_of::<Vertex>()) as i32,
            ) as *mut Vertex;

            let cell = GLYPH_PX * text.scale;
            let mut pen_x = text.x;
            let mut pen_y = text.y;
            let mut i = 0usize;

            for code in text.content.bytes() {
                match code {
                    b'\n' => {
                        pen_x = text.x;
                        pen_y += cell;
                        continue;
                    }
                    b' ' => {
                        pen_x += cell;
                        continue;
                    }
                    _ => {}
                }

                let (u0, v0) = glyph_uv(code);
                let u1 = u0 + GLYPH_UV;
                let v1 = v0 + GLYPH_UV;
                let x0 = pen_x;
                let y0 = pen_y;
                let x1 = pen_x + cell;
                let y1 = pen_y + cell;

                // Two triangles (cull is disabled, so winding is irrelevant).
                let quad = [
                    Vertex { u: u0, v: v0, x: x0, y: y0, z: 0.0 },
                    Vertex { u: u0, v: v1, x: x0, y: y1, z: 0.0 },
                    Vertex { u: u1, v: v1, x: x1, y: y1, z: 0.0 },
                    Vertex { u: u0, v: v0, x: x0, y: y0, z: 0.0 },
                    Vertex { u: u1, v: v1, x: x1, y: y1, z: 0.0 },
                    Vertex { u: u1, v: v0, x: x1, y: y0, z: 0.0 },
                ];
                core::ptr::copy_nonoverlapping(quad.as_ptr(), buf.add(i * 6), 6);

                pen_x += cell;
                i += 1;
            }

            sys::sceGumDrawArray(
                GuPrimitive::Triangles,
                VertexType::from_bits_retain(vertex_type.bits()),
                vert_count as i32,
                null(),
                buf as *const _,
            );
        }

        // Reset flat color and restore 3D state for the next frame's world pass.
        sceGuColor(COLOR_WHITE);
        sceGuDisable(GuState::Blend);
        sceGuEnable(GuState::DepthTest);
        sceGuEnable(GuState::CullFace);
    }
}

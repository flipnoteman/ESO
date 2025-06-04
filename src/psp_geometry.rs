use aligned_vec::{AVec, ConstAlign, avec};
use alloc::{boxed::Box, string::String, sync::{Arc, Weak}};
use bevy_ecs::component::Component;
use psp::sys::{GuPrimitive, TexturePixelFormat};

use crate::{psp_assets::TextureHandle, psp_image::load_png_swizzled};

#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub u: f32,
    pub v: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C, align(4))]
#[derive(Clone, Component)]
pub struct Material {
    pub handle: Option<Weak<TextureHandle>>,
    pub texture_format: TexturePixelFormat,
    pub swizzle: bool,
    pub blend: bool,

}

impl Default for Material {
    fn default() -> Self {
        Material {
            handle: None,
            texture_format: TexturePixelFormat::PsmT4,
            swizzle: false,
            blend: false,
        }
    }
}

impl Material {
    pub fn new(handle: &Arc<TextureHandle>, texture_format: TexturePixelFormat, swizzle: bool, blend: bool) -> Self {
        Material {
            handle: Some(Arc::downgrade(handle)),
            texture_format,
            swizzle,
            blend
        }
    }
}

#[repr(C, align(4))]
#[derive(Component)]
pub struct Mesh {
    pub vertices: AVec<Vertex, ConstAlign<16>>,
    pub indices: Option<AVec<u16, ConstAlign<16>>>,
    pub primitive_type: GuPrimitive,
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh {
            vertices: AVec::new(16),
            indices: None,
            primitive_type: GuPrimitive::Triangles
        }
    }
}

#[inline]
const fn v(x: f32, y: f32, z: f32, u: f32, v: f32) -> Vertex {
    Vertex { u, v, x, y, z }
}

impl Mesh {
    /// 36-vertex (12-triangle) unit cube, centred at the origin.
    /// NON-INDEXED. self.indexed will == None after this call, resulting in more VRAM usage
    pub fn cube(size: f32) -> Mesh {
        let h = size * 0.5; // half-extent

        Mesh {
            vertices: avec!(
                [16] |
                // +Z face
                v(-h,-h, h, 0.0, 0.0),
                v(-h, h, h, 0.0, 1.0),
                v(h, h, h, 1.0, 1.0),
                v(-h, -h, h, 0.0, 0.0),
                v(h, h, h, 1.0, 1.0),
                v(h, -h, h, 1.0, 0.0),
                // –Z
                v(-h, -h, -h, 1.0, 0.0),
                v(h, -h, -h, 0.0, 0.0),
                v(h, h, -h, 0.0, 1.0),
                v(-h, -h, -h, 1.0, 0.0),
                v(h, h, -h, 0.0, 1.0),
                v(-h, h, -h, 1.0, 1.0),
                // +X
                v(h, -h, -h, 0.0, 0.0),
                v(h, -h, h, 1.0, 0.0),
                v(h, h, h, 1.0, 1.0),
                v(h, -h, -h, 0.0, 0.0),
                v(h, h, h, 1.0, 1.0),
                v(h, h, -h, 0.0, 1.0),
                // –X
                v(-h, -h, -h, 1.0, 0.0),
                v(-h, h, -h, 0.0, 0.0),
                v(-h, h, h, 0.0, 1.0),
                v(-h, -h, -h, 1.0, 0.0),
                v(-h, h, h, 0.0, 1.0),
                v(-h, -h, h, 1.0, 1.0),
                // +Y
                v(-h, h, -h, 0.0, 0.0),
                v(h, h, -h, 1.0, 0.0),
                v(h, h, h, 1.0, 1.0),
                v(-h, h, -h, 0.0, 0.0),
                v(h, h, h, 1.0, 1.0),
                v(-h, h, h, 0.0, 1.0),
                // –Y
                v(-h, -h, -h, 1.0, 0.0),
                v(-h, -h, h, 0.0, 0.0),
                v(h, -h, h, 0.0, 1.0),
                v(-h, -h, -h, 1.0, 0.0),
                v(h, -h, h, 0.0, 1.0),
                v(h, -h, -h, 1.0, 1.0)
            ),
            ..Default::default()
        }
    }

    pub fn cube_indexed(size: f32) -> Mesh {
        let h = size * 0.5; // half-extent

        // ------- 24 unique vertices: 4 per face -------
          let verts = avec![
            [16] |
            // +Z (front) ---------------------------------------------------------
            v(-h, -h,  h, 0.0, 1.0), // 0
            v(-h,  h,  h, 0.0, 0.0), // 1
            v( h,  h,  h, 1.0, 0.0), // 2
            v( h, -h,  h, 1.0, 1.0), // 3
            // –Z (back) ----------------------------------------------------------
            v(-h, -h, -h, 1.0, 1.0), // 4
            v( h, -h, -h, 0.0, 1.0), // 5
            v( h,  h, -h, 0.0, 0.0), // 6
            v(-h,  h, -h, 1.0, 0.0), // 7
            // +X (right) ---------------------------------------------------------
            v( h, -h, -h, 0.0, 1.0), // 8
            v( h, -h,  h, 1.0, 1.0), // 9
            v( h,  h,  h, 1.0, 0.0), // 10
            v( h,  h, -h, 0.0, 0.0), // 11
            // –X (left) ----------------------------------------------------------
            v(-h, -h, -h, 1.0, 1.0), // 12
            v(-h,  h, -h, 0.0, 1.0), // 13
            v(-h,  h,  h, 0.0, 0.0), // 14
            v(-h, -h,  h, 1.0, 0.0), // 15
            // +Y (top) -----------------------------------------------------------
            v(-h,  h, -h, 0.0, 1.0), // 16
            v( h,  h, -h, 1.0, 1.0), // 17
            v( h,  h,  h, 1.0, 0.0), // 18
            v(-h,  h,  h, 0.0, 0.0), // 19
            // –Y (bottom) --------------------------------------------------------
            v(-h, -h, -h, 1.0, 1.0), // 20
            v(-h, -h,  h, 0.0, 1.0), // 21
            v( h, -h,  h, 0.0, 0.0), // 22
            v( h, -h, -h, 1.0, 0.0)  // 23
        ];

        // ------- 36 indices (two triangles per face) -------
        // Each face is [0,1,2, 0,2,3] with a +4 offset.
        let mut indices = AVec::<u16, ConstAlign<16>>::with_capacity(16, 36);
        for face in 0..6 {
            let o = (face * 4) as u16;
            indices.extend_from_slice(&[o, o + 1, o + 2, o, o + 2, o + 3]);
        }

        Mesh {
            vertices: verts,
            indices: Some(indices),
            ..Default::default()
        }
    }

    pub fn cube_stripped(size: f32) -> Mesh {
        let h = size * 0.5;

        // ---- vertex block is unchanged ---------------------------------------
        let verts = avec![[16] |
            // +Z front
            v(-h,-h, h, 0.0, 0.0), v(-h, h, h, 0.0, 1.0),
            v( h,-h, h, 1.0, 0.0), v( h, h, h, 1.0, 1.0),

            // +X right
            v( h,-h, h, 0.0, 0.0), v( h, h, h, 0.0, 1.0),
            v( h,-h,-h, 1.0, 0.0), v( h, h,-h, 1.0, 1.0),

            // –Z back
            v( h,-h,-h, 0.0, 0.0), v( h, h,-h, 0.0, 1.0),
            v(-h,-h,-h, 1.0, 0.0), v(-h, h,-h, 1.0, 1.0),

            // –X left
            v(-h,-h,-h, 0.0, 0.0), v(-h, h,-h, 0.0, 1.0),
            v(-h,-h, h, 1.0, 0.0), v(-h, h, h, 1.0, 1.0),

            // +Y top
            v(-h, h, h, 0.0, 0.0), v(-h, h,-h, 0.0, 1.0),
            v( h, h, h, 1.0, 0.0), v( h, h,-h, 1.0, 1.0),

            // –Y bottom
            v(-h,-h,-h, 0.0, 0.0), v(-h,-h, h, 0.0, 1.0),
            v( h,-h,-h, 1.0, 0.0), v( h,-h, h, 1.0, 1.0)
        ];

        // ---- 34-index strip:   BL  TR  TL  BR   (deg: last, first) ------------
        let inds = avec![[16] |
            /* front */  0, 3, 1, 2,
            /* link  */  2, 4,
            /* right */  7, 5, 6,
            /* link  */  6, 8,
            /* back  */ 11, 9,10,
            /* link  */ 10,12,
            /* left  */ 15,13,14,
            /* link  */ 14,16,
            /* top   */ 19,17,18,
            /* link  */ 18,20,
            /* bottom*/ 23,21,22
        ];

        Mesh {
            vertices: verts,
            primitive_type: GuPrimitive::TriangleStrip,
            indices: Some(inds),   // u16 indices on PSP
        }
    }

    /// 36-vertex (12-triangle) unit cube, centered at the origin.
    pub fn cuboid(x_len: f32, y_len: f32, z_len: f32) -> Mesh {
        let x = x_len * 0.5; // half-extent
        let y = y_len * 0.5; // half-extent
        let z = z_len * 0.5; // half-extent

        Mesh {
            vertices: avec!(
                [16] |
                // +Z face
                v(-x,-y, z, 0.0, 0.0),
                v(-x, y, z, 0.0, 1.0),
                v(x, y, z, 1.0, 1.0),
                v(-x, -y, z, 0.0, 0.0),
                v(x, y, z, 1.0, 1.0),
                v(x, -y, z, 1.0, 0.0),
                // –Z
                v(-x, -y, -z, 1.0, 0.0),
                v(x, -y, -z, 0.0, 0.0),
                v(x, y, -z, 0.0, 1.0),
                v(-x, -y, -z, 1.0, 0.0),
                v(x, y, -z, 0.0, 1.0),
                v(-x, y, -z, 1.0, 1.0),
                // +X
                v(x, -y, -z, 0.0, 0.0),
                v(x, -y, z, 1.0, 0.0),
                v(x, y, z, 1.0, 1.0),
                v(x, -y, -z, 0.0, 0.0),
                v(x, y, z, 1.0, 1.0),
                v(x, y, -z, 0.0, 1.0),
                // –X
                v(-x, -y, -z, 1.0, 0.0),
                v(-x, y, -z, 0.0, 0.0),
                v(-x, y, z, 0.0, 1.0),
                v(-x, -y, -z, 1.0, 0.0),
                v(-x, y, z, 0.0, 1.0),
                v(-x, -y, z, 1.0, 1.0),
                // +Y
                v(-x, y, -z, 0.0, 0.0),
                v(x, y, -z, 1.0, 0.0),
                v(x, y, z, 1.0, 1.0),
                v(-x, y, -z, 0.0, 0.0),
                v(x, y, z, 1.0, 1.0),
                v(-x, y, z, 0.0, 1.0),
                // –Y
                v(-x, -y, -z, 1.0, 0.0),
                v(-x, -y, z, 0.0, 0.0),
                v(x, -y, z, 0.0, 1.0),
                v(-x, -y, -z, 1.0, 0.0),
                v(x, -y, z, 0.0, 1.0),
                v(x, -y, -z, 1.0, 1.0)
            ),
            ..Default::default()
        }
    }

    // Calculates a plane for the psp gu, centered at the origin
    pub fn plane(x_len: f32, y_len: f32) -> Mesh {
        let x = x_len * 0.5;
        let y = y_len * 0.5;

        Mesh {
            vertices: avec!(
                [16] | v(-x, -y, 0.0, 0.0, 1.0),
                v(-x, y, 0.0, 0.0, 0.0),
                v(x, y, 0.0, 1.0, 0.0),
                v(-x, -y, 0.0, 0.0, 1.0),
                v(x, y, 0.0, 1.0, 0.0),
                v(x, -y, 0.0, 1.0, 1.0)
            ),
            ..Default::default()
        }
    }

    /// A plane centered at the origin, subdivided into `subdivs_x` × `subdivs_y` quads.
    pub fn subdivided_plane(x_len: f32, y_len: f32, subdivs_x: usize, subdivs_y: usize) -> Mesh {
        // half-extents
        let half_x = x_len * 0.5;
        let half_y = y_len * 0.5;

        // step per cell
        let dx = x_len / subdivs_x as f32;
        let dy = y_len / subdivs_y as f32;

        // We'll generate 6 vertices per quad
        let mut verts = AVec::new(16);

        // helper to construct your Vertex (assuming you have a const fn v)
        // const fn v(x: f32, y: f32, z: f32, u: f32, v: f32) -> Vertex { … }
        for i in 0..subdivs_x {
            for j in 0..subdivs_y {
                // compute quad corners in object-space
                let x0 = -half_x + (i as f32) * dx;
                let x1 = x0 + dx;
                let y0 = -half_y + (j as f32) * dy;
                let y1 = y0 + dy;

                // compute UVs from 0.0..1.0
                let u0 = (i    ) as f32 / subdivs_x as f32;
                let u1 = (i + 1) as f32 / subdivs_x as f32;
                let v0 = (j    ) as f32 / subdivs_y as f32;
                let v1 = (j + 1) as f32 / subdivs_y as f32;

                // two triangles
                let mut va = avec![
                    [16] |
                
                    v(x0, y0, 0.0, u0, v0),
                    v(x0, y1, 0.0, u0, v1),
                    v(x1, y1, 0.0, u1, v1),

                    v(x0, y0, 0.0, u0, v0),
                    v(x1, y1, 0.0, u1, v1),
                    v(x1, y0, 0.0, u1, v0)
                ];

                verts.append(&mut va);
            }
        }

        // now turn it into your Mesh – everything else is identical
        Mesh {
            vertices: verts,    // or whatever your Mesh expects
            ..Default::default()
        }
    }
}

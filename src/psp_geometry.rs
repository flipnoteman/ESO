use bevy_ecs::component::Component;
use aligned_vec::{avec, AVec, ConstAlign};

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
#[derive(Component)]
pub struct Material {

}

#[repr(C, align(4))] 
#[derive(Component)]
pub struct Mesh {
    pub vertices: AVec<Vertex, ConstAlign<16>>,
    pub indices: Option<AVec<u16, ConstAlign<16>>>,
}

#[inline]
const fn v(x: f32, y: f32, z: f32, u: f32, v: f32) -> Vertex {
    Vertex { u, v, x, y, z }
}

impl Mesh {

    /// 36-vertex (12-triangle) unit cube, centred at the origin.
    /// NON-INDEXED. self.indexed will == None after this call, resulting in more VRAM usage
    pub fn cube(size: f32) -> Mesh {

        let h = size * 0.5;           // half-extent
        
        Mesh { 
            vertices: avec!([16] |
                // +Z face
                v(-h,-h, h, 0.0, 0.0), v(-h, h, h, 0.0, 1.0), v( h, h, h, 1.0, 1.0),
                v(-h,-h, h, 0.0, 0.0), v( h, h, h, 1.0, 1.0), v( h,-h, h, 1.0, 0.0),
                // –Z
                v(-h,-h,-h, 1.0, 0.0), v( h,-h,-h, 0.0, 0.0), v( h, h,-h, 0.0, 1.0),
                v(-h,-h,-h, 1.0, 0.0), v( h, h,-h, 0.0, 1.0), v(-h, h,-h, 1.0, 1.0),
                // +X
                v( h,-h,-h, 0.0, 0.0), v( h,-h, h, 1.0, 0.0), v( h, h, h, 1.0, 1.0),
                v( h,-h,-h, 0.0, 0.0), v( h, h, h, 1.0, 1.0), v( h, h,-h, 0.0, 1.0),
                // –X
                v(-h,-h,-h, 1.0, 0.0), v(-h, h,-h, 0.0, 0.0), v(-h, h, h, 0.0, 1.0),
                v(-h,-h,-h, 1.0, 0.0), v(-h, h, h, 0.0, 1.0), v(-h,-h, h, 1.0, 1.0),
                // +Y
                v(-h, h,-h, 0.0, 0.0), v( h, h,-h, 1.0, 0.0), v( h, h, h, 1.0, 1.0),
                v(-h, h,-h, 0.0, 0.0), v( h, h, h, 1.0, 1.0), v(-h, h, h, 0.0, 1.0),
                // –Y
                v(-h,-h,-h, 1.0, 0.0), v(-h,-h, h, 0.0, 0.0), v( h,-h, h, 0.0, 1.0),
                v(-h,-h,-h, 1.0, 0.0), v( h,-h, h, 0.0, 1.0), v( h,-h,-h, 1.0, 1.0)
            ),
            indices: None
        }
    }

    pub fn cube_indexed(size: f32) -> Mesh {
        let h = size * 0.5;          // half-extent

        // ------- 24 unique vertices: 4 per face -------
        let verts = avec![[16] |
            // +Z (front) ---------------------------------------------------------
            v(-h,-h, h, 0.0,0.0),  // 0
            v(-h, h, h, 0.0,1.0),  // 1
            v( h, h, h, 1.0,1.0),  // 2
            v( h,-h, h, 1.0,0.0),  // 3

            // –Z (back) ----------------------------------------------------------
            v(-h,-h,-h, 1.0,0.0),  // 4
            v( h,-h,-h, 0.0,0.0),  // 5
            v( h, h,-h, 0.0,1.0),  // 6
            v(-h, h,-h, 1.0,1.0),  // 7

            // +X (right) ---------------------------------------------------------
            v( h,-h,-h, 0.0,0.0),  // 8
            v( h,-h, h, 1.0,0.0),  // 9
            v( h, h, h, 1.0,1.0),  //10
            v( h, h,-h, 0.0,1.0),  //11

            // –X (left) ----------------------------------------------------------
            v(-h,-h,-h, 1.0,0.0),  //12
            v(-h, h,-h, 0.0,0.0),  //13
            v(-h, h, h, 0.0,1.0),  //14
            v(-h,-h, h, 1.0,1.0),  //15

            // +Y (top) -----------------------------------------------------------
            v(-h, h,-h, 0.0,0.0),  //16
            v( h, h,-h, 1.0,0.0),  //17
            v( h, h, h, 1.0,1.0),  //18
            v(-h, h, h, 0.0,1.0),  //19

            // –Y (bottom) --------------------------------------------------------
            v(-h,-h,-h, 1.0,0.0),  //20
            v(-h,-h, h, 0.0,0.0),  //21
            v( h,-h, h, 0.0,1.0),  //22
            v( h,-h,-h, 1.0,1.0)  //23
        ];

        // ------- 36 indices (two triangles per face) -------
        // Each face is [0,1,2, 0,2,3] with a +4 offset.
        let mut indices = AVec::<u16, ConstAlign<16>>::with_capacity(16, 36);
        for face in 0..6 {
            let o = (face * 4) as u16;
            indices.extend_from_slice(&[o, o+1, o+2,  o, o+2, o+3]);
        }

        Mesh {
            vertices: verts,
            indices: Some(indices),
        }
    }

    /// 36-vertex (12-triangle) unit cube, centred at the origin.
    pub fn cuboid(x_len: f32, y_len: f32, z_len: f32) -> Mesh {

        let x = x_len * 0.5;           // half-extent
        let y = y_len * 0.5;           // half-extent
        let z = z_len * 0.5;           // half-extent
        
        Mesh {
            vertices: avec!([16] |
                // +Z face
                v(-x,-y, z, 0.0, 0.0), v(-x, y, z, 0.0, 1.0), v( x, y, z, 1.0, 1.0),
                v(-x,-y, z, 0.0, 0.0), v( x, y, z, 1.0, 1.0), v( x,-y, z, 1.0, 0.0),

                // –Z
                v(-x,-y,-z, 1.0, 0.0), v( x,-y,-z, 0.0, 0.0), v( x, y,-z, 0.0, 1.0),
                v(-x,-y,-z, 1.0, 0.0), v( x, y,-z, 0.0, 1.0), v(-x, y,-z, 1.0, 1.0),

                // +X
                v( x,-y,-z, 0.0, 0.0), v( x,-y, z, 1.0, 0.0), v( x, y, z, 1.0, 1.0),
                v( x,-y,-z, 0.0, 0.0), v( x, y, z, 1.0, 1.0), v( x, y,-z, 0.0, 1.0),

                // –X
                v(-x,-y,-z, 1.0, 0.0), v(-x, y,-z, 0.0, 0.0), v(-x, y, z, 0.0, 1.0),
                v(-x,-y,-z, 1.0, 0.0), v(-x, y, z, 0.0, 1.0), v(-x,-y, z, 1.0, 1.0),

                // +Y
                v(-x, y,-z, 0.0, 0.0), v( x, y,-z, 1.0, 0.0), v( x, y, z, 1.0, 1.0),
                v(-x, y,-z, 0.0, 0.0), v( x, y, z, 1.0, 1.0), v(-x, y, z, 0.0, 1.0),

                // –Y
                v(-x,-y,-z, 1.0, 0.0), v(-x,-y, z, 0.0, 0.0), v( x,-y, z, 0.0, 1.0),
                v(-x,-y,-z, 1.0, 0.0), v( x,-y, z, 0.0, 1.0), v( x,-y,-z, 1.0, 1.0)
            ),
            indices: None
        }
    }

    // Calculates a plane for the psp gu, centered at the origin
    pub fn plane(x_len: f32, y_len: f32) -> Mesh {
        let x = x_len * 0.5;
        let y = y_len * 0.5;

        Mesh {
            vertices: avec!([16] |
                v(-x, -y, 0.0, 0.0, 0.0), v(-x, y, 0.0, 0.0, 1.0), v(x, y, 0.0, 1.0, 1.0),
                v(-x, -y, 0.0, 0.0, 0.0), v(x, y, 0.0, 1.0, 1.0), v(x, -y, 0.0, 1.0, 0.0)
            ),
            indices: None,
        } 
    }
}


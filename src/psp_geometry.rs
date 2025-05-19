use bevy_ecs::component::Component;
use aligned_vec::{avec, AVec, ConstAlign};

#[repr(C, align(4))]
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
}

#[inline]
const fn v(x: f32, y: f32, z: f32, u: f32, v: f32) -> Vertex {
    Vertex { u, v, x, y, z }
}

impl Mesh {

    /// 36-vertex (12-triangle) unit cube, centred at the origin.
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
            )
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
            )
        }
    }
}


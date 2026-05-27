use core::ops::Add;

use alloc::vec::{self, Vec};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    resource::Resource,
    schedule::{IntoScheduleConfigs, Schedule},
    system::{Query, Res, ResMut},
    world::World,
};
use psp::sys::{self, GuPrimitive, GuState, ScePspFVector3, VertexType};

use crate::{Transform, asset_handling::ColorVertex};

#[derive(Clone, Copy)]
pub enum ColliderType {
    Quad(f32, f32),
    Cuboid(f32, f32, f32),
    AABB(f32, f32, f32),
    Sphere(f32),
    Capsule(f32, f32),
}

#[derive(Clone, Copy, Component)]
pub struct Collider {
    pub collider_type: ColliderType,
    pub fixed: bool,
}

#[derive(Resource)]
pub struct Collisions(Vec<(Entity, ScePspFVector3)>);

impl Default for Collisions {
    fn default() -> Self {
        Collisions(Vec::new())
    }
}

impl Collider {
    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        Collider {
            collider_type: ColliderType::Cuboid(x, y, z),
            fixed: false,
        }
    }

    pub fn aabb(x: f32, y: f32, z: f32) -> Self {
        Collider {
            collider_type: ColliderType::AABB(x, y, z),
            fixed: false,
        }
    }

    pub fn fixed(&mut self) -> Self {
        self.fixed = true;
        self.clone()
    }
}

/// Helper function for calculating the penetration vector for a aabb->aabb collision
fn test_aabb_aabb(
    transform_a: &Transform,
    ax: f32,
    ay: f32,
    az: f32,
    transform_b: &Transform,
    bx: f32,
    by: f32,
    bz: f32,
) -> Option<ScePspFVector3> {
    let d_x = (transform_a.translation.x - transform_b.translation.x).abs();
    let d_y = (transform_a.translation.y - transform_b.translation.y).abs();
    let d_z = (transform_a.translation.z - transform_b.translation.z).abs();

    let sum_x = 0.5 * ax + 0.5 * bx;
    let sum_y = 0.5 * ay + 0.5 * by;
    let sum_z = 0.5 * az + 0.5 * bz;

    if d_x < sum_x && d_y < sum_y && d_z < sum_z {
        let overlap = [sum_x - d_x, sum_y - d_y, sum_z - d_z];

        let (idx, _) = overlap
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        let mut out = ScePspFVector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        match idx {
            0 => {
                out.x =
                    (transform_b.translation.x - transform_a.translation.x).signum() * overlap[0];
            }
            1 => {
                out.y =
                    (transform_b.translation.y - transform_a.translation.y).signum() * overlap[1];
            }
            2 => {
                out.z =
                    (transform_b.translation.z - transform_a.translation.z).signum() * overlap[2];
            }
            _ => {}
        }
        return Some(out);
    }

    None
}

fn detect_collisions(
    mut query: Query<(&mut Transform, &Collider, Entity)>,
    mut c: ResMut<Collisions>,
) {
    // Clear collisions vector for this frame
    c.0.clear();
    // Bevy ecs is so clutch, this function is super useful for this
    let mut combinations = query.iter_combinations_mut::<2>();
    let mut collisions = Vec::new();
    while let Some(
        [
            (transform_a, collider_a, entity_a),
            (transform_b, collider_b, entity_b),
        ],
    ) = combinations.fetch_next()
    {
        // Test to see if there is a collision between entities, store in collisions vector if so
        if let Some(pen_vector) = match (collider_a.collider_type, collider_b.collider_type) {
            (ColliderType::AABB(ax, ay, az), ColliderType::AABB(bx, by, bz)) => {
                test_aabb_aabb(&transform_a, ax, ay, az, &transform_b, bx, by, bz)
            }
            _ => None,
        } {
            // If we get a penetration (collision), store the collision depending on fixed status
            if !collider_a.fixed && !collider_b.fixed {
                collisions.push((
                    entity_a,
                    ScePspFVector3 {
                        x: -pen_vector.x / 2.0,
                        y: -pen_vector.y / 2.0,
                        z: -pen_vector.z / 2.0,
                    },
                ));
                collisions.push((
                    entity_b,
                    ScePspFVector3 {
                        x: pen_vector.x / 2.0,
                        y: pen_vector.y / 2.0,
                        z: pen_vector.z / 2.0,
                    },
                ));
            } else if !collider_a.fixed {
                collisions.push((
                    entity_a,
                    ScePspFVector3 {
                        x: -pen_vector.x,
                        y: -pen_vector.y,
                        z: -pen_vector.z,
                    },
                ));
            } else if !collider_b.fixed {
                collisions.push((entity_b, pen_vector));
            }
        };
    }

    // Sort by entity
    collisions.sort_by_key(|a| a.0);
    // Chunk by entity
    for group in collisions.chunk_by_mut(|a, b| a.0 == b.0) {
        let mut vec = ScePspFVector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // Accumulate penetrations per entity
        for col in group.iter() {
            vec.x += col.1.x;
            vec.y += col.1.y;
            vec.z += col.1.z;
        }
        // Divide by group length to average out penetrations
        vec.x /= group.len() as f32;
        vec.y /= group.len() as f32;
        vec.z /= group.len() as f32;
        // push accumulated penetrations to vector
        c.0.push((group[0].0, vec));
    }
}

fn reconcile_physics(mut query: Query<&mut Transform>, mut collisions: ResMut<Collisions>) {
    for (entity, correction) in collisions.0.iter() {
        if let Ok(mut transform) = query.get_mut(*entity) {
            transform.translation.x += correction.x;
            transform.translation.y += correction.y;
            transform.translation.z += correction.z;
        }
    }
}

pub struct PhysicsEngine {
    schedule: Schedule,
}

impl PhysicsEngine {
    pub fn new(mut world: &mut World) -> Self {
        let mut schedule = Schedule::default();
        world.insert_resource(Collisions::default());

        schedule.add_systems((
            detect_collisions,
            reconcile_physics.after(detect_collisions),
        ));
        PhysicsEngine { schedule }
    }

    pub fn run(&mut self, world: &mut World) {
        self.schedule.run(world);
    }
}

pub fn render_collider_debug(query: Query<(&Transform, &Collider)>) {
    unsafe {
        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGuDisable(GuState::DepthTest);

        for (transform, collider) in query.iter() {
            let (hx, hy, hz) = match collider.collider_type {
                ColliderType::AABB(x, y, z) => (x * 0.5, y * 0.5, z * 0.5),
                ColliderType::Cuboid(x, y, z) => (x * 0.5, y * 0.5, z * 0.5),
                _ => continue,
            };

            // 8 corners of the box
            let corners = [
                (-hx, -hy, -hz),
                (hx, -hy, -hz),
                (hx, hy, -hz),
                (-hx, hy, -hz),
                (-hx, -hy, hz),
                (hx, -hy, hz),
                (hx, hy, hz),
                (-hx, hy, hz),
            ];

            // 12 edges as index pairs
            let edges = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0), // back face
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4), // front face
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7), // connecting edges
            ];

            let color: u32 = if collider.fixed {
                0xFF0000FF
            } else {
                0xFF00FF00
            };

            // 24 vertices (2 per edge)
            let buf = sys::sceGuGetMemory((24 * core::mem::size_of::<ColorVertex>()) as i32)
                as *mut ColorVertex;

            for (i, (a, b)) in edges.iter().enumerate() {
                let (ax, ay, az) = corners[*a];
                let (bx, by, bz) = corners[*b];
                *buf.add(i * 2) = ColorVertex {
                    color,
                    x: ax,
                    y: ay,
                    z: az,
                };
                *buf.add(i * 2 + 1) = ColorVertex {
                    color,
                    x: bx,
                    y: by,
                    z: bz,
                };
            }

            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
            sys::sceGumTranslate(&transform.translation);

            sys::sceGumDrawArray(
                GuPrimitive::Lines,
                VertexType::VERTEX_32BITF | VertexType::COLOR_8888 | VertexType::TRANSFORM_3D,
                24,
                core::ptr::null(),
                buf as *const _,
            );
        }

        sys::sceGuEnable(GuState::Texture2D);
        sys::sceGuEnable(GuState::DepthTest);
    }
}

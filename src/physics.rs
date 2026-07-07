use alloc::vec;
use alloc::vec::Vec;
use bevy_ecs::{
    component::Component,
    query::{Has, With},
    resource::Resource,
    system::{Query, Res, ResMut, Single},
};
use psp::sys::{self, GuPrimitive, GuState, ScePspFVector3, VertexType};

use crate::{
    Player, Time, Transform,
    asset_handling::ColorVertex,
    println,
    psp_math::{vadd, vcross, vdot, vfpu_cosf, vfpu_sinf, vlength, vneg, vnormalize, vscale, vsub},
};

#[derive(Clone, Copy)]
pub enum ColliderType {
    Quad(f32, f32),
    Cuboid(f32, f32, f32),
    Sphere(f32),
    Capsule(f32, f32),
}

impl ColliderType {
    /// Returns the furthest point on collider in a certain direction, used with GJK algorithm
    pub fn support(&self, dir: ScePspFVector3) -> ScePspFVector3 {
        match self {
            ColliderType::Cuboid(hx, hy, hz) => ScePspFVector3 {
                x: if dir.x >= 0.0 { *hx * 0.5 } else { -*hx * 0.5 },
                y: if dir.y >= 0.0 { *hy * 0.5 } else { -*hy * 0.5 },
                z: if dir.z >= 0.0 { *hz * 0.5 } else { -*hz * 0.5 },
            },
            ColliderType::Sphere(r) => {
                // Furthest point on sphere is just center + radius in direction
                let len = vlength(dir);

                ScePspFVector3 {
                    x: dir.x / len * r,
                    y: dir.y / len * r,
                    z: dir.z / len * r,
                }
            }
            ColliderType::Capsule(r, half_height) => {
                // project onto capsule axis, pick tip or base endpoint then add sphere radius
                let tip = ScePspFVector3 {
                    x: 0.0,
                    y: *half_height,
                    z: 0.0,
                };
                let base = ScePspFVector3 {
                    x: 0.0,
                    y: -*half_height,
                    z: 0.0,
                };
                let best = if dir.y >= 0.0 { tip } else { base };
                let len = vlength(dir);

                ScePspFVector3 {
                    x: best.x + dir.x / len * r,
                    y: best.y + dir.y / len * r,
                    z: best.z + dir.z / len * r,
                }
            }
            ColliderType::Quad(..) => ScePspFVector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }
}
fn rotate_vec(v: ScePspFVector3, r: ScePspFVector3) -> ScePspFVector3 {
    let (sx, cx) = (vfpu_sinf(r.x), vfpu_cosf(r.x));
    let (sy, cy) = (vfpu_sinf(r.y), vfpu_cosf(r.y));
    let (sz, cz) = (vfpu_sinf(r.z), vfpu_cosf(r.z));

    // Apply Rx → Ry → Rz
    let x1 = v.x;
    let y1 = cx * v.y - sx * v.z;
    let z1 = sx * v.y + cx * v.z;

    let x2 = cy * x1 + sy * z1;
    let y2 = y1;
    let z2 = -sy * x1 + cy * z1;

    ScePspFVector3 {
        x: cz * x2 - sz * y2,
        y: sz * x2 + cz * y2,
        z: z2,
    }
}

fn inverse_rotate_vec(v: ScePspFVector3, r: ScePspFVector3) -> ScePspFVector3 {
    let (sx, cx) = (vfpu_sinf(r.x), vfpu_cosf(r.x));
    let (sy, cy) = (vfpu_sinf(r.y), vfpu_cosf(r.y));
    let (sz, cz) = (vfpu_sinf(r.z), vfpu_cosf(r.z));

    // Apply Rz^T → Ry^T → Rx^T
    let x1 = cz * v.x + sz * v.y;
    let y1 = -sz * v.x + cz * v.y;
    let z1 = v.z;

    let x2 = cy * x1 - sy * z1;
    let y2 = y1;
    let z2 = sy * x1 + cy * z1;

    ScePspFVector3 {
        x: x2,
        y: cx * y2 + sx * z2,
        z: -sx * y2 + cx * z2,
    }
}

/// Returns local collider support vector in world space
fn world_support(
    collider: &ColliderType,
    transform: &Transform,
    dir: ScePspFVector3,
) -> ScePspFVector3 {
    let local_dir = inverse_rotate_vec(dir, transform.rotation);
    let local_sup = collider.support(local_dir);
    let world_sup = rotate_vec(local_sup, transform.rotation);

    ScePspFVector3 {
        x: world_sup.x + transform.translation.x,
        y: world_sup.y + transform.translation.y,
        z: world_sup.z + transform.translation.z,
    }
}

struct Simplex {
    points: [ScePspFVector3; 4],
    len: usize,
}

impl Simplex {
    fn push(&mut self, p: ScePspFVector3) {
        self.points[3] = self.points[2];
        self.points[2] = self.points[1];
        self.points[1] = self.points[0];
        self.points[0] = p;
        self.len = (self.len + 1).min(4);
    }

    fn default() -> Self {
        Self {
            points: [
                ScePspFVector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                ScePspFVector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                ScePspFVector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                ScePspFVector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            len: 0,
        }
    }
}

fn gjk(
    collider_a: &ColliderType,
    transform_a: &Transform,
    collider_b: &ColliderType,
    transform_b: &Transform,
) -> Option<Simplex> {
    // Seed search direction toward the other body's centroid for faster convergence
    let mut dir = vsub(transform_b.translation, transform_a.translation);
    if vdot(dir, dir) == 0.0 {
        dir = ScePspFVector3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
    }

    let support = |d: ScePspFVector3| {
        vsub(
            world_support(collider_a, transform_a, d),
            world_support(collider_b, transform_b, vneg(d)),
        )
    };

    let mut simplex = Simplex::default();
    simplex.push(support(dir));

    dir = vneg(simplex.points[0]);

    for _ in 0..32 {
        if vdot(dir, dir) < 1e-20 {
            return None;
        }

        let a = support(dir);

        if vdot(a, dir) < 0.0 {
            return None;
        }

        simplex.push(a);

        if do_simplex(&mut simplex, &mut dir) {
            return Some(simplex);
        }
    }

    None
}

fn same_direction(a: ScePspFVector3, b: ScePspFVector3) -> bool {
    vdot(a, b) > 0.0
}

fn do_line(simplex: &mut Simplex, dir: &mut ScePspFVector3) -> bool {
    let a = simplex.points[0]; //newest point
    let b = simplex.points[1];

    let ab = vsub(b, a);
    let ao = vneg(a); // from a to origin

    if same_direction(ab, ao) {
        let cross = vcross(ab, ao);
        let cross_len_sq = vdot(cross, cross);
        if cross_len_sq > 1e-10 {
            // Perpendicular to AB toward origin
            *dir = vcross(cross, ab);
        } else {
            // Origin is on the line AB — pick any direction perpendicular to AB
            let (dx, dy, dz) = (ab.x, ab.y, ab.z);
            if dx.abs() > dy.abs() && dx.abs() > dz.abs() {
                *dir = vcross(
                    ab,
                    ScePspFVector3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                );
            } else {
                *dir = vcross(
                    ab,
                    ScePspFVector3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                );
            }
        }
    } else {
        // Origin is behind A — A is closest
        simplex.len = 1;
        *dir = ao;
    }

    false
}
fn do_triangle(simplex: &mut Simplex, dir: &mut ScePspFVector3) -> bool {
    let a = simplex.points[0];
    let b = simplex.points[1];
    let c = simplex.points[2];

    let ab = vsub(b, a);
    let ac = vsub(c, a);
    let ao = vneg(a);

    // Degenerate: C == A → reduce to line [A, B]
    if vdot(ac, ac) < 1e-10 {
        simplex.len = 2;
        return do_line(simplex, dir);
    }

    // Degenerate: B == A → reduce to line [A, C]
    if vdot(ab, ab) < 1e-10 {
        simplex.points[1] = c;
        simplex.len = 2;
        return do_line(simplex, dir);
    }

    let abc = vcross(ab, ac); // face normal

    // Degenerate: triangle is flat (colinear points)
    if vdot(abc, abc) < 1e-10 {
        let ab_len = vdot(ab, ab);
        let ac_len = vdot(ac, ac);
        if ac_len > ab_len {
            simplex.points[1] = c;
        }
        simplex.len = 2;
        return do_line(simplex, dir);
    }

    if same_direction(vcross(abc, ac), ao) {
        if same_direction(ac, ao) {
            simplex.points[1] = c;
            simplex.len = 2;
            *dir = vcross(vcross(ac, ao), ac);
        } else {
            simplex.len = 2;
            return do_line(simplex, dir);
        }
    } else {
        // cross(ab, abc) = outward edge normal of AB (points away from C)
        if same_direction(vcross(ab, abc), ao) {
            // Origin is in the AB edge voronoi region
            simplex.len = 2;
            return do_line(simplex, dir);
        } else {
            // Origin is within the triangles face region
            if same_direction(abc, ao) {
                // Origin is above the face — direction is the face normal
                *dir = abc;
            } else {
                // Origin is below — flip winding so normal points toward origin
                simplex.points[1] = c;
                simplex.points[2] = b;
                *dir = vneg(abc);
            }
        }
    }

    false
}

fn do_tetrahedron(simplex: &mut Simplex, dir: &mut ScePspFVector3) -> bool {
    let a = simplex.points[0]; // newest — guaranteed to be on origin side of BCD
    let b = simplex.points[1];
    let c = simplex.points[2];
    let d = simplex.points[3];

    let ab = vsub(b, a);
    let ac = vsub(c, a);
    let ad = vsub(d, a);
    let ao = vneg(a);

    // Face normals for the three faces containing A
    // We never test face BCD — A was added toward the origin from there
    let abc = vcross(ab, ac);
    let acd = vcross(ac, ad);
    let adb = vcross(ad, ab);

    // If all face normals are zero, the tetrahedron is degenerate (flat/coplanar)
    if vdot(abc, abc) < 1e-10 && vdot(acd, acd) < 1e-10 && vdot(adb, adb) < 1e-10 {
        // Can't determine containment — let GJK try a different direction
        *dir = ao;
        return false;
    }

    // Skip degenerate faces (zero normal) — they don't bound a valid region
    if vdot(abc, abc) > 1e-10 && same_direction(abc, ao) {
        // Origin outside face ABC — reduce to triangle [A, B, C]
        simplex.len = 3;
        return do_triangle(simplex, dir);
    }

    if vdot(acd, acd) > 1e-10 && same_direction(acd, ao) {
        // Origin outside face ACD — reduce to triangle [A, C, D]
        simplex.points[1] = c;
        simplex.points[2] = d;
        simplex.len = 3;
        return do_triangle(simplex, dir);
    }

    if vdot(adb, adb) > 1e-10 && same_direction(adb, ao) {
        // Origin outside face ADB — reduce to triangle [A, D, B]
        simplex.points[1] = d;
        simplex.points[2] = b;
        simplex.len = 3;
        return do_triangle(simplex, dir);
    }

    // Origin is inside all four half-spaces — confirmed inside tetrahedron
    true
}

fn do_simplex(simplex: &mut Simplex, dir: &mut ScePspFVector3) -> bool {
    match simplex.len {
        2 => do_line(simplex, dir),
        3 => do_triangle(simplex, dir),
        4 => do_tetrahedron(simplex, dir),
        _ => false,
    }
}

pub struct EpaResult {
    pub normal: ScePspFVector3,
    pub depth: f32,
}

struct PolyFace {
    indices: [usize; 3],
    normal: ScePspFVector3,
    distance: f32,
}

fn make_face(pts: &[ScePspFVector3], a: usize, b: usize, c: usize, opposite: usize) -> PolyFace {
    // Calculate normal of cross product of difference of vectors
    let normal = vnormalize(vcross(vsub(pts[b], pts[a]), vsub(pts[c], pts[a])));

    if vdot(normal, vsub(pts[opposite], pts[a])) > 0.0 {
        let flipped = vneg(normal);
        PolyFace {
            indices: [a, c, b], // flip winding
            normal: flipped,
            distance: vdot(flipped, pts[a]),
        }
    } else {
        PolyFace {
            indices: [a, b, c],
            normal,
            distance: vdot(normal, pts[a]),
        }
    }
}

/// Build a face whose normal points away from the origin.
/// The origin is always inside the Minkowski-difference polytope (GJK invariant),
/// so it is the correct interior reference at every stage of EPA expansion —
/// unlike a cached centroid, which can land outside the expanded polytope when
/// the initial simplex is nearly flat.
fn make_face_from_origin(pts: &[ScePspFVector3], a: usize, b: usize, c: usize) -> PolyFace {
    let normal = vnormalize(vcross(vsub(pts[b], pts[a]), vsub(pts[c], pts[a])));
    // Signed distance from origin to the plane: positive means normal points away
    // from origin (outward, correct); negative means it points toward origin (inward, flip).
    let d = vdot(normal, pts[a]);
    if d >= 0.0 {
        PolyFace {
            indices: [a, b, c],
            normal,
            distance: d,
        }
    } else {
        let flipped = vneg(normal);
        PolyFace {
            indices: [a, c, b],
            normal: flipped,
            distance: -d,
        }
    }
}

fn epa(
    simplex: &mut Simplex,
    collider_a: &ColliderType,
    transform_a: &Transform,
    collider_b: &ColliderType,
    transform_b: &Transform,
) -> Option<EpaResult> {
    let mut points: Vec<ScePspFVector3> = simplex.points[..4].to_vec();
    let mut faces: Vec<PolyFace> = vec![
        make_face(&points, 0, 1, 2, 3), // ABC
        make_face(&points, 0, 1, 3, 2), // ABD
        make_face(&points, 0, 2, 3, 1), // ACD
        make_face(&points, 1, 2, 3, 0), // BCD
    ];

    let support = |dir: ScePspFVector3| {
        vsub(
            world_support(collider_a, transform_a, dir),
            world_support(collider_b, transform_b, vneg(dir)),
        )
    };
    for _ in 0..32 {
        let (closest_idx, closest) = faces.iter().enumerate().min_by(|(_, a), (_, b)| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(core::cmp::Ordering::Equal)
        })?;

        let normal = closest.normal;
        let distance = closest.distance;

        let sup = support(normal);
        let sup_dist = vdot(normal, sup);

        if sup_dist - distance < 1e-4 {
            return Some(EpaResult {
                normal,
                depth: distance,
            });
        }

        let visible: Vec<bool> = faces
            .iter()
            .map(|f| vdot(f.normal, vsub(sup, points[f.indices[0]])) > 0.0)
            .collect();

        let mut horizon: Vec<(usize, usize)> = Vec::new();
        for (i, face) in faces.iter().enumerate() {
            if !visible[i] {
                continue;
            }
            let [a, b, c] = face.indices;
            for edge in [(a, b), (b, c), (c, a)] {
                let rev = (edge.1, edge.0);
                // If the reverse edge is already in the list, both adjacent faces
                // are visible — it's an internal edge, remove it
                if let Some(pos) = horizon.iter().position(|&e| e == rev) {
                    horizon.remove(pos);
                } else {
                    horizon.push(edge);
                }
            }
        }

        let mut i = faces.len();
        while i > 0 {
            i -= 1;
            if visible[i] {
                faces.remove(i);
            }
        }

        let new_idx = points.len();
        points.push(sup);

        for (a, b) in horizon {
            faces.push(make_face_from_origin(&points, new_idx, b, a));
        }
    }

    None
}

#[derive(Clone, Copy, Component)]
pub struct Collider {
    pub collider_type: ColliderType,
}

#[derive(Clone, Copy, Component)]
pub struct StaticBody;

#[derive(Clone, Copy, Component)]
pub struct RigidBody {
    pub gravity: f32,
    pub velocity: ScePspFVector3,
    pub mass: f32,
    /// True when resting on a slope shallow enough to stand on. Input is redirected
    /// along the ground plane and fall speed is cleared so there's no creep.
    pub grounded: bool,
    /// True when touching a too-steep surface (and not grounded). Input is glued to
    /// the slope and stripped of any uphill component so the player can only slide.
    pub on_steep: bool,
    /// Separation normal of the surface we're standing on / sliding against. Kept
    /// from the previous frame so this frame's input can be projected onto it,
    /// keeping the player's velocity parallel to the surface.
    pub ground_normal: ScePspFVector3,
}

impl Default for RigidBody {
    fn default() -> Self {
        RigidBody {
            gravity: 9.8f32,
            velocity: ScePspFVector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            mass: 10.0,
            grounded: false,
            on_steep: false,
            ground_normal: ScePspFVector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        }
    }
}

impl Collider {
    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        Collider {
            collider_type: ColliderType::Cuboid(x, y, z),
        }
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Collider {
            collider_type: ColliderType::Capsule(radius, half_height),
        }
    }

    pub fn sphere(radius: f32) -> Self {
        Collider {
            collider_type: ColliderType::Sphere(radius),
        }
    }
}

#[derive(Resource)]
pub struct StaticColliderData {
    pub transforms: Vec<Transform>,
    pub colliders: Vec<Collider>,
}

impl Default for StaticColliderData {
    fn default() -> Self {
        Self {
            transforms: Vec::new(),
            colliders: Vec::new(),
        }
    }
}

pub fn collect_static_data(
    mut data: ResMut<StaticColliderData>,
    statics: Query<(&Transform, &Collider), With<StaticBody>>,
) {
    data.transforms.clear();
    data.colliders.clear();
    for (t, c) in statics.iter() {
        data.transforms.push(t.clone());
        data.colliders.push(*c);
    }
}
pub fn player_physics(
    mut data: Single<(&mut Transform, &mut RigidBody, &Collider), With<Player>>,
    time: Res<Time>,
    collision_data: Res<StaticColliderData>,
) {
    let (mut transform, mut body, player_collider) = data.into_inner();
    let dt = time.delta_seconds();

    // Small bias applied after depenetration to prevent the resolved position from
    // landing exactly on the surface and re-triggering a collision next frame.
    const SKIN: f32 = 0.001;
    const TERMINAL_VELOCITY: f32 = 30.0;
    // Steepest slope the player can stand/walk on. A contact whose separation
    // normal points more vertically than this counts as walkable ground; anything
    // steeper is treated as a slide surface. cos(50°) ≈ 0.643.
    const MIN_WALKABLE_NORMAL_Y: f32 = 0.643;
    // Max speed while sliding down a too-steep surface. Without this, gravity keeps
    // accumulating in the projected slide velocity and the player speeds up the
    // longer they touch the slope — which read as "velocity increases against the
    // overhang". Capping gives a steady, controlled slide instead.
    const STEEP_SLIDE_SPEED: f32 = 2.5;

    // Integrate gravity, clamped to terminal velocity. If we were grounded last
    // frame, velocity.y was cleared then, so this is a single small step that acts
    // as the "probe" that keeps us touching the ground.
    body.velocity.y -= body.gravity * dt;
    if body.velocity.y < -TERMINAL_VELOCITY {
        body.velocity.y = -TERMINAL_VELOCITY;
    }

    // Use only the yaw for collision. transform.rotation also carries the camera
    // pitch (look up/down), and feeding that into the collider would tilt the
    // player's capsule so its contact normals no longer reflect the true surface
    // it's standing on — breaking slope classification. The capsule is
    // rotationally symmetric about Y, so yaw doesn't matter either, but keeping it
    // costs nothing and stays correct if the shape ever changes.
    let player_rot = ScePspFVector3 {
        x: 0.0,
        y: transform.rotation.y,
        z: 0.0,
    };

    // Contacts for the player at a candidate position, as (separation_normal,
    // depth) pairs — normals negated from EPA's outward normal so they point the
    // direction the player must be pushed. Returns BOTH the deepest contact overall
    // and the deepest *walkable* one: with only the single deepest, a player wedged
    // between the floor and an overhang sees only the overhang, reads as airborne,
    // and gravity accumulates forever (the "speed grows on overhangs" bug).
    type Contact = (ScePspFVector3, f32);
    let find_contacts = |candidate: ScePspFVector3| -> (Option<Contact>, Option<Contact>) {
        let candidate_transform = Transform {
            translation: candidate,
            rotation: player_rot,
        };
        let mut best: Option<Contact> = None;
        let mut best_walkable: Option<Contact> = None;
        let mut max_depth = 0.0f32;
        let mut max_walk_depth = 0.0f32;
        for i in 0..collision_data.transforms.len() {
            if let Some(mut simplex) = gjk(
                &player_collider.collider_type,
                &candidate_transform,
                &collision_data.colliders[i].collider_type,
                &collision_data.transforms[i],
            ) {
                if let Some(r) = epa(
                    &mut simplex,
                    &player_collider.collider_type,
                    &candidate_transform,
                    &collision_data.colliders[i].collider_type,
                    &collision_data.transforms[i],
                ) {
                    let n = vneg(r.normal);
                    if r.depth > max_depth {
                        max_depth = r.depth;
                        best = Some((n, r.depth));
                    }
                    if n.y >= MIN_WALKABLE_NORMAL_Y && r.depth > max_walk_depth {
                        max_walk_depth = r.depth;
                        best_walkable = Some((n, r.depth));
                    }
                }
            }
        }
        (best, best_walkable)
    };

    // The horizontal movement the player is asking for this frame (update_player
    // wrote it into velocity.x/z).
    let input = ScePspFVector3 {
        x: body.velocity.x,
        y: 0.0,
        z: body.velocity.z,
    };

    // Compose the single velocity vector we actually integrate this frame, using
    // the surface we were touching last frame so movement stays parallel to it.
    let mut vel;
    if body.grounded && body.ground_normal.y >= MIN_WALKABLE_NORMAL_Y {
        // Walkable ground: map input onto the ground plane VERTICALLY — keep x/z
        // exactly as requested and pick the y that lies on the plane
        // (n·v = 0  →  y = -(n.x·x + n.z·z)/n.y). Horizontal speed is preserved
        // with no renormalization, and a zero input maps to exactly zero motion,
        // so there is no downhill creep while standing still. Then re-add the
        // one-frame gravity probe so we settle back onto the surface.
        let gn = body.ground_normal;
        vel = ScePspFVector3 {
            x: input.x,
            y: -(gn.x * input.x + gn.z * input.z) / gn.y + body.velocity.y,
            z: input.z,
        };
    } else if body.on_steep {
        // Glued to a too-steep surface. Remove the horizontal component of the
        // velocity toward OR away from the surface: toward would climb (the wall
        // clip in the loop would fight it), away would peel the player off the
        // slope mid-slide. What survives is sideways input plus gravity; the
        // contact clip in the loop turns the gravity into a down-slope slide.
        // NOTE: do NOT project onto the tilted slope plane here — that converts
        // horizontal approach into up-slope motion (the climb/jitter bug).
        let gn = body.ground_normal;
        let mut v = body.velocity;
        let wl = vlength(ScePspFVector3 {
            x: gn.x,
            y: 0.0,
            z: gn.z,
        });
        if wl > 1e-5 {
            let w = ScePspFVector3 {
                x: gn.x / wl,
                y: 0.0,
                z: gn.z / wl,
            };
            let c = vdot(v, w);
            v = vsub(v, vscale(w, c));
        }
        vel = v;
    } else {
        // Airborne: input + gravity, unconstrained.
        vel = body.velocity;
    }

    // === Collide and slide (single velocity vector) ===
    // Projecting one vector onto each contact plane can only remove speed, never
    // add or rotate it — this is what makes angled wall hits slow the player down
    // correctly and keeps the velocity parallel to whatever it's sliding on.
    // Apply the frame's movement ONCE, then iterate pure depenetration from the
    // resulting position. The previous structure re-applied the clipped move on
    // every loop iteration, so a frame that touched a wall moved the tangential
    // component up to twice — a speed BOOST while hugging any wall (even 90°),
    // flickering on and off with contact detection, which also read as camera
    // jumps. Depenetration alone produces the slide: it removes exactly the
    // into-surface component of the already-applied move and nothing else.
    let mut pos = vadd(transform.translation, vscale(vel, dt));
    let mut grounded = false;
    let mut on_steep = false;
    let mut surface_normal = ScePspFVector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    for _ in 0..4 {
        let (contact, walkable_contact) = find_contacts(pos);
        let Some((n, depth)) = contact else {
            break;
        };

        if n.y >= MIN_WALKABLE_NORMAL_Y {
            // Walkable ground: resolve straight UP, not along the tilted normal
            // (slanted depenetration converts vertical penetration into horizontal
            // drift — the old "slides down shallow ramps" bug). Re-map the stored
            // velocity onto the plane vertically (keep x/z, solve y).
            grounded = true;
            surface_normal = n;
            pos.y += (depth + SKIN) / n.y;
            vel = ScePspFVector3 {
                x: vel.x,
                y: -(n.x * vel.x + n.z * vel.z) / n.y,
                z: vel.z,
            };
        } else {
            // Non-walkable surface (steep slope, wall, or overhang). Treat it as a
            // WALL: depenetrate HORIZONTALLY and clip the velocity against the
            // horizontal part of the normal. Depenetrating along the tilted normal
            // converts horizontal approach into up-the-slope displacement (climb +
            // jitter) and vertical pumping under overhangs. Horizontal resolution
            // never lifts or sinks the player.
            if n.y > 0.0 {
                on_steep = true;
                surface_normal = n;
            }
            let wl = vlength(ScePspFVector3 {
                x: n.x,
                y: 0.0,
                z: n.z,
            });
            if wl > 0.3 {
                let w = ScePspFVector3 {
                    x: n.x / wl,
                    y: 0.0,
                    z: n.z / wl,
                };
                // Moving depth/wl along w removes `depth` of penetration along n
                // (w·n = wl).
                pos = vadd(pos, vscale(w, depth / wl + SKIN));
                let wv = vdot(vel, w);
                if wv < 0.0 {
                    vel = vsub(vel, vscale(w, wv));
                }
            } else {
                // Nearly-flat ceiling: no horizontal direction to resolve along,
                // push along the true normal.
                pos = vadd(pos, vscale(n, depth + SKIN));
            }
        }

        // Cancel the into-surface velocity component so it doesn't re-penetrate
        // next frame.
        let dot_v = vdot(vel, n);
        if dot_v < 0.0 {
            vel = vsub(vel, vscale(n, dot_v));
        }

        // If a walkable surface was also penetrated this iteration (wedged against
        // a wall/overhang while standing on the floor), don't lose the grounding —
        // otherwise the frame reads as airborne and gravity accumulates unbounded.
        if let Some((wn, _)) = walkable_contact {
            if !grounded {
                grounded = true;
                surface_normal = wn;
            }
        }
    }

    // Surface snap. After sliding, the player rests ~SKIN above the surface, so a
    // purely parallel move next frame penetrates nothing and the contact vanishes —
    // the player flickers to "air" every other frame, which lets them climb slopes
    // and makes velocity jitter. If we didn't resolve a contact this frame, probe a
    // short distance downward and, if a surface is there, snap onto it and adopt its
    // state so ground/steep contact stays stable frame to frame.
    if !grounded && !on_steep {
        const SNAP: f32 = 0.12;
        let probe = ScePspFVector3 {
            x: pos.x,
            y: pos.y - SNAP,
            z: pos.z,
        };
        let (contact, walkable_contact) = find_contacts(probe);
        // Prefer the walkable contact even when something else (e.g. an overhang
        // above) penetrates deeper — the floor is what should ground us.
        if let Some((n, depth)) = walkable_contact {
            grounded = true;
            surface_normal = n;
            pos = ScePspFVector3 {
                x: pos.x,
                y: probe.y + (depth + SKIN) / n.y,
                z: pos.z,
            };
        } else if let Some((n, depth)) = contact {
            if n.y > 0.0 {
                on_steep = true;
                surface_normal = n;
                // Resolve horizontally, like the main loop — pushing along the
                // tilted normal would lift the player off the slope again.
                let wl = vlength(ScePspFVector3 {
                    x: n.x,
                    y: 0.0,
                    z: n.z,
                });
                if wl > 0.3 {
                    let w = ScePspFVector3 {
                        x: n.x / wl,
                        y: 0.0,
                        z: n.z / wl,
                    };
                    pos = vadd(probe, vscale(w, depth / wl + SKIN));
                } else {
                    pos = vadd(probe, vscale(n, depth + SKIN));
                }
            }
        }
    }

    // Persist surface state for next frame's input projection. Grounded wins over
    // steep when both are touched (e.g. at the foot of a ramp).
    body.grounded = grounded;
    body.on_steep = on_steep && !grounded;
    if grounded || on_steep {
        body.ground_normal = surface_normal;
    }

    // Store the resolved (surface-parallel) velocity so the debug HUD and next
    // frame see it. On walkable ground, clear the vertical component so gravity
    // doesn't accumulate into a downhill slide. On a steep surface, cap the slide
    // speed so it stays steady instead of accelerating the whole time we touch it.
    body.velocity = vel;
    if body.grounded {
        body.velocity.y = 0.0;
    } else if body.on_steep {
        let s = vlength(body.velocity);
        if s > STEEP_SLIDE_SPEED {
            body.velocity = vscale(body.velocity, STEEP_SLIDE_SPEED / s);
        }
    }

    transform.translation = pos;
}

pub fn render_collider_debug(query: Query<(&Transform, &Collider, Has<StaticBody>)>) {
    unsafe {
        sys::sceGuDisable(GuState::Texture2D);
        sys::sceGuDisable(GuState::DepthTest);

        for (transform, collider, has_static) in query.iter() {
            let (hx, hy, hz) = match collider.collider_type {
                ColliderType::Cuboid(x, y, z) => (x * 0.5, y * 0.5, z * 0.5),
                ColliderType::Sphere(..) | ColliderType::Capsule(..) | ColliderType::Quad(..) => {
                    continue;
                }
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

            let color: u32 = if has_static { 0xFF0000FF } else { 0xFF00FF00 };

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
            if matches!(collider.collider_type, ColliderType::Cuboid(..)) {
                sys::sceGumRotateXYZ(&transform.rotation);
            }

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

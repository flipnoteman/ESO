use core::ptr::null;

use bevy_ecs::{
    query::With,
    schedule::{IntoScheduleConfigs, Schedule},
    system::{Query, ResMut},
    world::World,
};
use psp::{
    Align16, BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH,
    sys::{self, *},
    vram_alloc::get_vram_allocator,
};

use crate::{
    HudElement, Transform, WorldElement,
    asset_handling::{Material, Mesh, Vertex, server::AssetServer, texture::Texture},
    println,
    psp_math::vfpu_tanf,
};

static mut LIST: Align16<[u32; 0x40000]> = Align16([0; 0x40000]);
static mut CLIP_SCRATCH: Align16<[Vertex; 1024]> = Align16([Vertex::zero(); 1024]);

const NEAR_PLANE: f32 = 0.15;
const FAR_PLANE: f32 = 100.0;
const FOV: f32 = 70.0;

// Functions that are used to render anything to the screen or affect the execution context
pub struct Renderer {
    schedule: Schedule,
}

impl Renderer {
    pub fn new() -> Self {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            setup_gu.before(clear_screen),
            clear_screen,
            render_world.after(clear_screen),
            render_hud.after(render_world),
            finish_gu.after(render_hud),
        ));

        Renderer { schedule }
    }

    pub fn run(&mut self, world: &mut World) {
        self.schedule.run(world);
    }

    /// Initializes Renderer (GU) context
    pub fn init(&self) {
        unsafe {
            psp::enable_home_button();

            let allocator = get_vram_allocator().unwrap();
            let fbp0 = allocator.alloc_texture_pixels(
                BUF_WIDTH,
                SCREEN_HEIGHT,
                TexturePixelFormat::Psm8888,
            );
            let fbp1 = allocator.alloc_texture_pixels(
                BUF_WIDTH,
                SCREEN_HEIGHT,
                TexturePixelFormat::Psm8888,
            );
            let zbp = allocator.alloc_texture_pixels(
                BUF_WIDTH,
                SCREEN_HEIGHT,
                TexturePixelFormat::Psm4444,
            );
            // Attempting to free the three VRAM chunks at this point would give a
            // compile-time error since fbp0, fbp1 and zbp are used later on
            //allocator.free_all();

            // Load identity matrix into Gu
            sys::sceGumLoadIdentity();

            // Initialize Gu
            sys::sceGuInit();

            // Setup Gu for 3d
            sys::sceGuStart(
                GuContextType::Direct,
                &raw mut LIST.0 as *mut [u32; 0x40000] as *mut _,
            );
            sys::sceGuDrawBuffer(
                DisplayPixelFormat::Psm8888,
                fbp0.as_mut_ptr_from_zero() as _,
                BUF_WIDTH as i32,
            );
            sys::sceGuDispBuffer(
                SCREEN_WIDTH as i32,
                SCREEN_HEIGHT as i32,
                fbp1.as_mut_ptr_from_zero() as _,
                BUF_WIDTH as i32,
            );
            sys::sceGuDepthBuffer(zbp.as_mut_ptr_from_zero() as _, BUF_WIDTH as i32);
            sys::sceGuOffset(2048 - (SCREEN_WIDTH / 2), 2048 - (SCREEN_HEIGHT / 2));
            sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
            sys::sceGuDepthRange(65535, 0);
            sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
            sys::sceGuEnable(GuState::DepthTest);
            sys::sceGuEnable(GuState::Texture2D);
            sys::sceGuEnable(GuState::CullFace);
            sys::sceGuEnable(GuState::ClipPlanes);
            sys::sceGuEnable(GuState::ScissorTest);
            sys::sceGuDepthFunc(DepthFunc::Greater);
            sys::sceGuShadeModel(ShadingModel::Smooth);
            sys::sceGuFrontFace(FrontFaceDirection::Clockwise);
            sys::sceGuFinish();
            sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

            psp::sys::sceDisplayWaitVblankStart();

            sys::sceGuDisplay(true);
        }
    }
}

fn render_pipeline() {}

fn clear_screen() {
    unsafe {
        // clear screen
        sys::sceGuClearColor(0xff000000);
        sys::sceGuClearDepth(0);
        sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
    }
}

fn setup_gu() {
    unsafe {
        sys::sceGuStart(
            GuContextType::Direct,
            &raw mut LIST.0 as *mut [u32; 0x40000] as *mut _,
        )
    };
}

fn finish_gu(mut asset_server: ResMut<AssetServer>) {
    unsafe {
        // Finish Gu list and wait for all gu calls to finish
        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

        // Draw any debug text
        sys::sceGuDebugFlush();

        // Wait for vertical sync
        sys::sceDisplayWaitVblankStart();

        // Swap draw and display buffers
        sys::sceGuSwapBuffers();

        println!(
            "Handles: {:?}\nAssets: {}",
            asset_server.check_references::<Texture>("cell_brick.png"),
            asset_server.size()
        );

        // Drop any assets that have no attached entities or stored handles
        asset_server.drop_unused();
    }
}

#[derive(Clone, Copy)]
struct ClipVertex {
    model: Vertex,
    vx: f32,
    vy: f32,
    vz: f32,
}

impl ClipVertex {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        ClipVertex {
            model: lerp_vertex(&a.model, &b.model, t),
            vx: a.vx + t * (b.vx - a.vx),
            vy: a.vy + t * (b.vy - a.vy),
            vz: a.vz + t * (b.vz - a.vz),
        }
    }
}

/// Outputs a clipped bolygon given specific distance function [sd]
fn clip_polygon_plane(
    input: &[ClipVertex],
    output: &mut [ClipVertex; 9],
    sd: impl Fn(&ClipVertex) -> f32,
) -> usize {
    let mut out_len = 0;
    let n = input.len();
    if n == 0 {
        return 0;
    }

    for i in 0..n {
        let curr = &input[i];
        let next = &input[(i + 1) % n];
        let d_curr = sd(curr);
        let d_next = sd(next);

        if d_curr >= 0.0 {
            output[out_len] = *curr;
            out_len += 1;
        }
        if (d_curr >= 0.0) != (d_next >= 0.0) {
            let t = d_curr / (d_curr - d_next);
            output[out_len] = ClipVertex::lerp(curr, next, t);
            out_len += 1;
        }
    }
    out_len
}

fn clip_triangles_frustum(
    input: &[Vertex],
    model_mat: &ScePspFMatrix4,
    view_mat: &ScePspFMatrix4,
    near: f32,
    fov_y_deg: f32,
    aspect: f32,
) -> usize {
    // Compute transformation from aspect_ratio and fov
    let tan_y = vfpu_tanf(fov_y_deg * core::f32::consts::PI / 360.0);
    let tan_x = tan_y * aspect;

    // Get scratch buffer
    let scratch = unsafe { &mut CLIP_SCRATCH.0 };
    let mut out_len = 0;

    // default clip_vertex
    let zero_cv = ClipVertex {
        model: Vertex::zero(),
        vx: 0.0,
        vy: 0.0,
        vz: 0.0,
    };

    // Convert vertices in mesh from world and view spaces, and save in ClipVertex
    for tri in input.chunks_exact(3) {
        let mut poly = [zero_cv; 9];
        for i in 0..3 {
            let v = &tri[i];
            // model → world
            let wx =
                model_mat.x.x * v.x + model_mat.y.x * v.y + model_mat.z.x * v.z + model_mat.w.x;
            let wy =
                model_mat.x.y * v.x + model_mat.y.y * v.y + model_mat.z.y * v.z + model_mat.w.y;
            let wz =
                model_mat.x.z * v.x + model_mat.y.z * v.y + model_mat.z.z * v.z + model_mat.w.z;
            // world → view
            let vx = view_mat.x.x * wx + view_mat.y.x * wy + view_mat.z.x * wz + view_mat.w.x;
            let vy = view_mat.x.y * wx + view_mat.y.y * wy + view_mat.z.y * wz + view_mat.w.y;
            let vz = view_mat.x.z * wx + view_mat.y.z * wy + view_mat.z.z * wz + view_mat.w.z;

            poly[i] = ClipVertex {
                model: *v,
                vx,
                vy,
                vz,
            };
        }

        // allocate temporary cv buffer
        let mut temp = [zero_cv; 9];
        let mut poly_len;

        /// Helper macro to tidys up the clipping/copying logic
        macro_rules! clip_plane {
            ($sd:expr) => {{
                let n = clip_polygon_plane(&poly[..poly_len], &mut temp, $sd);
                if n == 0 {
                    continue;
                }
                poly[..n].copy_from_slice(&temp[..n]);
                poly_len = n;
            }};
        }

        // Clip against the five clip planes (near, left, right, top, bottom)
        poly_len = 3;
        clip_plane!(|v: &ClipVertex| -v.vz - near); // Near plane
        clip_plane!(|v: &ClipVertex| -v.vx - v.vz * tan_x); // left plane
        clip_plane!(|v: &ClipVertex| v.vx - v.vz * tan_x); // right plane
        clip_plane!(|v: &ClipVertex| -v.vy - v.vz * tan_y); // top plane
        clip_plane!(|v: &ClipVertex| v.vy - v.vz * tan_y); // bottom plane

        // copy new vertices to scratch buffer
        for i in 1..(poly_len - 1) {
            if out_len + 3 > unsafe { CLIP_SCRATCH.0.len() } {
                break;
            }
            scratch[out_len] = poly[0].model;
            out_len += 1;
            scratch[out_len] = poly[i].model;
            out_len += 1;
            scratch[out_len] = poly[i + 1].model;
            out_len += 1;
        }
    }

    out_len
}

/// Interpolates a new vertex
/// t: value for how far to interpolate towards v1
/// v0: initial vertice
/// v1: vertice to lerp towards
fn lerp_vertex(v0: &Vertex, v1: &Vertex, t: f32) -> Vertex {
    Vertex {
        u: v0.u + t * (v1.u - v0.u),
        v: v0.v + t * (v1.v - v0.v),
        x: v0.x + t * (v1.x - v0.x),
        y: v0.y + t * (v1.y - v0.y),
        z: v0.z + t * (v1.z - v0.z),
    }
}

pub(crate) fn render_hud(query: Query<(&Mesh, &Transform, &Material), With<HudElement>>) {
    unsafe {
        // Disable depth test since these are UI elements
        sys::sceGuDisable(GuState::CullFace);
        sys::sceGuDisable(GuState::DepthTest);

        // Load orthographic projection into projection matrix so we can use screen coordinates for
        // rendering
        sys::sceGumMatrixMode(sys::MatrixMode::Projection);
        sys::sceGumLoadIdentity();
        sys::sceGumOrtho(
            0.0,
            SCREEN_WIDTH as f32,
            SCREEN_HEIGHT as f32,
            0.0,
            -1.0,
            1.0,
        );

        // Reload identity into other matrices
        sys::sceGumMatrixMode(sys::MatrixMode::View);
        sys::sceGumLoadIdentity();

        let vertex_type =
            VertexType::VERTEX_32BITF | VertexType::TEXTURE_32BITF | VertexType::TRANSFORM_3D;

        for (mesh, transform, material) in query.iter() {
            if let Some(handle) = &material.handle() {
                if let Some(s_handle) = handle.get() {
                    let w = s_handle.width();
                    let h = s_handle.height();
                    let pitch_px = s_handle.pitch();

                    if material.blend() {
                        sceGuEnable(GuState::Blend);
                        sceGuBlendFunc(
                            sys::BlendOp::Add,
                            sys::BlendFactor::SrcAlpha,
                            sys::BlendFactor::OneMinusSrcAlpha,
                            0,
                            0,
                        );
                    }

                    // Setup Texture, we dont swizzle hud elements
                    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                    sys::sceGuTexImage(
                        MipmapLevel::None,
                        w as i32,
                        h as i32,
                        pitch_px as i32,
                        s_handle.raw_bytes(),
                    );
                    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba); // Texture Function
                    sys::sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear); // Texture filtering
                    sys::sceGuTexScale(1.0, 1.0); // Texture scale
                    sys::sceGuTexOffset(0.0, 0.0); // Texture offset

                    // Indicate that the next render will include a texture
                }
            }

            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
            sys::sceGumTranslate(&transform.translation);
            //             sys::sceGumRotateXYZ(&transform.rotation);
            //
            sys::sceGumDrawArray(
                mesh.primitive_type(),
                VertexType::from_bits_retain(vertex_type.bits()),
                mesh.vertices().len() as i32,
                null(),
                mesh.vertices().as_ptr() as *const _,
            );

            if material.blend() {
                sys::sceGuDisable(GuState::Blend);
            }
        }

        // Renable depth testing for 3d world context
        sys::sceGuEnable(GuState::DepthTest);
        sys::sceGuEnable(GuState::CullFace);
    }
}
pub(crate) fn render_world(
    query: Query<(&Mesh, &Transform, Option<&Material>), With<WorldElement>>,
) {
    unsafe {
        // Get current view matrix for custom clipping
        sys::sceGumMatrixMode(sys::MatrixMode::View);
        let mut view_mat: sys::ScePspFMatrix4 = core::mem::zeroed();
        sys::sceGumStoreMatrix(&mut view_mat);

        // Setup matrices for rendering
        sys::sceGumMatrixMode(sys::MatrixMode::Projection);
        sys::sceGumLoadIdentity();

        // Fov, Aspect Ratio, Near clipping field, far clipping field
        sys::sceGumPerspective(
            FOV,
            SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32,
            NEAR_PLANE,
            FAR_PLANE,
        );

        // Have set load the identity matrix into the model matrix so that the model we spawn isn't
        // at some "random" orientation/permutation
        sys::sceGumMatrixMode(sys::MatrixMode::Model);
        sys::sceGumLoadIdentity();

        // Vertex definition is 5 f32s, with u,v,x,y,z, so we need these types
        let mut vertex_type =
            VertexType::TEXTURE_32BITF | VertexType::VERTEX_32BITF | VertexType::TRANSFORM_3D;

        for (mesh, transform, material) in query.iter() {
            // If there is a material component on this entity
            let mut blend_enabled = false;
            let has_texture = if let Some(mat) = material {
                // If there is a mat component on this entity, does it have a valid handle
                if let Some(handle) = mat.handle() {
                    // If it has a valid handle, see if it has valid data
                    if let Some(s_handle) = handle.get() {
                        // There is a valid texture on this entity, load it
                        let w = s_handle.width();
                        let h = s_handle.height();
                        let pitch_px = s_handle.pitch();
                        let swizzle = s_handle.is_swizzled() as i32;

                        if mat.blend() {
                            blend_enabled = true;
                            sceGuEnable(GuState::Blend);
                            sceGuBlendFunc(
                                sys::BlendOp::Add,
                                sys::BlendFactor::SrcAlpha,
                                sys::BlendFactor::OneMinusSrcAlpha,
                                0,
                                0,
                            );
                        }

                        // Setup Texture
                        // Textures need to be swizzled
                        sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, swizzle);
                        sys::sceGuTexImage(
                            MipmapLevel::None,
                            w as i32,
                            h as i32,
                            pitch_px as i32,
                            s_handle.raw_bytes(),
                        );
                        sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba); // Texture Function
                        sys::sceGuTexFilter(TextureFilter::Nearest, TextureFilter::Nearest); // Texture filtering
                        sys::sceGuTexScale(1.0, 1.0); // Texture scale
                        sys::sceGuTexOffset(0.0, 0.0); // Texture offset

                        // Indicate that the next render will include a texture
                        vertex_type.set(VertexType::TEXTURE_32BITF, true);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !has_texture {
                sys::sceGuDisable(GuState::Texture2D);
                sys::sceGuColor(0xFFFF00FF);
            }

            // Set to model manipulation mode
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();

            // Place mesh
            sys::sceGumTranslate(&transform.translation);
            sys::sceGumRotateXYZ(&transform.rotation);
            // sys::sceGumScale(&ScePspFVector3 {
            //     x: 2.0,
            //     y: 2.0,
            //     z: 3.0,
            // });

            let mut model_mat = core::mem::zeroed::<sys::ScePspFMatrix4>();
            sys::sceGumStoreMatrix(&mut model_mat);

            // See if mesh was created with indices or full vertex descriptions
            let (vertices, indices, vert_count) = match &mesh.indices() {
                // If it was created with indices, use them
                Some(p) => {
                    vertex_type.set(VertexType::INDEX_16BIT, true);
                    (
                        mesh.vertices().as_ptr() as *const _,
                        p.as_ptr() as *const _,
                        p.len() as i32,
                    )
                }
                // Else, make sure we unset the bit value
                None => {
                    vertex_type.set(VertexType::INDEX_16BIT, false);
                    // Clip screen triangles against view frustum
                    // This fixes aggressive culling issues on psp
                    let count = clip_triangles_frustum(
                        mesh.vertices().as_slice(),
                        &model_mat,
                        &view_mat,
                        NEAR_PLANE,
                        FOV,
                        SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32,
                    );

                    // Since our scratch space is used multiple times between draw calls (before
                    // flushing)
                    // we'll allocate temp display list memory for vertices
                    let buf = sys::sceGuGetMemory(
                        (count as usize * core::mem::size_of::<Vertex>()) as i32,
                    ) as *mut Vertex;
                    // Copy into temp buf
                    core::ptr::copy_nonoverlapping(CLIP_SCRATCH.0.as_ptr(), buf, count as usize);

                    (buf as *const _, null(), count as i32)
                    // (
                    //     mesh.vertices().as_ptr() as *const _,
                    //     ptr::null(),
                    //     mesh.vertices().len() as i32,
                    // )
                }
            };

            // Since we're not writing scratch stuff for GE access we don't need to cache writeback
            // sys::sceKernelDcacheWritebackInvalidateRange(
            //     CLIP_SCRATCH.0.as_ptr() as *const _,
            //     (vert_count as usize * core::mem::size_of::<Vertex>()) as u32,
            // );

            // Draw
            sys::sceGumDrawArray(
                mesh.primitive_type(),
                // GuPrimitive::Lines,
                VertexType::from_bits_retain(vertex_type.bits()),
                vert_count,
                indices,
                vertices,
            );

            if !has_texture {
                sys::sceGuEnable(GuState::Texture2D);
            }

            if has_texture && blend_enabled {
                sys::sceGuDisable(GuState::Blend);
            }
        }
    }
}

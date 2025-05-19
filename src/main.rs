#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(dead_code)]
#![feature(asm_experimental_arch)]

use core::{ptr, f32::consts::PI};
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::system::{Query, Res, ResMut, Single};
use bevy_ecs::world::World;
use psp::Align16;
use psp::sys::{
    self, ClearBuffer, CtrlButtons, DepthFunc, DisplayPixelFormat, FrontFaceDirection, GuContextType, GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, MipmapLevel,  ScePspFVector3,  ShadingModel, TextureColorComponent, TextureEffect, TextureFilter, TexturePixelFormat, VertexType
};
use psp::vram_alloc::get_vram_allocator;
use psp::{BUF_WIDTH, SCREEN_WIDTH, SCREEN_HEIGHT};
use bevy_ecs::component;

extern crate alloc;

use psp_geometry::Mesh;
use spin::Once;

mod psp_image;
mod psp_geometry;
mod psp_input;
mod psp_print;
mod psp_math;
mod psp_text;
use psp_image::load_png_swizzled;

psp::module!("ESO", 1, 1);

static mut LIST: Align16<[u32; 0x40000]> = Align16([0; 0x40000]);
static CELL_BRICK_TEXTURE: Once<(u32, u32, usize, alloc::boxed::Box<[u8]>)> = Once::new();

// Game constants
const PLAYER_SPEED: f32 = 2.5;
const CAMERA_ROTATION_SPEED: f32 = PI;

#[derive(Debug, component::Component)]
struct Transform{
    translation: ScePspFVector3,
    rotation: ScePspFVector3,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            translation: ScePspFVector3 {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            rotation: ScePspFVector3 {
                x: 0.,
                y: 0.,
                z: 0.,
            }
        }
    }
}

impl Transform {
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Transform {
            translation: ScePspFVector3 { x, y, z },
            ..Default::default()
        }
    }
    
    pub fn with_translation(&self, x: f32, y: f32, z: f32) -> Self {
        let rotation = self.rotation;
   
        Transform {
            translation: ScePspFVector3 { x, y, z },
            rotation 
        }
    }
    
    pub fn with_rotation(&mut self, x: f32, y: f32, z: f32) -> Self {
        let translation = self.translation;
   
        Transform {
            translation,
            rotation: ScePspFVector3 { x, y, z }, 
        }
    }
}

#[derive(component::Component, Debug)]
struct Player;

#[derive(Resource, Debug)]
struct Controller {
    buttons: CtrlButtons,
    analog: [f32; 2]
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            buttons: CtrlButtons::default(),
            analog: [0.0, 0.0]
        }
    }
}

#[derive(Resource, Debug)]
struct Time {
    delta: i64,
    total: i64,
    time: i64
}

impl Default for Time {
    fn default() -> Self {
        let now = unsafe { psp::sys::sceKernelGetSystemTimeWide() };  
        Self {
            delta: 0,
            total: 0,
            time: now,
        }
    }
}

impl Time {
    #[inline]
    pub fn delta_seconds(&self) -> f32 {
        self.delta as f32 * 1.0e-6
    }
}

fn update_time(mut time: ResMut<Time>){
    unsafe {
        let now = psp::sys::sceKernelGetSystemTimeWide();  
        let delta = now - time.time;
        
        time.delta = delta;
        time.total += delta;
        time.time = now;
    }
}

fn update_controls(mut controller: ResMut<Controller>) {
    let (buttons, (sx, sy)) = psp_input::poll_inputs();
    controller.analog[0] = sx;
    controller.analog[1] = sy;
    controller.buttons = buttons;
}

fn update_player(mut transform: Single<&mut Transform, With<Player>>, time: Res<Time>, controller: Res<Controller>) {
    unsafe {

        // set Gu Matrix mode to edit the View
        sys::sceGumMatrixMode(sys::MatrixMode::View);
        sys::sceGumLoadIdentity();

        // Get analog stick state
        let sx = controller.analog[0];
        let sy = controller.analog[1];

        // Calculate the cos and sin of the current camera rotation
        let sin = psp_math::vfpu_sinf(transform.rotation.y);
        let cos = psp_math::vfpu_cosf(transform.rotation.y);
        
        // Calculate the movement based on the camera rotation
        let dx = sx * cos - sy * sin;
        let dz = sx * sin + sy * cos;
         
        // Calculate delta time and set the players translation to the new coordinates based one
        // motion
        let dt = time.delta_seconds();
        transform.translation.x += dx * PLAYER_SPEED * dt;
        transform.translation.z += dz * PLAYER_SPEED * dt;
        
        // Edit players rotation based on square and circle input
        if controller.buttons.contains(CtrlButtons::SQUARE) {
           transform.rotation.y -= CAMERA_ROTATION_SPEED * dt; 
        }
        if controller.buttons.contains(CtrlButtons::CIRCLE) {
           transform.rotation.y += CAMERA_ROTATION_SPEED * dt; 
        }
        
        // If rotation is greater than PI or less than PI then reset it so that it doesn't go out
        // of bounds
        if transform.rotation.y > PI {transform.rotation.y  -= 2.0 * PI } 
        if transform.rotation.y < -PI {transform.rotation.y  += 2.0 * PI } 
       
        // Rotate camera
        sys::sceGumRotateY(transform.rotation.y);
        
        // Create translation vector based on the negatives of our translation
        // This is because We want everything to move in the opposite direction of the camera
        let t = ScePspFVector3 {
            x: -transform.translation.x,
            y: 0.0,
            z: -transform.translation.z
        };
        
        // Move the camera
        sys::sceGumTranslate(&t);
    }
}
 


fn init_textures() {
    CELL_BRICK_TEXTURE.call_once(||unsafe { load_png_swizzled(include_bytes!("../assets/cell_brick.png")).expect("Bad PNG") });
}

unsafe fn init_Gu() {
    psp::enable_home_button();

    let allocator = get_vram_allocator().unwrap();
    let fbp0 = allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
    let fbp1 = allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm8888);
    let zbp = allocator.alloc_texture_pixels(BUF_WIDTH, SCREEN_HEIGHT, TexturePixelFormat::Psm4444);
    // Attempting to free the three VRAM chunks at this point would give a
    // compile-time error since fbp0, fbp1 and zbp are used later on
    //allocator.free_all();
    

    // Load identity matrix into Gu
    sys::sceGumLoadIdentity();

    // Initialize Gu
    sys::sceGuInit();

    // Setup Gu for 3d
    sys::sceGuStart(GuContextType::Direct, &raw mut LIST.0 as *mut [u32; 0x40000] as *mut _);
    sys::sceGuDrawBuffer(DisplayPixelFormat::Psm8888, fbp0.as_mut_ptr_from_zero() as _, BUF_WIDTH as i32);
    sys::sceGuDispBuffer(SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32, fbp1.as_mut_ptr_from_zero() as _, BUF_WIDTH as i32);
    sys::sceGuDepthBuffer(zbp.as_mut_ptr_from_zero() as _, BUF_WIDTH as i32);
    sys::sceGuOffset(2048 - (SCREEN_WIDTH / 2), 2048 - (SCREEN_HEIGHT / 2));
    sys::sceGuViewport(2048, 2048, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
    sys::sceGuDepthRange(65535, 0);
    sys::sceGuScissor(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
    sys::sceGuEnable(GuState::ScissorTest);
    sys::sceGuDepthFunc(DepthFunc::GreaterOrEqual);
    sys::sceGuEnable(GuState::DepthTest);
    sys::sceGuFrontFace(FrontFaceDirection::Clockwise);
    sys::sceGuShadeModel(ShadingModel::Smooth);
    sys::sceGuEnable(GuState::CullFace);
    sys::sceGuEnable(GuState::Texture2D);
    sys::sceGuEnable(GuState::ClipPlanes);
    sys::sceGuFinish();
    sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

    psp::sys::sceDisplayWaitVblankStart();

    sys::sceGuDisplay(true);
    
    // Load assets
    init_textures();
}

fn clear_screen() {
    unsafe {
        // clear screen
        sys::sceGuClearColor(0xff554433);
        sys::sceGuClearDepth(0);
        sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
    }
}


fn draw_world(query: Query<(&Mesh, &Transform)>) {
    unsafe {
        
        // setup matrices for cube
        sys::sceGumMatrixMode(sys::MatrixMode::Projection);
        sys::sceGumLoadIdentity();
        sys::sceGumPerspective(75.0, 16.0 / 9.0, 0.5, 1000.0);

        sys::sceGumMatrixMode(sys::MatrixMode::Model);
        sys::sceGumLoadIdentity();


        let (w, h, pitch_px, tex) = &CELL_BRICK_TEXTURE.wait();

        // Setup Texture
        // Textures need to be swizzled
        sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 1);
        sys::sceGuTexImage(MipmapLevel::None, *w as i32, *h as i32, *pitch_px as i32, tex.as_ptr() as *const _ );
        sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgb); // Texture Function
        sys::sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear); // Texture filtering
        sys::sceGuTexScale(1.0, 1.0); // Texture scale
        sys::sceGuTexOffset(0.0, 0.0); // Texture offset
 
        for (mesh, transform) in query.iter() {
            
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
            // Place mesh
            sys::sceGumTranslate(&transform.translation);
            sys::sceGumRotateXYZ(&transform.rotation);
            
            // draw cube
            sys::sceGumDrawArray(
                GuPrimitive::Triangles,
                VertexType::TEXTURE_32BITF | VertexType::VERTEX_32BITF | VertexType::TRANSFORM_3D,
                mesh.vertices.len() as i32,
                ptr::null_mut(),
                mesh.vertices.as_ptr() as *const _
            );
            
        }

    }
}

unsafe fn psp_main_inner() {

    // Create world and resources
    let mut world = World::new();
    world.insert_resource(Time::default());
    world.insert_resource(Controller::default());

    // Create schedule
    let mut update_schedule = Schedule::default();
    let mut render_schedule = Schedule::default();
    update_schedule.add_systems(
        (
            update_time,
            update_controls, 
            update_player,
        )
    );

    render_schedule.add_systems(
        (
            clear_screen,
            draw_world.after(clear_screen)
        )
    );

    // Spawn components and entities
    world.spawn((
        Player, 
        Transform::default(),
    ));

    world.spawn((
        Mesh::cube(1.0),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));

    world.spawn((
        Mesh::cuboid(0.5, 2.0, 3.0),
        Transform::from_xyz(3.0, 0.5, -2.0).with_rotation(0.0, PI/2.0, 0.0),
    ));

    // Initialize the psp Gu system
    init_Gu();

    // Main game loop
    loop {
        // This updated game logic 
        update_schedule.run(&mut world);
        
        sys::sceGuStart(GuContextType::Direct, &raw mut LIST.0 as *mut [u32; 0x40000] as *mut _);
        

        render_schedule.run(&mut world);
        
        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

//         sys::sceGuDebugFlush();

        sys::sceDisplayWaitVblankStart();
        sys::sceGuSwapBuffers();
    }

    // psp::sys::sceKernelExitGame();
}

fn psp_main() {
    unsafe { psp_main_inner() }
}

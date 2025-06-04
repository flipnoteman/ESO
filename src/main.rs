#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(dead_code)]
#![feature(asm_experimental_arch)]

use core::{ptr, f32::consts::PI};
use alloc::sync::Arc;
use alloc::{format, vec};
use bevy_ecs::query::{With, WorldQuery};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::system::{Commands, Query, Res, ResMut, Single};
use bevy_ecs::world::World;
use psp::Align16;
use psp::sys::{
    self, ClearBuffer, CtrlButtons, DepthFunc, DisplayPixelFormat, FrontFaceDirection, GuContextType, GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, MipmapLevel,  ScePspFVector3,  ShadingModel, TextureColorComponent, TextureEffect, TextureFilter, TexturePixelFormat, VertexType
};
use psp::vram_alloc::get_vram_allocator;
use psp::{BUF_WIDTH, SCREEN_WIDTH, SCREEN_HEIGHT};
use bevy_ecs::component;

extern crate alloc;

use psp_assets::{Asset, AssetServer, Font, Image};
use psp_geometry::{Material, Mesh};
use spin::Once;

mod psp_image;
mod psp_geometry;
mod psp_input;
mod psp_print;
mod psp_math;
mod psp_text;
mod psp_assets;
use psp_image::{load_png, load_png_swizzled};

psp::module!("ESO", 1, 1);

static mut LIST: Align16<[u32; 0x40000]> = Align16([0; 0x40000]);

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
    
    pub fn with_rotation(&self, x: f32, y: f32, z: f32) -> Self {
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

#[allow(non_snake_case)]
fn init_Gu() {
    unsafe {
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
        sys::sceGuDepthFunc(DepthFunc::Greater);
        sys::sceGuEnable(GuState::DepthTest);
        sys::sceGuShadeModel(ShadingModel::Smooth);
        sys::sceGuEnable(GuState::CullFace);
        sys::sceGuFrontFace(FrontFaceDirection::Clockwise);
        sys::sceGuEnable(GuState::Texture2D);
        sys::sceGuEnable(GuState::ClipPlanes);
        sys::sceGuFinish();
        sys::sceGuSync(GuSyncMode::Finish, GuSyncBehavior::Wait);

        psp::sys::sceDisplayWaitVblankStart();

        sys::sceGuDisplay(true);
    }
}

fn clear_screen() {
    unsafe {
        // clear screen
        sys::sceGuClearColor(0xff554433);
        sys::sceGuClearDepth(0);
        sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
    }
}


fn render_world(query: Query<(&Mesh, &Transform, &Material)>) {
    unsafe {
        
        // Setup matrices for rendering
        sys::sceGumMatrixMode(sys::MatrixMode::Projection);
        sys::sceGumLoadIdentity();
        // Fov, Aspect Ratio, Near clipping field, far clipping field
        sys::sceGumPerspective(90.0, 16.0 / 9.0, 0.50, 40.0);

        // Have set load the identity matrix into the model matrix so that the model we spawn isn't
        // at some "random" orientation/permutation
        sys::sceGumMatrixMode(sys::MatrixMode::Model);
        sys::sceGumLoadIdentity();

        let mut vertex_type = VertexType::VERTEX_32BITF | VertexType::TRANSFORM_3D;
        
        for (mesh, transform, material) in query.iter() {
            if let Some(handle) = &material.handle {
                if let Some(s_handle) = handle.upgrade() {
                    let w = s_handle.width();
                    let h = s_handle.height();
                    let pitch_px = s_handle.pitch();
                    
                    // Setup Texture
                    // Textures need to be swizzled
                    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 1);
                    sys::sceGuTexImage(MipmapLevel::None, w as i32, h as i32, pitch_px as i32, s_handle.raw_bytes());
                    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgb); // Texture Function
                    sys::sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear); // Texture filtering
                    sys::sceGuTexScale(1.0, 1.0); // Texture scale
                    sys::sceGuTexOffset(0.0, 0.0); // Texture offset
                   
                    // Indicate that the next render will include a texture
                    vertex_type.set(VertexType::TEXTURE_32BITF, true);
                }
            };
            
            // Set to model manipulation mode
            sys::sceGumMatrixMode(sys::MatrixMode::Model);
            sys::sceGumLoadIdentity();
            
            // Place mesh
            sys::sceGumTranslate(&transform.translation);
            sys::sceGumRotateXYZ(&transform.rotation);
            
            // See if mesh was created with indices or full vertex descriptions
            let ind = match &mesh.indices {
                // If it was created with indices, use them
                Some(p) => {
                    vertex_type.set(VertexType::INDEX_16BIT, true);
                    p.as_ptr() as *const _
                },
                // Else, make sure we unset the bit value
                None => {
                    vertex_type.set(VertexType::INDEX_16BIT, false);
                    ptr::null_mut()
                }
            };

            // draw cube
            sys::sceGumDrawArray(
                mesh.primitive_type,
                VertexType::from_bits_retain(vertex_type.bits()),
                mesh.vertices.len() as i32,
                ind,
                mesh.vertices.as_ptr() as *const _
            );
            
        }

    }
}

fn setup_gu() {
    unsafe { sys::sceGuStart(GuContextType::Direct, &raw mut LIST.0 as *mut [u32; 0x40000] as *mut _) };
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

        println!("Handles: {:?}\nAssets: {}", asset_server.check_references("cell_brick.png"), asset_server.size());
        
        // Drop any assets that have no attached entities or stored handles
        asset_server.drop_unused(); 
    }
}

fn setup_world(
    world: &mut World,
) {

    let mut asset_server = world.resource_mut::<AssetServer>();

    let font_path = "ms0:/psp/game/cat_dev/eso/assets/default_font.png";
    let font = Font::new(font_path);
    let font_handle = asset_server.add(font).expect(format!("Could not add image: {}", font_path).as_str());

    let brick_path = "ms0:/psp/game/cat_dev/eso/assets/cell_brick.png";
    let image = Image::new(brick_path);
    let brick_handle = asset_server.add(image).expect(format!("Could not add image: {}", brick_path).as_str());
     
    // Spawn components and entities
    world.spawn((
        Player, 
        Transform::default(),
    ));
    
    let brick_material = Material::new(&brick_handle, TexturePixelFormat::Psm8888, true);
    
    // Spawn world objects
    world.spawn_batch(vec![
        (
            Mesh::cube_indexed(1.0),
            Transform::from_xyz(0.0, 0.0, -2.0),
            brick_material.clone() // Should only clone a weak handle to the texture
        ),
        (
            Mesh::cuboid(0.5, 2.0, 3.0),
            Transform::from_xyz(3.0, 0.5, -2.0).with_rotation(0.0, PI/2.0, 0.0),
            brick_material.clone()
        ),
        (
            Mesh::subdivided_plane(10.0, 10.0, 2, 2),
            Transform::from_xyz(0.0, -0.5, -0.0).with_rotation(-PI/2.0, 0.0, 0.0),
            brick_material
        ),
    ]);
}

unsafe fn psp_main_inner() {

    // Create world and resources
    let mut world = World::new();
    world.insert_resource(Time::default());
    world.insert_resource(Controller::default());
    world.insert_resource(AssetServer::default());

    // Create schedule
    let mut startup_schedule = Schedule::default();
    let mut update_schedule = Schedule::default();
    let mut render_schedule = Schedule::default();

    // Functions to only be run once
    startup_schedule.add_systems(
        (
            setup_world,
            init_Gu.after(setup_world),
//             init_textures.after(init_Gu),
        )
    );
    
    // Functions that are separate to render functions that will run in primary context (before
    // Gu context swap
    update_schedule.add_systems(
        (
            update_time,
            update_controls, 
            update_player.after(update_controls),
        )
    );

    // Functions that are used to render anything to the screen or affect the execution context
    render_schedule.add_systems(
        (
            setup_gu.before(clear_screen),
            clear_screen,
            render_world.after(clear_screen),
            finish_gu.after(render_world)
        )
    );

    
    // Run startup functions
    startup_schedule.run(&mut world);

    // Main game loop
    loop {
        // This updates game logic 
        update_schedule.run(&mut world);

        render_schedule.run(&mut world);
    }

    // psp::sys::sceKernelExitGame();
}

fn psp_main() {
    unsafe { psp_main_inner() }
}

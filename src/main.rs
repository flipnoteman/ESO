#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(dead_code)]
#![allow(unused)]
#![feature(asm_experimental_arch)]
#![allow(unused_imports)]
#![allow(linker_messages)]

use aligned_vec::{AVec, ConstAlign, avec};
use alloc::{format, sync::Arc, vec};
use bevy_ecs::component::{self, Component};
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::system::{Local, Query, Res, ResMut, Single};
use bevy_ecs::world::World;
use bytemuck::Zeroable;
use core::ptr::null;
use core::{f32::consts::PI, ptr};
use psp::Align16;
use psp::sys::{
    self, ClearBuffer, CtrlButtons, DepthFunc, DisplayPixelFormat, FrontFaceDirection,
    GuContextType, GuPrimitive, GuState, GuSyncBehavior, GuSyncMode, MipmapLevel, ScePspFMatrix4,
    ScePspFVector3, ShadingModel, TextureColorComponent, TextureEffect, TextureFilter,
    TexturePixelFormat, VertexType, sceGuBlendFunc, sceGuEnable,
};
use psp::vram_alloc::get_vram_allocator;
use psp::{BUF_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH};

extern crate alloc;

// Project includes
mod asset_handling;
mod physics;
mod psp_input;
mod psp_math;
mod psp_print;
mod render;
mod text;

use crate::asset_handling::mesh::MeshAsset;
use crate::asset_handling::server::AssetServer;
use crate::asset_handling::texture::Texture;
use crate::asset_handling::{Material, Mesh, Vertex};
use crate::physics::{
    collect_static_data, player_physics, Collider, RigidBody, StaticBody, StaticColliderData,
};
use crate::render::*;
use crate::text::Text;

psp::module!("ESO", 1, 1);

// Game constants
const PLAYER_SPEED: f32 = 2.5;
const CAMERA_ROTATION_SPEED: f32 = PI;

#[derive(Debug, Clone, component::Component)]
struct Transform {
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
            },
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
            rotation,
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
    analog: [f32; 2],
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            buttons: CtrlButtons::default(),
            analog: [0.0, 0.0],
        }
    }
}

#[derive(Resource, Debug)]
struct Time {
    delta: i64,
    total: i64,
    time: i64,
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

    #[inline]
    pub fn total_seconds(&self) -> f32 {
        self.total as f32 * 1.0e-6
    }
}

fn update_time(mut time: ResMut<Time>) {
    unsafe {
        let now = psp::sys::sceKernelGetSystemTimeWide();
        // Cap at ~30fps minimum. Without this, the first frame's delta equals the
        // full startup duration (GU init + asset loading), causing physics to move
        // the player dozens of metres in one step and tunnel through the floor.
        let delta = (now - time.time).min(33_333);

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

/// Everything to do with player (camera)
fn update_player(
    mut data: Single<(&mut Transform, &mut RigidBody), With<Player>>,
    time: Res<Time>,
    controller: Res<Controller>,
) {
    let (mut transform, mut body) = data.into_inner();

    // Get analog stick state
    let sx = controller.analog[0];
    let sy = controller.analog[1];

    // Calculate the cos and sin of the current camera rotation
    let sin = psp_math::vfpu_sinf(transform.rotation.y);
    let cos = psp_math::vfpu_cosf(transform.rotation.y);

    // Calculate delta time
    let dt = time.delta_seconds();

    //================= Control definitions and effects

    if controller.buttons.contains(CtrlButtons::LTRIGGER) {
        // StrafeLock control — set horizontal velocity from analog
        body.velocity.x = (sx * cos - sy * sin) * PLAYER_SPEED;
        body.velocity.z = (sx * sin + sy * cos) * PLAYER_SPEED;

        // TODO: Implement Camera lock on when enemies are present
    } else if controller.buttons.contains(CtrlButtons::RTRIGGER) {
        // Freelook Control — no movement
        body.velocity.x = 0.0;
        body.velocity.z = 0.0;

        // Translate stick to camera movement
        transform.rotation.x += sy * CAMERA_ROTATION_SPEED * dt;
        transform.rotation.y += sx * CAMERA_ROTATION_SPEED * dt;
    } else {
        // Normal control
        // Stick y controls forward/backwards movement
        body.velocity.x = -sy * sin * PLAYER_SPEED;
        body.velocity.z = sy * cos * PLAYER_SPEED;

        // Stick x controls horizontal camera movement
        transform.rotation.y += sx * CAMERA_ROTATION_SPEED * dt;

        // Camera exponential decay back to zero
        let decay = 8.0;
        transform.rotation.x *= 1.0 - (decay * dt).min(1.0);
    }

    //================= This is like constraints and junk

    // Clamp x rotation
    transform.rotation.x = transform
        .rotation
        .x
        .clamp(-core::f32::consts::FRAC_PI_2, core::f32::consts::FRAC_PI_2);

    // If rotation is greater than PI or less than PI then reset it so that it doesn't go out of bounds
    if transform.rotation.y > PI {
        transform.rotation.y -= 2.0 * PI
    }
    if transform.rotation.y < -PI {
        transform.rotation.y += 2.0 * PI
    }
}

#[derive(component::Component)]
struct HudElement;

#[derive(component::Component)]
struct WorldElement;

fn setup_ui(world: &mut World) {
    let mut asset_server = world.resource_mut::<AssetServer>();

    // let crosshair_path = "ms0:/psp/game/cat_dev/eso/assets/crosshair.png";
    let crosshair_path = "./assets/crosshair.png";
    let crosshair = Texture::new(crosshair_path, false);
    let crosshair_handle = asset_server
        .add(crosshair)
        .expect(format!("Could not add image: {}", crosshair_path).as_str());

    // HUD text. AssetServer::add dedups by filename, so this returns the same
    // font handle whether or not setup_world loaded it first. Load before any
    // spawn so the asset_server borrow is released before we touch `world`.
    let font_path = "./assets/default_font.png";
    let font_handle = asset_server
        .add(Texture::new(font_path, false))
        .expect(format!("Could not add image: {}", font_path).as_str());

    world.spawn((
        Mesh::plane(20.0, 20.0),
        Transform::from_xyz(SCREEN_WIDTH as f32 / 2.0, SCREEN_HEIGHT as f32 / 2.0, 0.0),
        Material::new(crosshair_handle, TexturePixelFormat::Psm8888, true),
        HudElement,
    ));

    // Debug overlay: a single small (native 8px) Text in the top-left whose
    // content is rewritten every frame by `update_debug_overlay`. Green tint so
    // it reads over the mostly-dark 3D scene.
    world.spawn((
        Text::new("", font_handle)
            .at(4.0, 4.0)
            .scale(1.0)
            .color(0xFF00FF00),
        DebugOverlay,
    ));
}

#[derive(component::Component)]
struct DebugOverlay;

/// Edge-detected button toggle for the whole debug view (text overlay + collider
/// wireframes). Press SELECT to flip it. `Local<bool>` remembers last frame's
/// button state so holding the button toggles only once.
///
/// To use a combo instead, require multiple buttons, e.g.:
/// `let pressed = b.contains(CtrlButtons::SELECT) && b.contains(CtrlButtons::LTRIGGER);`
fn toggle_debug(
    controller: Res<Controller>,
    mut debug: ResMut<RenderDebug>,
    mut was_pressed: Local<bool>,
) {
    let pressed = controller.buttons.contains(CtrlButtons::SELECT);
    if pressed && !*was_pressed {
        debug.0 = !debug.0;
    }
    *was_pressed = pressed;
}

/// Rewrites the debug overlay text each frame with frame timing and player state.
/// When the debug view is off, the overlay is cleared so it draws nothing.
fn update_debug_overlay(
    mut text: Single<&mut Text, With<DebugOverlay>>,
    time: Res<Time>,
    debug: Res<RenderDebug>,
    player: Single<(&Transform, &RigidBody), With<Player>>,
) {
    if !debug.0 {
        text.content.clear();
        return;
    }

    let (transform, body) = player.into_inner();

    // delta is clamped to >= 1/30s in update_time, so FPS reads at most ~30 even
    // if the frame was faster — good enough as a coarse health indicator.
    let dt = time.delta_seconds();
    let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

    let p = &transform.translation;
    let v = &body.velocity;

    text.content = format!(
        "FPS {:>5.1}  DT {:>5.1}ms\nPOS {:>7.2} {:>7.2} {:>7.2}\nVEL {:>7.2} {:>7.2} {:>7.2}",
        fps,
        dt * 1000.0,
        p.x,
        p.y,
        p.z,
        v.x,
        v.y,
        v.z,
    );
}

#[derive(Clone, Component)]
struct ChickenTag;

fn move_chicken(mut transform: Single<(&mut Transform), With<ChickenTag>>, time: Res<Time>) {
    let rotation = ScePspFVector3 {
        x: transform.rotation.x,
        y: transform.rotation.y + 1.5 * time.delta_seconds(),
        z: transform.rotation.z,
    };

    transform.rotation = rotation;

    let amplitude = 0.25;
    let speed = 2.0;

    transform.translation.y = amplitude * psp_math::vfpu_sinf(time.total_seconds() * speed);
}

fn setup_world(world: &mut World) {
    let mut asset_server = world.resource_mut::<AssetServer>();


    // let brick_path = "ms0:/psp/game/cat_dev/eso/assets/cell_brick.png";
    let brick_path = "./assets/cell_brick.png";
    let image = Texture::new(brick_path, true);

    let brick_handle = asset_server
        .add(image)
        .expect(format!("Could not add image: {}", brick_path).as_str());

    // let chicken_path = "ms0:/psp/game/cat_dev/eso/assets/meshes/chicken.mesh";
    let chicken_path = "./assets/meshes/chicken.mesh";
    let chicken_mesh = MeshAsset::new(chicken_path);
    let chicken_handle = asset_server.add(chicken_mesh).expect("Could not add mesh");
    let chicken_texture_path = "./assets/meshes/mati_chicken_Diffuse256.png";
    let chicken_texhandle = asset_server
        .add(Texture::new(chicken_texture_path, true))
        .expect(format!("Could not add image: {}", chicken_texture_path).as_str());

    // Spawn components and entities
    world.spawn((
        Player,
        // Spawn a few units above the floor so the player falls and lands on it,
        // rather than starting overlapping/inside the floor collider.
        Transform::from_xyz(0.0, 5.0, 0.0),
        Collider::cuboid(0.5, 1.0, 0.5),
        RigidBody::default(),
    ));

    // Spawn chicken
    world.spawn_batch(vec![(
        Mesh::from_handle(&chicken_handle).expect("Mesh not loaded"),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Material::new(chicken_texhandle.clone(), TexturePixelFormat::Psm8888, true),
        WorldElement,
        ChickenTag,
    )]);

    // Spawn world objects
    world.spawn_batch(vec![
        (
            Mesh::cube_indexed(1.0),
            Transform::from_xyz(0.0, 0.0, -2.0),
            Material::new(brick_handle.clone(), TexturePixelFormat::Psm8888, false),
            WorldElement,
            Collider::cuboid(1.0, 1.0, 1.0),
            StaticBody,
        ),
        (
            Mesh::cuboid(0.5, 2.0, 3.0),
            Transform::from_xyz(3.0, 0.5, -2.0).with_rotation(0.0, PI / 2.0, 0.0),
            Material::new(brick_handle.clone(), TexturePixelFormat::Psm8888, false),
            WorldElement,
            Collider::cuboid(0.5, 2.0, 3.0),
            StaticBody, // not dynamic
        ),
        (
            Mesh::subdivided_plane(10.0, 10.0, 2, 2),
            Transform::from_xyz(0.0, -0.5, 0.0).with_rotation(-PI / 2.0, 0.0, 0.0),
            Material::new(brick_handle.clone(), TexturePixelFormat::Psm8888, false),
            WorldElement,
            Collider::cuboid(10.0, 10.0, 0.1),
            StaticBody,
        ),
        (
            Mesh::cuboid(2.0, 2.0, 0.5),
            Transform::from_xyz(-1.0, 0.0, 0.0).with_rotation(-0.5, 0.0, 0.0),
            Material::new(brick_handle.clone(), TexturePixelFormat::Psm8888, false),
            WorldElement,
            Collider::cuboid(2.0, 2.0, 0.5),
            StaticBody,
            ),
    ]);
}

unsafe fn psp_main_inner() {
    // Create world and resources
    let mut world = World::new();
    let mut renderer = Renderer::new(&mut world);

    Renderer::init();
    // Start with the debug view hidden; press SELECT in-game to toggle it on.
    Renderer::set_debug(&mut world, false);

    world.insert_resource(Time::default());
    world.insert_resource(Controller::default());
    world.insert_resource(AssetServer::default());

    // Static collision data (updated each frame before player_physics)
    world.insert_resource(StaticColliderData::default());

    // Create schedule
    let mut startup_schedule = Schedule::default();
    let mut update_schedule = Schedule::default();

    // Functions to only be run once
    startup_schedule.add_systems((setup_world, setup_ui));

    // Functions that are separate to render functions that will run in primary context (before
    // Gu context swap
    update_schedule.add_systems((
        update_time,
        update_controls,
        update_player.after(update_controls),
        toggle_debug.after(update_controls),
        move_chicken,
        collect_static_data.before(player_physics),
        player_physics,
        update_debug_overlay.after(player_physics),
    ));

    // Run startup functions
    startup_schedule.run(&mut world);

    // Main game loop
    loop {
        // This updates game logic
        update_schedule.run(&mut world);

        renderer.run(&mut world);
    }

    psp::sys::sceKernelExitGame();
}

fn psp_main() {
    unsafe { psp_main_inner() }
}

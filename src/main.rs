#![allow(dead_code, unused_imports, unused_variables)]

use bevy::{
    app::AppExit,
    asset::AssetServerSettings,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    gltf::{Gltf, GltfMesh},
    prelude::*,
    render::{
        camera::PerspectiveProjection,
        mesh::Indices,
        pass::ClearColor,
        pipeline::PrimitiveTopology,
        texture::{Extent3d, TextureDimension, TextureFormat},
    },
    sprite::collide_aabb::{collide, Collision},
};
use bevy_kira_audio::{Audio, AudioChannel, AudioPlugin};
//use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};
use serde::Deserialize;
use std::{collections::HashMap, f32::consts::*, fs::File, io::Read};

#[cfg(debug_assertions)]
use bevy_inspector_egui::{WorldInspectorParams, WorldInspectorPlugin};

mod boot;
mod error;
mod inventory;
mod level;
mod loader;
mod mainmenu;
mod serialize;
mod text_asset;

use crate::{
    boot::{BootPlugin, UiResources},
    error::Error,
    inventory::{
        Buildable, Inventory, InventoryPlugin, RegenerateInventoryUiEvent, SelectSlot,
        SelectSlotEvent, Slot, SlotState, UpdateInventorySlots,
    },
    level::{Level, LevelNameText, LevelPlugin, LoadLevel, LoadLevelEvent},
    loader::{Loader, LoaderPlugin},
    mainmenu::MainMenuPlugin,
    serialize::{Buildables, Levels, SerializePlugin},
    text_asset::{TextAsset, TextAssetPlugin},
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum AppState {
    /// Boot sequence (critical assets loading).
    Boot,
    /// Main menu.
    MainMenu,
    /// Playing a game level.
    InGame,
    /// End screen.
    TheEnd,
}

struct EntityManager {
    // HACK to delete everything on TheEnd screen
    all_entities: Vec<Entity>,
}

impl EntityManager {
    pub fn new() -> EntityManager {
        EntityManager {
            all_entities: vec![],
        }
    }
}

// fn exit_system(mut exit: EventWriter<AppExit>) {
//     exit.send(AppExit);
// }

pub struct ResetPlateEvent;

struct Plate {
    entity: Entity,
    rotate_speed: f32,
}

impl Plate {
    pub fn new(entity: Entity) -> Plate {
        Plate {
            entity,
            rotate_speed: 10.0,
        }
    }
}

fn plate_reset_system(
    mut commands: Commands,
    mut ev_reset_plate: EventReader<ResetPlateEvent>,
    mut grid: ResMut<Grid>,
    query_plate: Query<(&Plate,)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Consume all reset events, do the work once
    if let Some(_) = ev_reset_plate.iter().last() {
        trace!("plate_reset_system() - GOT EVENT");

        // Clear grid
        grid.clear(Some(&mut commands));

        // Rebuild plate with N copies of a single 'cell' mesh laid out in grid
        if let Ok((plate,)) = query_plate.single() {
            // TODO - cache mesh
            let cell_mesh = meshes.add(Mesh::from(shape::Box::new(1.0, 0.1, 1.0)));
            grid.regenerate(&mut commands, cell_mesh.clone(), plate.entity);
        }
    }
}

#[derive(Debug)]
pub struct Cursor {
    pos: IVec2,
    move_speed: f32,
    //weight: f32,
    cursor_entity: Entity,
    cursor_mesh: Handle<Mesh>,
    cursor_mat: Handle<StandardMaterial>,
    spawn_root_entity: Entity,
}

impl Cursor {
    pub fn new(cursor_entity: Entity, spawn_root_entity: Entity) -> Cursor {
        Cursor {
            pos: IVec2::ZERO,
            move_speed: 1.0,
            //weight: 1.0,
            cursor_entity,
            cursor_mesh: Default::default(),
            cursor_mat: Default::default(),
            spawn_root_entity,
        }
    }

    pub fn set_cursor(&mut self, mesh: Handle<Mesh>, mat: Handle<StandardMaterial>) {
        self.cursor_mesh = mesh;
        self.cursor_mat = mat;
    }

    // pub fn set_alpha(&mut self, alpha: f32) {
    //      self.cursor_mat
    // }
}

#[derive(Debug)]
pub struct Grid {
    size: IVec2,
    content: Vec<f32>,
    /// Origin offset. Odd sizes have the middle cell of the grid at the world origin, while even sizes
    /// are offset by 0.5 units such that the center of the grid (between cells) is at the world origin.
    foffset: Vec2,
    grid_blocks: Vec<Entity>,
    entities: Vec<Entity>,
    material: Handle<StandardMaterial>,
}

impl Grid {
    pub fn new() -> Grid {
        let mut grid = Grid {
            size: IVec2::ZERO,
            content: vec![],
            foffset: Vec2::ZERO,
            grid_blocks: vec![],
            entities: vec![],
            material: Default::default(),
        };
        grid.set_size(&IVec2::new(8, 8));
        grid
    }

    pub fn set_material(&mut self, material: Handle<StandardMaterial>) {
        self.material = material;
    }

    pub fn set_size(&mut self, size: &IVec2) {
        trace!("Grid::set_size({}, {})", size.x, size.y);
        self.size = *size;
        self.foffset = Vec2::new((1 - self.size.x % 2) as f32, (1 - self.size.y % 2) as f32) * 0.5;
        self.clear(None);
    }

    pub fn regenerate(&mut self, commands: &mut Commands, mesh: Handle<Mesh>, parent: Entity) {
        trace!("Grid::regenerate() size={}", self.size);

        // Destroy previous grid
        for ent in self.grid_blocks.iter() {
            commands.entity(*ent).despawn_recursive();
        }
        self.grid_blocks.clear();

        // Regenerate
        let min = self.min_pos();
        let max = self.max_pos();
        for j in min.y..max.y + 1 {
            for i in min.x..max.x + 1 {
                let fpos = self.fpos(&IVec2::new(i, j));
                self.grid_blocks.push(
                    commands
                        .spawn_bundle(PbrBundle {
                            mesh: mesh.clone(),
                            material: self.material.clone(),
                            transform: Transform::from_translation(Vec3::new(fpos.x, 0.0, -fpos.y)),
                            ..Default::default()
                        })
                        .insert(Name::new(format!("Tile({},{})", i, j)))
                        .insert(Parent(parent))
                        .id(),
                );
            }
        }
    }

    pub fn min_pos(&self) -> IVec2 {
        let x_min = -self.size.x / 2;
        let y_min = -self.size.y / 2;
        IVec2::new(x_min, y_min)
    }

    pub fn max_pos(&self) -> IVec2 {
        let x_max = (self.size.x - 1) / 2;
        let y_max = (self.size.y - 1) / 2;
        IVec2::new(x_max, y_max)
    }

    pub fn clamp(&self, pos: IVec2) -> IVec2 {
        let min = self.min_pos();
        let max = self.max_pos();
        IVec2::new(pos.x.clamp(min.x, max.x), pos.y.clamp(min.y, max.y))
    }

    pub fn hit_test(&self, pos: &Vec2) -> Option<IVec2> {
        let min = self.min_pos();
        let max = self.max_pos();
        if pos.x <= min.x as f32
            || pos.x >= max.x as f32
            || pos.y <= min.y as f32
            || pos.y >= max.y as f32
        {
            None
        } else {
            let x = pos.x as i32;
            let y = pos.y as i32;
            Some(IVec2::new(x, y))
        }
    }

    pub fn index(&self, pos: &IVec2) -> usize {
        let min = self.min_pos();
        let i0 = (pos.x - min.x) as usize;
        let j0 = (pos.y - min.y) as usize;
        i0 + j0 * self.size.x as usize
    }

    /// Position of the center of the cell from its grid coordinates.
    pub fn fpos(&self, pos: &IVec2) -> Vec2 {
        Vec2::new(pos.x as f32 + self.foffset.x, pos.y as f32 + self.foffset.y)
    }

    pub fn can_spawn_item(&mut self, pos: &IVec2) -> bool {
        let index = self.index(pos);
        self.content[index] < 0.1
    }

    pub fn spawn_item(&mut self, pos: &IVec2, weight: f32, entity: Entity) {
        let index = self.index(pos);
        self.content[index] += weight;
        self.entities.push(entity);
    }

    pub fn calc_cog_offset(&self, balance_factor: f32) -> Vec2 {
        let min = self.min_pos();
        let max = self.max_pos();
        let mut w00 = Vec2::ZERO;
        //println!("calc_rot: min={:?} max={:?}", min, max);
        for j in min.y..max.y + 1 {
            for i in min.x..max.x + 1 {
                let ij = IVec2::new(i, j);
                let index = self.index(&ij);
                let fpos = self.fpos(&ij);
                // println!(
                //     "calc_rot: index={:?} ij={},{} fpos={:?} w={}",
                //     index, i, j, fpos, self.content[index]
                // );
                w00 += self.content[index] * fpos;
            }
        }
        //println!("calc_rot: w00={:?}", w00);
        w00
    }

    pub fn calc_rot(&self, balance_factor: f32) -> Quat {
        let w00 = self.calc_cog_offset(balance_factor);
        let rot_x = FRAC_PI_6 * w00.x * balance_factor;
        let rot_y = FRAC_PI_6 * w00.y * balance_factor;
        //println!("calc_rot: w00={:?} rx={} ry={}", w00, rot_x, rot_y);
        Quat::from_rotation_x(-rot_y) * Quat::from_rotation_z(-rot_x)
    }

    pub fn clear(&mut self, commands: Option<&mut Commands>) {
        trace!(
            "Grid::clear({})",
            if commands.is_some() { "commands" } else { "-" }
        );
        self.content.clear();
        self.content
            .resize(self.size.x as usize * self.size.y as usize, 0.0);
        if let Some(commands) = commands {
            self.entities.iter().for_each(|ent| {
                commands.entity(*ent).despawn_recursive();
            });
        }
    }

    pub fn is_victory(&self, balance_factor: f32, victory_margin: f32) -> bool {
        let w00 = self.calc_cog_offset(balance_factor);
        debug!("victory: w00={:?} len={}", w00, w00.length());
        w00.length() < victory_margin
    }
}

#[cfg(debug_assertions)]
fn inspector_toggle(
    keyboard_input: ResMut<Input<KeyCode>>,
    mut inspector: ResMut<WorldInspectorParams>,
) {
    if keyboard_input.just_pressed(KeyCode::F1) {
        inspector.enabled = !inspector.enabled;
    }
}

static DEBUG: &str = "debug";

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut diag = LogDiagnosticsPlugin::default();
    diag.debug = true;

    let mut app = App::build();
    app
        // Logging and diagnostics
        .insert_resource(bevy::log::LogSettings {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,bevy_render=info,libracity=trace".to_string(),
        })
        .add_plugin(diag)
        //.add_plugin(FrameTimeDiagnosticsPlugin::default())
        // Asset server configuration
        .insert_resource(AssetServerSettings {
            asset_folder: "assets".to_string(),
        })
        // Main window
        //.insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .insert_resource(WindowDescriptor {
            title: "Libra City".to_string(),
            vsync: true,
            ..Default::default()
        });

    // Only enable MSAA on non-web platforms
    #[cfg(not(target_arch = "wasm32"))]
    app.insert_resource(Msaa { samples: 4 });

    app
        // Helper to exit with ESC key
        .add_system(bevy::input::system::exit_on_esc_system.system())
        // Default plugins
        .add_plugins(DefaultPlugins);

    // Browsers don't support wgpu, use the WebGL2 rendering backend instead.
    #[cfg(target_arch = "wasm32")]
    app.add_plugin(bevy_webgl2::WebGL2Plugin);

    // // Shaders shipped with bevy_prototype_debug_lines are not compatible with WebGL due to version
    // // https://github.com/mrk-its/bevy_webgl2/issues/21
    // #[cfg(not(target_arch = "wasm32"))]
    // app.add_plugin(DebugLinesPlugin)
    //     .insert_resource(DebugLines {
    //         depth_test: true,
    //         ..Default::default()
    //     });

    // In Debug build only, add egui inspector to help
    #[cfg(debug_assertions)]
    app.add_plugin(WorldInspectorPlugin::new())
        .add_system(inspector_toggle.system());

    app
        // Audio (Kira)
        .add_plugin(AudioPlugin)
        .add_startup_system(start_background_audio.system())
        // Events
        .add_event::<CheckLevelResultEvent>()
        .add_event::<ResetPlateEvent>()
        // Resources
        .insert_resource(Grid::new())
        .insert_resource(EntityManager::new())
        // Asset loading
        .add_plugin(TextAssetPlugin)
        .add_plugin(SerializePlugin)
        .add_plugin(LoaderPlugin)
        // Level management
        .add_plugin(LevelPlugin)
        // Inventory management
        .add_plugin(InventoryPlugin)
        // == Boot state ==
        .add_plugin(BootPlugin)
        // == MainMenu state ==
        .add_plugin(MainMenuPlugin)
        // == InGame state ==
        .add_system_set(
            SystemSet::on_enter(AppState::InGame).with_system(setup3d.system().label("setup3d")),
        )
        // FIXME - Broken in 0.5 apparently
        // .add_system_set_to_stage(
        //     CoreStage::PreUpdate,
        //     SystemSet::on_update(AppState::InGame).with_system(inputs_system.system()),
        // )
        .add_system_set(SystemSet::on_update(AppState::InGame).with_system(inputs_system.system()))
        .add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_system(
                    plate_movement_system
                        .system()
                        .label("plate_movement_system"),
                )
                .with_system(plate_reset_system.system())
                // .with_system(
                //     draw_debug_axes_system
                //         .system()
                //         .label("draw_debug_axes_system"),
                // )
                .with_system(
                    cursor_movement_system
                        .system()
                        .label("cursor_movement_system"),
                )
                .with_system(plate_balance_system.system().label("plate_balance_system"))
                .with_system(
                    check_victory_condition
                        .system()
                        .label("check_victory_condition"),
                ),
        )
        //.add_stage_after(CoreStage::Update, DEBUG, SystemStage::single_threaded())
        .add_system_set(
            SystemSet::on_exit(AppState::InGame).with_system(
                cleanup3d
                    .system()
                    .after("setup3d")
                    .after("plate_movement_system")
                    //.after("draw_debug_axes_system")
                    .after("cursor_movement_system")
                    .after("plate_balance_system")
                    .after("check_victory_condition"),
            ),
        ) // https://github.com/bevyengine/bevy/issues/1743#issuecomment-806335175
        // == TheEnd state ==
        .add_system_set(
            SystemSet::on_enter(AppState::TheEnd).with_system(spawn_end_screen.system()),
        )
        // Initial state
        .add_state(AppState::Boot)
        //.add_state(AppState::MainMenu)
        //.add_state(AppState::InGame)
        //.add_state(AppState::TheEnd)
        .run();
}

fn check_victory_condition(
    grid: Res<Grid>,
    level: Res<Level>,
    levels: Res<Levels>,
    mut ev_check_level: EventReader<CheckLevelResultEvent>,
    mut ev_load_level: EventWriter<LoadLevelEvent>,
) {
    if let Some(ev) = ev_check_level.iter().last() {
        let level_index = level.index();
        let level_desc = &levels.levels()[level_index];
        if grid.is_victory(level_desc.balance_factor, level_desc.victory_margin) {
            info!("VICTORY!");
            // Try to transition to next level. If there's none, this will transition
            // automatically to next stage ([`TheEnd`]).
            // TODO - Instead of deciding here that "end of current level == load next",
            //        send an event to some game/level manager that will decide what to do,
            //        which avoids having to expose AppState::TheEnd to the Level module.
            //        This also allows playing some "level cleared" transition while loading.
            ev_load_level.send(LoadLevelEvent(LoadLevel::Next));
        }
    }
}

fn start_background_audio(asset_server: Res<AssetServer>, audio: Res<Audio>) {
    audio.play_looped(asset_server.load("audio/ambient1.ogg"));
}

fn inputs_system(
    keyboard_input: ResMut<Input<KeyCode>>,
    mut ev_select_slot: EventWriter<SelectSlotEvent>,
) {
    // Change selected slot
    if keyboard_input.just_pressed(KeyCode::Q) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Prev));
    }
    if keyboard_input.just_pressed(KeyCode::E) || keyboard_input.just_pressed(KeyCode::Tab) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Next));
    }
    if keyboard_input.just_pressed(KeyCode::Key1) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Index(0)));
    }
    if keyboard_input.just_pressed(KeyCode::Key2) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Index(1)));
    }
    if keyboard_input.just_pressed(KeyCode::Key3) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Index(2)));
    }
    if keyboard_input.just_pressed(KeyCode::Key4) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Index(3)));
    }
    if keyboard_input.just_pressed(KeyCode::Key5) {
        ev_select_slot.send(SelectSlotEvent(SelectSlot::Index(4)));
    }
}

fn create_line_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList);
    mesh.set_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1])));
    mesh
}

fn create_axes_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList);
    mesh.set_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
    );
    mesh.set_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1, 2, 3, 4, 5])));
    mesh
}

// #[cfg(debug_assertions)]
// #[cfg(not(target_arch = "wasm32"))]
// fn draw_debug_axes_system(mut query: Query<(&Plate, &Transform)>, mut lines: ResMut<DebugLines>) {
//     // if let Ok((cursor, transform)) = query.single_mut() {
//     //     //lines.line_colored(Vec3::ZERO, *transform * Vec3::X, 0.0, Color::RED);
//     //     //lines.line_colored(Vec3::ZERO, *transform * Vec3::Y, 0.0, Color::GREEN);
//     //     //lines.line_colored(Vec3::ZERO, *transform * Vec3::Z, 0.0, Color::BLUE);
//     //     lines.line_colored(Vec3::ZERO, *transform * Vec3::Y, 0.0, Color::BLACK);
//     // }
// }

#[cfg(target_arch = "wasm32")]
fn draw_debug_axes_system() {}

fn plate_movement_system(
    time: Res<Time>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&Plate, &mut Transform)>,
) {
    if let Ok((plate, mut transform)) = query.single_mut() {
        let mut rot = 0.0;
        if keyboard_input.pressed(KeyCode::Q) {
            rot -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::E) {
            rot += 1.0;
        }
        rot *= plate.rotate_speed * time.delta_seconds();
        let delta_rot = Quat::from_rotation_ypr(rot, 0.0, 0.0);
        let rotation = &mut transform.rotation;
        *rotation *= delta_rot;
    }
}

struct CheckLevelResultEvent();

fn cursor_movement_system(
    mut ev_check_level: EventWriter<CheckLevelResultEvent>,
    mut ev_update_slots: EventWriter<UpdateInventorySlots>,
    //time: Res<Time>,
    mut grid: ResMut<Grid>,
    mut commands: Commands,
    level: Res<Level>,
    levels: Res<Levels>,
    keyboard_input: Res<Input<KeyCode>>,
    buildables: Res<Buildables>,
    mut inventory: ResMut<Inventory>,
    mut query: Query<(&mut Cursor, &mut Transform, &mut Visible)>,
) {
    if let Ok((mut cursor, mut transform, mut visible)) = query.single_mut() {
        // Move cursor around the grid
        let mut pos = cursor.pos;
        if keyboard_input.just_pressed(KeyCode::Left) || keyboard_input.just_pressed(KeyCode::A) {
            pos.x -= 1;
        }
        if keyboard_input.just_pressed(KeyCode::Right) || keyboard_input.just_pressed(KeyCode::D) {
            pos.x += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Up) || keyboard_input.just_pressed(KeyCode::W) {
            pos.y += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Down) || keyboard_input.just_pressed(KeyCode::S) {
            pos.y -= 1;
        }
        pos = grid.clamp(pos);
        if cursor.pos != pos {
            cursor.pos = pos;
            //let delta_pos = cursor.move_speed * time.delta_seconds();
            let fpos = grid.fpos(&cursor.pos);
            let translation = &mut transform.translation;
            *translation = Vec3::new(fpos.x, 0.1, -fpos.y);
        }

        // Spawn buildable at cursor position
        if keyboard_input.just_pressed(KeyCode::Space) {
            if grid.can_spawn_item(&cursor.pos) {
                if let Some(slot) = inventory.selected_slot_mut() {
                    if let Some(buildable_ref) = slot.pop_item() {
                        if let Some(buildable) = buildables.get(&buildable_ref) {
                            let fpos = grid.fpos(&cursor.pos);
                            debug!("Spawn buildable at pos={:?} fpos={:?}", cursor.pos, fpos);
                            let entity = commands
                                .spawn_bundle(PbrBundle {
                                    mesh: buildable.mesh().clone(),
                                    material: buildable.material().clone(),
                                    transform: Transform::from_translation(Vec3::new(
                                        fpos.x, 0.1, -fpos.y,
                                    )),
                                    ..Default::default()
                                })
                                .insert(Parent(cursor.spawn_root_entity))
                                .id();
                            grid.spawn_item(&cursor.pos, buildable.weight(), entity);
                            // Check if current slot has any item available left
                            if slot.is_empty() {
                                // Try to select another slot with some item(s) left
                                if let Some(slot_index) = inventory.find_non_empty_slot_index() {
                                    inventory.select_slot(&SelectSlot::Index(slot_index as usize));
                                    let bref = inventory.selected_slot().unwrap().bref();
                                    let buildable = buildables.get(bref).unwrap();
                                    ev_update_slots.send(UpdateInventorySlots);
                                } else {
                                    // No more of any item in any slot; hide cursor and check level result
                                    visible.is_visible = false;
                                    ev_update_slots.send(UpdateInventorySlots);
                                    ev_check_level.send(CheckLevelResultEvent {});
                                }
                            } else {
                                // If current slot still has items, update anyway
                                ev_update_slots.send(UpdateInventorySlots);
                            }
                        }
                    }
                }
            }
        }

        // Restart level
        if keyboard_input.just_pressed(KeyCode::R) {
            // Clear grid
            grid.clear(Some(&mut commands));
            // Reset inventory
            let level_index = level.index();
            let level_desc = &levels.levels()[level_index];
            inventory.set_slots(
                level_desc
                    .inventory
                    .iter()
                    .map(|(bref, &count)| Slot::new(bref.clone(), count)),
            );
            // Re-show cursor
            visible.is_visible = true;
            // Update inventory slots
            ev_update_slots.send(UpdateInventorySlots);
        }
    }
}

fn plate_balance_system(
    grid: Res<Grid>,
    level: Res<Level>,
    levels: Res<Levels>,
    mut query: Query<(&Plate, &mut Transform)>,
) {
    if let Ok((plate, mut transform)) = query.single_mut() {
        let level_index = level.index();
        let level = &levels.levels()[level_index];
        let rot = grid.calc_rot(level.balance_factor);
        transform.rotation = rot;
    }
}

fn create_grid_tex() -> Texture {
    const TEX_SIZE: u32 = 32;
    let mut data = Vec::<u8>::with_capacity(TEX_SIZE as usize * TEX_SIZE as usize * 4);
    for j in 0..TEX_SIZE {
        for i in 0..TEX_SIZE {
            if i == 0 || i == TEX_SIZE - 1 || j == 0 || j == TEX_SIZE - 1 {
                data.push(192);
                data.push(192);
                data.push(192);
                data.push(255);
            } else {
                data.push(128);
                data.push(128);
                data.push(128);
                data.push(255);
            }
        }
    }
    Texture::new(
        Extent3d::new(TEX_SIZE, TEX_SIZE, 1),
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
    )
}

/// set up a simple 3D scene
fn setup3d(
    mut entity_manager: ResMut<EntityManager>,
    asset_server: Res<AssetServer>,
    level: Res<Level>,
    levels: Res<Levels>,
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut textures: ResMut<Assets<Texture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ev_load_level: EventWriter<LoadLevelEvent>,
) {
    let level_index = level.index();
    let level = &levels.levels()[level_index];

    // Setup grid
    grid.set_size(&level.grid_size);

    // Create grid material
    let grid_texture = textures.add(create_grid_tex());
    let grid_material = materials.add(StandardMaterial {
        base_color_texture: Some(grid_texture),
        unlit: true,
        ..Default::default()
    });
    grid.set_material(grid_material.clone());

    // // Axes
    // commands.spawn_bundle(PbrBundle {
    //     mesh: meshes.add(create_axes_mesh()),
    //     material: materials.add(StandardMaterial {
    //         base_color: Color::rgba(1.0, 1.0, 1.0, 0.0),
    //         unlit: true,
    //         ..Default::default()
    //     }),
    //     transform: Transform::from_scale(Vec3::new(5.0, 5.0, 5.0)),
    //     ..Default::default()
    // });

    // // plane
    // commands.spawn_bundle(PbrBundle {
    //     mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
    //     material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
    //     ..Default::default()
    // });

    // Plate
    let mut plate_cmds = commands.spawn();
    let plate = plate_cmds.id();
    //entity_manager.all_entities.push(plate);
    plate_cmds
        .insert(Name::new("Plate"))
        .insert(Transform::identity())
        .insert(GlobalTransform::identity())
        .insert(Plate::new(plate));

    // Grid blocks
    let cell_mesh = meshes.add(Mesh::from(shape::Box::new(1.0, 0.1, 1.0)));
    grid.regenerate(&mut commands, cell_mesh.clone(), plate);

    // Cursor
    let cursor_mesh = meshes.add(Mesh::from(shape::Cube { size: 0.9 }));
    let cursor_mat = materials.add(Color::rgb(0.6, 0.7, 0.8).into());
    let cursor_fpos = grid.fpos(&IVec2::ZERO);
    debug!("Spawn cursor at fpos={:?}", cursor_fpos);
    let mut cursor_entity_cmds = commands.spawn_bundle(PbrBundle {
        mesh: cursor_mesh.clone(),
        material: cursor_mat.clone(),
        transform: Transform::from_translation(Vec3::new(cursor_fpos.x, 0.1, -cursor_fpos.y))
            * Transform::from_scale(Vec3::new(1.0, 0.3, 1.0)),
        ..Default::default()
    });
    cursor_entity_cmds
        .insert(Name::new("Cursor"))
        .insert(Parent(plate));
    let mut cursor = Cursor::new(cursor_entity_cmds.id(), plate);
    cursor.set_cursor(cursor_mesh, cursor_mat);
    cursor_entity_cmds.insert(cursor);

    // Light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    // Camera
    //entity_manager.all_entities.push(
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-3.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        // perspective_projection: PerspectiveProjection {
        //     fov: 60.0,
        //     aspect_ratio: 1.0,
        //     near: 0.01,
        //     far: 100.0,
        // },
        ..Default::default()
    });

    // UI camera
    commands.spawn_bundle(UiCameraBundle::default());

    // Title
    let title = commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: Rect {
                    bottom: Val::Px(5.0),
                    left: Val::Px(15.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            text: Text::with_section(
                level.name.clone(),
                TextStyle {
                    font: asset_server.load("fonts/pacifico/Pacifico-Regular.ttf"),
                    font_size: 100.0,
                    color: Color::rgb_u8(111, 188, 165),
                },
                TextAlignment {
                    horizontal: HorizontalAlign::Left,
                    ..Default::default()
                },
            ),
            ..Default::default()
        })
        .insert(Name::new("LevelName"))
        .insert(LevelNameText) // marker to allow finding this text to change it
        .id();
    entity_manager.all_entities.push(title);

    // Load first level by default (this allows skipping the main menu while developping)
    ev_load_level.send(LoadLevelEvent(LoadLevel::ByIndex(0)));
}

fn cleanup3d(
    //mut query: Query<(&mut Visible,)>,
    mut entity_manager: ResMut<EntityManager>,
    mut commands: Commands,
    // mut query: Query<(&mut Transform,)>,
    mut inventory: ResMut<Inventory>,
) {
    // LAZY HACK -- Hide literally EVERYTHING since we didn't keep track of things we need to hide/despawn
    // for (mut vis,) in query.iter_mut() {
    //     vis.is_visible = false;
    // }

    trace!("Entities: {}", entity_manager.all_entities.len());
    for ent in entity_manager.all_entities.iter() {
        trace!("Entity: {:?}", *ent);
        commands.entity(*ent).despawn_recursive();
    }
    entity_manager.all_entities.clear();

    inventory.clear_entities(&mut commands);
}

fn spawn_end_screen(
    asset_server: Res<AssetServer>,
    ui_resouces: Res<UiResources>,
    mut commands: Commands,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn_bundle(UiCameraBundle::default());

    commands
        .spawn_bundle(NodeBundle {
            // root
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            //material: materials2d.add(Color::NONE.into()),
            material: materials2d.add(Color::rgb(0.15, 0.15, 0.15).into()),
            ..Default::default()
        })
        .with_children(|parent| {
            parent
                .spawn_bundle(NodeBundle {
                    style: Style {
                        size: Size::new(Val::Px(800.0), Val::Px(350.0)),
                        position: Rect::all(Val::Px(0.0)),
                        position_type: PositionType::Relative,

                        // I expect one of these to center the text in the node
                        align_content: AlignContent::Center,
                        align_items: AlignItems::Center,
                        align_self: AlignSelf::Center,

                        // this line aligns the content
                        justify_content: JustifyContent::Center,

                        ..Default::default()
                    },
                    material: materials2d.add(Color::rgb(0.15, 0.15, 0.15).into()),
                    ..Default::default()
                })
                //.insert(Parent(root_entity))
                .with_children(|parent| {
                    // The End
                    parent.spawn_bundle(TextBundle {
                        text: Text::with_section(
                            "The End",
                            TextStyle {
                                font: ui_resouces.title_font(),
                                font_size: 250.0,
                                color: Color::rgb_u8(111, 188, 165),
                            },
                            TextAlignment {
                                horizontal: HorizontalAlign::Center,
                                vertical: VerticalAlign::Center,
                            },
                        ),
                        ..Default::default()
                    });
                });

            parent
                .spawn_bundle(NodeBundle {
                    style: Style {
                        size: Size::new(Val::Px(800.0), Val::Px(100.0)),
                        position: Rect {
                            bottom: Val::Px(50.0),
                            ..Default::default()
                        },
                        position_type: PositionType::Absolute,

                        // I expect one of these to center the text in the node
                        align_content: AlignContent::Center,
                        align_items: AlignItems::Center,
                        align_self: AlignSelf::Center,

                        // this line aligns the content
                        justify_content: JustifyContent::Center,

                        ..Default::default()
                    },
                    material: materials2d.add(Color::rgb(0.15, 0.15, 0.15).into()),
                    ..Default::default()
                })
                //.insert(Parent(root_entity))
                .with_children(|parent| {
                    // Press ESC
                    parent.spawn_bundle(TextBundle {
                        text: Text::with_section(
                            "Press [ESC] to quit",
                            TextStyle {
                                font: ui_resouces.text_font(),
                                font_size: 48.0,
                                color: Color::rgb_u8(192, 192, 192),
                            },
                            TextAlignment {
                                horizontal: HorizontalAlign::Center,
                                vertical: VerticalAlign::Center,
                            },
                        ),
                        ..Default::default()
                    });
                });
        });
}

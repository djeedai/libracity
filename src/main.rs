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
use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};
use std::f32::consts::*;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    MainMenu,
    InGame,
    TheEnd,
}

#[derive(Debug, Clone)]
struct Buildable {
    name: String,
    weight: f32,
    //stackable: bool,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    frame_material: Handle<ColorMaterial>,
    frame_material_selected: Handle<ColorMaterial>,
    frame_material_empty: Handle<ColorMaterial>,
}

impl Buildable {
    pub fn get_material(&self, count: u32, selected: bool) -> Handle<ColorMaterial> {
        if count == 0 {
            self.frame_material_empty.clone()
        } else {
            if selected {
                self.frame_material_selected.clone()
            } else {
                self.frame_material.clone()
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Inventory {
    items: Vec<(Buildable, u32)>, // TODO - ref to Buildable static data, not copy
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory { items: vec![] }
    }

    pub fn pop_item(&mut self, index: u32) -> Option<&Buildable> {
        let index = index as usize;
        if index < self.items.len() && self.items[index].1 > 0 {
            self.items[index].1 -= 1;
            println!(
                "Removed 1 item from slot #{}, left: {}",
                index, self.items[index].1
            );
            Some(&self.items[index].0)
        } else {
            None
        }
    }

    pub fn item_count(&self, index: u32) -> u32 {
        let index = index as usize;
        self.items[index].1
    }

    pub fn has_any_item(&self) -> bool {
        self.items.iter().fold(0u32, |acc, x| acc + x.1) > 0
    }

    pub fn find_non_empty_slot(&self) -> Option<u32> {
        for (index, item) in self.items.iter().enumerate() {
            if item.1 > 0 {
                return Some(index as u32);
            }
        }
        None
    }
}

struct Level {
    name: String,
    grid_size: IVec2,
    balance_factor: f32,
    victory_margin: f32,
    inventory: Inventory,
}

struct GameData {
    levels: Vec<Level>,
    current_level_index: u32,
    inventory: Inventory, // TODO - ref? or just number of items + ref into which items
    current_inventory_index: i32,
    frame_material: Handle<ColorMaterial>,
    inventory_ui_root_node: Option<Entity>,
    // HACK to delete everything on TheEnd screen
    all_entities: Vec<Entity>,
}

impl GameData {
    pub fn new() -> GameData {
        GameData {
            levels: vec![],
            current_level_index: 0,
            inventory: Inventory::new(),
            current_inventory_index: 0,
            frame_material: Default::default(),
            inventory_ui_root_node: None,
            all_entities: vec![],
        }
    }

    pub fn set_frame_material(&mut self, frame_material: Handle<ColorMaterial>) {
        self.frame_material = frame_material;
    }

    pub fn add_level(&mut self, level: Level) {
        self.levels.push(level);
    }

    pub fn level(&self) -> &Level {
        &self.levels[self.current_level_index as usize]
    }

    // pub fn level_mut(&mut self) -> &mut Level {
    //     let index = self.current_level_index as usize;
    //     &mut self.levels[index]
    // }

    pub fn set_level(&mut self, index: u32) {
        self.current_level_index = index;
        self.inventory = self.level().inventory.clone();
    }

    pub fn set_next_level(&mut self) -> Option<&Level> {
        if ((self.current_level_index + 1) as usize) < self.levels.len() {
            self.current_level_index += 1;
            self.inventory = self.level().inventory.clone();
            Some(self.level())
        } else {
            None
        }
    }

    pub fn selected_slot(&self) -> &Buildable {
        &self.inventory.items[self.current_inventory_index as usize].0
    }

    pub fn select_prev(&mut self) -> Option<&Buildable> {
        let len = self.inventory.items.len() as i32;
        let prev_index = ((self.current_inventory_index + len - 1) % len) as usize;
        if self.inventory.items[prev_index].1 > 0 {
            self.current_inventory_index = prev_index as i32;
            Some(&self.inventory.items[prev_index].0)
        } else {
            None
        }
    }

    pub fn select_next(&mut self) -> Option<&Buildable> {
        let len = self.inventory.items.len() as i32;
        let next_index = ((self.current_inventory_index + 1) % len) as usize;
        if self.inventory.items[next_index].1 > 0 {
            self.current_inventory_index = next_index as i32;
            Some(&self.inventory.items[next_index].0)
        } else {
            None
        }
    }
}

// fn exit_system(mut exit: EventWriter<AppExit>) {
//     exit.send(AppExit);
// }

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

struct Cursor {
    pos: IVec2,
    move_speed: f32,
    weight: f32,
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
            weight: 1.0,
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

struct Grid {
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
        println!("Grid::set_size({}, {})", size.x, size.y);
        self.size = *size;
        self.foffset = Vec2::new((1 - self.size.x % 2) as f32, (1 - self.size.y % 2) as f32) * 0.5;
        self.clear(None);
    }

    pub fn regenerate(&mut self, commands: &mut Commands, mesh: Handle<Mesh>, parent: Entity) {
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

    pub fn cell_size(&self) -> Vec2 {
        Vec2::new(1.0 / self.size.x as f32, 1.0 / self.size.y as f32)
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
            let cs = self.cell_size();
            let x = (pos.x / cs.x) as i32;
            let y = (pos.y / cs.y) as i32;
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
        let cs = self.cell_size();
        Vec2::new(
            (pos.x as f32 + self.foffset.x) * cs.x,
            (pos.y as f32 + self.foffset.y) * cs.y,
        )
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
        println!("CLEAR TABLE");
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
        println!("victory: w00={:?} len={}", w00, w00.length());
        w00.length() < victory_margin
    }
}

static DEBUG: &str = "debug";

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut diag = LogDiagnosticsPlugin::default();
    diag.debug = true;
    let mut app = App::build();
    app // Window
        .insert_resource(AssetServerSettings {
            asset_folder: "assets".to_string(),
        })
        .insert_resource(WindowDescriptor {
            title: "Libra City".to_string(),
            vsync: true,
            ..Default::default()
        })
        // .insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .insert_resource(Msaa { samples: 4 })
        .add_system(bevy::input::system::exit_on_esc_system.system())
        //.add_plugin(FrameTimeDiagnosticsPlugin::default())
        // Plugins
        .add_plugins(DefaultPlugins);

    #[cfg(target_arch = "wasm32")]
    app.add_plugin(bevy_webgl2::WebGL2Plugin);

    // Shaders shipped with bevy_prototype_debug_lines are not compatible with WebGL due to version
    // https://github.com/mrk-its/bevy_webgl2/issues/21
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugin(DebugLinesPlugin)
        .insert_resource(DebugLines {
            depth_test: true,
            ..Default::default()
        });

    app.add_plugin(diag)
        // Audio (Kira)
        .add_plugin(AudioPlugin)
        .add_startup_system(start_background_audio.system())
        // Resources
        .add_event::<CheckLevelResultEvent>()
        .add_event::<RegenerateInventoryUiEvent>()
        .add_event::<UpdateInventorySlots>()
        .insert_resource(Grid::new())
        .insert_resource(GameData::new())
        .add_startup_system(load_level_assets.system())
        // == MainMenu state ==
        .add_system_set(
            SystemSet::on_enter(AppState::MainMenu).with_system(setup_main_menu.system()),
        )
        .add_system_set(
            SystemSet::on_update(AppState::MainMenu).with_system(handle_ui_buttons.system()),
        )
        .add_system_set(
            SystemSet::on_exit(AppState::MainMenu).with_system(close_main_menu.system()),
        )
        // == InGame state ==
        .add_system_set(
            SystemSet::on_enter(AppState::InGame).with_system(setup3d.system().label("setup3d")),
        )
        .add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_system(
                    plate_movement_system
                        .system()
                        .label("plate_movement_system"),
                )
                .with_system(
                    draw_debug_axes_system
                        .system()
                        .label("draw_debug_axes_system"),
                )
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
                )
                .with_system(
                    regenerate_inventory_ui
                        .system()
                        .label("regenerate_inventory_ui"),
                )
                .with_system(inventory_ui_system.system()),
        )
        //.add_stage_after(CoreStage::Update, DEBUG, SystemStage::single_threaded())
        .add_system_set(
            SystemSet::on_exit(AppState::InGame).with_system(
                cleanup3d
                    .system()
                    .after("setup3d")
                    .after("regenerate_inventory_ui")
                    .after("plate_movement_system")
                    .after("draw_debug_axes_system")
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
        .add_state(AppState::MainMenu)
        //.add_state(AppState::InGame)
        //.add_state(AppState::TheEnd)
        .run();
}

fn check_victory_condition(
    mut commands: Commands,
    mut ev_check_level: EventReader<CheckLevelResultEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut grid: ResMut<Grid>,
    mut game_data: ResMut<GameData>,
    query1: Query<(&Plate,)>,
    mut query2: Query<(&Cursor, &mut Visible, &mut Transform)>,
    mut query3: Query<&mut Text, With<LevelNameText>>,
    mut ev_regen_ui: EventWriter<RegenerateInventoryUiEvent>,
    mut state: ResMut<State<AppState>>,
) {
    for ev in ev_check_level.iter() {
        let level = game_data.level();
        if grid.is_victory(level.balance_factor, level.victory_margin) {
            println!("VICTORY!");
            // Try to transition to the next level after this one
            if let Some(level) = game_data.set_next_level() {
                // Load new grid
                grid.clear(Some(&mut commands));
                grid.set_size(&level.grid_size);
                // Rebuild grid entity
                let cell_size = grid.cell_size();
                let cell_mesh =
                    meshes.add(Mesh::from(shape::Box::new(cell_size.x, 0.1, cell_size.y))); // THIS IS WHY WE SHOULDN'T SCALE THE GRID BUT ONLY EXTEND IT, CAN'T REUSE THIS WHEN CELL SIZE CHANGES
                if let Ok((plate,)) = query1.single() {
                    grid.regenerate(&mut commands, cell_mesh.clone(), plate.entity);
                }
                // Show cursor
                if let Ok((cursor, mut visible, mut transform)) = query2.single_mut() {
                    visible.is_visible = true;
                    let cursor_fpos = grid.fpos(&cursor.pos);
                    let cell_size = grid.cell_size();
                    *transform =
                        Transform::from_translation(Vec3::new(cursor_fpos.x, 0.1, -cursor_fpos.y))
                            * Transform::from_scale(Vec3::new(
                                cell_size.x * 3.0,
                                cell_size.x,
                                cell_size.x * 3.0,
                            )); // TODO - xy?
                }
                // Change title text
                if let Ok(mut text) = query3.single_mut() {
                    text.sections[0].value = level.name.clone();
                }
                // Reset inventory
                ev_regen_ui.send(RegenerateInventoryUiEvent {});
            } else {
                println!("=== THE END ===");
                state.set(AppState::TheEnd).unwrap();
            }
        }
    }
}

fn start_background_audio(asset_server: Res<AssetServer>, audio: Res<Audio>) {
    //audio.play_looped(asset_server.load("audio/ambient1.mp3"));
}

fn load_level_assets(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    meshes: Res<Assets<Mesh>>,
    gltfs: Res<Assets<Gltf>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
    mut ev_regen_ui: EventWriter<RegenerateInventoryUiEvent>,
) {
    println!("load_level_assets");

    // OK
    //commands.spawn_scene(asset_server.load("models/hut.gltf#Scene0"));

    // OK
    // let hh: Handle<Mesh> = asset_server.load("models/hut.gltf#Mesh0/Primitive0");
    // commands.insert_resource(hh.clone());
    // commands.spawn_bundle(PbrBundle{
    //     material: materials.add(StandardMaterial {
    //         base_color: Color::rgb(0.8, 0.7, 0.6),
    //         ..Default::default()
    //     }),
    //     mesh: hh.clone(),
    //     ..Default::default()
    // });

    // Load all models in the models/ folder in parallel
    // Not working with WASM? -- https://github.com/bevyengine/bevy/issues/2916
    //let _: Vec<HandleUntyped> = asset_server.load_folder("models").unwrap();

    let color_unselected = Color::rgba(1.0, 1.0, 1.0, 0.5);
    let color_selected = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let color_empty = Color::rgba(1.0, 0.8, 0.8, 0.5);

    // Hut
    //let hut_mesh: Handle<Mesh> = asset_server.get_handle("models/hut.gltf#Mesh0/Primitive0");
    let hut_mesh: Handle<Mesh> = asset_server.load("models/hut.gltf#Mesh0/Primitive0");
    commands.insert_resource(hut_mesh.clone());
    let hut_material = materials.add(StandardMaterial {
        // TODO - from file?
        base_color: Color::rgb(0.8, 0.7, 0.6),
        ..Default::default()
    });
    let hut_frame_texture: Handle<Texture> = asset_server.load("textures/frame_hut.png");
    let hut_frame_material = materials2d.add(ColorMaterial {
        color: color_unselected,
        texture: Some(hut_frame_texture.clone()),
    });
    let hut_frame_material_selected = materials2d.add(ColorMaterial {
        color: color_selected,
        texture: Some(hut_frame_texture.clone()),
    });
    let hut_frame_material_empty = materials2d.add(ColorMaterial {
        color: color_empty,
        texture: Some(hut_frame_texture),
    });

    // Chieftain Hut
    //let chieftain_hut_mesh: Handle<Mesh> = asset_server.get_handle("models/chieftain_hut.gltf#Mesh0/Primitive0");
    let chieftain_hut_mesh: Handle<Mesh> =
        asset_server.load("models/chieftain_hut.gltf#Mesh0/Primitive0");
    commands.insert_resource(chieftain_hut_mesh.clone());
    let chieftain_hut_material = materials.add(StandardMaterial {
        // TODO - from file?
        base_color: Color::rgb(0.6, 0.7, 0.8),
        ..Default::default()
    });
    let chieftain_hut_frame_texture: Handle<Texture> =
        asset_server.load("textures/frame_chieftain_hut.png");
    let chieftain_hut_frame_material = materials2d.add(ColorMaterial {
        color: color_unselected,
        texture: Some(chieftain_hut_frame_texture.clone()),
    });
    let chieftain_hut_frame_material_selected = materials2d.add(ColorMaterial {
        color: color_selected,
        texture: Some(chieftain_hut_frame_texture.clone()),
    });
    let chieftain_hut_frame_material_empty = materials2d.add(ColorMaterial {
        color: color_empty,
        texture: Some(chieftain_hut_frame_texture),
    });

    // Level 1
    game_data.add_level(Level {
        name: "Hut".to_string(),
        grid_size: IVec2::new(3, 3),
        balance_factor: 1.0,
        victory_margin: 0.001, // only 1 exact solution
        inventory: Inventory {
            items: vec![(
                Buildable {
                    name: "Hut".to_string(),
                    weight: 1.0,
                    mesh: hut_mesh.clone(),
                    material: hut_material.clone(),
                    frame_material: hut_frame_material.clone(),
                    frame_material_selected: hut_frame_material_selected.clone(),
                    frame_material_empty: hut_frame_material_empty.clone(),
                },
                1,
            )],
        },
    });

    // Level 2
    game_data.add_level(Level {
        name: "Neighborhood".to_string(),
        grid_size: IVec2::new(5, 5),
        balance_factor: 0.5,
        victory_margin: 0.1, // TODO
        inventory: Inventory {
            items: vec![(
                Buildable {
                    name: "Hut".to_string(),
                    weight: 1.0,
                    mesh: hut_mesh.clone(),
                    material: hut_material.clone(),
                    frame_material: hut_frame_material.clone(),
                    frame_material_selected: hut_frame_material_selected.clone(),
                    frame_material_empty: hut_frame_material_empty.clone(),
                },
                4,
            )],
        },
    });

    // Level 3
    game_data.add_level(Level {
        name: "Village".to_string(),
        grid_size: IVec2::new(5, 5),
        balance_factor: 0.5,
        victory_margin: 0.1, // TODO
        inventory: Inventory {
            items: vec![
                (
                    Buildable {
                        name: "Hut".to_string(),
                        weight: 1.0,
                        mesh: hut_mesh.clone(),
                        material: hut_material.clone(),
                        frame_material: hut_frame_material.clone(),
                        frame_material_selected: hut_frame_material_selected.clone(),
                        frame_material_empty: hut_frame_material_empty.clone(),
                    },
                    2,
                ),
                (
                    Buildable {
                        name: "Chieftain Hut".to_string(),
                        weight: 2.0,
                        mesh: chieftain_hut_mesh.clone(),
                        material: chieftain_hut_material.clone(),
                        frame_material: chieftain_hut_frame_material.clone(),
                        frame_material_selected: chieftain_hut_frame_material_selected.clone(),
                        frame_material_empty: chieftain_hut_frame_material_empty.clone(),
                    },
                    1,
                ),
            ],
        },
    });

    // Level 3
    game_data.add_level(Level {
        name: "Village 2".to_string(),
        grid_size: IVec2::new(5, 5),
        balance_factor: 0.5,
        victory_margin: 0.1, // TODO
        inventory: Inventory {
            items: vec![
                (
                    Buildable {
                        name: "Hut".to_string(),
                        weight: 1.0,
                        mesh: hut_mesh.clone(),
                        material: hut_material.clone(),
                        frame_material: hut_frame_material.clone(),
                        frame_material_selected: hut_frame_material_selected.clone(),
                        frame_material_empty: hut_frame_material_empty.clone(),
                    },
                    2,
                ),
                (
                    Buildable {
                        name: "Chieftain Hut".to_string(),
                        weight: 2.0,
                        mesh: chieftain_hut_mesh.clone(),
                        material: chieftain_hut_material.clone(),
                        frame_material: chieftain_hut_frame_material.clone(),
                        frame_material_selected: chieftain_hut_frame_material_selected.clone(),
                        frame_material_empty: chieftain_hut_frame_material_empty.clone(),
                    },
                    3,
                ),
            ],
        },
    });

    // Create frame material for UI
    let frame_texture = asset_server.load("textures/frame.png");
    let frame_material = materials2d.add(ColorMaterial {
        color: Color::rgb(1.0, 1.0, 1.0),
        texture: Some(frame_texture),
    });
    game_data.set_frame_material(frame_material);

    // Load first level by default (this allows skipping the main menu while developping)
    game_data.set_level(0);
}

struct RegenerateInventoryUiEvent;

struct InventorySlot {
    index: u32,
    count: u32,
    text: Entity,
}

impl InventorySlot {
    pub fn new(index: u32, count: u32, text: Entity) -> InventorySlot {
        InventorySlot { index, count, text }
    }
}

fn regenerate_inventory_ui(
    mut commands: Commands,
    mut ev_regen_ui: EventReader<RegenerateInventoryUiEvent>,
    mut game_data: ResMut<GameData>,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    for ev in ev_regen_ui.iter() {
        println!("regenerate_inventory_ui() -- GOT EVENT!");
        if let Some(root) = game_data.inventory_ui_root_node {
            commands.entity(root).despawn_recursive();
        }
        game_data.inventory_ui_root_node = Some(
            commands
                .spawn_bundle(NodeBundle {
                    style: Style {
                        size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                        justify_content: JustifyContent::FlexEnd,
                        ..Default::default()
                    },
                    material: materials2d.add(Color::NONE.into()),
                    ..Default::default()
                })
                .with_children(|parent| {
                    let mut xpos = 100.0 + 200.0 * (game_data.inventory.items.len() - 1) as f32;
                    let mut index = 0;
                    let font: Handle<Font> =
                        asset_server.load("fonts/montserrat/Montserrat-Regular.ttf"); // TODO -- save somewhere
                    for (buildable, count) in game_data.inventory.items.iter() {
                        // Item slot with frame and item image
                        let mut frame = parent.spawn_bundle(NodeBundle {
                            style: Style {
                                size: Size::new(Val::Px(128.0), Val::Px(128.0)),
                                position_type: PositionType::Absolute,
                                position: Rect {
                                    bottom: Val::Px(100.0),
                                    right: Val::Px(xpos),
                                    ..Default::default()
                                },

                                // I expect one of these to center the text in the node
                                align_content: AlignContent::Center,
                                align_items: AlignItems::Center,
                                align_self: AlignSelf::Center,

                                // this line aligns the content
                                justify_content: JustifyContent::Center,
                                ..Default::default()
                            },
                            material: buildable.get_material(*count, index == 0),
                            ..Default::default()
                        });
                        let text = frame
                            .with_children(|parent| {
                                // Item count in slot
                                parent.spawn_bundle(TextBundle {
                                    text: Text::with_section(
                                        format!("{}", *count).to_string(),
                                        TextStyle {
                                            font: font.clone(),
                                            font_size: 90.0,
                                            color: Color::rgb_u8(111, 188, 165),
                                        },
                                        Default::default(), // TextAlignment
                                    ),
                                    ..Default::default()
                                });
                            })
                            .id();
                        frame.insert(InventorySlot::new(index, *count, text));
                        xpos -= 200.0;
                        index += 1;
                    }
                })
                .id(),
        );
    }
}

struct UpdateInventorySlots;

fn inventory_ui_system(
    keyboard_input: ResMut<Input<KeyCode>>,
    mut game_data: ResMut<GameData>,
    mut query: Query<(&mut InventorySlot, &mut Handle<ColorMaterial>, &Children)>,
    mut text_query: Query<&mut Text>,
    mut ev: EventReader<UpdateInventorySlots>,
    mut cursor_query: Query<(&mut Cursor,)>,
) {
    // Change selected buildable from inventory
    let mut changed = false;
    if keyboard_input.just_pressed(KeyCode::Q) {
        if let Some(buildable) = game_data.select_prev() {
            changed = true;
            if let Ok((mut cursor,)) = cursor_query.single_mut() {
                cursor.weight = buildable.weight;
            }
        }
    }
    if keyboard_input.just_pressed(KeyCode::E) || keyboard_input.just_pressed(KeyCode::Tab) {
        if let Some(buildable) = game_data.select_next() {
            changed = true;
            if let Ok((mut cursor,)) = cursor_query.single_mut() {
                cursor.weight = buildable.weight;
            }
        }
    }

    // Update all inventory slots
    if changed || ev.iter().count() > 0 {
        let selected_index = game_data.current_inventory_index;
        println!("UpdateInventorySlots: sel={}", selected_index);
        for (mut slot, mut material, children) in query.iter_mut() {
            let mut text = text_query.get_mut(children[0]).unwrap();
            let (buildable, count) = &game_data.inventory.items[slot.index as usize];
            slot.count = *count;
            text.sections[0].value = format!("{}", slot.count).to_string();
            println!("-- slot: idx={} cnt={}", slot.index, slot.count);
            *material = buildable.get_material(slot.count, slot.index == selected_index as u32);
        }
    }
}

struct MenuData {
    //root_entity: Entity,
    entities: Vec<Entity>,
}

fn setup_main_menu(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    audio: Res<Audio>,
) {
    let font = asset_server.load("fonts/pacifico/Pacifico-Regular.ttf");
    let text_align = TextAlignment {
        horizontal: HorizontalAlign::Center,
        vertical: VerticalAlign::Center,
    };

    let mut menu_data = MenuData { entities: vec![] };

    // // Root
    // let root_entity = commands
    //     .spawn_bundle(NodeBundle {
    //         style: Style {
    //             min_size: Size::new(Val::Px(800.0), Val::Px(600.0)),
    //             position_type: PositionType::Absolute,
    //             position: Rect {
    //                 left: Val::Percent(10.0),
    //                 right: Val::Percent(10.0),
    //                 bottom: Val::Percent(10.0),
    //                 top: Val::Percent(10.0),
    //                 ..Default::default()
    //             },
    //             ..Default::default()
    //         },
    //         material: materials.add(Color::rgb(0.15, 0.5, 0.35).into()),
    //         ..Default::default()
    //     })
    //     .id();

    // UI camera
    menu_data.entities.push(
        commands
            .spawn_bundle(UiCameraBundle::default())
            //.insert(Parent(root_entity))
            .id(),
    );

    // Title
    // Using the NodeBundle from the hack of https://github.com/bevyengine/bevy/issues/676 as a background
    menu_data.entities.push(
        commands
            .spawn_bundle(NodeBundle {
                style: Style {
                    min_size: Size::new(Val::Px(800.0), Val::Px(300.0)),
                    position: Rect::all(Val::Px(0.0)),
                    position_type: PositionType::Absolute,

                    // I expect one of these to center the text in the node
                    align_content: AlignContent::Center,
                    align_items: AlignItems::Center,
                    align_self: AlignSelf::Center,

                    // this line aligns the content
                    justify_content: JustifyContent::Center,

                    ..Default::default()
                },
                material: materials.add(Color::rgb(0.15, 0.15, 0.15).into()),
                ..Default::default()
            })
            //.insert(Parent(root_entity))
            .with_children(|parent| {
                // Title itself
                parent.spawn_bundle(TextBundle {
                    text: Text::with_section(
                        "Libra City",
                        TextStyle {
                            font: font.clone(),
                            font_size: 250.0,
                            color: Color::rgb_u8(111, 188, 165),
                        },
                        text_align,
                    ),
                    ..Default::default()
                });
            })
            .id(),
    );
    menu_data.entities.push(
        commands
            .spawn_bundle(NodeBundle {
                style: Style {
                    min_size: Size::new(Val::Px(800.0), Val::Px(300.0)),
                    position: Rect {
                        bottom: Val::Px(100.0),
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        ..Default::default()
                    },
                    position_type: PositionType::Absolute,
                    align_content: AlignContent::Center,
                    align_items: AlignItems::Center,
                    align_self: AlignSelf::Center,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                },
                material: materials.add(Color::rgb(0.15, 0.15, 0.15).into()),
                ..Default::default()
            })
            //.insert(Parent(root_entity))
            .with_children(|parent| {
                // Title itself
                parent.spawn_bundle(TextBundle {
                    text: Text {
                        // Construct a `Vec` of `TextSection`s
                        sections: vec![
                            TextSection {
                                value: "Press RETURN to start".to_string(),
                                style: TextStyle {
                                    font: asset_server
                                        .load("fonts/montserrat/Montserrat-Regular.ttf"),
                                    font_size: 40.0,
                                    color: Color::WHITE,
                                },
                            },
                            TextSection {
                                value: "\nThis game plays with a keyboard only".to_string(),
                                style: TextStyle {
                                    font: asset_server
                                        .load("fonts/montserrat/Montserrat-Regular.ttf"),
                                    font_size: 20.0,
                                    color: Color::GRAY,
                                },
                            },
                        ],
                        alignment: TextAlignment {
                            vertical: VerticalAlign::Center,
                            horizontal: HorizontalAlign::Center,
                        },
                    },
                    ..Default::default()
                });
            })
            .id(),
    );

    commands.insert_resource(menu_data);

    // let music = asset_server.load("audio/ambient1.mp3");
    // audio.play(music);
}

fn handle_ui_buttons(
    mut keyboard_input: ResMut<Input<KeyCode>>,
    mut state: ResMut<State<AppState>>,
) {
    if keyboard_input.just_pressed(KeyCode::Return) {
        state.set(AppState::InGame).unwrap();
        // BUGBUG -- https://bevy-cheatbook.github.io/programming/states.html
        keyboard_input.reset(KeyCode::Return);
    }
}

fn close_main_menu(mut commands: Commands, menu_data: Res<MenuData>) {
    // BUGBUG - Didn't manage to root all UI entities to a single one to despawn a tree, always got errors or warnings,
    //          so ended up with a flat list of entities to despawn here.
    //commands.entity(menu_data.root_entity).despawn_recursive();
    menu_data.entities.iter().for_each(|ent| {
        commands.entity(*ent).despawn_recursive();
    });
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

#[cfg(not(target_arch = "wasm32"))]
fn draw_debug_axes_system(mut query: Query<(&Plate, &Transform)>, mut lines: ResMut<DebugLines>) {
    if let Ok((cursor, transform)) = query.single_mut() {
        //lines.line_colored(Vec3::ZERO, *transform * Vec3::X, 0.0, Color::RED);
        //lines.line_colored(Vec3::ZERO, *transform * Vec3::Y, 0.0, Color::GREEN);
        //lines.line_colored(Vec3::ZERO, *transform * Vec3::Z, 0.0, Color::BLUE);
        lines.line_colored(Vec3::ZERO, *transform * Vec3::Y, 0.0, Color::BLACK);
    }
}

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
    time: Res<Time>,
    mut game_data: ResMut<GameData>,
    mut grid: ResMut<Grid>,
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut Cursor, &mut Transform, &mut Visible)>,
) {
    if let Ok((mut cursor, mut transform, mut visible)) = query.single_mut() {
        // Move cursor around the grid
        let mut moved = false;
        if keyboard_input.just_pressed(KeyCode::Left) || keyboard_input.just_pressed(KeyCode::A) {
            cursor.pos.x -= 1;
            moved = true;
        }
        if keyboard_input.just_pressed(KeyCode::Right) || keyboard_input.just_pressed(KeyCode::D) {
            cursor.pos.x += 1;
            moved = true;
        }
        if keyboard_input.just_pressed(KeyCode::Up) || keyboard_input.just_pressed(KeyCode::W) {
            cursor.pos.y += 1;
            moved = true;
        }
        if keyboard_input.just_pressed(KeyCode::Down) || keyboard_input.just_pressed(KeyCode::S) {
            cursor.pos.y -= 1;
            moved = true;
        }
        if moved {
            cursor.pos = grid.clamp(cursor.pos);
            //let delta_pos = cursor.move_speed * time.delta_seconds();
            let fpos = grid.fpos(&cursor.pos);
            let translation = &mut transform.translation;
            *translation = Vec3::new(fpos.x, 0.1, -fpos.y);
        }

        // Spawn buildable at cursor position
        if keyboard_input.just_pressed(KeyCode::Space) {
            let slot_index = game_data.current_inventory_index as u32;
            let can_spawn = grid.can_spawn_item(&cursor.pos);
            if can_spawn {
                if let Some(buildable) = game_data.inventory.pop_item(slot_index) {
                    let fpos = grid.fpos(&cursor.pos);
                    println!("Spawn buildable at pos={:?} fpos={:?}", cursor.pos, fpos);
                    let cell_size = grid.cell_size();
                    let entity = commands
                        .spawn_bundle(PbrBundle {
                            mesh: buildable.mesh.clone(),
                            material: buildable.material.clone(),
                            transform: Transform::from_translation(Vec3::new(fpos.x, 0.1, -fpos.y))
                                * Transform::from_scale(Vec3::new(
                                    cell_size.x,
                                    cell_size.x,
                                    cell_size.x,
                                )), // TODO -- can we really handle non-uniform cell size?!
                            ..Default::default()
                        })
                        .insert(Parent(cursor.spawn_root_entity))
                        .id();
                    grid.spawn_item(&cursor.pos, cursor.weight, entity);
                    // Check if current slot has any item available left
                    if game_data.inventory.item_count(slot_index) == 0 {
                        // Try to select another slot with some item(s) left
                        if let Some(index) = game_data.inventory.find_non_empty_slot() {
                            game_data.current_inventory_index = index as i32;
                            cursor.weight = game_data.selected_slot().weight;
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

        // Restart level
        if keyboard_input.just_pressed(KeyCode::R) {
            // Clear grid
            grid.clear(Some(&mut commands));
            // Reset inventory
            game_data.inventory = game_data.level().inventory.clone();
            // Re-show cursor
            visible.is_visible = true;
            // Update inventory slots
            ev_update_slots.send(UpdateInventorySlots);
        }
    }
}

fn plate_balance_system(
    grid: Res<Grid>,
    game_data: Res<GameData>,
    mut query: Query<(&Plate, &mut Transform)>,
) {
    if let Ok((plate, mut transform)) = query.single_mut() {
        let rot = grid.calc_rot(game_data.level().balance_factor);
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

struct LevelNameText; // marker

/// set up a simple 3D scene
fn setup3d(
    mut game_data: ResMut<GameData>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut textures: ResMut<Assets<Texture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ev_regen_ui: EventWriter<RegenerateInventoryUiEvent>,
) {
    // Setup grid
    grid.set_size(&game_data.level().grid_size);

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
    //game_data.all_entities.push(plate);
    plate_cmds
        .insert(Transform::identity())
        .insert(GlobalTransform::identity())
        .insert(Plate::new(plate));

    // Grid blocks
    let cell_size = grid.cell_size();
    let cell_mesh = meshes.add(Mesh::from(shape::Box::new(cell_size.x, 0.1, cell_size.y))); // THIS IS WHY WE SHOULDN'T SCALE THE GRID BUT ONLY EXTEND IT, CAN'T REUSE THIS WHEN CELL SIZE CHANGES
    grid.regenerate(&mut commands, cell_mesh.clone(), plate);

    // Cursor
    let cursor_mesh = meshes.add(Mesh::from(shape::Cube {
        size: cell_size.x * 0.9,
    })); // TODO xy
    let cursor_mat = materials.add(Color::rgb(0.6, 0.7, 0.8).into());
    let cursor_fpos = grid.fpos(&IVec2::ZERO);
    println!("Spawn cursor at fpos={:?}", cursor_fpos);
    let mut cursor_entity_cmds = commands.spawn_bundle(PbrBundle {
        mesh: cursor_mesh.clone(),
        material: cursor_mat.clone(),
        transform: Transform::from_translation(Vec3::new(cursor_fpos.x, 0.1, -cursor_fpos.y))
            * Transform::from_scale(Vec3::new(cell_size.x * 3.0, cell_size.x, cell_size.x * 3.0)), // TODO - xy?
        ..Default::default()
    });
    cursor_entity_cmds.insert(Parent(plate));
    let mut cursor = Cursor::new(cursor_entity_cmds.id(), plate);
    cursor.set_cursor(cursor_mesh, cursor_mat);
    cursor_entity_cmds.insert(cursor);

    // Light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    // Camera
    //game_data.all_entities.push(
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-0.7, 1.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
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
                game_data.level().name.clone(),
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
        .insert(LevelNameText) // marker to allow finding this text to change it
        .id();
    game_data.all_entities.push(title);

    // Regenerate the inventory UI before starting to play
    ev_regen_ui.send(RegenerateInventoryUiEvent {});
}

fn cleanup3d(
    mut query: Query<(&mut Visible,)>,
    mut game_data: ResMut<GameData>,
    mut commands: Commands,
    // mut query: Query<(&mut Transform,)>,
) {
    // LAZY HACK -- Hide literally EVERYTHING since we didn't keep track of things we need to hide/despawn
    // for (mut vis,) in query.iter_mut() {
    //     vis.is_visible = false;
    // }

    println!("Entities: {}", game_data.all_entities.len());
    if let Some(ent) = game_data.inventory_ui_root_node {
        game_data.all_entities.push(ent);
    }
    for ent in game_data.all_entities.iter() {
        println!("Entity: {:?}", *ent);
        commands.entity(*ent).despawn_recursive();
    }
    game_data.all_entities.clear();
}

fn spawn_end_screen(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
) {
    let font: Handle<Font> = asset_server.load("fonts/pacifico/Pacifico-Regular.ttf"); // TODO -- save somewhere

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
                                font: font.clone(),
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
                            "Press ESCAPE to quit",
                            TextStyle {
                                font: asset_server.load("fonts/montserrat/Montserrat-Regular.ttf"), // TODO -- save somewhere
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

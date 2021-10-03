#![allow(dead_code, unused_imports, unused_variables)]

use bevy::{
    app::AppExit,
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
}

#[derive(Debug, Clone)]
struct Buildable {
    name: String,
    weight: f32,
    //stackable: bool,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    frame_material: Handle<ColorMaterial>,
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
            Some(&self.items[index].0)
        } else {
            None
        }
    }

    pub fn item_count(&self, index: u32) -> u32 {
        let index = index as usize;
        self.items[index].1
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

    pub fn select_prev(&mut self) -> &Buildable {
        let len = self.inventory.items.len() as i32;
        self.current_inventory_index = (self.current_inventory_index + len - 1) % len;
        &self.inventory.items[self.current_inventory_index as usize].0
    }

    pub fn select_next(&mut self) -> &Buildable {
        let len = self.inventory.items.len() as i32;
        self.current_inventory_index = (self.current_inventory_index + 1) % len;
        &self.inventory.items[self.current_inventory_index as usize].0
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
                //println!("calc_rot: index={:?}", index);
                let fpos = self.fpos(&ij);
                w00 += self.content[index] * fpos;
            }
        }
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

fn main() {
    let mut diag = LogDiagnosticsPlugin::default();
    diag.debug = true;
    App::build()
        // Window
        .insert_resource(WindowDescriptor {
            title: "Libra City".to_string(),
            vsync: true,
            ..Default::default()
        })
        // .insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .insert_resource(Msaa { samples: 4 })
        .add_system(bevy::input::system::exit_on_esc_system.system())
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugLinesPlugin)
        .insert_resource(DebugLines {
            depth_test: true,
            ..Default::default()
        })
        .add_plugin(diag)
        //.add_plugin(FrameTimeDiagnosticsPlugin::default())
        // Audio (Kira)
        .add_plugin(AudioPlugin)
        .add_startup_system(start_background_audio.system())
        // Resources
        .add_event::<CheckLevelResultEvent>()
        .add_event::<RegenerateInventoryUiEvent>()
        .insert_resource(Grid::new())
        .insert_resource(GameData::new())
        .add_startup_system(load_level_assets.system().label("load_level_assets"))
        .add_startup_system(load_models.system().after("load_level_assets"))
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
        .add_system_set(SystemSet::on_enter(AppState::InGame).with_system(setup3d.system()))
        .add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_system(plate_movement_system.system())
                .with_system(draw_debug_axes_system.system())
                .with_system(cursor_movement_system.system())
                .with_system(plate_balance_system.system())
                .with_system(check_victory_condition.system())
                .with_system(regenerate_inventory_ui.system()),
        )
        // Initial state
        .add_state(AppState::InGame)
        //.add_state(AppState::MainMenu)
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
                                cell_size.x,
                                cell_size.x,
                                cell_size.x,
                            )); // TODO - xy?
                }
                // Change title text
                if let Ok(mut text) = query3.single_mut() {
                    text.sections[0].value = level.name.clone();
                }
                // Reset inventory
                ev_regen_ui.send(RegenerateInventoryUiEvent {});
            }
        }
    }
}

fn start_background_audio(asset_server: Res<AssetServer>, audio: Res<Audio>) {
    audio.play_looped(asset_server.load("audio/ambient1.mp3"));
}

fn load_models(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    meshes: Res<Assets<Mesh>>,
    gltfs: Res<Assets<Gltf>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("load_models");
    //gltfs.iter().for_each(|(id, gltf)| println!("GLTF: {:?} {:?}", id, gltf));
    for (id, gltf) in gltfs.iter() {
        println!("GLTF: {:?} {:?}", id, gltf);
    }
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
    let _: Vec<HandleUntyped> = asset_server.load_folder("models").unwrap();

    // Hut
    let hut_mesh: Handle<Mesh> = asset_server.get_handle("models/hut.gltf#Mesh0/Primitive0");
    commands.insert_resource(hut_mesh.clone());
    let hut_material = materials.add(StandardMaterial {
        // TODO - from file?
        base_color: Color::rgb(0.8, 0.7, 0.6),
        ..Default::default()
    });
    let hut_frame_texture: Handle<Texture> = asset_server.load("textures/frame_hut.png");
    let hut_frame_material = materials2d.add(hut_frame_texture.into());

    // Chieftain Hut
    let chieftain_hut_mesh: Handle<Mesh> =
        asset_server.get_handle("models/chieftain_hut.gltf#Mesh0/Primitive0");
    commands.insert_resource(chieftain_hut_mesh.clone());
    let chieftain_hut_material = materials.add(StandardMaterial {
        // TODO - from file?
        base_color: Color::rgb(0.6, 0.7, 0.8),
        ..Default::default()
    });
    let chieftain_hut_frame_texture: Handle<Texture> = asset_server.load("textures/frame_chieftain_hut.png");
    let chieftain_hut_frame_material = materials2d.add(chieftain_hut_frame_texture.into());

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
                },
                1,
            )],
        },
    });

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
                },
                4,
            )],
        },
    });

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
                    },
                    2,
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
    println!("Sending RegenerateInventoryUiEvent from load_level_assets()...");
    ev_regen_ui.send(RegenerateInventoryUiEvent {});
}

struct RegenerateInventoryUiEvent;

fn regenerate_inventory_ui(
    mut commands: Commands,
    mut ev_regen_ui: EventReader<RegenerateInventoryUiEvent>,
    mut game_data: ResMut<GameData>,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
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
                    for (buildable, count) in game_data.inventory.items.iter() {
                        parent.spawn_bundle(NodeBundle {
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
                            material: buildable.frame_material.clone(),
                            ..Default::default()
                        });
                        xpos -= 200.0;
                    }
                })
                .id(),
        );
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
                    text: Text::with_section(
                        "Press Start",
                        TextStyle {
                            font: font.clone(),
                            font_size: 80.0,
                            color: Color::rgb_u8(192, 192, 192),
                        },
                        text_align,
                    ),
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

fn draw_debug_axes_system(mut query: Query<(&Plate, &Transform)>, mut lines: ResMut<DebugLines>) {
    if let Ok((cursor, transform)) = query.single_mut() {
        lines.line_colored(Vec3::ZERO, *transform * Vec3::X, 0.0, Color::RED);
        lines.line_colored(Vec3::ZERO, *transform * Vec3::Y, 0.0, Color::GREEN);
        lines.line_colored(Vec3::ZERO, *transform * Vec3::Z, 0.0, Color::BLUE);
    }
}

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

        // Change selected buildable from inventory
        if keyboard_input.just_pressed(KeyCode::Q) {
            let buildable = game_data.select_prev();
        }
        if keyboard_input.just_pressed(KeyCode::E) {
            let buildable = game_data.select_next();
        }

        // Spawn buildable at cursor position
        if keyboard_input.just_pressed(KeyCode::Space) {
            if let Some(buildable) = game_data.inventory.pop_item(0) {
                let fpos = grid.fpos(&cursor.pos);
                println!(
                    "Spawn buildable at pos={:?} fpos={:?}: {:?} -- {:?}",
                    cursor.pos, fpos, buildable.mesh, buildable.material
                );
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
                if game_data.inventory.item_count(0) == 0 {
                    // No more of this item, hide cursor
                    visible.is_visible = false;
                    ev_check_level.send(CheckLevelResultEvent {});
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
    game_data: Res<GameData>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut textures: ResMut<Assets<Texture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let level = game_data.level();

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
        transform: Transform::from_translation(Vec3::new(cursor_fpos.x, 0.1, -cursor_fpos.y)),
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
    commands
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
        .insert(LevelNameText); // marker to allow finding this text to change it
}

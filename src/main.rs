#![allow(dead_code, unused_imports, unused_variables)]

use bevy::{
    app::AppExit,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::{
        mesh::Indices,
        pass::ClearColor,
        pipeline::PrimitiveTopology,
        texture::{Extent3d, TextureDimension, TextureFormat},
    },
    sprite::collide_aabb::{collide, Collision},
};
use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};
use std::f32::consts::*;

fn exit_system(mut exit: EventWriter<AppExit>) {
    exit.send(AppExit);
}

struct Plate {
    rotate_speed: f32,
}

impl Plate {
    pub fn new() -> Plate {
        Plate { rotate_speed: 10.0 }
    }
}

struct Cursor {
    pos: IVec2,
    move_speed: f32,
    weight: f32,
    cursor_mesh: Handle<Mesh>,
    cursor_mat: Handle<StandardMaterial>,
    plate: Entity,
}

struct Grid {
    size: IVec2,
    content: Vec<f32>,
}

impl Grid {
    pub fn new() -> Grid {
        let size = IVec2::new(8, 8);
        let mut content = Vec::<f32>::new();
        content.resize(size.x as usize * size.y as usize, 0f32);
        Grid { size, content }
    }

    pub fn set_size(&mut self, x: i32, y: i32) {
        self.size = IVec2::new(x, y);
        self.clear();
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

    pub fn fpos(&self, pos: &IVec2) -> Vec2 {
        let cs = self.cell_size();
        Vec2::new((pos.x as f32 + 0.5) * cs.x, (pos.y as f32 + 0.5) * cs.y)
    }

    pub fn spawn(&mut self, pos: &IVec2, weight: f32) {
        let index = self.index(pos);
        self.content[index] += weight;
    }

    pub fn calc_rot(&self) -> Quat {
        let min = self.min_pos();
        let max = self.max_pos();
        let mut w00 = Vec2::ZERO;
        //println!("calc_rot: min={:?} max={:?}", min, max);
        for j in min.y..max.y + 1 {
            for i in min.x..max.x + 1 {
                let index = self.index(&IVec2::new(i, j));
                //println!("calc_rot: index={:?}", index);
                let pos = Vec2::new(i as f32 + 0.5, j as f32 + 0.5);
                w00 += self.content[index] * pos;
            }
        }
        let rot_x = FRAC_PI_6 * w00.x * 0.05;
        let rot_y = FRAC_PI_6 * w00.y * 0.05;
        //println!("calc_rot: w00={:?} rx={} ry={}", w00, rot_x, rot_y);
        Quat::from_rotation_x(rot_y) * Quat::from_rotation_z(-rot_x)
    }

    pub fn clear(&mut self) {
        println!("CLEAR TABLE");
        self.content.clear();
        self.content
            .resize(self.size.x as usize * self.size.y as usize, 0.0);
    }
}

fn main() {
    let mut diag = LogDiagnosticsPlugin::default();
    diag.debug = true;
    App::build()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugLinesPlugin)
        .insert_resource(DebugLines {
            depth_test: true,
            ..Default::default()
        })
        .add_system(bevy::input::system::exit_on_esc_system.system())
        .add_plugin(diag)
        //.add_plugin(FrameTimeDiagnosticsPlugin::default())
        // .insert_resource(Scoreboard { score: 0 })
        // .insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .insert_resource(Grid::new())
        .add_startup_system(setup3d.system())
        .add_system(plate_movement_system.system().label("plate_move"))
        .add_system(draw_debug_axes_system.system().after("plate_move"))
        .add_system(cursor_movement_system.system())
        .add_system(plate_balance_system.system())
        .run();
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

fn cursor_movement_system(
    time: Res<Time>,
    mut grid: ResMut<Grid>,
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut Cursor, &mut Transform)>,
) {
    if let Ok((mut cursor, mut transform)) = query.single_mut() {
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
            let translation = &mut transform.translation;
            *translation = Vec3::new(
                (cursor.pos.x as f32 + 0.5) * grid.cell_size().x,
                0.1,
                (cursor.pos.y as f32 + 0.5) * grid.cell_size().y,
            );
        }

        if keyboard_input.just_pressed(KeyCode::Space) {
            grid.spawn(&cursor.pos, cursor.weight);
            let fpos = grid.fpos(&cursor.pos);
            commands
                .spawn_bundle(PbrBundle {
                    mesh: cursor.cursor_mesh.clone(),
                    material: cursor.cursor_mat.clone(),
                    transform: Transform::from_translation(Vec3::new(fpos.x, 0.1, fpos.y)),
                    ..Default::default()
                })
                .insert(Parent(cursor.plate));
        }

        if keyboard_input.just_pressed(KeyCode::C) {
            grid.clear();
        }
    }
}

fn plate_balance_system(grid: Res<Grid>, mut query: Query<(&Plate, &mut Transform)>) {
    if let Ok((plate, mut transform)) = query.single_mut() {
        let rot = grid.calc_rot();
        transform.rotation = rot;
    }
}

/// set up a simple 3D scene
fn setup3d(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut textures: ResMut<Assets<Texture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Setup grid
    grid.set_size(8, 8);

    // Create grid texture
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
    let texture_handle = textures.add(Texture::new(
        Extent3d::new(TEX_SIZE, TEX_SIZE, 1),
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
    ));

    // Create grid material
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        unlit: true,
        ..Default::default()
    });

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
    let plate = commands
        .spawn()
        .insert(Transform::identity())
        .insert(GlobalTransform::identity())
        .insert(Plate::new())
        .id();

    // Grid blocks
    let min = grid.min_pos();
    let max = grid.max_pos();
    let cell_size = grid.cell_size();
    let cell_mesh = meshes.add(Mesh::from(shape::Box::new(cell_size.x, 0.1, cell_size.y)));
    for j in min.y..max.y + 1 {
        let y = (j as f32 + 0.5) * cell_size.x;
        for i in min.x..max.x + 1 {
            let x = (i as f32 + 0.5) * cell_size.y;
            commands
                .spawn_bundle(PbrBundle {
                    mesh: cell_mesh.clone(),
                    material: material_handle.clone(), //materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
                    transform: Transform::from_translation(Vec3::new(x, 0.0, y)),
                    ..Default::default()
                })
                .insert(Parent(plate));
        }
    }

    // Cursor
    let cursor_mesh = meshes.add(Mesh::from(shape::Cube { size: 0.1 }));
    let cursor_mat = materials.add(Color::rgb(0.6, 0.7, 0.8).into());
    let cursor = Cursor {
        pos: IVec2::ZERO,
        move_speed: 1.0,
        weight: 1.0,
        cursor_mesh: cursor_mesh.clone(),
        cursor_mat: cursor_mat.clone(),
        plate,
    };
    let fpos = grid.fpos(&cursor.pos);
    commands
        .spawn_bundle(PbrBundle {
            mesh: cursor_mesh,
            material: cursor_mat,
            transform: Transform::from_translation(Vec3::new(fpos.x, 0.1, fpos.y)),
            ..Default::default()
        })
        .insert(Parent(plate))
        .insert(cursor);

    // Light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    // Camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-0.7, 1.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
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
                "Libra City",
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
        });
}

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
    pos: (i32, i32),
    move_speed: f32,
}

struct Grid {
    size: (i32, i32),
}

impl Grid {
    pub fn new() -> Grid {
        Grid { size: (8, 8) }
    }

    pub fn min_pos(&self) -> (i32, i32) {
        let x_min = -self.size.0 / 2;
        let y_min = -self.size.1 / 2;
        (x_min, y_min)
    }

    pub fn max_pos(&self) -> (i32, i32) {
        let x_max = (self.size.0 - 1) / 2;
        let y_max = (self.size.1 - 1) / 2;
        (x_max, y_max)
    }

    pub fn clamp(&self, pos: (i32, i32)) -> (i32, i32) {
        let (x_min, y_min) = self.min_pos();
        let (x_max, y_max) = self.max_pos();
        (pos.0.clamp(x_min, x_max), pos.1.clamp(y_min, y_max))
    }

    pub fn cell_size(&self) -> (f32, f32) {
        (1.0 / self.size.0 as f32, 1.0 / self.size.1 as f32)
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
        .add_system(plate_movement_system.system().label("plate"))
        .add_system(draw_debug_axes_system.system().after("plate"))
        .add_system(cursor_movement_system.system())
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
    grid: Res<Grid>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut Cursor, &mut Transform)>,
) {
    if let Ok((mut cursor, mut transform)) = query.single_mut() {
        let mut pos_x = cursor.pos.0;
        let mut pos_y = cursor.pos.1;
        if keyboard_input.just_pressed(KeyCode::Left) || keyboard_input.just_pressed(KeyCode::A) {
            pos_x -= 1;
        }
        if keyboard_input.just_pressed(KeyCode::Right) || keyboard_input.just_pressed(KeyCode::D) {
            pos_x += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Up) || keyboard_input.just_pressed(KeyCode::W) {
            pos_y += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Down) || keyboard_input.just_pressed(KeyCode::S) {
            pos_y -= 1;
        }
        cursor.pos = grid.clamp((pos_x, pos_y));
        //let delta_pos = cursor.move_speed * time.delta_seconds();
        let translation = &mut transform.translation;
        *translation = Vec3::new(
            (cursor.pos.0 as f32 + 0.5) * grid.cell_size().0,
            0.1,
            (cursor.pos.1 as f32 + 0.5) * grid.cell_size().1,
        );
    }
}

/// set up a simple 3D scene
fn setup3d(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut textures: ResMut<Assets<Texture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Setup grid
    grid.size = (8, 8);

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
    // Base
    let (x_min, y_min) = grid.min_pos();
    let (x_max, y_max) = grid.max_pos();
    let cell_size = grid.cell_size();
    let cell_mesh = meshes.add(Mesh::from(shape::Box::new(cell_size.0, 0.1, cell_size.1)));
    for j in y_min..y_max + 1 {
        let y = (j as f32 + 0.5) * cell_size.1;
        for i in x_min..x_max + 1 {
            let x = (i as f32 + 0.5) * cell_size.0;
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
    // cursor
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 0.1 })),
            material: materials.add(Color::rgb(0.6, 0.7, 0.8).into()),
            transform: Transform::from_translation(Vec3::new(0.0, 0.1, 0.0)), // Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2)), // *
            ..Default::default()
        })
        .insert(Parent(plate))
        .insert(Cursor {
            pos: (0, 0),
            move_speed: 1.0,
        });
    // light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });
    // camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-1.0, 1.5, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

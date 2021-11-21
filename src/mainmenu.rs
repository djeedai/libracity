use crate::{boot::UiResources, loader::Loader, AppState};
use bevy::prelude::*;

/// Main menu component.
struct MainMenu {
    can_start: bool,
    //root_entity: Entity,
    entities: Vec<Entity>,
}

impl MainMenu {
    pub fn new() -> Self {
        MainMenu {
            can_start: false,
            entities: vec![],
        }
    }
}

struct StatusText;

fn mainmenu_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ui_resouces: Res<UiResources>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Start loading game assets
    let mut loader = Loader::new();
    loader.enqueue("levels.json");
    loader.submit();

    let title_font = ui_resouces.title_font();
    let text_font = ui_resouces.text_font();

    let text_align = TextAlignment {
        horizontal: HorizontalAlign::Center,
        vertical: VerticalAlign::Center,
    };

    let mut menu_data = MainMenu::new();

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
                            font: title_font.clone(),
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
                parent
                    .spawn_bundle(TextBundle {
                        text: Text {
                            // Construct a `Vec` of `TextSection`s
                            sections: vec![
                                TextSection {
                                    value: "Loading...".to_string(),
                                    style: TextStyle {
                                        font: text_font.clone(),
                                        font_size: 40.0,
                                        color: Color::WHITE,
                                    },
                                },
                                TextSection {
                                    value: "\nThis game plays with a keyboard only".to_string(),
                                    style: TextStyle {
                                        font: text_font.clone(),
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
                    })
                    .insert(StatusText);
            })
            .id(),
    );

    // Spawn main menu
    commands.spawn().insert(menu_data).insert(loader);
}

fn mainmenu(
    asset_server: Res<AssetServer>,
    mut menu_query: Query<(&mut Loader, &mut MainMenu)>,
    mut status_text_query: Query<&mut Text, With<StatusText>>,
    mut keyboard_input: ResMut<Input<KeyCode>>,
    mut state: ResMut<State<AppState>>,
) {
    if let Ok((loader, mut main_menu)) = menu_query.single_mut() {
        // Once all assets are loaded, allow the user to start playing
        if loader.is_done() {
            if let Ok(mut text) = status_text_query.single_mut() {
                text.sections[0].value = "Press [ENTER] to start".to_owned();
            }
            main_menu.can_start = true;
        }

        if main_menu.can_start {
            if keyboard_input.just_pressed(KeyCode::P) {
                state.set(AppState::InGame).unwrap();
                // BUGBUG -- https://bevy-cheatbook.github.io/programming/states.html
                keyboard_input.reset(KeyCode::Return);
            }
        }
    }
}

fn mainmenu_exit(mut commands: Commands, mut query: Query<(&mut MainMenu,)>) {
    if let Ok((main_menu,)) = query.single_mut() {
        // BUGBUG - Didn't manage to root all UI entities to a single one to despawn a tree, always got errors or warnings,
        //          so ended up with a flat list of entities to despawn here.
        //commands.entity(menu_data.root_entity).despawn_recursive();
        main_menu.entities.iter().for_each(|ent| {
            commands.entity(*ent).despawn_recursive();
        });
    }
}

/// Plugin to handle the main menu.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system_set(
            SystemSet::on_enter(AppState::MainMenu).with_system(mainmenu_setup.system()),
        )
        .add_system_set(SystemSet::on_update(AppState::MainMenu).with_system(mainmenu.system()))
        .add_system_set(SystemSet::on_exit(AppState::MainMenu).with_system(mainmenu_exit.system()));
    }
}

use crate::{
    boot::UiResources,
    inventory::Buildable,
    loader::Loader,
    serialize::{BuildableRef, Buildables, GameDataArchive, LevelDesc, Levels},
    text_asset::TextAsset,
    AppState, Config, Error,
};
use bevy::{app::AppExit, prelude::*};
use bevy_kira_audio::{Audio, AudioSource};
use bevy_tweening::{
    lens::{TextColorLens, UiPositionLens},
    Animator, EaseFunction, EaseMethod, Tween, TweeningType,
};
use std::{collections::HashMap, time::Duration};

/// Main menu component.
#[derive(Component)]
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

#[derive(Component)]
struct StatusText;

fn mainmenu_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ui_resouces: Res<UiResources>,
    //mut materials: ResMut<Assets<ColorMaterial>>,
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

    let transparent_color = Color::NONE;
    let background_color = Color::rgb(0.15, 0.15, 0.15);
    let title_color = Color::rgb_u8(111, 188, 165);

    // Background filling the entire screen
    // Also using that as the hack of https://github.com/bevyengine/bevy/issues/676 to align the text
    let root = commands
        .spawn_bundle(NodeBundle {
            style: Style {
                position: Rect::all(Val::Px(0.0)),
                position_type: PositionType::Absolute,
                ..Default::default()
            },
            color: UiColor(background_color),
            ..Default::default()
        })
        .id();
    menu_data.entities.push(root);

    // Title
    let title_tween = Tween::new(
        EaseFunction::QuadraticInOut,
        TweeningType::Once,
        Duration::from_secs(3),
        UiPositionLens {
            start: Rect {
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(30.0),
                bottom: Val::Px(0.0),
            },
            end: Rect::all(Val::Px(0.0)),
        },
    );
    let text_tween = Tween::new(
        EaseMethod::Linear,
        TweeningType::Once,
        Duration::from_secs(3),
        TextColorLens {
            start: background_color, // BUG #3204 // transparent_color,
            end: title_color,
            section: 0,
        },
    );
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
                color: UiColor(transparent_color),
                ..Default::default()
            })
            .insert(Animator::new(title_tween))
            .insert(Parent(root))
            .with_children(|parent| {
                // Title itself
                parent
                    .spawn_bundle(TextBundle {
                        text: Text::with_section(
                            "Libra City",
                            TextStyle {
                                font: title_font.clone(),
                                font_size: 250.0,
                                color: background_color,
                            },
                            text_align,
                        ),
                        ..Default::default()
                    })
                    .insert(Animator::new(text_tween));
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
                color: UiColor(Color::rgb(0.15, 0.15, 0.15)),
                ..Default::default()
            })
            .insert(Parent(root))
            .with_children(|parent| {
                // Title itself
                parent
                    .spawn_bundle(TextBundle {
                        text: Text {
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
    commands
        .spawn()
        .insert(Name::new("MainMenu"))
        .insert(menu_data)
        .insert(loader);
}

fn mainmenu(
    asset_server: Res<AssetServer>,
    mut menu_query: Query<(&mut Loader, &mut MainMenu)>,
    mut status_text_query: Query<&mut Text, With<StatusText>>,
    mut keyboard_input: ResMut<Input<KeyCode>>,
    mut state: ResMut<State<AppState>>,
    text_assets: Res<Assets<TextAsset>>,
    commands: Commands,
    mut levels_res: ResMut<Levels>,
    mut buildables_res: ResMut<Buildables>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: EventWriter<AppExit>,
) {
    let (mut loader, mut main_menu) = menu_query.single_mut();
    // Once all assets are loaded, allow the user to start playing
    if loader.is_done() {
        // Retrieve and parse JSON, load assets from it
        let handle = loader.take("levels.json").unwrap().typed::<TextAsset>();
        let json_content = text_assets.get(handle).unwrap();
        let mut game_data_archive = match GameDataArchive::from_json(&json_content.value[..]) {
            Ok(game_data_archive) => game_data_archive,
            Err(err) => {
                error!("Error loading game data: {:?}", err);
                exit.send(AppExit);
                return;
            }
        };

        // Reset the loader, so that is_done() returns false
        loader.reset();

        let color_unselected = Color::rgba(1.0, 1.0, 1.0, 0.5);
        let color_selected = Color::rgba(1.0, 1.0, 1.0, 1.0);
        let color_empty = Color::rgba(1.0, 0.8, 0.8, 0.5);

        // Load referenced assets
        let mut buildables = HashMap::new();
        for (item_name, rules) in game_data_archive.inventory.iter() {
            // Load 3D model
            let mesh: Handle<Scene> = asset_server.load(&format!("models/{}", rules.model)[..]);
            let material = materials.add(StandardMaterial {
                // TODO - from file?
                base_color: Color::rgb(0.8, 0.7, 0.6),
                ..Default::default()
            });

            // Load 2D frame
            let frame_image: Handle<Image> =
                asset_server.load(&format!("textures/{}", rules.frame)[..]);

            // Create Buildable
            buildables.insert(
                BuildableRef(item_name.clone()),
                Buildable::new(
                    &rules.name,
                    rules.weight,
                    false,
                    mesh,
                    material,
                    frame_image,
                    color_unselected,
                    color_selected,
                    color_empty,
                ),
            );
        }
        *buildables_res = Buildables::with_buildables(buildables);

        // Convert levels
        let levels: Vec<_> = game_data_archive
            .levels
            .drain(..)
            .map(|desc| LevelDesc {
                name: desc.name,
                grid_size: desc.grid_size,
                balance_factor: desc.balance_factor,
                victory_margin: desc.victory_margin,
                inventory: desc
                    .inventory
                    .iter()
                    .map(|(k, v)| (BuildableRef(k.clone()), *v))
                    .collect(),
            })
            .collect();
        *levels_res = Levels::with_levels(levels);

        // Update status text
        let mut text = status_text_query.single_mut();
        text.sections[0].value = "Press [ENTER] to start".to_owned();

        // Enable player input
        main_menu.can_start = true;
    }

    if main_menu.can_start {
        if keyboard_input.just_pressed(KeyCode::Return) {
            state.set(AppState::InGame).unwrap();
            // BUGBUG -- https://bevy-cheatbook.github.io/programming/states.html
            keyboard_input.reset(KeyCode::Return);
        }
    }
}

fn mainmenu_exit(mut commands: Commands, mut query: Query<&mut MainMenu>) {
    let main_menu = query.single_mut();
    // BUGBUG - Didn't manage to root all UI entities to a single one to despawn a tree, always got errors or warnings,
    //          so ended up with a flat list of entities to despawn here.
    //commands.entity(menu_data.root_entity).despawn_recursive();
    main_menu.entities.iter().for_each(|ent| {
        commands.entity(*ent).despawn_recursive();
    });
}

fn start_background_audio(asset_server: Res<AssetServer>, audio: Res<Audio>, config: Res<Config>) {
    if config.sound.enabled {
        let source: Handle<AudioSource> = asset_server.load("audio/ambient1.ogg");
        audio.set_volume(config.sound.volume);
        audio.play_looped(source);
    }
}

/// Plugin to handle the main menu.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(AppState::MainMenu)
                .with_system(mainmenu_setup)
                .with_system(start_background_audio),
        )
        .add_system_set(SystemSet::on_update(AppState::MainMenu).with_system(mainmenu))
        .add_system_set(SystemSet::on_exit(AppState::MainMenu).with_system(mainmenu_exit));
    }
}

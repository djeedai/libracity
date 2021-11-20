use bevy::{app::AppExit, prelude::*};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::Read};

use crate::{inventory::Buildable, text_asset::TextAsset, AppState, Error};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildableRef(pub String);

impl From<&str> for BuildableRef {
    fn from(s: &str) -> Self {
        BuildableRef(s.to_owned())
    }
}

impl From<String> for BuildableRef {
    fn from(s: String) -> Self {
        BuildableRef(s)
    }
}

impl From<&String> for BuildableRef {
    fn from(s: &String) -> Self {
        BuildableRef(s.clone())
    }
}

/// Description of a single level.
pub struct LevelDesc {
    /// Level display name.
    name: String,
    /// Plate grid size.
    grid_size: IVec2,
    /// Balance factor for COG excentricity to plate rotation.
    balance_factor: f32,
    /// Victor margin for COG excentricity.
    victory_margin: f32,
    /// Map of available buildables count when starting level.
    inventory: HashMap<BuildableRef, u32>,
}

impl LevelDesc {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inventory(&self) -> &HashMap<BuildableRef, u32> {
        &self.inventory
    }

    pub fn grid_size(&self) -> &IVec2 {
        &self.grid_size
    }

    pub fn balance_factor(&self) -> f32 {
        self.balance_factor
    }

    pub fn victory_margin(&self) -> f32 {
        self.victory_margin
    }
}

/// Resource describing of all available levels and their rules.
pub struct Levels {
    levels: Vec<LevelDesc>,
}

impl Levels {
    pub fn new() -> Self {
        Levels { levels: vec![] }
    }

    pub fn levels(&self) -> &[LevelDesc] {
        &self.levels
    }
}

/// Resource describing of all buildable items and their characteristics.
pub struct Buildables {
    buildables: HashMap<BuildableRef, Buildable>,
}

impl Buildables {
    pub fn new() -> Self {
        Buildables {
            buildables: HashMap::new(),
        }
    }

    pub fn get(&self, id: &BuildableRef) -> Option<&Buildable> {
        self.buildables.get(id)
    }
}

/// Rules for a buildable serialized.
#[derive(Deserialize)]
struct BuildableRulesArchive {
    /// Display name.
    name: String,
    /// Path to the 3D model asset, relative to the models/ folder.
    model: String,
    /// Path to the frame 2D texture asset, relative to the textures/ folder.
    frame: String,
    /// Weight of the buildable.
    weight: f32,
}

impl BuildableRulesArchive {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }
}

/// Description of a single level serialized.
#[derive(Deserialize)]
struct LevelDescArchive {
    /// Level display name.
    name: String,
    /// Plate grid size.
    grid_size: IVec2,
    /// Balance factor for COG excentricity to plate rotation.
    balance_factor: f32,
    /// Victor margin for COG excentricity.
    victory_margin: f32,
    /// Map of available buildables count when starting level.
    inventory: HashMap<String, u32>,
}

/// Game data serialized.
#[derive(Deserialize)]
struct GameDataArchive {
    inventory: HashMap<String, BuildableRulesArchive>,
    levels: Vec<LevelDescArchive>,
}

fn load_level_assets(json_content: &str) -> Result<GameDataArchive, Error> {
    let file: GameDataArchive = serde_json::from_str(json_content)?;
    debug!("Loaded levels.json:");
    for (index, l) in file.levels.iter().enumerate() {
        let inv = l
            .inventory
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .fold(String::new(), |acc, x| {
                if acc.is_empty() {
                    x
                } else {
                    format!("{},{}", acc, x)
                }
            });
        debug!(
            "+ Level #{} '{}' ({}x{}): {}",
            index, l.name, l.grid_size.x, l.grid_size.y, inv
        );
    }
    Ok(file)
}

enum ConfigLoadState {
    Unloaded,
    Pending(Handle<TextAsset>),
    Loaded,
}

pub struct ConfigLoadedEvent;

/// System loading the game rules and the referenced level assets, and building the in-memory game data.
/// This updates the [`Levels`] and [`Buildables`] resources, and sends a [`ConfigLoadedEvent`] once done.
fn load_config(
    asset_server: Res<AssetServer>,
    text_assets: Res<Assets<TextAsset>>,
    commands: Commands,
    mut levels_res: ResMut<Levels>,
    mut buildables_res: ResMut<Buildables>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
    mut exit: EventWriter<AppExit>,
    mut config_load_state: ResMut<ConfigLoadState>,
    mut ev_config_loaded: EventWriter<ConfigLoadedEvent>,
) {
    let state = &*config_load_state;
    match state {
        ConfigLoadState::Unloaded => {
            // Start asynchronous loading
            let handle: Handle<TextAsset> = asset_server.load("levels.json");
            trace!("level.json asset: {:?}", handle);
            *config_load_state = ConfigLoadState::Pending(handle);
        }
        ConfigLoadState::Pending(handle) => {
            // Check if loading finished
            if let Some(json_content) = text_assets.get(handle) {
                trace!("level.json finished loading.");
                *config_load_state = ConfigLoadState::Loaded;

                let mut game_data_archive = match load_level_assets(&json_content.value[..]) {
                    Ok(game_data_archive) => game_data_archive,
                    Err(err) => {
                        error!("Error loading game data: {:?}", err);
                        exit.send(AppExit);
                        return;
                    }
                };
                let color_unselected = Color::rgba(1.0, 1.0, 1.0, 0.5);
                let color_selected = Color::rgba(1.0, 1.0, 1.0, 1.0);
                let color_empty = Color::rgba(1.0, 0.8, 0.8, 0.5);

                // Load referenced assets
                let mut buildables = HashMap::new();
                for (item_name, rules) in game_data_archive.inventory.iter() {
                    // Load 3D model
                    let mesh: Handle<Mesh> =
                        asset_server.load(&format!("models/{}", rules.model)[..]);
                    let material = materials.add(StandardMaterial {
                        // TODO - from file?
                        base_color: Color::rgb(0.8, 0.7, 0.6),
                        ..Default::default()
                    });
                    // Load 2D frame
                    let frame_texture: Handle<Texture> =
                        asset_server.load(&format!("textures/{}", rules.frame)[..]);
                    let frame_material = materials2d.add(ColorMaterial {
                        color: color_unselected,
                        texture: Some(frame_texture.clone()),
                    });
                    let frame_material_selected = materials2d.add(ColorMaterial {
                        color: color_selected,
                        texture: Some(frame_texture.clone()),
                    });
                    let frame_material_empty = materials2d.add(ColorMaterial {
                        color: color_empty,
                        texture: Some(frame_texture),
                    });
                    // Create Buildable
                    buildables.insert(
                        BuildableRef(item_name.clone()),
                        Buildable::new(
                            rules.name(),
                            rules.weight(),
                            false,
                            mesh,
                            material,
                            frame_material,
                            frame_material_selected,
                            frame_material_empty,
                        ),
                    );
                }
                *buildables_res = Buildables { buildables };

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
                *levels_res = Levels { levels };

                ev_config_loaded.send(ConfigLoadedEvent);
            } else {
                trace!("level.json pending loading...");
            }
        }
        _ => {}
    }
}

/// Plugin for game data loading. This inserts a [`Levels`] resource and a [`Buildables`]
/// resource.
pub struct SerializePlugin;

impl Plugin for SerializePlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Levels::new())
            .insert_resource(ConfigLoadState::Unloaded)
            .insert_resource(Buildables::new())
            .add_event::<ConfigLoadedEvent>()
            .add_system_set(
                SystemSet::on_update(AppState::MainMenu).with_system(load_config.system()),
            );
    }
}

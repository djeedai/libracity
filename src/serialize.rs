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
#[derive(Debug)]
pub struct LevelDesc {
    /// Level display name.
    pub name: String,
    /// Plate grid size.
    pub grid_size: IVec2,
    /// Balance factor for COG excentricity to plate rotation.
    pub balance_factor: f32,
    /// Victor margin for COG excentricity.
    pub victory_margin: f32,
    /// Map of available buildables count when starting level.
    pub inventory: HashMap<BuildableRef, u32>,
}

/// Resource describing of all available levels and their rules.
#[derive(Debug)]
pub struct Levels {
    levels: Vec<LevelDesc>,
}

impl Levels {
    pub fn new() -> Self {
        Levels { levels: vec![] }
    }

    pub fn with_levels(levels: Vec<LevelDesc>) -> Self {
        Levels { levels }
    }

    pub fn levels(&self) -> &[LevelDesc] {
        &self.levels
    }
}

/// Resource describing of all buildable items and their characteristics.
#[derive(Debug)]
pub struct Buildables {
    buildables: HashMap<BuildableRef, Buildable>,
}

impl Buildables {
    pub fn new() -> Self {
        Buildables {
            buildables: HashMap::new(),
        }
    }

    pub fn with_buildables(buildables: HashMap<BuildableRef, Buildable>) -> Self {
        Buildables { buildables }
    }

    pub fn get(&self, id: &BuildableRef) -> Option<&Buildable> {
        self.buildables.get(id)
    }
}

/// Rules for a buildable serialized.
#[derive(Debug, Deserialize)]
pub struct BuildableRulesArchive {
    /// Display name.
    pub name: String,
    /// Path to the 3D model asset, relative to the models/ folder.
    pub model: String,
    /// Path to the frame 2D texture asset, relative to the textures/ folder.
    pub frame: String,
    /// Weight of the buildable.
    pub weight: f32,
}

/// Description of a single level serialized.
#[derive(Debug, Deserialize)]
pub struct LevelDescArchive {
    /// Level display name.
    pub name: String,
    /// Plate grid size.
    pub grid_size: IVec2,
    /// Balance factor for COG excentricity to plate rotation.
    pub balance_factor: f32,
    /// Victor margin for COG excentricity.
    pub victory_margin: f32,
    /// Map of available buildables count when starting level.
    pub inventory: HashMap<String, u32>,
}

/// Game data serialized.
#[derive(Debug, Deserialize)]
pub struct GameDataArchive {
    pub inventory: HashMap<String, BuildableRulesArchive>,
    pub levels: Vec<LevelDescArchive>,
}

impl GameDataArchive {
    pub fn from_json(json_content: &str) -> Result<GameDataArchive, Error> {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigLoadState {
    Unloaded,
    Pending(Handle<TextAsset>),
    Loaded,
}

/// Plugin for game data loading. This inserts a [`Levels`] resource and a [`Buildables`]
/// resource.
pub struct SerializePlugin;

impl Plugin for SerializePlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Levels::new())
            .insert_resource(ConfigLoadState::Unloaded)
            .insert_resource(Buildables::new());
    }
}

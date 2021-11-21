use bevy::{app::CoreStage, prelude::*};

use crate::{
    inventory::{Inventory, Slot},
    serialize::{Buildables, Levels},
    AppState, Cursor, RegenerateInventoryUiEvent, ResetPlateEvent, Grid,
};

pub enum LoadLevel {
    Next,
    ByName(String),
    ByIndex(usize),
}

/// Event to load a level.
pub struct LoadLevelEvent(pub LoadLevel);

/// Marker for the Text component displaying the level name.
pub struct LevelNameText;

/// Resource representing the current level being played.
pub struct Level {
    /// Index into [`Levels`].
    index: usize,
    /// Display name.
    name: String,
}

impl Level {
    pub fn new() -> Self {
        Level {
            index: 0,
            name: String::new(),
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// System reacting to the [`LoadLevelEvent`] event to load the specified level.
/// The system runs at the very end of the frame, after all other stages.
fn load_level_system(
    mut level: ResMut<Level>,
    mut inventory: ResMut<Inventory>,
    levels: Res<Levels>,
    buildables: Res<Buildables>,
    grid: Res<Grid>,
    mut ev_load_level: EventReader<LoadLevelEvent>,
    mut query_level_name_text: Query<&mut Text, With<LevelNameText>>,
    mut query_cursor: Query<(&Cursor, &mut Visible, &mut Transform)>,
    mut state: ResMut<State<AppState>>,
    mut ev_regen_ui: EventWriter<RegenerateInventoryUiEvent>,
    mut ev_reset_plate: EventWriter<ResetPlateEvent>,
) {
    // Consume all events, and only act on last one, ignoring others
    if let Some(load_level_event) = ev_load_level.iter().last() {
        // Find level to load
        let (level_index, level_desc) = match &load_level_event.0 {
            LoadLevel::Next => {
                info!("Load level: Next");
                let next_level_index = level.index() + 1;
                let levels = levels.levels();
                if next_level_index < levels.len() {
                    let level_desc = &levels[next_level_index];
                    info!(
                        "=> Next level: #{} '{}'",
                        next_level_index,
                        level_desc.name
                    );
                    (next_level_index, level_desc)
                } else {
                    info!("=== THE END ===");
                    state.set(AppState::TheEnd).unwrap();
                    return;
                }
            }
            LoadLevel::ByName(level_name) => {
                info!("Load level: {}", level_name);
                // Find by name
                if let Some((level_index, level_desc)) = levels
                    .levels()
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.name == *level_name)
                {
                    info!("=> Level '{}': #{}", level_name, level_index);
                    (level_index, level_desc)
                } else {
                    error!(
                        "Failed to handle LoadLevelEvent: Cannot find level '{}'.",
                        level_name
                    );
                    return;
                }
            }
            LoadLevel::ByIndex(level_index) => {
                info!("Load level: #{}", level_index);
                // Find by index
                let level_index = *level_index;
                if level_index < levels.levels().len() {
                    let level_desc = &levels.levels()[level_index];
                    info!("=> Level #{}: '{}'", level_index, level_desc.name);
                    (level_index, level_desc)
                } else {
                    error!(
                        "Failed to handle LoadLevelEvent: Cannot find level #{}.",
                        level_index
                    );
                    return;
                }
            }
        };

        // Load level
        *level = Level {
            index: level_index,
            name: level_desc.name.clone(),
        };
        inventory.set_slots(
            level_desc
                .inventory
                .iter()
                .map(|(bref, &count)| Slot::new(bref.clone(), count)),
        );

        // Update level name in UI
        if let Ok(mut text) = query_level_name_text.single_mut() {
            text.sections[0].value = level_desc.name.clone();
        }

        // Show cursor
        if let Ok((cursor, mut visible, mut transform)) = query_cursor.single_mut() {
            visible.is_visible = true;
            let cursor_fpos = grid.fpos(&cursor.pos);
            *transform = Transform::from_translation(Vec3::new(cursor_fpos.x, 0.1, -cursor_fpos.y))
                * Transform::from_scale(Vec3::new(1.0, 0.3, 1.0));
        }

        // Regenerate inventory UI from new level data
        ev_regen_ui.send(RegenerateInventoryUiEvent);

        // Reset plate
        ev_reset_plate.send(ResetPlateEvent);
    }
}

static LOAD_LEVEL_STAGE: &str = "load_level";

/// Plugin for loading levels. This inserts a [`Level`] resource and update it when
/// a [`LoadLevelEvent`] is received.
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut AppBuilder) {
        // Add Level resource and event
        app.insert_resource(Level::new())
            .add_event::<LoadLevelEvent>();

        // Insert stage after last built-in stage and run load_level_system() there, at the very end
        // of the frame, to ensure that there's no pending entity or component being created/destroyed.
        app.add_stage_after(
            CoreStage::Last,
            LOAD_LEVEL_STAGE,
            SystemStage::single_threaded(),
        )
        .add_system_to_stage(LOAD_LEVEL_STAGE, load_level_system.system());
    }
}

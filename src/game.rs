use crate::{
    AppState, CheckLevelResultEvent, Cursor, Grid, Level, Levels, LoadLevel, LoadLevelEvent,
};
use bevy::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GameSequence {
    //Tutorial,
    Intro,
    Play,
    Victory,
}

pub struct Game {
    sequence: GameSequence,
    timer: Timer,
}

impl Game {
    pub fn new() -> Self {
        Game {
            sequence: GameSequence::Intro,
            timer: Timer::from_seconds(3.0, false),
        }
    }

    pub fn reset_sequence(&mut self) {
        self.timer.reset();
        self.sequence = GameSequence::Intro;
    }

    pub fn advance_sequence(&mut self) -> GameSequence {
        self.timer.reset();
        let prev_sequence = self.sequence;
        self.sequence = match prev_sequence {
            GameSequence::Intro => GameSequence::Play,
            GameSequence::Play => GameSequence::Victory,
            GameSequence::Victory => {
                panic!("Cannot advance sequence from last sequence (Victory).")
            }
        };
        trace!("Game sequence: {:?} => {:?}", prev_sequence, self.sequence);
        self.sequence
    }
}

fn game_sequence(
    time: Res<Time>,
    grid: Res<Grid>,
    level: Res<Level>,
    levels: Res<Levels>,
    mut game: ResMut<Game>,
    mut ev_check_level: EventReader<CheckLevelResultEvent>,
    mut ev_load_level: EventWriter<LoadLevelEvent>,
    mut app_state: ResMut<State<AppState>>,
    mut query: Query<(&mut Cursor, &mut Visibility)>,
) {
    match game.sequence {
        GameSequence::Intro => {
            if game.timer.tick(time.delta()).just_finished() {
                let (mut cursor, mut visibility) = query.single_mut();
                cursor.set_enabled(true);
                visibility.is_visible = true;
                game.advance_sequence();
            }
        }
        GameSequence::Play => {
            // Check if some system requested the level victory condition to be evaluated.
            // This is generally sent after the last builable has been added to the plate,
            // once the inventory is empty.
            if let Some(ev) = ev_check_level.iter().last() {
                let level_index = level.index();
                let level_desc = &levels.levels()[level_index];
                // If current level was cleared, move to Victory sequence
                if grid.is_victory(level_desc.balance_factor, level_desc.victory_margin) {
                    info!(
                        "Victory! Level #{} '{}' cleared.",
                        level_index, level_desc.name
                    );
                    let (mut cursor, mut visibility) = query.single_mut();
                    cursor.set_enabled(false);
                    visibility.is_visible = false;
                    game.advance_sequence();
                }
            }
        }
        GameSequence::Victory => {
            // TODO - tick sequence animation
            if game.timer.tick(time.delta()).just_finished() {
                let level_index = level.index();
                if level_index + 1 < levels.levels().len() {
                    trace!("Game sequence: Victory => Intro(next)");
                    game.reset_sequence();
                    ev_load_level.send(LoadLevelEvent(LoadLevel::Next));
                } else {
                    trace!("Game sequence: Victory => TheEnd");
                    app_state.set(AppState::TheEnd).unwrap();
                }
            }
        }
    }
}

/// Plugin to handle the game logic.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Game::new())
            .add_system_set(SystemSet::on_update(AppState::InGame).with_system(game_sequence));
    }
}

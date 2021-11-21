use crate::{loader::Loader, AppState};
use bevy::prelude::*;

pub struct UiResources {
    title_font: Handle<Font>,
    text_font: Handle<Font>,
}

impl UiResources {
    pub fn new() -> Self {
        UiResources {
            title_font: Default::default(),
            text_font: Default::default(),
        }
    }

    pub fn title_font(&self) -> Handle<Font> {
        self.title_font.clone()
    }

    pub fn text_font(&self) -> Handle<Font> {
        self.text_font.clone()
    }
}

/// Marker component for the boot sequence entity holding the [`Loader`] which
/// handles the critical boot assets.
struct Boot;

fn boot_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut loader = Loader::new();
    loader.enqueue("fonts/pacifico/Pacifico-Regular.ttf");
    loader.enqueue("fonts/mochiy_pop_one/MochiyPopOne-Regular.ttf");
    loader.submit();
    commands.spawn().insert(Boot).insert(loader);
}

fn boot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &mut Loader, With<Boot>)>,
    mut ui_resouces: ResMut<UiResources>,
    mut state: ResMut<State<AppState>>,
) {
    if let Ok((id, loader, _)) = query.single_mut() {
        if loader.is_done() {
            // Destroy the Boot entity
            commands.entity(id).despawn();

            // Populate the UI resources
            let title_font: Handle<Font> = asset_server.load("fonts/pacifico/Pacifico-Regular.ttf");
            assert!(asset_server.get_load_state(&title_font) == bevy::asset::LoadState::Loaded);
            let text_font: Handle<Font> =
                asset_server.load("fonts/mochiy_pop_one/MochiyPopOne-Regular.ttf");
            assert!(asset_server.get_load_state(&text_font) == bevy::asset::LoadState::Loaded);
            *ui_resouces = UiResources {
                title_font,
                text_font,
            };

            // Change app state to load the main menu
            assert!(*state.current() == AppState::Boot);
            state.set(AppState::MainMenu).unwrap();
        }
    }
}

/// Plugin to load the critical assets before the any loading screen can be displayed.
pub struct BootPlugin;

impl Plugin for BootPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(UiResources::new())
            .add_system_set(SystemSet::on_enter(AppState::Boot).with_system(boot_setup.system()))
            .add_system_set(SystemSet::on_update(AppState::Boot).with_system(boot.system()));
    }
}

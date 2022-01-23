use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};

/// A plain text utf-8 encoded asset.
#[derive(Debug, TypeUuid)]
#[uuid = "08588ad8-7dda-46bf-8857-1e896e4264f5"]
pub struct TextAsset {
    pub value: String,
}

/// Asset loader for deserializing `*.txt` / `*.json` into a [`TextAsset`].
#[derive(Default)]
struct TextAssetLoader;

impl AssetLoader for TextAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, anyhow::Result<(), anyhow::Error>> {
        Box::pin(async move {
            let s = std::str::from_utf8(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(TextAsset {
                value: s.to_owned(),
            }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["txt", "json"]
    }
}

/// Plugin to register the [`TextAsset`] and its loader.
pub struct TextAssetPlugin;

impl Plugin for TextAssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<TextAsset>()
            .init_asset_loader::<TextAssetLoader>();
    }
}

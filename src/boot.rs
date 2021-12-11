use crate::{loader::Loader, text_asset::TextAsset, AppState, Config};
use bevy::{
    core::Byteable,
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::OrthographicProjection,
        mesh::shape,
        pipeline::{PipelineDescriptor, RenderPipeline},
        render_graph::{base, RenderGraph, RenderResourcesNode},
        renderer::{RenderResource, RenderResources},
        shader::{ShaderStage, ShaderStages},
    },
};

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

// #[derive(RenderResource, Default, TypeUuid)]
// #[uuid = "463e4b8b-d555-4fc2-ba9f-4c880063ba92"]
// #[repr(C)]
// pub struct Color32 {
//     red: u8,
//     green: u8,
//     blue: u8,
//     alpha: u8,
// }

// // Implement the Byteable trait, required for RenderResource
// unsafe impl Byteable for Color32 {}

// impl Into<Color32> for Color {
//     fn into(self) -> Color32 {
//         if let Color::Rgba {
//             red,
//             green,
//             blue,
//             alpha,
//         } = self.as_rgba()
//         {
//             Color32 {
//                 red: (red * u8::MAX as f32) as u8,
//                 green: (green * u8::MAX as f32) as u8,
//                 blue: (blue * u8::MAX as f32) as u8,
//                 alpha: (alpha * u8::MAX as f32) as u8,
//             }
//         } else {
//             panic!("as_rgba() didn't return a Color::Rgba");
//         }
//     }
// }

/// Uniform render resource to pass data from CPU to GPU.
#[derive(RenderResources, Default, Debug, TypeUuid)]
#[uuid = "463e4b8a-d555-4fc2-ba9f-4c880063ba92"]
//#[render_resources(from_self)] // BUG #3295 - does not work
//#[repr(C)]
struct ProgressBarUniform {
    /// Progress bar background color.
    back_color: Color,
    /// Progress bar fill color.
    fill_color: Color,
    /// Progress bar fraction in [0:1].
    loading_fraction: f32,
}

// Implement the Byteable trait, required for RenderResource
//unsafe impl Byteable for ProgressBarUniform {}

const VERTEX_SHADER: &str = r#"
#version 450

layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec2 Vertex_Uv;
layout(location = 0) out vec2 v_Uv;

layout(set = 0, binding = 0) uniform CameraViewProj {
    mat4 ViewProj;
};

layout(set = 1, binding = 0) uniform Transform {
    mat4 Model;
};

void main() {
    gl_Position = ViewProj * Model * vec4(Vertex_Position, 1.0);
    v_Uv = Vertex_Uv;
}
"#;

const FRAGMENT_SHADER: &str = r#"
#version 450

layout(location = 0) in vec2 v_Uv;
layout(location = 0) out vec4 o_Target;

layout(set = 2, binding = 0) uniform ProgressBarUniform_back_color {
    vec4 back_color;
};
layout(set = 2, binding = 1) uniform ProgressBarUniform_fill_color {
    vec4 fill_color;
};
layout(set = 2, binding = 2) uniform ProgressBarUniform_loading_fraction {
    float loading_fraction;
};

void main() {
    float threshold = step(loading_fraction, v_Uv.x);
    vec3 mixed = mix(fill_color.rgb, back_color.rgb, threshold);
    o_Target = vec4(mixed, 1.0);
}
"#;

/// Component for the boot sequence entity holding the [`Loader`] which
/// handles the critical boot assets, and the progress bar associated with
/// it for user feedback.
#[derive(Debug)]
struct Boot {
    /// Actual realtime boot progress, based on number of loaded assets.
    progress: f32,
    /// Displayed progress, based on [`progress`] and smoothed for a nice animated effect.
    anim_progress: f32,
    /// Maximum progress speed, in percent per second. This is the maximum speed at which
    /// [`anim_progress`] tries to catch up to [`progress`]. Keep this fast to avoid overly
    /// slowing down the boot sequence.
    speed: f32,
    /// Collection of entities of the boot screen, to delete once boot is done.
    entities: Vec<Entity>,
}

impl Default for Boot {
    fn default() -> Self {
        Boot {
            progress: 0.0,
            anim_progress: 0.0,
            speed: 1.0, // percent per second; 1.0 = 100% in 1 second
            entities: vec![],
        }
    }
}

impl Boot {
    /// Update the boot progress based on the [`percent_done`] in [0:1] and the current
    /// frame delta time in seconds (for progress smoothing animation).
    pub fn progress(&mut self, percent_done: f32, dt: f32) -> f32 {
        self.progress = percent_done.clamp(0.0, 1.0);
        let delta_p = (self.progress - self.anim_progress) / self.speed;
        let anim_progress = self.anim_progress + dt * delta_p;
        self.anim_progress = anim_progress.min(self.progress);
        self.anim_progress
    }
}

/// Setup the boot sequence and its display screen, preparing the loader with all critical assets
/// to load, and the progress bar associated with it (and all the rendering resources to render it).
fn boot_setup(
    asset_server: Res<AssetServer>,
    mut clear_color: ResMut<ClearColor>,
    mut commands: Commands,
    mut pipelines: ResMut<Assets<PipelineDescriptor>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut render_graph: ResMut<RenderGraph>,
) {
    trace!("boot_setup");

    // Set clear color to background color
    clear_color.0 = Color::rgba(0.1, 0.1, 0.1, 0.0);

    // Create a new shader pipeline
    let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
        vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
        fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
    }));

    // Add a `RenderResourcesNode` to our `RenderGraph`. This will bind `ProgressBarUniform` to our
    // shader.
    render_graph.add_system_node(
        "progress_bar_uniform",
        RenderResourcesNode::<ProgressBarUniform>::new(true),
    );

    // Add a `RenderGraph` edge connecting our new "progress_bar_uniform" node to the main pass node. This
    // ensures that "progress_bar_uniform" runs before the main pass.
    render_graph
        .add_node_edge("progress_bar_uniform", base::node::MAIN_PASS)
        .unwrap();

    let mut boot = Boot::default();

    // Spawn the progress bar
    boot.entities.push(
        commands
            .spawn_bundle(MeshBundle {
                mesh: meshes.add(Mesh::from(shape::Quad::new(Vec2::new(200.0, 3.0)))),
                render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
                    pipeline_handle,
                )]),
                transform: Transform::identity(),
                ..Default::default()
            })
            .insert(ProgressBarUniform {
                loading_fraction: 0.0,
                back_color: Color::rgba(0.2, 0.2, 0.2, 1.0), //.into(),
                fill_color: Color::rgba(0.3, 0.4, 0.3, 1.0), //.into(),
            })
            .id(),
    );

    // Spawn a camera to render the progress bar
    boot.entities.push(
        commands
            .spawn_bundle(OrthographicCameraBundle::new_2d())
            .id(),
    );

    // Create the loader component itself, and enqueue all asset loading requests
    let mut loader = Loader::new();
    loader.enqueue("config.json");
    loader.enqueue("fonts/pacifico/Pacifico-Regular.ttf");
    loader.enqueue("fonts/mochiy_pop_one/MochiyPopOne-Regular.ttf");
    loader.submit();

    // Create the boot entity itself
    commands
        .spawn()
        .insert(Name::new("Boot"))
        .insert(boot)
        .insert(loader);
}

fn boot(
    time: Res<Time>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    text_assets: Res<Assets<TextAsset>>,
    mut config: ResMut<Config>,
    mut query: Query<(Entity, &mut Loader, &mut Boot)>,
    mut ui_resouces: ResMut<UiResources>,
    mut state: ResMut<State<AppState>>,
    mut shader_query: Query<&mut ProgressBarUniform>,
) {
    if let Ok((id, mut loader, mut boot)) = query.single_mut() {
        if loader.is_done() {
            // Mark the Boot entity for later destruction (at the end of the stage)
            commands.entity(id).despawn();

            // Also delete all related entities for the boot screen
            for id in &boot.entities {
                commands.entity(*id).despawn();
            }

            // Assign the loaded config if any
            if let Some(handle) = loader.take("config.json") {
                let handle = handle.typed::<TextAsset>();
                let json_config = text_assets.get(handle).unwrap();
                *config = Config::from_json(&json_config.value[..]).unwrap();
            }

            // Assign the UI resources for the main menu, which will immediately replace the
            // boot sequence to allow user interaction and optionally continue loading some other
            // assets, but this time with a basic set of assets (fonts, notably) already loaded,
            // allowing to render some less terse user interface than a single progress bar without
            // any text.
            let title_font: Handle<Font> = loader
                .take("fonts/pacifico/Pacifico-Regular.ttf")
                .unwrap()
                .typed::<Font>();
            let text_font: Handle<Font> = loader
                .take("fonts/mochiy_pop_one/MochiyPopOne-Regular.ttf")
                .unwrap()
                .typed::<Font>();
            *ui_resouces = UiResources {
                title_font,
                text_font,
            };

            // Change app state to transition to the main menu
            assert!(*state.current() == AppState::Boot);
            state.set(AppState::MainMenu).unwrap();
        } else {
            // Update the progress bar based on the fraction of assets already loaded, smoothed with
            // a snappy animation to be visually pleasant without too much artifically delaying the
            // boot sequence.
            let percent_done = loader.percent_done();
            let percent_done = boot.progress(percent_done, time.delta_seconds());
            let mut time_uniform = shader_query.single_mut().unwrap();
            time_uniform.loading_fraction = percent_done;
        }
    }
}

/// Plugin to load the critical assets before the main menu can be displayed.
pub struct BootPlugin;

impl Plugin for BootPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Config::default())
            .insert_resource(UiResources::new())
            .add_startup_system(boot_setup.system())
            .add_system_set(SystemSet::on_update(AppState::Boot).with_system(boot.system()));
    }
}

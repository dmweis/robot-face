use anyhow::Context;
use bevy::{
    core::FrameCount,
    diagnostic::{
        EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    prelude::*,
    window::{CursorGrabMode, PresentMode, WindowLevel, WindowResolution, WindowTheme},
};
use bevy_prototype_lyon::prelude::*;
use clap::Parser;
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin, PerfUiRoot};
use noise::{BasicMulti, MultiFractal, NoiseFn, Perlin};
use thiserror::Error;
use tokio::{
    runtime,
    sync::mpsc::{channel, Receiver, Sender},
};
use zenoh::prelude::r#async::*;

const WIDTH_DIVIDER: f64 = 60.0;
const HEIGHT_MULTIPLIER: f64 = 400.0;
const SEGMENT_WIDTH: f32 = 5.0;
const FRAME_TIME_DIVIDER: f64 = 8.0;
const PERLIN_NOISE_OCTAVES: usize = 2;

const LINE_WIDTH: f32 = 2.0;
const PERLIN_NOISE_SEED: u32 = 100;

#[derive(Resource)]
struct NoiseGeneratorSettings {
    width_divider: f64,
    height_multiplier: f64,
    segment_width: f32,
    frame_time_divider: f64,
}

impl Default for NoiseGeneratorSettings {
    fn default() -> Self {
        Self {
            width_divider: WIDTH_DIVIDER,
            height_multiplier: HEIGHT_MULTIPLIER,
            segment_width: SEGMENT_WIDTH,
            frame_time_divider: FRAME_TIME_DIVIDER,
        }
    }
}

/// Run robot face animation
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
    /// Run in dev mode
    #[arg(short, long)]
    dev_mode: bool,
}

fn main() {
    let args = Args::parse();

    let mut window_settings = Window {
        title: "robot face".into(),
        name: Some("face.app".into()),
        resolution: WindowResolution::new(480., 800.).with_scale_factor_override(1.0),
        present_mode: PresentMode::AutoVsync,
        window_theme: Some(WindowTheme::Dark),
        enabled_buttons: bevy::window::EnabledButtons {
            maximize: false,
            minimize: false,
            ..Default::default()
        },
        visible: false,
        window_level: WindowLevel::AlwaysOnTop,
        mode: bevy::window::WindowMode::BorderlessFullscreen,
        cursor: bevy::window::Cursor {
            visible: false,
            grab_mode: CursorGrabMode::Confined,
            ..default()
        },
        ..default()
    };

    if args.dev_mode {
        window_settings.window_level = WindowLevel::Normal;
        window_settings.mode = bevy::window::WindowMode::Windowed;
        window_settings.cursor.grab_mode = CursorGrabMode::None;
        window_settings.cursor.visible = true;
    }

    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(NoiseGeneratorSettings::default())
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window_settings),
                ..default()
            }),
            // LogDiagnosticsPlugin::default(),
            // FrameTimeDiagnosticsPlugin,
            // EntityCountDiagnosticsPlugin,
            // SystemInformationDiagnosticsPlugin,
        ))
        .add_plugins(ShapePlugin)
        // .add_plugins(PerfUiPlugin)
        .add_systems(Startup, (setup_system, start_zenoh_worker))
        .add_systems(
            Update,
            (
                // toggle_perf_ui.before(iyes_perf_ui::PerfUiSet::Setup),
                toggle_fullscreen,
                bevy::window::close_on_esc,
                close_on_right_click,
                make_visible,
                update_noise_plot,
                process_noise_generator_update_messages,
            ),
        )
        .run();
}

#[derive(Component)]
struct NoiseWave;

fn setup_system(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    let points = [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)].map(|x| x * 10000.);

    let shape = shapes::Polygon {
        points: points.into_iter().collect(),
        // radius: 1.0,
        closed: false,
    };

    // spawn two shapes one hidden
    // to allow 1 frame buffering on the raspberry pi
    // This prevents flickering while the texture is loading
    commands.spawn((
        ShapeBundle {
            path: GeometryBuilder::build_as(&shape),
            spatial: SpatialBundle {
                visibility: Visibility::Hidden,
                ..default()
            },
            ..default()
        },
        Stroke::new(Color::WHITE, LINE_WIDTH),
        Fill::color(Color::NONE),
        NoiseWave,
    ));
    commands.spawn((
        ShapeBundle {
            path: GeometryBuilder::build_as(&shape),
            spatial: SpatialBundle {
                visibility: Visibility::Visible,
                ..default()
            },
            ..default()
        },
        Stroke::new(Color::WHITE, LINE_WIDTH),
        Fill::color(Color::NONE),
        NoiseWave,
    ));

    let mut perlin_noise = BasicMulti::<Perlin>::new(PERLIN_NOISE_SEED);
    perlin_noise = perlin_noise.set_octaves(PERLIN_NOISE_OCTAVES);

    commands.insert_resource(NoiseGenerator {
        generator: perlin_noise,
        elapsed_step: 0.0,
    });
}

fn make_visible(mut window: Query<&mut Window>, frames: Res<FrameCount>) {
    // The delay may be different for your app or system.
    if frames.0 == 3 {
        // At this point the gpu is ready to show the app so we can make the window visible.
        // Alternatively, you could toggle the visibility in Startup.
        // It will work, but it will have one white frame before it starts rendering
        window.single_mut().visible = true;
    }
}

// This system will toggle the color theme used by the window
fn toggle_fullscreen(mut windows: Query<&mut Window>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::KeyF) {
        let mut window = windows.single_mut();

        window.mode = match window.mode {
            bevy::window::WindowMode::BorderlessFullscreen => bevy::window::WindowMode::Windowed,
            bevy::window::WindowMode::Windowed => bevy::window::WindowMode::BorderlessFullscreen,
            _ => bevy::window::WindowMode::Windowed,
        };
    }
}

fn toggle_perf_ui(
    mut commands: Commands,
    q_root: Query<Entity, With<PerfUiRoot>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    if mouse.just_pressed(MouseButton::Middle) {
        if let Ok(e) = q_root.get_single() {
            // despawn the existing Perf UI
            commands.entity(e).despawn_recursive();
        } else {
            // create a simple Perf UI with default settings
            // and all entries provided by the crate:
            commands.spawn(PerfUiCompleteBundle::default());
        }
    }
}

fn close_on_right_click(
    mut commands: Commands,
    focused_windows: Query<(Entity, &Window)>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
) {
    for (window, focus) in focused_windows.iter() {
        if !focus.focused {
            continue;
        }

        if mouse_button_input.just_pressed(MouseButton::Right) {
            commands.entity(window).despawn();
        }
    }
}

#[derive(Resource)]
struct NoiseGenerator {
    generator: BasicMulti<Perlin>,
    /// keep elapsed steps to maintain continuity
    elapsed_step: f64,
}

fn update_noise_plot(
    mut query: Query<(&mut Path, &mut Visibility), With<NoiseWave>>,
    query_camera: Query<&OrthographicProjection>,
    time: Res<Time>,
    mut noise_generator: ResMut<NoiseGenerator>,
    noise_generator_settings: Res<NoiseGeneratorSettings>,
) {
    // add to elapsed step to maintain continuity
    let step_addition = time.delta_seconds_f64() / noise_generator_settings.frame_time_divider;
    noise_generator.elapsed_step += step_addition;
    let step = noise_generator.elapsed_step;

    let mut resolution = Rect::default();
    for camera in query_camera.iter() {
        resolution = camera.area;
    }

    let width = (resolution.width() / noise_generator_settings.segment_width) as usize;

    let mut noise = Vec::with_capacity(width);

    for i in 0..=(width + 1) {
        let next_noise = noise_generator
            .generator
            .get([step, i as f64 / noise_generator_settings.width_divider]);
        noise.push(next_noise);
    }

    for (mut path, mut visibility) in query.iter_mut() {
        // swap displayed shape
        match *visibility {
            Visibility::Hidden => {
                *visibility = Visibility::Visible;
                continue;
            }
            Visibility::Visible => {
                *visibility = Visibility::Hidden;
            }
            _ => {
                *visibility = Visibility::Hidden;
            }
        }

        let points = noise
            .iter()
            .enumerate()
            .map(|(index, point)| {
                Vec2::new(
                    resolution.min.x + (index as f32) * noise_generator_settings.segment_width,
                    (*point * noise_generator_settings.height_multiplier) as f32,
                )
            })
            .collect();

        let shape = shapes::Polygon {
            points,
            closed: false,
        };

        *path = ShapePath::build_as(&shape);
    }
}

#[derive(serde::Deserialize)]
struct NoiseGeneratorSettingsUpdate {
    #[serde(default)]
    width_divider: Option<f64>,
    #[serde(default)]
    height_multiplier: Option<f64>,
    #[serde(default)]
    segment_width: Option<f32>,
    #[serde(default)]
    frame_time_divider: Option<f64>,
    #[serde(default)]
    perlin_noise_octaves: Option<usize>,
}

#[derive(Resource, Deref, DerefMut)]
struct StreamReceiver(Receiver<NoiseGeneratorSettingsUpdate>);

fn start_zenoh_worker(mut commands: Commands) {
    let (mut tx, rx) = channel::<NoiseGeneratorSettingsUpdate>(10);

    std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .build()
            .expect("Failed to build tokio runtime");
        rt.block_on(async {
            loop {
                if let Err(error) = run_zenoh_loop(&mut tx).await {
                    error!(?error, "Zenoh loop failed");
                }
            }
        });
    });

    commands.insert_resource(StreamReceiver(rx));
}

async fn run_zenoh_loop(tx: &mut Sender<NoiseGeneratorSettingsUpdate>) -> anyhow::Result<()> {
    let zenoh_config = zenoh::config::Config::default();
    let session = zenoh::open(zenoh_config)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)
        .context("Failed to create zenoh session")?
        .into_arc();

    let settings_subscriber = session
        .declare_subscriber("face/settings")
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)
        .context("Failed to create subscriber")?;

    let display_subscriber = session
        .declare_subscriber("face/display")
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)
        .context("Failed to create subscriber")?;

    tokio::spawn(async move {
        while let Ok(message) = display_subscriber.recv_async().await {
            let json_message: String = message
                .value
                .try_into()
                .expect("Failed to convert value to string");
            let display_control_message: DisplayControlMessage =
                serde_json::from_str(&json_message).expect("Failed to parse json");
            if display_control_message.display_on {
                info!("Turning on display");
                turn_on_display().await.expect("failed to turn on display");
            } else {
                info!("Turning off display");
                turn_off_display().await.expect("failed to turn on display");
            }
        }
    });

    while let Ok(message) = settings_subscriber.recv_async().await {
        let json_message: String = message
            .value
            .try_into()
            .context("Failed to convert value to string")?;
        let settings_update: NoiseGeneratorSettingsUpdate =
            serde_json::from_str(&json_message).context("Failed to parse json")?;
        tx.send(settings_update)
            .await
            .context("Failed to send message on channel")?;
    }
    Ok(())
}

fn process_noise_generator_update_messages(
    mut receiver: ResMut<StreamReceiver>,
    mut noise_generator: ResMut<NoiseGenerator>,
    mut noise_generator_settings: ResMut<NoiseGeneratorSettings>,
) {
    while let Ok(message) = receiver.try_recv() {
        if let Some(width_divider) = message.width_divider {
            info!(width_divider, "Updating width_divider");
            noise_generator_settings.width_divider = width_divider;
        }
        if let Some(height_multiplier) = message.height_multiplier {
            info!(height_multiplier, "Updating height_multiplier");
            noise_generator_settings.height_multiplier = height_multiplier;
        }
        if let Some(segment_width) = message.segment_width {
            info!(segment_width, "Updating segment_width");
            noise_generator_settings.segment_width = segment_width;
        }
        if let Some(frame_time_divider) = message.frame_time_divider {
            info!(frame_time_divider, "Updating frame_time_divider");
            noise_generator_settings.frame_time_divider = frame_time_divider;
        }

        if let Some(perlin_noise_octaves) = message.perlin_noise_octaves {
            info!(perlin_noise_octaves, "Updating perlin_noise_octaves");
            noise_generator.generator = noise_generator
                .generator
                .clone()
                .set_octaves(perlin_noise_octaves);
        }
    }
}

#[derive(Error, Debug)]
pub enum ErrorWrapper {
    #[error("Zenoh error {0:?}")]
    ZenohError(#[from] zenoh::Error),
}

#[derive(serde::Deserialize)]
struct DisplayControlMessage {
    #[serde(default)]
    display_on: bool,
}

#[cfg(not(target_os = "linux"))]
pub async fn turn_on_display() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn turn_on_display() -> anyhow::Result<()> {
    // wlr-randr --output HDMI-A-1 --on --transform 90
    let status = tokio::process::Command::new("wlr-randr")
        .arg("--output")
        .arg("HDMI-A-1")
        .arg("--on")
        .arg("--transform")
        .arg("270")
        .status()
        .await?;
    info!("Turning on display {:?}", status);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub async fn turn_off_display() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn turn_off_display() -> anyhow::Result<()> {
    // wlr-randr --output HDMI-A-1 --off
    let status = tokio::process::Command::new("wlr-randr")
        .arg("--output")
        .arg("HDMI-A-1")
        .arg("--off")
        .status()
        .await?;
    info!("Turning off display {:?}", status);
    Ok(())
}

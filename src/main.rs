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
use std::collections::VecDeque;

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
        mode: bevy::window::WindowMode::Fullscreen,
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
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window_settings),
                ..default()
            }),
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin,
            EntityCountDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
        ))
        .add_plugins(ShapePlugin)
        .add_plugins(PerfUiPlugin)
        .add_systems(Startup, setup_system)
        .add_systems(
            Update,
            toggle_perf_ui.before(iyes_perf_ui::PerfUiSet::Setup),
        )
        .add_systems(
            Update,
            (
                toggle_fullscreen,
                bevy::window::close_on_esc,
                mouse_click_system,
                make_visible,
            ),
        )
        .add_systems(FixedUpdate, update_noise_plot)
        .run();
}

#[derive(Component)]
struct ExampleShape;

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

    commands.spawn((
        ShapeBundle {
            path: GeometryBuilder::build_as(&shape),
            ..default()
        },
        Stroke::new(Color::WHITE, 2.0),
        Fill::color(Color::NONE),
        NoiseWave,
    ));

    let mut perlin_noise = BasicMulti::<Perlin>::new(100);
    perlin_noise = perlin_noise.set_octaves(2);

    commands.insert_resource(NoiseGenerator {
        generator: perlin_noise,
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
            bevy::window::WindowMode::Fullscreen => bevy::window::WindowMode::Windowed,
            bevy::window::WindowMode::Windowed => bevy::window::WindowMode::Fullscreen,
            _ => bevy::window::WindowMode::Windowed,
        };
    }
}

fn mouse_click_system(
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
}

const WIDTH_COEFFICIENT: f64 = 100.0;
const HEIGHT_COEFFICIENT: f32 = 100.0;

fn update_noise_plot(
    mut query: Query<&mut Path, With<NoiseWave>>,
    query_camera: Query<&OrthographicProjection>,
    time: Res<Time>,
    noise_generator: ResMut<NoiseGenerator>,
) {
    let step = time.elapsed_seconds_f64();

    let mut resolution = Rect::default();
    for camera in query_camera.iter() {
        resolution = camera.area;
    }

    let width = resolution.width() as usize;

    let mut noise = Vec::with_capacity(width);

    for i in 0..width {
        let next_noise = noise_generator
            .generator
            .get([step, (i as f64 / WIDTH_COEFFICIENT)]);
        noise.push(next_noise);
    }

    for mut path in query.iter_mut() {
        let points = noise
            .iter()
            .enumerate()
            .map(|(index, point)| {
                Vec2::new(
                    resolution.min.x + index as f32,
                    *point as f32 * HEIGHT_COEFFICIENT,
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

// fn map(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
//     (value - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
// }

#[derive(Resource)]
struct MovingNoiseGenerator {
    generator: BasicMulti<Perlin>,
    noise: VecDeque<f64>,
}

fn update_moving_noise_plot(
    mut query: Query<&mut Path, With<NoiseWave>>,
    query_camera: Query<&OrthographicProjection>,
    time: Res<Time>,
    mut noise_generator: ResMut<MovingNoiseGenerator>,
) {
    let step = time.elapsed_seconds_f64();

    let next_noise = noise_generator.generator.get([step, 0.]);
    noise_generator.noise.push_back(next_noise);

    let mut resolution = Rect::default();
    for camera in query_camera.iter() {
        resolution = camera.area;
    }

    let width = resolution.width() as usize;
    while noise_generator.noise.len() > width {
        noise_generator.noise.pop_front();
    }

    for mut path in query.iter_mut() {
        let points = noise_generator
            .noise
            .iter()
            .enumerate()
            .map(|(index, point)| Vec2::new(resolution.min.x + index as f32, *point as f32 * 400.0))
            .collect();

        let shape = shapes::Polygon {
            points,
            closed: false,
        };

        *path = ShapePath::build_as(&shape);
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

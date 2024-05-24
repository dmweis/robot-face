mod display;
mod messaging;
mod noise_plugin;
mod utils;

use bevy::{
    diagnostic::{
        EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    prelude::*,
    window::{CursorGrabMode, PresentMode, WindowLevel, WindowResolution, WindowTheme},
};
use clap::Parser;
use iyes_perf_ui::PerfUiPlugin;

use crate::{
    messaging::start_zenoh_worker,
    noise_plugin::NoisePlugin,
    utils::{close_on_right_click, make_visible, toggle_fullscreen, toggle_perf_ui},
};

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
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window_settings),
                ..default()
            }),
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin,
            EntityCountDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
            NoisePlugin,
            PerfUiPlugin,
        ))
        .add_systems(Startup, (start_zenoh_worker, setup_camera_system))
        .add_systems(
            Update,
            (
                toggle_perf_ui.before(iyes_perf_ui::PerfUiSet::Setup),
                toggle_fullscreen,
                bevy::window::close_on_esc,
                close_on_right_click,
                make_visible,
            ),
        )
        .run();
}

fn setup_camera_system(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

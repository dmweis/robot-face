mod display;
mod noise_plugin;

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
use clap::Parser;
use display::DisplayControlMessage;
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin, PerfUiRoot};
use noise_plugin::{NoiseGeneratorSettingsUpdate, NoisePlugin};
use thiserror::Error;
use tokio::{
    runtime,
    sync::mpsc::{channel, Receiver, Sender},
};
use zenoh::prelude::r#async::*;

use crate::display::{turn_off_display, turn_on_display};

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
        .add_systems(Startup, start_zenoh_worker)
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

#[derive(Resource, Deref, DerefMut)]
struct StreamReceiver(Receiver<NoiseGeneratorSettingsUpdate>);

fn start_zenoh_worker(mut commands: Commands) {
    let (mut tx, rx) = channel::<NoiseGeneratorSettingsUpdate>(10);

    std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_io()
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

#[derive(Error, Debug)]
pub enum ErrorWrapper {
    #[error("Zenoh error {0:?}")]
    ZenohError(#[from] zenoh::Error),
}

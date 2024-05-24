use bevy::{ core::FrameCount, prelude::*};
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiRoot};

pub fn make_visible(mut window: Query<&mut Window>, frames: Res<FrameCount>) {
    // The delay may be different for your app or system.
    if frames.0 == 3 {
        // At this point the gpu is ready to show the app so we can make the window visible.
        // Alternatively, you could toggle the visibility in Startup.
        // It will work, but it will have one white frame before it starts rendering
        window.single_mut().visible = true;
    }
}

// This system will toggle the color theme used by the window
pub fn toggle_fullscreen(mut windows: Query<&mut Window>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::KeyF) {
        let mut window = windows.single_mut();

        window.mode = match window.mode {
            bevy::window::WindowMode::BorderlessFullscreen => bevy::window::WindowMode::Windowed,
            bevy::window::WindowMode::Windowed => bevy::window::WindowMode::BorderlessFullscreen,
            _ => bevy::window::WindowMode::Windowed,
        };
    }
}

pub fn toggle_perf_ui(
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

pub fn close_on_right_click(
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

use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;
use noise::{BasicMulti, MultiFractal, NoiseFn, Perlin};

use crate::StreamReceiver;

pub struct NoisePlugin;

impl Plugin for NoisePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NoiseGeneratorSettings::default())
            .add_plugins(ShapePlugin)
            .add_systems(Startup, setup_noise_system)
            .add_systems(
                Update,
                (update_noise_plot, process_noise_generator_update_messages),
            );
    }
}

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

#[derive(Component)]
struct NoiseWave;

fn setup_noise_system(mut commands: Commands) {
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
pub struct NoiseGeneratorSettingsUpdate {
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

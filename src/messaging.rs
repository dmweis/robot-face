use anyhow::Context;
use bevy::prelude::*;
use thiserror::Error;
use tokio::{
    runtime,
    sync::mpsc::{channel, Receiver, Sender},
};
use zenoh::prelude::r#async::*;

use crate::{
    display::{turn_off_display, turn_on_display, DisplayControlMessage},
    noise_plugin::NoiseGeneratorSettingsUpdate,
};

#[derive(Resource, Deref, DerefMut)]
pub struct StreamReceiver(Receiver<NoiseGeneratorSettingsUpdate>);

pub fn start_zenoh_worker(mut commands: Commands) {
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

pub async fn run_zenoh_loop(tx: &mut Sender<NoiseGeneratorSettingsUpdate>) -> anyhow::Result<()> {
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

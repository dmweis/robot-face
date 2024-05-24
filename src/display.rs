use bevy::prelude::*;

#[derive(serde::Deserialize)]
pub struct DisplayControlMessage {
    #[serde(default)]
    pub display_on: bool,
}

#[cfg(not(target_os = "linux"))]
pub async fn turn_on_display() -> anyhow::Result<()> {
    info!("Ignoring turn_on_display on windows");
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
    info!("Ignoring turn_off_display on windows");
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

# Robot Face

## Dev mode

`cargo watch -x "run -- -d"`  

## Running over SSH

```shell
DISPLAY=":0" cargo run --release
# or 
WAYLAND_DISPLAY="wayland-1" cargo run --release

# for X11 or Wayland
```

## Wayland vs X11

<https://bevy-cheatbook.github.io/platforms/linux.html#x11-and-wayland>

## Backend

For Raspberry Pi the default backend is now Vulkan

use `WGPU_BACKEND="gl"` to switch to OpenGL. But it doesn't seem to work

## Building on linux

```shell
sudo apt-get update && sudo apt-get install librust-alsa-sys-dev libudev-dev librust-wayland-sys-dev -y
```

## Raspberry Pi startup

```txt
[autostart]
face = WAYLAND_DISPLAY="" DISPLAY=":0" ~/Desktop/face
```

## Zenoh commands

```shell
z_put --key face/settings --value '{"width_divider": 100.0}'
z_put --key face/settings --value '{"perlin_noise_octaves": 1}'
z_put --key face/settings --value '{"frame_time_divider": 2.0}'
z_put --key face/settings --value '{"segment_width": 5.0}'
z_put --key face/settings --value '{"height_multiplier": 400.0}'
```

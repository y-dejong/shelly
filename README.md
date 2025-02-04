# Shelly
A collection of custom scripts and functionality for Window management with Hyprland

## Features

### Window Zipping Daemon

Running `shelly daemon` automatically shifts to lower workspaces whenever a workspace is empty.For example, when the last window is closed or moved out of workspace 2, all workspaces from 3 onward will shift down by one to fill in the gap.

### Window Move Functions

Provides custom commands to change the functionality of moving windows between workspaces.

`shelly movetoworkspace (left/right) [create]`: Moves a window to an adjacent workspace. If `create` is provided, shelly inserts a new workspace dedicated for that window. If `create` is not provided, shelly only moves the window if an adjacent workspace already exists, meant for making split screen workspaces.

`shelly workspace (left/right): Same as `hyprctl workspace (+1/-1)`, but never creates new workspaces and does not wrap around.

## Recommended configuration

Typical usage is to add something like this to your `hyprland.conf`:

```
exec-once = shelly daemon

bind = SUPER, left, exec, shelly workspace left
bind = SUPER, right, exec, shelly workspace right
bind = SHIFT SUPER, left, exec, shelly movetoworkspace left
bind = SHIFT SUPER, right, exec, shelly movetoworkspace right
bind = CTRL SUPER, left, exec, shelly movetoworkspace left create
bind = CTRL SUPER, right, exec, shelly movetoworkspace right create
```

## Building

Shelly only relies on `rust` and `cargo`, and uses the sockets provided by Hyprland for interaction.

```bash
git clone https://github.com/y-dejong/shelly.git
cd shelly

cargo build --release
# OR to install to ~/.cargo/bin
cargo install --path .
```

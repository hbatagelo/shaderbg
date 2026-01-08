# ShaderBG

[![Build](https://github.com/hbatagelo/shaderbg/actions/workflows/build.yml/badge.svg)](https://github.com/hbatagelo/shaderbg/actions/workflows/build.yml)
[![Test](https://github.com/hbatagelo/shaderbg/actions/workflows/test.yml/badge.svg)](https://github.com/hbatagelo/shaderbg/actions/workflows/test.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

Shader wallpaper utility for Wayland.

<img src="demo.gif" alt="ShaderBG on Sway" width="800">

<sup>[Colorful underwater bubbles II](https://www.shadertoy.com/view/mlBSWc) on [Sway](https://swaywm.org/)</sup>

ShaderBG is a wallpaper tool that renders shaders as live wallpapers in Wayland compositors that support the [wlr-layer-shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) protocol. Shaders can be chosen from a built-in set, imported from [ShaderToy](https://www.shadertoy.com/), or customized via TOML.

On compositors that don't support wlr-layer-shell, ShaderBG falls back to a regular window application.

See the `shaderbg(1)` man page for usage instructions.

## Features

* Import from ShaderToy
* Multi-monitor support
* Configuration hot reload
* Cross fading between frames
* Anti-aliasing (SSAA)

## Installation

### From packages

Debian (`.deb`), Fedora (`.rpm`), and Arch (`.pkg`) packages for x86-64 are available on the [releases page](https://github.com/hbatagelo/shaderbg/releases).

<details>
<summary>Verify package checksums</summary>

Package checksums are signed with [A6265CB619139199](https://keys.openpgp.org/search?q=A22ED235E445A5C402D536E1A6265CB619139199):

```sh
# Import public GPG key
gpg --keyserver keys.openpgp.org --recv-keys A22ED235E445A5C402D536E1A6265CB619139199
# Verify the checksums signature
gpg --verify SHA256SUMS.asc
# Verify the package checksum
gpg --decrypt SHA256SUMS.asc | sha256sum -c --ignore-missing
```

</details>

### From source

1. [Set up Rust](https://www.rust-lang.org/tools/install)

2. Install dependencies

   <details>
   <summary>Arch and derivatives</summary>

   ```sh
   sudo pacman -S base-devel gtk4-layer-shell
   ```

   </details>

   <details>
   <summary>Debian and derivatives</summary>

   ```sh
   sudo apt install build-essential libgtk-4-dev libssl-dev libgtk4-layer-shell-dev
   ```

   </details>

   <details>
   <summary>Fedora and derivatives</summary>

   ```sh
   sudo dnf install gcc-c++ gtk4-devel libepoxy-devel openssl-devel gtk4-layer-shell-devel
   ```

   </details>

   If GTK4 Layer Shell is unavailable in your distribution, [build it from source](https://github.com/wmww/gtk4-layer-shell?tab=readme-ov-file#building-from-source).

3. Clone this repository
   ```sh
   git clone https://github.com/hbatagelo/shaderbg.git
   cd shaderbg
   ```

4. Build

   ```sh
   cargo build --release
   ```

5. Create the user data directory
   ```sh
   mkdir -p ~/.local/share/shaderbg
   ```

6. Copy data assets (textures and presets)
   ```sh
   cp -r data/* ~/.local/share/shaderbg/
   ```

## Usage

Run with no arguments to randomly select a shader configuration from the presets:

```sh
shaderbg
```

* Presets are located at `$XDG_DATA_HOME/shaderbg/presets` or `$HOME/.local/share/shaderbg/presets`.

Import a shader from ShaderToy using a shader ID and API key:

```sh
shaderbg <shader_id> <api_key>
```

* The shader must be set to 'public + api' in ShaderToy.
* See <https://www.shadertoy.com/howto> for information on how to get an API key.
* The imported shader is added to the presets as `<shader_id>.toml` and rewrites any previous file with the same name.

> \[!NOTE]
> The ShaderToy API is currently unavailable (hopefully temporarily) due to safety measures taken against AI crawlers.

You can also load a custom preset file:

```sh
shaderbg <file>
```

where `<file>` is a TOML file containing the shader settings. An example preset corresponding to <https://www.shadertoy.com/view/wfjcR3> is given below:

```toml
id = "wfjcR3"
name = "Spiral Wobble"
username = "hbatagelo"
description = "A simple wobbling spiral texture effect with multi-pass rendering."

# 'Common' shader
[common]
shader = """
vec2 rotate2D(vec2 uv, float angle) {
    return vec2(
        uv.x * cos(angle) - uv.y * sin(angle),
        uv.x * sin(angle) + uv.y * cos(angle)
    );
}
"""

# 'Buffer A' pass
[buffer_a]
shader = """
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float aspect = iResolution.x / iResolution.y;

    float spirals = 1.0 + 3.0 * (0.5 + 0.5 * sin(iTime * 0.5));
    vec2 centeredUV = (uv - 0.5) * vec2(aspect, 1.0);
    float angle = sin(iTime + length(centeredUV) * spirals);
    vec2 rotatedUV = rotate2D(centeredUV, angle) / vec2(aspect, 1.0) + 0.5;

    vec4 curColor = texture(iChannel1, rotatedUV);
    vec4 prevColor = texture(iChannel0, uv);

    fragColor = mix(curColor, prevColor, 0.5);
}
"""

[buffer_a.input_0]
type = "misc"
name = "Buffer A"
wrap = "repeat"
filter = "nearest"
vflip = true

[buffer_a.input_1]
type = "texture"
name = "Abstract 2"
wrap = "repeat"
filter = "mipmap"
vflip = true

# 'Image' pass
[image]
shader = """
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 color = texture(iChannel0, uv);
    float vignette = 1.0 - length(uv - 0.5) * 0.7;
    fragColor = color * vignette;
}
"""

[image.input_0]
type = "misc"
name = "Buffer A"
wrap = "repeat"
filter = "nearest"
vflip = true

# Other rendering settings
resolution_scale = 1.0
filter_mode = "linear"
time_scale = 1.0
time_offset = "0s"
screen_bounds_policy = "all_monitors"
monitor_selection = ["*"]
layout_mode = "stretch"
interval_between_frames = "0s"
crossfade_overlap_ratio = 0.0
```

The shader is reloaded automatically when its TOML file is edited while in use.

Whenever a shader is (re)loaded, a text overlay containing the shader name and author is displayed for a few seconds. Use `--no-overlay` to hide the overlay.

## ShaderToy support

Render passes:

* \[x] Buffers A..D
* \[x] Common
* \[x] Cubemap
* \[x] Image
* \[ ] Sound
* \[ ] VR

Inputs:

* \[x] Buffers A..D
* \[x] Cubemap
* \[x] Custom textures
* \[ ] Keyboard
* \[ ] Microphone
* \[x] Mouse
* \[ ] Music
* \[ ] Soundcloud
* \[x] Texture
* \[x] Volume
* \[ ] Video
* \[ ] Webcam

## Preset file format

The preset file supports the following keys:

### Shader metadata

* `id`, `name`, `username`, `description` (**string**): These correspond to the shader metadata imported from ShaderToy. Default is `""` for all keys.

### Render and animation settings

* `resolution_scale` (**float**): Scale factor to scale the resolution of the rendered frame. Use <1 to downsample, >1 to upsample (e.g., 2 for 2x SSAA, 4 for 4x SSAA, etc). Default is `1.0` (no scaling).
* `filter_mode` (**string**): Filtering mode when blitting the rendered frame onto the screen. Allowed values:
  * `"nearest"`: nearest neighbor filtering
  * `"linear"`: bilinear filtering (default)
  * `"mipmap"`: trilinear filtering
* `layout_mode` (**string**): How each frame is laid out on screen. Allowed values:
  * `"stretch"`: scales to fill the screen (default)
  * `"center"`: centers without scaling (may underscan)
  * `"repeat"`: tiles by repeating the frame
  * `"mirrored_repeat"`: tiles using mirror-repeat wrapping
* `interval_between_frames` (**string**): Minimum time between frames. Use this to limit the frame rate (e.g., `"100ms"` to cap at 10 frames per second) or create a slideshow effect (e.g., `"60s"` to render a new frame each minute). Default is `"0s"` (non-throttled animation).
* `crossfade_overlap_ratio` (**float**): Controls smooth frame transitions through cross fading, from 0 (no overlap; default) to 1 (always transitioning). Cross fading is enabled only if this setting and `interval_between_frames` are non-zero.

### Time scale and offset

* `time_scale` (**float**): Scale factor applied to `iTime` and `iTimeDelta` uniforms. Use <1 to slow down and >1 to speed up. Default is `1.0` (no scaling).
* `time_offset` (**string**): Offset applied to `iTime`. Use this to offset time in time-based animations. Examples: `"500ms"` for 500 milliseconds, `"10s"` for 10 seconds. Default is `"0s"` (no offset).

### Multi-monitor settings

* `screen_bounds_policy` (**string**): How screen bounds are calculated. Allowed values are:
  * `"all_monitors"`: union of all monitors (default)
  * `"selection_monitors"`: union of selected monitors (see also `monitor_selection`)
  * `"cloned"`: per-monitor isolation (clone mode)
* `monitor_selection` (**array of strings**): Monitor selection using DRM connector names (e.g., `["HDMI-1", "HDMI-3"]`), or `"*"` (default) to select all available monitors.

### Render passes

* `common` (**dictionary**). This contains a single key:
  * `shader` (**string**): Common shader code shared by all passes. Default is `""`.
* `buffer_a`, `buffer_b`, `buffer_c`, `buffer_d`, `cube_a`, `image` (**dictionary**). Render pass settings supporting the following keys:
  * `shader` (**string**): Shader code for the render pass. Default is `""` for all passes except for `image`, which defaults to:
    ```glsl
    void mainImage(out vec4 fragColor, in vec2 fragCoord)
    {
        vec2 uv = fragCoord / iResolution.xy;
        vec3 col = .5 + .5 * cos(iTime + uv.xyx + vec3(0, 2, 4));
        fragColor = vec4(col, 1);
    }
    ```
  * `input_0`, `input_1`, `input_2`, `input_3` (**dictionary**): Input channels corresponding to 'Channel0..3' inputs in ShaderToy. The following keys are supported:
    * `type` (**string**): Input type, one of:
      * `"misc"` (default)
      * `"texture"`
      * `"cubemap"`
      * `"volume"`
    * `name` (**string**): Input name. Allowed input names depend on the value of `type`. If `type` is `"misc"`, `name` must be a buffer name such as `"Buffer A"`, `"Buffer B"`, `"Buffer C"`, `"Buffer D"`, or `"Cubemap A"`. The following table shows the complete set:
      | `type`      | Allowed values for `name` |
      |-------------|---------------------------|
      | `"misc"`    | Buffer name: `"Buffer A"`, `"Buffer B"`, `"Buffer C"`, `"Buffer D"`, or `"Cubemap A"` |
      | `"texture"` | Path to a jpeg/png file, or a predefined ShaderToy texture name: `"Abstract 1"`, `"Abstract 2"`, `"Abstract 3"`, `"Bayer"`, `"Blue Noise"`, `"Font 1"`, `"Gray Noise Medium"`, `"Gray Noise Small"`, `"Lichen"`, `"London"`, `"Nyancat"`, `"Organic 1"`, `"Organic 2"`, `"Organic 3"`, `"Organic 4"`, `"Pebbles"`, `"RGBA Noise Medium"`, `"RGBA Noise Small"`, `"Rock Tiles"`, `"Rusty Metal"`, `"Stars"`, or `"Wood"`. |
      | `"cubemap"` | Path to a jpeg/png file, or a predefined ShaderToy cubemap name: `"Forest"`, `"Forest Blurred"`, `"St. Peter's Basilica"`, `"St. Peter's Basilica Blurred"`, `"Uffizi Gallery"`, or `"Uffizi Gallery Blurred"`. If a file path is specified, it is assumed that the file contains the textures of each cube side laid out in a row in the order +x, -x, +y, -y, +z, -z. |
      | `"volume"`  | Path to a jpeg/png file, or a predefined ShaderToy volume name: `"Grey Noise3D"` or `"RGBA Noise3D"`. When a file path is specified, it is assumed that the file contains the 2D slices of the volume laid out in a row, from slice 0 to slice N-1, where N is the square root of the volume size. |
    * `wrap` (**string**): Wrap mode: `"clamp"` (default) or `"repeat"`.
    * `filter`(**string**): Filtering mode: `"linear"` (default), `"nearest"`, or `"mipmap"`.
    * `vflip` (**boolean**): Vertical flip: `true` or `false` (default).

## License

ShaderBG is licensed under the terms of GPLv3. See [LICENSE](https://github.com/hbatagelo/shaderbg/blob/main/LICENSE) for the full text.

ShaderBG uses the ShaderToy API to import shaders. The predefined textures have been downloaded and adapted from ShaderToy.

# ShaderBG

[![Build](https://github.com/hbatagelo/shaderbg/actions/workflows/build.yml/badge.svg)](https://github.com/hbatagelo/shaderbg/actions/workflows/build.yml)
[![Test](https://github.com/hbatagelo/shaderbg/actions/workflows/test.yml/badge.svg)](https://github.com/hbatagelo/shaderbg/actions/workflows/test.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

Shader wallpaper utility for Wayland.

<img src="teaser_bubbles.gif" alt="ShaderBG on Sway" width="800">

<sup>[Colorful underwater bubbles II](https://www.shadertoy.com/view/mlBSWc) on [Sway](https://swaywm.org/)</sup>

<img src="teaser_quake.gif" alt="ShaderBG on Cosmic" width="800">

<sup>[Quake Intro](https://www.shadertoy.com/view/lsKfWd) on [Cosmic](https://system76.com/cosmic/)</sup>

ShaderBG is a wallpaper tool that runs GLSL shaders as interactive live wallpapers on Wayland compositors supporting the [wlr-layer-shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) protocol.

Shaders can be selected from a built-in collection, imported from [ShaderToy](https://www.shadertoy.com/) JSON exports, or customized via TOML configuration files.

On compositors that do not support wlr-layer-shell, ShaderBG falls back to a top-level window.

See the `shaderbg(1)` man page for usage instructions.

## Key features

* Import shaders from ShaderToy
* Multi-monitor support
* Mouse and keyboard input
* Configuration hot reload
* Slideshow mode with cross-fade transitions
* Configurable render scaling (downsampling / upsampling)
* Layout modes (stretch, centered, repeat, mirrored repeat)

## Installation

### From packages

Arch (`.pkg`), Debian (`.deb`), and Fedora (`.rpm` packages for x86\_64 are available on the [releases page](https://github.com/hbatagelo/shaderbg/releases).

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
   <summary>Arch</summary>

   ```sh
   sudo pacman -S base-devel gtk4-layer-shell
   ```

   </details>

   <details>
   <summary>Debian/Ubuntu</summary>

   ```sh
   sudo apt install build-essential libgtk-4-dev libgtk4-layer-shell-dev
   ```

   </details>

   <details>
   <summary>Fedora</summary>

   ```sh
   sudo dnf install gcc-c++ gtk4-devel libepoxy-devel gtk4-layer-shell-devel
   ```

   </details>

   If GTK4 Layer Shell is unavailable in your distribution's repositories, [build it from source](https://github.com/wmww/gtk4-layer-shell?tab=readme-ov-file#building-from-source).

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

Run without arguments to randomly select a shader from the presets:

```sh
shaderbg
```

* Presets are located at `$XDG_DATA_HOME/shaderbg/presets` or `$HOME/.local/share/shaderbg/presets`.

Import from ShaderToy JSON export:

```sh
shaderbg <json_file>
```

* The JSON file is the response from the API request `https://www.shadertoy.com/api/v1/shaders/<shader_id>?key=<api_key>`, where `<shader_id>` is the ID of the shader set to "public + api" visibility in ShaderToy, and `<api_key>` is the API key. See <https://www.shadertoy.com/howto> for more information on how to use the ShaderToy API.
* The imported shader is added to the presets as `<shader_id>.toml` and overwrites any previous file with the same name.

You can also load a custom preset file:

```sh
shaderbg <toml_file>
```

An example TOML preset corresponding to <https://www.shadertoy.com/view/wfjcR3> is shown below:

```toml
id = "wfjcR3"
name = "Spiral Wobble"
username = "hbatagelo"
description = """
A simple wobbling spiral texture effect with multi-pass rendering.

Use up/down arrow keys to zoom in/out."""

# 'Common' shader
[common]
shader = """
#define UP_KEYCODE   38
#define DOWN_KEYCODE 40

float keyDown(sampler2D kb, float key) {
    return texture(kb, vec2((float(key) + 0.5) / 256.0, 0.0)).r;
}

vec2 rotate2D(vec2 uv, float angle) {
    return vec2(
        uv.x * cos(angle) - uv.y * sin(angle),
        uv.x * sin(angle) + uv.y * cos(angle)
    );
}"""

# 'Buffer A' pass
[buffer_a]
shader = """
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float aspect = iResolution.x / iResolution.y;

    // Keyboard input
    float up   = keyDown(iChannel2, UP_KEYCODE);
    float down = keyDown(iChannel2, DOWN_KEYCODE);

    // Read previous frame (rgb = color, a = log(zoom))
    vec4 prev = texture(iChannel0, uv);

    // Update zoom
    if (iFrame == 0) prev.a = 0.0;
    prev.a += (up - down) * iTimeDelta;
    prev.a = clamp(prev.a, log(0.2), log(4.0));

    // Spiral coordinates
    float spirals = 1.0 + 3.0 * (0.5 + 0.5 * sin(iTime * 0.5));
    vec2 centeredUV = (uv - 0.5) * vec2(aspect, 1.0) / exp(prev.a);
    float angle = sin(iTime + length(centeredUV) * spirals);
    vec2 rotatedUV = rotate2D(centeredUV, angle) / vec2(aspect, 1.0) + 0.5;

    // Temporal feedback
    vec4 curColor = texture(iChannel1, rotatedUV);
    float feedback = exp(-15.0 * iTimeDelta);
    vec3 color = mix(curColor.rgb, prev.rgb, feedback);

    fragColor = vec4(color, prev.a);
}"""

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

[buffer_a.input_2]
type = "keyboard"

# 'Image' pass
[image]
shader = """
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 color = texture(iChannel0, uv).rgb;
    float vignette = 1.0 - length(uv - 0.5) * 0.7;
    fragColor = vec4(color * vignette, 1.0);
}"""

[image.input_0]
type = "misc"
name = "Buffer A"
wrap = "clamp"
filter = "nearest"
vflip = true

# Other settings
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

If the TOML file is not found, ShaderBG will automatically look for it in the presets directory.

The shader is automatically reloaded when its TOML file is edited while in use.

Whenever a shader is loaded or reloaded, a text overlay containing the shader name and author is displayed for a few seconds. Use `--no-overlay` to hide the overlay.

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
* \[x] Keyboard
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

* `resolution_scale` (**float**): Scale factor to scale the resolution of the rendered frame. Use values <1 to downsample and >1 to upsample (e.g., 2 for 2x SSAA, 4 for 4x SSAA, etc). Default is `1.0` (no scaling).
* `filter_mode` (**string**): Filtering mode when blitting the rendered frame onto the screen. Allowed values:
  * `"nearest"`: nearest neighbor filtering
  * `"linear"`: bilinear filtering (default)
  * `"mipmap"`: trilinear filtering
* `layout_mode` (**string**): How each frame is laid out on screen. Allowed values:
  * `"stretch"`: scales to fill the screen (default)
  * `"center"`: centers without scaling (may underscan)
  * `"repeat"`: tiles by repeating the frame
  * `"mirrored_repeat"`: tiles using mirror-repeat wrapping
* `interval_between_frames` (**string**): Minimum time between frames. Use this to limit the frame rate and save energy (e.g., `"100ms"` to cap at 10 frames per second) or create a slideshow effect (e.g., `"60s"` to render a new frame each minute). Default is `"0s"` (non-throttled animation).
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
      * `"keyboard"`
    * `name` (**string**): Input name. Allowed input names depend on the value of `type`. If `type` is `"misc"`, `name` must be a buffer name such as `"Buffer A"`, `"Buffer B"`, `"Buffer C"`, `"Buffer D"`, or `"Cubemap A"`. The following table shows the complete set:
      | `type`      | Allowed values for `name` |
      |-------------|---------------------------|
      | `"misc"`    | Buffer name: `"Buffer A"`, `"Buffer B"`, `"Buffer C"`, `"Buffer D"`, or `"Cubemap A"` |
      | `"texture"` | Path to a jpeg/png file, or a predefined ShaderToy texture name: `"Abstract 1"`, `"Abstract 2"`, `"Abstract 3"`, `"Bayer"`, `"Blue Noise"`, `"Font 1"`, `"Gray Noise Medium"`, `"Gray Noise Small"`, `"Lichen"`, `"London"`, `"Nyancat"`, `"Organic 1"`, `"Organic 2"`, `"Organic 3"`, `"Organic 4"`, `"Pebbles"`, `"RGBA Noise Medium"`, `"RGBA Noise Small"`, `"Rock Tiles"`, `"Rusty Metal"`, `"Stars"`, or `"Wood"`. |
      | `"cubemap"` | Path to a jpeg/png file, or a predefined ShaderToy cubemap name: `"Forest"`, `"Forest Blurred"`, `"St. Peter's Basilica"`, `"St. Peter's Basilica Blurred"`, `"Uffizi Gallery"`, or `"Uffizi Gallery Blurred"`. If a file path is specified, it is assumed that the file contains the textures of each cube side laid out in a row in the order +x, -x, +y, -y, +z, -z. |
      | `"volume"`  | Path to a jpeg/png file, or a predefined ShaderToy volume name: `"Grey Noise3D"` or `"RGBA Noise3D"`. When a file path is specified, it is assumed that the file contains the 2D slices of the volume laid out in a row, from slice 0 to slice N-1, where N is the square root of the volume size. |
      | `"keyboard"`  | Value is ignored. |
    * `wrap` (**string**): Wrap mode: `"clamp"` (default) or `"repeat"`.
    * `filter` (**string**): Filtering mode: `"linear"` (default), `"nearest"`, or `"mipmap"`.
    * `vflip` (**boolean**): Vertical flip: `true` or `false` (default).

## License

ShaderBG is licensed under the terms of the GPLv3. See [LICENSE](https://github.com/hbatagelo/shaderbg/blob/main/LICENSE) for the full text.

Presets distributed with this project are released under CC0 ([Creative Commons CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/)).

All textures originate from the Shadertoy media library and are included solely for shader compatibility.
They are not part of ShaderBG, remain the property of their respective copyright holders, and are not relicensed by this project.

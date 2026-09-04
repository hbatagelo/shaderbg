# SHADERBG(1)

## NAME

shaderbg - Shader wallpaper utility for Wayland

## SYNOPSIS

**shaderbg** [OPTIONS] [FILE]

**shaderbg** [**-h**|**--help**]
**shaderbg** [**-V**|**--version**]

## DESCRIPTION

**shaderbg** renders shaders as live wallpapers in Wayland compositors that support the wlr-layer-shell protocol. Shaders can be loaded from preset files or imported from ShaderToy JSON exports.

## ARGUMENTS

*FILE*
: Optional path to a TOML preset file or ShaderToy JSON export file

## OPTIONS

**--no-overlay**
: Disable the shader information overlay display

**--exclusive-zone** *ZONE*
: Override the layer-shell exclusive zone of the render window. Default is -1, or 0 on River

**--input-layer** *LAYER*
: Layer of the input-capture window: background, bottom, top, or overlay. Default is background, or bottom on COSMIC. Only has an effect when input passthrough is disabled

**-h**, **--help**
: Print help information and exit

**-V**, **--version**
: Print version information and exit

## FILES

**~/.local/share/shaderbg/assets/**
: Default directory for assets (ShaderToy predefined textures)

**~/.local/share/shaderbg/presets/**
: Directory containing preset files

## EXAMPLES

**shaderbg**
: Start with a random shader preset

**shaderbg ~/.local/share/shaderbg/presets/galaxy.toml**
: Load a specific shader preset

**shaderbg shadertoy-export.json**
: Import from ShaderToy JSON export file

**shaderbg my-shader.toml --no-overlay**
: Load preset without displaying the shader information overlay

**shaderbg --input-layer bottom**
: Run the input-capture window on the bottom layer instead of the default for the current compositor

## ENVIRONMENT

**XDG_CURRENT_DESKTOP**
: Used to auto-detect compositor-specific defaults (exclusive zone 0 on River, input window on the bottom layer on COSMIC)

The application may use standard XDG environment variables for configuration directory location.

## NOTES

This utility requires OpenGL 4.2+ and a Wayland compositor with wlr-layer-shell support.

## AUTHOR

Written by Harlen Batagelo <hbatagelo@gmail.com>

## COPYRIGHT

Copyright © 2025 Harlen Batagelo. License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.

This is free software: you are free to change and redistribute it. There is NO WARRANTY, to the extent permitted by law.

## SEE ALSO

**swaybg**(1)

ShaderToy website: <https://www.shadertoy.com/>

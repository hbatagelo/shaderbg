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

## ENVIRONMENT

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

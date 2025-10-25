# SHADERBG(1)

## NAME

shaderbg - Shader wallpaper utility for Wayland

## SYNOPSIS

**shaderbg** [*file*] | [*shader_id* *api_key*] [**--no-overlay**]

**shaderbg** [**-h**|**--help**] [**-V**|**--version**]

## DESCRIPTION

**shaderbg** renders shaders as live wallpapers in Wayland compositors that support the wlr-layer-shell protocol. It can load shaders from preset files or directly from ShaderToy using the ShaderToy API.

## ARGUMENTS

*file*
: Path to a TOML preset file containing shader configuration

*shader_id*
: ShaderToy shader ID (6-character identifier from ShaderToy URL)

*api_key*
: ShaderToy API key (required when using shader_id)

## OPTIONS

**--no-overlay**
: Disable the shader information overlay display

**-h**, **--help**
: Print help information and exit

**-V**, **--version**
: Print version information and exit

## USAGE

Run with no arguments to use a random preset:
```
shaderbg
```

Load a specific TOML preset file:
```
shaderbg my-shader.toml
```

Load a shader directly from ShaderToy:
```
shaderbg XsXXDN your-api-key
```

Disable the information overlay when loading a preset:
```
shaderbg my-shader.toml --no-overlay
```

## FILES

**~/.local/share/shaderbg/assets/**
: Default directory for assets (ShaderToy predefined textures)

**~/.local/share/shaderbg/presets/**
: Directory containing preset files

## EXAMPLES

**shaderbg**
: Start with a random shader preset

**shaderbg ~/.local/share/shaderbg/presets/neonwave_sunrise.toml**
: Load a specific shader preset

**shaderbg 4dXGR4 abc123**
: Load ShaderToy shader with ID "4dXGR4" using API key "abc123"

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

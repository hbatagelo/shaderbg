{
  lib,
  rustPlatform,
  pkg-config,
  gtk4,
  gtk4-layer-shell,
  libepoxy,
  libGL,
}:
rustPlatform.buildRustPackage {
  pname = "shaderbg";
  version = (lib.importTOML ./Cargo.toml).package.version;
  src = lib.cleanSource ./.;
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    gtk4
    gtk4-layer-shell
    libepoxy
    libGL
  ];
  env.SHADERBG_SYSTEM_DATA_DIR = "${placeholder "out"}/share";
  postInstall = ''
    mkdir -p $out/share/shaderbg
    cp -r data/* $out/share/shaderbg/
  '';
  meta = {
    description = "Render shaders as live wallpapers on Wayland compositors";
    homepage = "https://github.com/hbatagelo/shaderbg";
    license = lib.licenses.gpl3Only;
    platforms = lib.platforms.linux;
    mainProgram = "shaderbg";
  };
}

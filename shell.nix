{
  system ? builtins.currentSystem,
  # The package whose build environment the shell reproduces; arguments
  # (pinned nixpkgs, distribute) are those of default.nix.
  client ? import ./default.nix { inherit system; },
}:

let
  pkgs = client.pkgs;
  inherit (pkgs) lib;

  # Libraries Bevy loads at run time (winit, wgpu, gilrs); dlopen'ed, so they
  # must be on LD_LIBRARY_PATH rather than only linked.
  bevyRuntimeLibs = lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
    vulkan-loader
    libxkbcommon
    wayland
    libx11
    libxcursor
    libxi
    libxrandr
    udev
    alsa-lib
  ]);
in
pkgs.mkShell {
  name = "metin2client";

  # Toolchain, libraries and the macOS miniaudio build, as in default.nix.
  inputsFrom = [ client ];

  packages = with pkgs; [
    # clangd, clang-format and clang-tidy, driven by .clang-format, .clang-tidy
    # and build/compile_commands.json.
    clang-tools

    # Rust port (docs/bevy-port-plan.md).
    cargo
    rustc
    clippy
    rustfmt
    rust-analyzer
  ] ++ bevyRuntimeLibs;

  # mkShell takes only the inputs from inputsFrom, not the environment.
  inherit (client) PYTHON_INCLUDE_DIR hardeningDisable;

  # rust-analyzer needs the std sources.
  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

  shellHook = ''
    export CMAKE_GENERATOR=Ninja
    export CMAKE_EXPORT_COMPILE_COMMANDS=ON
  '' + lib.optionalString (bevyRuntimeLibs != [ ]) ''
    export LD_LIBRARY_PATH="${lib.makeLibraryPath bevyRuntimeLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  '';
}

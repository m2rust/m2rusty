{
  system ? builtins.currentSystem,
  # The package whose build environment the shell reproduces; arguments
  # (pinned nixpkgs, distribute) are those of default.nix.
  client ? import ./default.nix { inherit system; },
}:

let
  pkgs = client.pkgs;
in
pkgs.mkShell {
  name = "metin2client";

  # Toolchain, libraries and the macOS miniaudio build, as in default.nix.
  inputsFrom = [ client ];

  # clangd, clang-format and clang-tidy, driven by .clang-format, .clang-tidy
  # and build/compile_commands.json.
  packages = with pkgs; [
    clang-tools
  ];

  # mkShell takes only the inputs from inputsFrom, not the environment.
  inherit (client) PYTHON_INCLUDE_DIR hardeningDisable;

  shellHook = ''
    export CMAKE_GENERATOR=Ninja
    export CMAKE_EXPORT_COMPILE_COMMANDS=ON
  '';
}

{
  description = "OpenMT2 Metin2 client";

  # nixpkgs stays pinned inside default.nix; engine and client live in this
  # repository, so the flake has no inputs.
  outputs = { self }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAllSystems = f: builtins.listToAttrs (map (system: {
        name = system;
        value = f system;
      }) systems);
    in
    {
      packages = forAllSystems (system:
        let
          client = distribute: import ./default.nix { inherit system distribute; };
          rel = client true;
          dev = client false;
        in
        {
          metin2client-rel = rel;
          metin2client-dev = dev;
          metin2client = rel;
          default = rel;
        });

      devShells = forAllSystems (system: {
        default = import ./shell.nix {
          inherit system;
          client = self.packages.${system}.metin2client-dev;
        };
      });
    };
}

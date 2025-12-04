{
  description = "Description for the project";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    disko.url = "github:nix-community/disko";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } (
      { lib, ... }:
      {
        imports = [
          ./dev-shells/default.nix
          ./checks/default.nix
        ];

        perSystem =
          {
            pkgs,
            config,
            system,
            ...
          }:
          let
            diskoZfsLib = import ./lib { inherit inputs lib; };
          in
          {
            packages.disko-zfs = pkgs.callPackage ./package.nix { };
            packages.default = config.packages.disko-zfs;
          };

        flake.nixosModules.default = lib.modules.importApply ./nixos/modules/default.nix {
          inherit inputs;
        };
        flake.overlays.default = final: _: {
          disko-zfs = final.callPackage ./package.nix { };
        };

        systems = [
          "x86_64-linux"
          "aarch64-linux"
          "aarch64-darwin"
          "x86_64-darwin"
        ];
      }
    );
}

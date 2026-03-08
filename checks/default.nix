{ lib, inputs, ... }:
let
  diskoZfsLib = import ../lib { inherit inputs lib; };
in
{
  perSystem =
    {
      lib,
      pkgs,
      system,
      inputs',
      ...
    }:
    {
      checks =
        let
          importArgs = {
            inherit
              lib
              pkgs
              system
              inputs
              inputs'
              ;
          };

          diskoChecks = lib.pipe (builtins.readDir ./disko) [
            (lib.filterAttrs (n: v: v == "regular" && n != "default.nix"))
            (lib.mapAttrs (
              name: _:
              let
                imported = import ./disko/${name};
              in
              if lib.isFunction imported then imported importArgs else imported
            ))
            (lib.mapAttrs' (
              name: value: {
                name = "disko-" + lib.removeSuffix ".nix" name;
                value = diskoZfsLib.mkDiskoZfsTest (value // { inherit pkgs name; });
              }
            ))
          ];
          diskolessChecks = lib.pipe (builtins.readDir ./diskoless) [
            (lib.filterAttrs (n: v: v == "regular" && n != "default.nix"))
            (lib.mapAttrs (
              name: _:
              let
                imported = import ./diskoless/${name};
              in
              if lib.isFunction imported then imported importArgs else imported
            ))
            (lib.mapAttrs' (
              name: value: {
                name = "diskoless-" + lib.removeSuffix ".nix" name;
                value = value;
              }
            ))
          ];
        in
        lib.mkIf (system == "x86_64-linux") (
          lib.foldl lib.trivial.mergeAttrs { } [
            diskoChecks
            diskolessChecks
          ]
        );
    };
}

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
      ...
    }:
    {
      checks = lib.mkIf (system == "x86_64-linux") (
        lib.pipe (builtins.readDir ./.) [
          (lib.filterAttrs (n: v: v == "regular" && n != "default.nix"))
          (lib.mapAttrs (
            name: _:
            let
              imported = import ./${name};
            in
            if lib.isFunction imported then imported { inherit lib; } else imported
          ))
          (lib.mapAttrs' (
            name: value: {
              name = lib.removeSuffix ".nix" name;
              value = diskoZfsLib.mkDiskoZfsTest (value // { inherit pkgs name; });
            }
          ))
        ]
      );
    };
}

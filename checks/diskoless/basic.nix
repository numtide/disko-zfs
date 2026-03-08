{
  lib,
  pkgs,
  system,
  inputs,
  ...
}:
let
  configuration = inputs.nixpkgs.lib.nixosSystem {
    inherit system;
    modules = lib.singleton (
      { modulesPath, ... }:
      {
        imports = [
          inputs.self.nixosModules.default
        ];

        boot.isContainer = true;

        disko.zfs = {
          enable = true;

          settings.datasets = {
            "zroot/ds1/home" = {
              properties = {
                mountpoint = "legacy";
              };
            };
          };
        };
      }
    );
  };
in
assert lib.assertMsg (configuration.config.systemd.services ? disko-zfs) "disko-zfs unit not found";
pkgs.writeText "diskoless-basic.txt" ''
  Evaluation result: ${builtins.unsafeDiscardStringContext configuration.config.system.build.toplevel}
''

{ inputs }:
{
  pkgs,
  lib,
  config,
  ...
}:
let
  cfg = config.disko.zfs;
  configFile = (pkgs.formats.json { }).generate "spec.json" cfg.settings;
in
{
  options.disko.zfs = {
    enable = lib.mkEnableOption "Enable declarative ZFS dataset management";

    package = lib.mkPackageOption pkgs "disko-zfs" { };

    settings.datasets = lib.mkOption {
      type = lib.types.lazyAttrsOf (
        (lib.types.submodule {
          options.properties = lib.mkOption {
            type = lib.types.attrsOf (lib.types.either lib.types.int lib.types.str);
          };
        })
      );
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        nixpkgs.overlays = [
          inputs.self.overlays.default
        ];

        systemd.services."disko-zfs" = {
          unitConfig.DefaultDependencies = false;
          requiredBy = [
            "local-fs.target"
            "zfs-mount.service"
          ];
          before = [
            "local-fs.target"
            "zfs-mount.service"
          ];
          after = [
            "zfs-import.target"
          ];
          script = ''
            ${lib.getExe cfg.package} --spec ${configFile} apply
          '';
        };
      }
      (lib.mkIf (config ? disko) {
        disko.zfs.settings.datasets = lib.pipe config.disko.devices.zpool [
          (lib.mapAttrsToList (n: v: lib.nameValuePair n v.datasets))
          (lib.map (
            { name, value }:
            lib.mapAttrsToList (
              dataset: settings: lib.nameValuePair "${name}/${dataset}" { properties = settings.options; }
            ) value
          ))
          lib.flatten
          lib.listToAttrs
        ];
      })
    ]
  );
}

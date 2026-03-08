{ inputs }:
{
  pkgs,
  lib,
  config,
  ...
}:
let
  cfg = config.disko.zfs;
  configFile = (pkgs.formats.json { }).generate "disko-zfs-spec.json" cfg.settings;

  command = action: ''
    ${lib.getExe cfg.package} \
      --log-level ${cfg.settings.logLevel} \
        ${action} \
        --spec ${configFile}
  '';
in
{
  options.disko.zfs = {
    enable = lib.mkEnableOption ''
      Enable declarative ZFS dataset management

      Along with a `disko-zfs` service this option will add a activation script
      which only runs during dry activation. When executed it will display the
      changes `disko-zfs` would make.
    '';

    package = lib.mkPackageOption pkgs "disko-zfs" { };

    settings = {
      logLevel = lib.mkOption {
        type = lib.types.enum [
          "error"
          "warn"
          "info"
          "debug"
          "trace"
        ];
        default = "info";
        description = ''
          Log level to run `disko-zfs` with. If set to `trace`, `disko-zfs` will
          very verbosely explain all decisions it's making and why. If you're trying
          to understand a change it wants to make, try this option.
        '';
      };

      ignoredDatasets = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = ''
          Datasets to ignore, supports shell style globs. When a dataset is ignored,
          `disko-zfs` will neither create it nor suggent its destruction.
        '';
        example = ''
          ["zroot/root/persist/*"]
        '';
      };

      ignoredProperties = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = ''
          Properties to ignore, supports shell style globs. When a property is ignored,
          `disko-zfs` will not set it or unset it.
        '';
        example = ''
          ["com.sun:auto-snapshot"]
        '';
      };

      datasets = lib.mkOption {
        type = lib.types.lazyAttrsOf (
          (lib.types.submodule {
            options.properties = lib.mkOption {
              type = lib.types.attrsOf (lib.types.either lib.types.int lib.types.str);
              default = { };
              description = ''
                Properties that this dataset should have.
              '';
            };
          })
        );
        description = ''
          Declaration of datasets that should exist on this system.
        '';
      };
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
            "local-fs-pre.target"
            "zfs-mount.service"
          ];
          before = [
            "zfs-mount.service"
            "local-fs-pre.target"
          ];
          after = [
            "zfs-import.target"
          ];

          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
          };

          script = ''
            export PATH="$PATH:/run/booted-system/sw/bin"
            ${command "apply"}
          '';
        };

        system.activationScripts."disko-zfs" = {
          text = ''
            (
              if [[ "$NIXOS_ACTION" == "dry-activate" ]] ; then
                echo "-- disko-zfs --"
                export PATH="$PATH:/run/booted-system/sw/bin"
                ${command "plan"}
                echo "-- disko-zfs --"
              fi
            )
          '';
          supportsDryActivation = true;
        };
      }
      (lib.mkIf (config.disko or { } ? devices) {
        disko.zfs.settings.datasets = lib.pipe config.disko.devices.zpool [
          (lib.mapAttrsToList (n: v: lib.nameValuePair n v.datasets))
          (lib.map (
            { name, value }:
            lib.mapAttrsToList (
              dataset: settings: lib.nameValuePair "${name}/${dataset}" { properties = settings.options; }
            ) (lib.filterAttrs (name: _: name != "__root") value)
            ++ [
              {
                inherit name;
                value = {
                  properties = value.__root.options;
                };
              }
            ]
          ))
          lib.flatten
          lib.listToAttrs
        ];
      })
    ]
  );
}

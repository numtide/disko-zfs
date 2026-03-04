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

      ignoredDependencies = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ "/" "/nix" "/tmp" "/boot" ];
        description = ''
          We run disko-zfs before mounting any legacy zfs partitions, except for those mounted at the following paths. This can be used for instance if a partition contains a key for another zfs partition, but also make sure to add the dependency with something like:
          ```
          systemd.services."disko-zfs" = {
            after = [ "my-key-folder.mount" ];
            wants = [ "my-key-folder.mount" ];
          }
          ```
        '';
        example = ''
          [ "/" "/nix" "/tmp" "/boot" "/myfolder/containing/zfs/key"]
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

        systemd.services."disko-zfs" =
          let
            # Compute all services to start after disko-zfs (zfs-mount.service only deals with non-legacy mount points)
            deps = (lib.pipe config.disko.devices.zpool [
              (lib.mapAttrsToList (n: v: lib.nameValuePair n v.datasets))
              (lib.map (
                { name, value }:
                lib.mapAttrsToList (
                  dataset: settings: (if settings.options.mountpoint == "legacy" || false then settings.mountpoint or "" else "")
                ) (lib.filterAttrs (name: attr: name != "__root") value)
              ))
              lib.flatten
              # Some partitions may be mounted. sure what happens if we add these repositories in a ZFS
              (lib.filter (x: x != "" && builtins.elem x cfg.settings.ignoredDependencies == false))
              # Turn / into -
              (lib.map (x: (builtins.replaceStrings ["/"] ["-"] x) + ".mount"))
              # Removes the leading -
              (lib.map (x: builtins.elemAt (builtins.match "^-(.*)$" x) 0))
            ]);
          in
            {
              unitConfig.DefaultDependencies = false;
              # Don't use requiredBy otherwise adding a new dataset will unmount all filesystems...
              # But anyway it will most of the time do it since adding a disko datasets reload the import for this disk,
              # which unmounts it.
              wantedBy = [
                "local-fs.target"
                "zfs-mount.service"
              ] ++ deps;
              before = [
                "local-fs.target"
                "zfs-mount.service"
              ] ++ deps;
              after = [
                "zfs-import.target"
              ];

              serviceConfig = {
                Type = "oneshot"; # Make sure to wait for the end of the script before starting to mount other elements
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
      (lib.mkIf (config ? disko) {
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
